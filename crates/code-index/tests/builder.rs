mod common;
use code_index::{builder::*, recipes::Context};
use serde_json::json;

fn prepared(context: &Context) -> Request {
    prepare(
        context,
        "Add a kind",
        "teacher-pinned",
        &[SourceRequest {
            path: "src/lib.rs".into(),
            symbol: Some("demo::Kind".into()),
            start: 0,
            end: 0,
        }],
        Budget::default(),
    )
    .unwrap()
}

#[test]
fn frozen_builder_replays_to_identical_artifacts_and_rejects_tampering() {
    let dir = common::fixture();
    let context = Context::open(dir.path(), "HEAD", "test").unwrap();
    let request = prepared(&context);
    let response = Response {
        request_digest: request_digest(&request).unwrap(),
        model: "teacher-pinned".into(),
        content: Content {
            artifact: Some(common::artifact(&context)),
            need_sources: vec![],
        },
        usage: Some((100, 50)),
        cost_usd: None,
    };
    let built = build(
        &context,
        request,
        &mut Frozen {
            responses: vec![response].into(),
        },
    )
    .unwrap();
    let repeated = replay(&context, &built).unwrap();
    assert_eq!(
        serde_json::to_vec(&built.artifact).unwrap(),
        serde_json::to_vec(&repeated.artifact).unwrap()
    );
    assert_eq!(built.cost_usd, None);
    let mut tampered = built;
    tampered.responses[0].request_digest = "wrong".into();
    assert!(replay(&context, &tampered).is_err());
}

#[test]
fn private_context_and_input_budget_stop_before_the_teacher_call() {
    struct Never;
    impl Teacher for Never {
        fn generate(&mut self, _: &Request) -> Result<Response, String> {
            panic!("must not call")
        }
    }
    let dir = common::fixture();
    let context = Context::open(dir.path(), "HEAD", "test").unwrap();
    let mut request = prepared(&context);
    request.private_context = Some(json!({"text":"private"}));
    assert!(build(&context, request, &mut Never).is_err());
    let mut request = prepared(&context);
    request.budget.total_input_bytes = 1;
    assert!(build(&context, request, &mut Never).is_err());
}

#[test]
fn source_expansion_is_bounded_and_invented_references_fail() {
    let dir = common::fixture();
    let context = Context::open(dir.path(), "HEAD", "test").unwrap();
    let mut request = prepared(&context);
    request.budget.calls = 1;
    let response = Response {
        request_digest: request_digest(&request).unwrap(),
        model: "teacher".into(),
        content: Content {
            artifact: None,
            need_sources: vec![SourceRequest {
                path: "src/lib.rs".into(),
                symbol: Some("demo::name".into()),
                start: 0,
                end: 0,
            }],
        },
        usage: None,
        cost_usd: None,
    };
    assert!(
        build(
            &context,
            request,
            &mut Frozen {
                responses: vec![response].into()
            }
        )
        .unwrap_err()
        .contains("model-call budget")
    );
    let request = prepared(&context);
    let mut artifact = common::artifact(&context);
    artifact.recipes[0].roles[0].references[0].symbol = Some("demo::Missing".into());
    let response = Response {
        request_digest: request_digest(&request).unwrap(),
        model: "teacher".into(),
        content: Content {
            artifact: Some(artifact),
            need_sources: vec![],
        },
        usage: None,
        cost_usd: None,
    };
    assert!(
        build(
            &context,
            request,
            &mut Frozen {
                responses: vec![response].into()
            }
        )
        .is_err()
    );
}

#[test]
fn transport_refuses_remote_plaintext_and_embedded_credentials() {
    assert!(HttpTeacher::new("http://example.org/completions", None, 1).is_err());
    assert!(HttpTeacher::new("https://user:password@example.org/completions", None, 1).is_err());
}

#[test]
fn http_teacher_sends_selected_model_and_decodes_structured_content() {
    use std::io::{Read, Write};
    let dir = common::fixture();
    let context = Context::open(dir.path(), "HEAD", "test").unwrap();
    let request = prepared(&context);
    let response=serde_json::to_vec(&json!({"model":"teacher-pinned","choices":[{"message":{"content":serde_json::to_string(&Content{artifact:Some(common::artifact(&context)),need_sources:vec![]}).unwrap()}}],"usage":{"prompt_tokens":20,"completion_tokens":10}})).unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let worker = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(std::time::Duration::from_secs(3)))
            .unwrap();
        let mut received = Vec::new();
        let mut buf = [0; 4096];
        loop {
            let n = socket.read(&mut buf).unwrap();
            assert_ne!(n, 0);
            received.extend_from_slice(&buf[..n]);
            if let Some(end) = received.windows(4).position(|v| v == b"\r\n\r\n") {
                let header = String::from_utf8_lossy(&received[..end]).to_lowercase();
                let length: usize = header
                    .lines()
                    .find_map(|l| l.strip_prefix("content-length: "))
                    .unwrap()
                    .parse()
                    .unwrap();
                if received.len() >= end + 4 + length {
                    let body: serde_json::Value =
                        serde_json::from_slice(&received[end + 4..end + 4 + length]).unwrap();
                    assert_eq!(body["model"], "teacher-pinned");
                    assert_eq!(body["max_tokens"], 12000);
                    assert!(!header.contains("authorization:"));
                    break;
                }
            }
        }
        write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",response.len()).unwrap();
        socket.write_all(&response).unwrap();
    });
    let mut teacher =
        HttpTeacher::new(&format!("http://{address}/chat/completions"), None, 3).unwrap();
    let result = build(&context, request, &mut teacher).unwrap();
    assert_eq!(result.responses[0].usage, Some((20, 10)));
    worker.join().unwrap();
}
