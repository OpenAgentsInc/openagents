use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

use arc_client::{
    ArcEnvironmentInfo, ArcOpenScorecardRequest, ArcRemoteClient, ArcRemoteRetryPolicy,
    LocalArcEnvironment, RemoteArcEnvironment,
};
use arc_core::{ArcAction, ArcActionKind, ArcGameState, ArcOperationMode, ArcTaskId};

fn fixture_path(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("engine")
        .join("fixtures")
        .join(name)
}

#[test]
fn local_wrapper_executes_translated_fixture() {
    let info = ArcEnvironmentInfo {
        game_id: ArcTaskId::new("bt11-fd9df0622a1a").expect("task id should validate"),
        title: Some("BT11".to_owned()),
        tags: Vec::new(),
        private_tags: Vec::new(),
        level_tags: Vec::new(),
        baseline_actions: vec![4],
        class_name: None,
        local_package_path: None,
    };

    let mut environment = LocalArcEnvironment::load_from_path(
        info,
        fixture_path("upstream/bt11-fd9df0622a1a.json"),
        "local-card",
    )
    .expect("local fixture should load");

    let reset = environment.reset().expect("reset should succeed");
    assert_eq!(reset.game_state, ArcGameState::NotFinished);
    assert!(reset.full_reset);
    assert_eq!(
        reset.available_actions,
        vec![ArcActionKind::Action3, ArcActionKind::Action4]
    );
    assert!(environment.guid().starts_with("local-bt11-fd9df0622a1a-"));

    let step = environment
        .step(ArcAction::Action3)
        .expect("step should succeed");
    assert_eq!(step.game_state, ArcGameState::NotFinished);
    assert_eq!(step.action, ArcAction::Action3);
    assert_eq!(step.guid, reset.guid);
    assert_eq!(environment.scorecard_id(), "local-card");
    assert_eq!(
        environment.action_space(),
        Some([ArcActionKind::Action3, ArcActionKind::Action4].as_slice())
    );
    let recording = environment
        .recording()
        .expect("local recording should be readable")
        .expect("local recording should exist after reset + step");
    assert_eq!(recording.operation_mode, Some(ArcOperationMode::Offline));
    assert_eq!(recording.steps.len(), 2);
}

