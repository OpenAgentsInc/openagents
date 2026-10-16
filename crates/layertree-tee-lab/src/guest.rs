//! Bounded access to the Confidential Space launcher's public PKI evidence.
use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    time::Duration,
};

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn unchunk(mut input: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut output = Vec::new();
    loop {
        let end = input
            .windows(2)
            .position(|p| p == b"\r\n")
            .ok_or("missing chunk size")?;
        let length = std::str::from_utf8(&input[..end])?
            .split(';')
            .next()
            .ok_or("chunk size")?;
        let size = usize::from_str_radix(length, 16)?;
        input = &input[end + 2..];
        if size == 0 {
            return Ok(output);
        }
        let chunk = input.get(..size).ok_or("truncated chunk")?;
        if input.get(size..size.checked_add(2).ok_or("chunk overflow")?) != Some(b"\r\n") {
            return Err("missing chunk terminator".into());
        }
        output.extend_from_slice(chunk);
        input = &input[size + 2..];
    }
}
pub fn token(report: &[u8; 64]) -> Result<String, Box<dyn std::error::Error>> {
    token_with_nonces(&[hex(&report[..32])])
}

/// Bind the report, caller challenge, and actual TLS certificate in one token.
pub fn channel_token(
    report: &[u8; 64],
    challenge: &[u8; 32],
    certificate_sha256: &[u8; 32],
) -> Result<String, Box<dyn std::error::Error>> {
    token_with_nonces(&[hex(&report[..32]), hex(challenge), hex(certificate_sha256)])
}

fn token_with_nonces(nonces: &[String]) -> Result<String, Box<dyn std::error::Error>> {
    let body = serde_json::json!({
        "audience":"openagents.layertree.route.v1",
        "token_type":"PKI",
        "nonces":nonces,
    })
    .to_string();
    let request = format!(
        "POST /v1/token HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let socket = std::env::var("LAYERTREE_TEESERVER")
        .unwrap_or_else(|_| "/run/container_launcher/teeserver.sock".into());
    let mut stream = UnixStream::connect(socket)?;
    stream.set_read_timeout(Some(Duration::from_secs(40)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    stream.write_all(request.as_bytes())?;
    let mut response = Vec::new();
    stream.take(131_073).read_to_end(&mut response)?;
    if response.len() > 131_072 {
        return Err("the attestation response exceeds 128 KiB".into());
    }
    let split = response
        .windows(4)
        .position(|p| p == b"\r\n\r\n")
        .ok_or("invalid HTTP response")?;
    let head = std::str::from_utf8(&response[..split])?;
    if !head
        .lines()
        .next()
        .is_some_and(|s| s.split_whitespace().nth(1) == Some("200"))
    {
        return Err("the launcher refused the attestation request".into());
    }
    let data = &response[split + 4..];
    let data = if head
        .lines()
        .any(|s| s.eq_ignore_ascii_case("transfer-encoding: chunked"))
    {
        unchunk(data)?
    } else {
        data.to_vec()
    };
    let token = std::str::from_utf8(&data)?.trim();
    if token.split('.').count() != 3 {
        return Err("the launcher did not return a JWT".into());
    }
    Ok(token.to_owned())
}
