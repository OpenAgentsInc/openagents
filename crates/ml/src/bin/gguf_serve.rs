use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::thread;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut path = None;
    let mut bind = "127.0.0.1".to_string();
    let mut port = 9898u16;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--path" => {
                if let Some(value) = args.next() {
                    path = Some(PathBuf::from(value));
                }
            }
            "--bind" => {
                if let Some(value) = args.next() {
                    bind = value;
                }
            }
            "--port" => {
                if let Some(value) = args.next() {
                    port = value.parse().unwrap_or(9898);
                }
            }
            _ => {}
        }
    }

    let path = path.unwrap_or_else(|| {
        PathBuf::from("crates/ml/models/gpt-oss-20b/gpt-oss-20b-Q8_0.gguf")
    });
    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("gpt-oss-20b-Q8_0.gguf")
        .to_string();

    if !path.exists() {
        eprintln!("GGUF file not found: {}", path.display());
        std::process::exit(1);
    }

    let addr = format!("{bind}:{port}");
    let listener = TcpListener::bind(&addr)?;
    println!("GGUF server listening on http://{addr}/{file_name}");
    println!("Serving: {}", path.display());

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let path = path.clone();
                let file_name = file_name.clone();
                thread::spawn(move || {
                    if let Err(err) = handle_connection(stream, &path, &file_name) {
                        eprintln!("GGUF server error: {err}");
                    }
                });
            }
            Err(err) => {
                eprintln!("GGUF server accept error: {err}");
            }
        }
    }

    Ok(())
}

fn handle_connection(
    mut stream: TcpStream,
    path: &Path,
    file_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line)? == 0 {
        return Ok(());
    }

    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let req_path = parts.next().unwrap_or("/");

    let mut range_header = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            break;
        }
        if let Some((key, value)) = line.split_once(':') {
            if key.eq_ignore_ascii_case("range") {
                range_header = Some(value.trim().to_string());
            }
        }
    }

    if !matches!(method, "GET" | "HEAD") {
        respond_status(&mut stream, 405, "Method Not Allowed")?;
        return Ok(());
    }

    if req_path != "/" && req_path != "/gguf" && !req_path.ends_with(file_name) {
        respond_status(&mut stream, 404, "Not Found")?;
        return Ok(());
    }

    let total = path.metadata()?.len();
    let (start, end) = match range_header
        .as_deref()
        .and_then(|value| parse_range(value, total))
    {
        Some(range) => range,
        None => {
            respond_status(&mut stream, 416, "Range Required")?;
            return Ok(());
        }
    };

    let len = end.saturating_sub(start).saturating_add(1);
    write_headers(&mut stream, start, end, total, len)?;

    if method == "HEAD" {
        return Ok(());
    }

    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(start))?;
    let mut buffer = vec![0u8; len as usize];
    file.read_exact(&mut buffer)?;
    stream.write_all(&buffer)?;
    Ok(())
}

fn parse_range(value: &str, total: u64) -> Option<(u64, u64)> {
    let value = value.trim();
    let value = value.strip_prefix("bytes=")?;
    let (start_str, end_str) = value.split_once('-')?;
    let start = start_str.parse::<u64>().ok()?;
    let end = if end_str.is_empty() {
        total.saturating_sub(1)
    } else {
        end_str.parse::<u64>().ok()?
    };
    if start > end || start >= total {
        return None;
    }
    let end = end.min(total.saturating_sub(1));
    Some((start, end))
}

fn write_headers(
    stream: &mut TcpStream,
    start: u64,
    end: u64,
    total: u64,
    len: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    write!(
        stream,
        "HTTP/1.1 206 Partial Content\r\nContent-Type: application/octet-stream\r\nAccept-Ranges: bytes\r\nContent-Range: bytes {}-{}/{}\r\nContent-Length: {}\r\n\r\n",
        start, end, total, len
    )?;
    Ok(())
}

fn respond_status(
    stream: &mut TcpStream,
    code: u16,
    message: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    write!(
        stream,
        "HTTP/1.1 {code} {message}\r\nContent-Length: 0\r\n\r\n"
    )?;
    Ok(())
}