#[test]
fn remote_client_keeps_cookie_affinity_across_wrapper_steps() {
    let responses = vec![
        MockResponse::json(
            200,
            r#"[{"game_id":"bt11-fd9df0622a1a","title":"BT11"}]"#.to_owned(),
            &[],
        ),
        MockResponse::json(200, r#"{"card_id":"card-1"}"#.to_owned(), &[]),
        MockResponse::json(
            200,
            frame_response_json(
                "bt11-fd9df0622a1a",
                "guid-1",
                "NOT_FINISHED",
                0,
                2,
                0,
                None,
                true,
                &[3, 4],
            ),
            &[(
                "Set-Cookie".to_owned(),
                "AWSALB=sticky-1; Path=/".to_owned(),
            )],
        ),
        MockResponse::json(
            200,
            frame_response_json(
                "bt11-fd9df0622a1a",
                "guid-1",
                "NOT_FINISHED",
                0,
                2,
                3,
                None,
                false,
                &[3, 4],
            ),
            &[],
        ),
    ];
    let (base_url, handle) = spawn_mock_server(responses);

    let client = ArcRemoteClient::new(base_url, "test-key").expect("client should initialize");
    let games = client.list_games().expect("games should load");
    assert_eq!(games.len(), 1);
    assert_eq!(games[0].game_id.as_str(), "bt11-fd9df0622a1a");

    let scorecard = client
        .open_scorecard(&ArcOpenScorecardRequest {
            source_url: None,
            tags: vec!["smoke".to_owned()],
            opaque: None,
            competition_mode: None,
        })
        .expect("scorecard should open");
    assert_eq!(scorecard.card_id, "card-1");

    let mut environment = RemoteArcEnvironment::new(client, games[0].clone(), "card-1");
    let reset = environment.reset().expect("reset should succeed");
    assert_eq!(reset.game_state, ArcGameState::NotFinished);
    assert!(reset.full_reset);

    let step = environment
        .step(ArcAction::Action3)
        .expect("remote step should succeed");
    assert_eq!(step.action, ArcAction::Action3);
    assert_eq!(step.guid, "guid-1");
    assert_eq!(
        environment.action_space(),
        Some([ArcActionKind::Action3, ArcActionKind::Action4].as_slice())
    );
    let recording = environment
        .recording()
        .expect("remote recording should be readable")
        .expect("remote recording should exist after reset + step");
    assert_eq!(recording.operation_mode, Some(ArcOperationMode::Online));
    assert_eq!(recording.steps.len(), 2);

    let requests = handle.join().expect("server should join cleanly");
    assert_eq!(requests.len(), 4);
    assert_eq!(requests[0].path, "/api/games");
    assert_eq!(requests[1].path, "/api/scorecard/open");
    assert_eq!(requests[2].path, "/api/cmd/RESET");
    assert_eq!(requests[3].path, "/api/cmd/ACTION3");
    assert_eq!(
        requests[2].headers.get("x-api-key").map(String::as_str),
        Some("test-key")
    );
    assert_eq!(
        requests[3].headers.get("cookie").map(String::as_str),
        Some("AWSALB=sticky-1")
    );
    assert!(requests[2].body.contains("\"card_id\":\"card-1\""));
    assert!(requests[3].body.contains("\"guid\":\"guid-1\""));
}

#[test]
fn remote_client_retries_rate_limits_without_losing_cookie_affinity() {
    let responses = vec![
        MockResponse::json(
            200,
            r#"[{"game_id":"bt11-fd9df0622a1a","title":"BT11"}]"#.to_owned(),
            &[],
        ),
        MockResponse::json(
            200,
            r#"{"card_id":"card-1"}"#.to_owned(),
            &[(
                "Set-Cookie".to_owned(),
                "AWSALB=sticky-1; Path=/".to_owned(),
            )],
        ),
        MockResponse::json(
            429,
            r#"{"error":"rate_limited"}"#.to_owned(),
            &[("Retry-After".to_owned(), "0".to_owned())],
        ),
        MockResponse::json(
            200,
            frame_response_json(
                "bt11-fd9df0622a1a",
                "guid-1",
                "NOT_FINISHED",
                0,
                2,
                0,
                None,
                true,
                &[3, 4],
            ),
            &[],
        ),
    ];
    let (base_url, handle) = spawn_mock_server(responses);

    let client = ArcRemoteClient::new(base_url, "test-key")
        .expect("client should initialize")
        .with_retry_policy(ArcRemoteRetryPolicy {
            max_retries: 1,
            initial_delay: Duration::ZERO,
            backoff_factor: 1.0,
            max_delay: Duration::ZERO,
        });
    let games = client.list_games().expect("games should load");
    let scorecard = client
        .open_scorecard(&ArcOpenScorecardRequest {
            source_url: None,
            tags: vec!["smoke".to_owned()],
            opaque: None,
            competition_mode: None,
        })
        .expect("scorecard should open");

    let mut environment = RemoteArcEnvironment::new(client, games[0].clone(), scorecard.card_id);
    environment
        .reset()
        .expect("reset should succeed after one retry");

    let requests = handle.join().expect("server should join cleanly");
    assert_eq!(requests.len(), 4);
    assert_eq!(requests[2].path, "/api/cmd/RESET");
    assert_eq!(requests[3].path, "/api/cmd/RESET");
    assert_eq!(
        requests[2].headers.get("cookie").map(String::as_str),
        Some("AWSALB=sticky-1")
    );
    assert_eq!(
        requests[3].headers.get("cookie").map(String::as_str),
        Some("AWSALB=sticky-1")
    );
}

#[derive(Debug)]
struct CapturedRequest {
    path: String,
    headers: BTreeMap<String, String>,
    body: String,
}

#[derive(Clone)]
struct MockResponse {
    status_code: u16,
    body: String,
    headers: Vec<(String, String)>,
}

impl MockResponse {
    fn json(status_code: u16, body: String, headers: &[(String, String)]) -> Self {
        let mut all_headers = vec![("Content-Type".to_owned(), "application/json".to_owned())];
        all_headers.extend_from_slice(headers);
        Self {
            status_code,
            body,
            headers: all_headers,
        }
    }
}

fn spawn_mock_server(
    responses: Vec<MockResponse>,
) -> (String, thread::JoinHandle<Vec<CapturedRequest>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
    let addr = listener
        .local_addr()
        .expect("listener should have an address");
    let handle = thread::spawn(move || serve_requests(listener, responses));
    (format!("http://{addr}"), handle)
}

fn serve_requests(listener: TcpListener, responses: Vec<MockResponse>) -> Vec<CapturedRequest> {
    let mut captured = Vec::with_capacity(responses.len());
    for response in responses {
        let (mut stream, _) = listener.accept().expect("request should arrive");
        let request = read_request(&mut stream).expect("request should parse");
        write_response(&mut stream, &response).expect("response should write");
        captured.push(request);
    }
    captured
}

fn read_request(stream: &mut TcpStream) -> std::io::Result<CapturedRequest> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;

    let path = request_line
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .to_owned();

    let mut headers = BTreeMap::new();
    let mut content_length = 0usize;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line)?;
        if line == "\r\n" || line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            let value = value.trim().to_owned();
            if name.eq_ignore_ascii_case("content-length") {
                content_length = value.parse().unwrap_or_default();
            }
            headers.insert(name.to_ascii_lowercase(), value);
        }
    }

    let mut body = vec![0u8; content_length];
    reader.read_exact(&mut body)?;

    Ok(CapturedRequest {
        path,
        headers,
        body: String::from_utf8_lossy(&body).into_owned(),
    })
}

fn write_response(stream: &mut TcpStream, response: &MockResponse) -> std::io::Result<()> {
    let status_text = match response.status_code {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        429 => "Too Many Requests",
        _ => "Response",
    };
    let mut raw = format!(
        "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nConnection: close\r\n",
        response.status_code,
        status_text,
        response.body.len()
    );
    for (name, value) in &response.headers {
        raw.push_str(name);
        raw.push_str(": ");
        raw.push_str(value);
        raw.push_str("\r\n");
    }
    raw.push_str("\r\n");
    raw.push_str(&response.body);
    stream.write_all(raw.as_bytes())
}

fn frame_response_json(
    game_id: &str,
    guid: &str,
    state: &str,
    levels_completed: u16,
    win_levels: u16,
    action_id: u8,
    action6: Option<(u8, u8)>,
    full_reset: bool,
    available_actions: &[u8],
) -> String {
    let mut action_input = serde_json::json!({ "id": action_id, "data": {} });
    if let Some((x, y)) = action6 {
        action_input = serde_json::json!({ "id": action_id, "data": { "x": x, "y": y } });
    }
    serde_json::json!({
        "game_id": game_id,
        "guid": guid,
        "frame": [solid_frame(7)],
        "state": state,
        "levels_completed": levels_completed,
        "win_levels": win_levels,
        "action_input": action_input,
        "available_actions": available_actions,
        "full_reset": full_reset,
    })
    .to_string()
}

fn solid_frame(color: u8) -> Vec<Vec<u8>> {
    vec![vec![color; 64]; 64]
}
