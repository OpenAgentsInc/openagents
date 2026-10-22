//! Local-only Jev endpoint for the Haunted House creator controls.

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::time::Duration;

    let port: u16 = std::env::var("HAUNT_DIRECTOR_PORT")
        .unwrap_or_else(|_| "0".into())
        .parse()?;
    let origin = std::env::var("HAUNT_EDITOR_ORIGIN")?;
    let client = kiln::director::client()?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    let host = format!("127.0.0.1:{}", listener.local_addr()?.port());
    println!("http://{host}");
    std::io::stdout().flush()?;
    // One inference in flight. Every request has bounded reads and no retries.
    for mut stream in listener.incoming().flatten() {
        stream.set_read_timeout(Some(Duration::from_secs(3)))?;
        stream.set_write_timeout(Some(Duration::from_secs(3)))?;
        let result = (|| -> Result<serde_json::Value, &'static str> {
            let mut reader = BufReader::new(
                stream
                    .try_clone()
                    .map_err(|_| "Connection closed.")?
                    .take(8193),
            );
            let mut head = String::new();
            loop {
                let mut line = String::new();
                if reader
                    .read_line(&mut line)
                    .map_err(|_| "Couldn't read the request.")?
                    == 0
                {
                    return Err("Incomplete request.");
                }
                head.push_str(&line);
                if head.len() > 4096 {
                    return Err("Request headers are too large.");
                }
                if line == "\r\n" {
                    break;
                }
            }
            let mut lines = head.lines();
            let first = lines.next().unwrap_or_default();
            let headers: std::collections::BTreeMap<_, _> = lines
                .filter_map(|l| l.split_once(':'))
                .map(|(k, v)| (k.to_ascii_lowercase(), v.trim().to_string()))
                .collect();
            if headers.get("host") != Some(&host) {
                return Err("Use the local editor address.");
            }
            if first.starts_with("GET /health ") {
                return Ok(serde_json::json!({"available":true}));
            }
            if !first.starts_with("POST /decide ")
                || headers.get("origin") != Some(&origin)
                || !headers
                    .get("content-type")
                    .is_some_and(|s| s.starts_with("application/json"))
                || headers.contains_key("transfer-encoding")
            {
                return Err("Use the scene controls in the local editor.");
            }
            let length: usize = headers
                .get("content-length")
                .and_then(|s| s.parse().ok())
                .filter(|n| *n <= 4096)
                .ok_or("Request is too large or incomplete.")?;
            let mut bytes = vec![0; length];
            reader
                .read_exact(&mut bytes)
                .map_err(|_| "Incomplete request.")?;
            let request: kiln::director::Request =
                serde_json::from_slice(&bytes).map_err(|_| "Describe one scene change.")?;
            let decision = runtime
                .block_on(kiln::director::decide(&client, &request))
                .map_err(
                    |_| "The decision service is unavailable. Use the scene controls or try again.",
                )?;
            serde_json::to_value(decision).map_err(|_| "Couldn't read the decision.")
        })();
        let (status, body) = match result {
            Ok(value) => ("200 OK", value),
            Err(message) => (
                "422 Unprocessable Content",
                serde_json::json!({"error":message}),
            ),
        };
        let body = body.to_string();
        let _ = write!(
            stream,
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
    }
    Ok(())
}
