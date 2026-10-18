mod common;

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use archil::{Error, FileSource, Sandbox, SandboxFileUploadOptions, TransferOperation};
use bytes::Bytes;
use common::ws::*;
use common::*;
use futures_util::stream;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer};

const SID: &str = "11111111-1111-1111-1111-111111111111";
const CHUNK: usize = 512 * 1024;

async fn sandbox(
    server: &MockServer,
    url: String,
) -> (Sandbox, Arc<std::sync::atomic::AtomicUsize>) {
    Mock::given(method("GET"))
        .and(path(format!("/api/sandboxes/{SID}")))
        .respond_with(ok(sandbox_json(SID, "running")))
        .mount(server)
        .await;
    let hits = mount_connections(server, SID, vec![url]).await;
    (client(server).sandboxes().get(SID).await.unwrap(), hits)
}

/// Fake runtime for uploads: records the start request and everything piped to stdin.
async fn upload_runtime(
    record: Arc<Mutex<(serde_json::Value, Vec<u8>)>>,
    exit_with: Option<(&'static str, i64)>,
) -> String {
    runtime(move |mut ws, i| {
        let record = record.clone();
        async move {
            let _ = i;
            let first = recv_json(&mut ws).await;
            if first["type"] == "kill" {
                send_json(&mut ws, json!({"type": "killed"})).await;
                let _ = futures_util::StreamExt::next(&mut ws).await;
                return;
            }
            started(&mut ws, "up").await;
            let mut body = Vec::new();
            loop {
                match futures_util::StreamExt::next(&mut ws).await {
                    Some(Ok(tokio_tungstenite::tungstenite::Message::Binary(b))) => {
                        body.extend_from_slice(&b)
                    }
                    Some(Ok(tokio_tungstenite::tungstenite::Message::Text(t))) => {
                        assert_eq!(t.as_str(), r#"{"type":"close_stdin"}"#);
                        break;
                    }
                    _ => return,
                }
            }
            *record.lock().unwrap() = (first, body);
            match exit_with {
                None => exit(&mut ws, "completed", 0, Some(0)).await,
                Some((stderr, code)) => {
                    send_out(&mut ws, 2, 0, stderr.as_bytes()).await;
                    exit(&mut ws, "failed", stderr.len() as u64, Some(code)).await;
                }
            }
            let _ = futures_util::StreamExt::next(&mut ws).await;
        }
    })
    .await
}

#[tokio::test]
async fn upload_pipes_bytes_through_the_atomic_write_script() {
    let server = MockServer::start().await;
    let record = Arc::new(Mutex::new((json!(null), vec![])));
    let url = upload_runtime(record.clone(), None).await;
    let (sb, _) = sandbox(&server, url).await;
    sb.files()
        .upload_file(
            b"hello world".to_vec(),
            "/srv/app/data.txt",
            SandboxFileUploadOptions { mode: 0o755 },
        )
        .await
        .unwrap();
    let (start, body) = record.lock().unwrap().clone();
    assert_eq!(body, b"hello world");
    assert_eq!(start["type"], "start");
    let env = &start["env"];
    assert_eq!(env["ARCHIL_FILE_PARENT"], "/srv/app");
    assert_eq!(env["ARCHIL_FILE_TARGET"], "/srv/app/data.txt");
    assert_eq!(env["ARCHIL_FILE_MODE"], "755", "mode is passed in octal");
    assert!(
        env["ARCHIL_FILE_TEMP"]
            .as_str()
            .unwrap()
            .starts_with("/srv/app/.archil-upload-")
    );
    let cmd = start["command"].as_str().unwrap();
    assert!(
        cmd.contains("cat > \"$ARCHIL_FILE_TEMP\"")
            && cmd.contains("mv -f \"$ARCHIL_FILE_TEMP\" \"$ARCHIL_FILE_TARGET\"")
    );
}

#[tokio::test]
async fn upload_accepts_streams_and_readers_and_root_files() {
    let server = MockServer::start().await;
    let record = Arc::new(Mutex::new((json!(null), vec![])));
    let url = upload_runtime(record.clone(), None).await;
    let (sb, _) = sandbox(&server, url).await;
    let chunks = stream::iter(vec![
        Ok(Bytes::from_static(b"ab")),
        Ok(Bytes::from_static(b"cd")),
    ]);
    sb.files()
        .upload_file(FileSource::stream(chunks), "/top.bin", Default::default())
        .await
        .unwrap();
    let (start, body) = record.lock().unwrap().clone();
    assert_eq!(body, b"abcd");
    assert_eq!(
        start["env"]["ARCHIL_FILE_PARENT"], "/",
        "a file in / has parent /"
    );
    assert_eq!(start["env"]["ARCHIL_FILE_MODE"], "644", "default mode");

    let reader = std::io::Cursor::new(vec![9u8; 700_000]);
    sb.files()
        .upload_file(FileSource::reader(reader), "/r.bin", Default::default())
        .await
        .unwrap();
    assert_eq!(record.lock().unwrap().1.len(), 700_000);
}

#[tokio::test]
async fn failed_upload_reports_stderr_and_kills_the_process() {
    let server = MockServer::start().await;
    let record = Arc::new(Mutex::new((json!(null), vec![])));
    let url = upload_runtime(record, Some(("No space left on device\n", 1))).await;
    let (sb, conns) = sandbox(&server, url).await;
    let e = sb
        .files()
        .upload_file("x", "/data/f", Default::default())
        .await
        .unwrap_err();
    let Error::FileTransfer(ref t) = e else {
        panic!("{e:?}")
    };
    assert_eq!(
        (t.operation, t.path.as_str()),
        (TransferOperation::Upload, "/data/f")
    );
    assert_eq!(
        e.to_string(),
        "Sandbox file upload failed for /data/f: No space left on device"
    );
    assert_eq!(
        (e.status(), e.code()),
        (Some(500), Some("SANDBOX_FILE_TRANSFER_FAILED"))
    );
    assert_eq!(
        conns.load(Ordering::SeqCst),
        1,
        "an already-exited process needs no kill"
    );
}

#[tokio::test]
async fn upload_source_error_kills_the_still_running_process() {
    let server = MockServer::start().await;
    let record = Arc::new(Mutex::new((json!(null), vec![])));
    let url = upload_runtime(record, None).await;
    let (sb, conns) = sandbox(&server, url).await;
    let chunks = stream::iter(vec![
        Ok(Bytes::from_static(b"partial")),
        Err(std::io::Error::new(
            std::io::ErrorKind::BrokenPipe,
            "source went away",
        )),
    ]);
    let e = sb
        .files()
        .upload_file(FileSource::stream(chunks), "/data/f", Default::default())
        .await
        .unwrap_err();
    assert!(matches!(e, Error::FileTransfer(_)));
    assert!(e.to_string().contains("source went away"), "{e}");
    assert_eq!(
        conns.load(Ordering::SeqCst),
        2,
        "the running process is killed over a control connection"
    );
}

#[tokio::test]
async fn upload_validates_arguments_before_connecting() {
    let server = MockServer::start().await;
    let record = Arc::new(Mutex::new((json!(null), vec![])));
    let url = upload_runtime(record, None).await;
    let (sb, conns) = sandbox(&server, url).await;
    for bad in ["relative/path", "/dir/", "/a\0b"] {
        let e = sb
            .files()
            .upload_file("x", bad, Default::default())
            .await
            .unwrap_err();
        assert!(matches!(e, Error::InvalidArgument(_)), "{bad:?}: {e:?}");
    }
    let e = sb
        .files()
        .upload_file("x", "/ok", SandboxFileUploadOptions { mode: 0o10000 })
        .await
        .unwrap_err();
    assert_eq!(e.to_string(), "mode must be between 0 and 0o7777");
    assert_eq!(conns.load(Ordering::SeqCst), 0);
}

/// Fake runtime that emulates the download shell loop over `file`.
async fn download_runtime(file: Vec<u8>) -> String {
    runtime(move |mut ws, _| {
        let file = file.clone();
        async move {
            let start = recv_json(&mut ws).await;
            assert!(
                start["command"]
                    .as_str()
                    .unwrap()
                    .contains("dd bs=\"$count\" count=1 <&3")
            );
            assert!(
                start["env"]["ARCHIL_FILE_TEMP"]
                    .as_str()
                    .unwrap()
                    .starts_with("/tmp/.archil-download-")
            );
            started(&mut ws, "dl").await;
            let (mut pos, mut offset) = (0usize, 0u64);
            loop {
                let req = recv_bin(&mut ws).await;
                let count: usize = std::str::from_utf8(&req).unwrap().trim().parse().unwrap();
                let end = (pos + count).min(file.len());
                let chunk = &file[pos..end];
                pos = end;
                let header = format!("{}\n", chunk.len());
                // Split the reply across frames to exercise client-side buffering.
                send_out(&mut ws, 1, offset, header.as_bytes()).await;
                offset += header.len() as u64;
                for piece in chunk.chunks(100_000) {
                    send_out(&mut ws, 1, offset, piece).await;
                    offset += piece.len() as u64;
                }
                if chunk.len() != count {
                    break;
                }
            }
            exit(&mut ws, "completed", offset, Some(0)).await;
            let _ = futures_util::StreamExt::next(&mut ws).await;
        }
    })
    .await
}

fn pattern(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8).collect()
}

#[tokio::test]
async fn download_returns_files_of_every_size_class() {
    for len in [0, 10, CHUNK - 1, CHUNK, CHUNK + 1, 2 * CHUNK + 17] {
        let server = MockServer::start().await;
        let file = pattern(len);
        let url = download_runtime(file.clone()).await;
        let (sb, _) = sandbox(&server, url).await;
        let got = sb
            .files()
            .download_bytes("/data/file.bin")
            .await
            .unwrap_or_else(|e| panic!("{len}: {e}"));
        assert_eq!(got.len(), len, "length for {len}");
        assert!(got == file, "contents for {len}");
    }
}

#[tokio::test]
async fn download_streams_chunks_and_writes_to_an_async_writer() {
    let server = MockServer::start().await;
    let file = pattern(CHUNK + 5);
    let url = download_runtime(file.clone()).await;
    let (sb, _) = sandbox(&server, url).await;
    let mut dl = sb.files().download("/f").await.unwrap();
    let mut sizes = vec![];
    while let Some(c) = dl.next_chunk().await.unwrap() {
        sizes.push(c.len());
    }
    assert_eq!(sizes, [CHUNK, 5]);
    assert!(
        dl.next_chunk().await.unwrap().is_none(),
        "finished downloads stay finished"
    );

    let url = download_runtime(file.clone()).await;
    let server2 = MockServer::start().await;
    let (sb2, _) = sandbox(&server2, url).await;
    let mut out = Vec::new();
    sb2.files().download_file("/f", &mut out).await.unwrap();
    assert!(out == file);
}

#[tokio::test]
async fn download_of_a_missing_file_reports_stderr_without_hanging() {
    let server = MockServer::start().await;
    let url = runtime(|mut ws, _| async move {
        let _ = recv_json(&mut ws).await;
        started(&mut ws, "dl").await;
        let msg = b"sh: 3: cannot open /nope: No such file\n";
        send_out(&mut ws, 2, 0, msg).await;
        // The process dies but the socket stays open, as in the real runtime.
        send_json(
            &mut ws,
            json!({"type": "exit", "status": "failed", "cursor": msg.len(), "exit_code": 2}),
        )
        .await;
        loop {
            match futures_util::StreamExt::next(&mut ws).await {
                Some(Ok(_)) => continue,
                _ => return,
            }
        }
    })
    .await;
    let (sb, _) = sandbox(&server, url).await;
    let e = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        sb.files().download_bytes("/nope"),
    )
    .await
    .expect("must not hang")
    .unwrap_err();
    let Error::FileTransfer(ref t) = e else {
        panic!("{e:?}")
    };
    assert_eq!(
        (t.operation, t.path.as_str()),
        (TransferOperation::Download, "/nope")
    );
    assert!(e.to_string().contains("cannot open /nope"), "{e}");
}

