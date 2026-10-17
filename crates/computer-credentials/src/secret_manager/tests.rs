use super::*;
use serde_json::json;
use std::{
    io::{Read, Write},
    net::TcpListener,
};
struct Token;
#[async_trait]
impl AccessToken for Token {
    async fn token(&self) -> Result<Secret, Failure> {
        Secret::new(b"synthetic-google-token".to_vec())
    }
}
fn version() -> Version {
    Version::parse("projects/123/secrets/model/versions/7").unwrap()
}
fn server(
    status: u16,
    body: serde_json::Value,
    location: Option<String>,
) -> (GoogleSecretManager, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let worker = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut request = Vec::new();
        let mut chunk = [0; 2048];
        while !request.windows(4).any(|s| s == b"\r\n\r\n") {
            let n = stream.read(&mut chunk).unwrap();
            assert!(n > 0);
            request.extend_from_slice(&chunk[..n]);
        }
        let request = String::from_utf8(request).unwrap();
        assert!(
            request
                .starts_with("GET /v1/projects/123/secrets/model/versions/7:access HTTP/1.1\r\n")
        );
        assert!(
            request
                .to_ascii_lowercase()
                .contains("authorization: bearer synthetic-google-token\r\n")
        );
        let body = serde_json::to_vec(&body).unwrap();
        let location = location.map_or(String::new(), |v| format!("Location: {v}\r\n"));
        write!(stream,"HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{location}Connection: close\r\n\r\n",body.len()).unwrap();
        let _ = stream.write_all(&body);
    });
    (
        GoogleSecretManager::local(Arc::new(Token), origin).unwrap(),
        worker,
    )
}
fn response(value: &[u8]) -> serde_json::Value {
    json!({"name":version().as_str(),"payload":{"data":STANDARD.encode(value),"dataCrc32c":crc32c::crc32c(value).to_string()}})
}
#[test]
fn only_numeric_pinned_secret_versions_are_valid() {
    for bad in [
        "projects/project-name/secrets/x/versions/1",
        "projects/123/secrets/x/versions/latest",
        "projects/123/secrets/x/versions/01",
        "projects/123/secrets/x/versions/0",
        "projects/0123/secrets/x/versions/1",
        "projects/123/secrets/../versions/1",
        "projects/123/secrets/x%2fy/versions/1",
        "https://secretmanager.googleapis.com/projects/123/secrets/x/versions/1",
    ] {
        assert!(Version::parse(bad).is_err(), "{bad}");
    }
    assert!(format!("{:?}", version()).contains("registered source"));
    assert!(!format!("{:?}", version()).contains("projects/"));
}
#[tokio::test]
async fn a_google_response_requires_the_pinned_name_and_valid_crc() {
    let (reader, worker) = server(200, response(b"synthetic-value"), None);
    let value = reader.access(&version()).await.unwrap();
    assert_eq!(value.bytes(), b"synthetic-value");
    worker.join().unwrap();
    for mode in 0..4 {
        let mut body = response(b"synthetic-value");
        match mode {
            0 => body["name"] = json!("projects/123/secrets/another/versions/7"),
            1 => body["payload"]["dataCrc32c"] = json!("0"),
            2 => {
                body["payload"]
                    .as_object_mut()
                    .unwrap()
                    .remove("dataCrc32c");
            }
            _ => body["payload"]["data"] = json!("not-base64"),
        }
        let (reader, worker) = server(200, body, None);
        assert_eq!(
            reader.access(&version()).await.unwrap_err(),
            Failure::Unavailable
        );
        worker.join().unwrap();
    }
}
#[tokio::test]
async fn redirects_outages_and_oversized_payloads_do_not_return_a_value() {
    let target = TcpListener::bind("127.0.0.1:0").unwrap();
    target.set_nonblocking(true).unwrap();
    let (reader, worker) = server(
        302,
        json!({}),
        Some(format!("http://{}/secret", target.local_addr().unwrap())),
    );
    assert_eq!(
        reader.access(&version()).await.unwrap_err(),
        Failure::Unavailable
    );
    worker.join().unwrap();
    assert_eq!(
        target.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    for (status, body) in [
        (403, json!({"error":"synthetic"})),
        (503, json!({})),
        (200, response(&vec![b'a'; MAX_SECRET_BYTES + 1])),
    ] {
        let (reader, worker) = server(status, body, None);
        assert_eq!(
            reader.access(&version()).await.unwrap_err(),
            Failure::Unavailable
        );
        worker.join().unwrap();
    }
}
