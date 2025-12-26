use std::io::{Read, Write};
use std::net::TcpListener;

fn main() -> std::io::Result<()> {
    let mut hostname = "127.0.0.1".to_string();
    let mut port: u16 = 4096;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "serve" => {}
            "--port" => {
                if let Some(value) = args.next() {
                    if let Ok(parsed) = value.parse() {
                        port = parsed;
                    }
                }
            }
            "--hostname" => {
                if let Some(value) = args.next() {
                    hostname = value;
                }
            }
            _ => {}
        }
    }

    let addr = format!("{}:{}", hostname, port);
    let listener = TcpListener::bind(addr)?;

    for stream in listener.incoming() {
        let mut stream = stream?;
        let mut buffer = [0u8; 1024];
        let bytes = stream.read(&mut buffer)?;
        if bytes == 0 {
            continue;
        }

        let request = String::from_utf8_lossy(&buffer[..bytes]);
        let path = request
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .unwrap_or("/");

        let (status, body, content_type) = if path == "/global/health" {
            (
                "HTTP/1.1 200 OK",
                r#"{\"healthy\":true,\"version\":\"test\"}"#,
                "application/json",
            )
        } else {
            ("HTTP/1.1 404 NOT FOUND", "not found", "text/plain")
        };

        let response = format!(
            "{status}\r\nContent-Length: {}\r\nContent-Type: {}\r\n\r\n{}",
            body.len(),
            content_type,
            body
        );
        stream.write_all(response.as_bytes())?;
    }

    Ok(())
}