#[tokio::test]
async fn download_rejects_bad_chunk_sizes_and_output_gaps() {
    let server = MockServer::start().await;
    let starts = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let url = runtime(move |mut ws, _| {
        let starts = starts.clone();
        async move {
            let first = recv_json(&mut ws).await;
            if first["type"] == "kill" {
                send_json(&mut ws, json!({"type": "killed"})).await;
                let _ = futures_util::StreamExt::next(&mut ws).await;
                return;
            }
            let i = starts.fetch_add(1, Ordering::SeqCst);
            started(&mut ws, "dl").await;
            let _ = recv_bin(&mut ws).await;
            match i {
                0 => send_out(&mut ws, 1, 0, b"abc\n").await,
                1 => send_out(&mut ws, 1, 0, b"999999999\n").await,
                _ => {
                    send_out(&mut ws, 1, 0, b"3\n").await;
                    send_out(&mut ws, 1, 50, b"xyz").await; // bytes 2..50 never arrive
                }
            }
            let _ = futures_util::StreamExt::next(&mut ws).await;
        }
    })
    .await;
    let (sb, _) = sandbox(&server, url).await;
    let e = sb.files().download_bytes("/f").await.unwrap_err();
    assert!(
        e.to_string().contains(r#"invalid chunk size: "abc""#),
        "{e}"
    );
    let e = sb.files().download_bytes("/f").await.unwrap_err();
    assert!(e.to_string().contains("invalid chunk size"), "{e}");
    let e = sb.files().download_bytes("/f").await.unwrap_err();
    assert!(
        e.to_string()
            .contains("output gap: expected offset 2, received 50"),
        "{e}"
    );
}

#[tokio::test]
async fn download_validates_the_path() {
    let server = MockServer::start().await;
    let (sb, conns) = sandbox(&server, "ws://127.0.0.1:1/unused".into()).await;
    let e = sb.files().download_bytes("relative").await.unwrap_err();
    assert!(matches!(e, Error::InvalidArgument(_)));
    assert_eq!(conns.load(Ordering::SeqCst), 0);
}
