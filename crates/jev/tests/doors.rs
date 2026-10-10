//! Jev's fallback doors with stand-in doors on loopback (`jev::doors`): a
//! TypeSafe that cannot pay falls through to the gateway, then OpenRouter;
//! every door's answer reads the same; a refusal of the question itself
//! never fails over; the answering door is named.

use std::sync::Arc;
use std::time::Duration;

use jev::doors::{self, Door, Failover, Naming};
use jev::{ApiKey, Client, Config, Error, RetryPolicy, SystemOneRequest};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Mutex;

type Outcome = Result<(), Box<dyn std::error::Error>>;

/// TypeSafe's answer to the fixture request.
const TYPESAFE_ANSWER: &str = include_str!("fixtures/systemone-response.json");

/// What one stand-in door does with every request.
#[derive(Clone)]
enum Behavior {
    /// Answer with this status and body.
    Answer(u16, String),
    /// Answer after a wait.
    Slow(Duration, u16, String),
    /// Close the connection unread.
    Drop,
}

/// One request a stand-in door read: its path, its bearer, and its body.
#[derive(Debug, Clone)]
struct Seen {
    path: String,
    bearer: String,
    body: Value,
}

/// A stand-in door that behaves the same for every request and records
/// what it read.
async fn door(behavior: Behavior) -> Result<(String, Arc<Mutex<Vec<Seen>>>), std::io::Error> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let base = format!("http://{}", listener.local_addr()?);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let recording = Arc::clone(&seen);
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let recording = Arc::clone(&recording);
            let behavior = behavior.clone();
            tokio::spawn(async move {
                let (status, body) = match behavior {
                    Behavior::Drop => return,
                    Behavior::Answer(status, body) => (status, body),
                    Behavior::Slow(wait, status, body) => {
                        tokio::time::sleep(wait).await;
                        (status, body)
                    }
                };
                let Some(request) = read(&mut socket).await else {
                    return;
                };
                recording.lock().await.push(request);
                let head = format!(
                    "HTTP/1.1 {status} Door\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(head.as_bytes()).await;
                let _ = socket.flush().await;
            });
        }
    });
    Ok((base, seen))
}

async fn read(socket: &mut tokio::net::TcpStream) -> Option<Seen> {
    let mut bytes = Vec::new();
    let split = loop {
        let mut chunk = [0; 4096];
        let read = socket.read(&mut chunk).await.ok()?;
        if read == 0 {
            return None;
        }
        bytes.extend_from_slice(&chunk[..read]);
        if let Some(at) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break at;
        }
    };
    let head = String::from_utf8(bytes[..split].to_vec()).ok()?;
    let mut lines = head.split("\r\n");
    let path = lines.next()?.split_whitespace().nth(1)?.to_string();
    let mut length = 0;
    let mut bearer = String::new();
    for line in lines {
        let (name, value) = line.split_once(':')?;
        match name.to_lowercase().as_str() {
            "content-length" => length = value.trim().parse().ok()?,
            "authorization" => bearer = value.trim().to_string(),
            _ => {}
        }
    }
    while bytes.len() < split + 4 + length {
        let mut chunk = [0; 4096];
        let read = socket.read(&mut chunk).await.ok()?;
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..read]);
    }
    Some(Seen {
        path,
        bearer,
        body: serde_json::from_slice(&bytes[split + 4..]).unwrap_or(Value::Null),
    })
}

/// The gateway's answer: TypeSafe's shape under its model name, with its
/// routing and cost (Vercel, "TypeSafe API with AI Gateway").
fn gateway_answer() -> String {
    let mut answer: Value = serde_json::from_str(TYPESAFE_ANSWER).unwrap();
    answer["model"] = json!("typesafe-ai/jev");
    answer["provider_metadata"] = json!({"gateway": {
        "routing": {"originalModelId": "typesafe-ai/jev", "finalProvider": "typesafe-ai"},
        "cost": "0.00001978",
        "generationId": "gen_test"
    }});
    answer.to_string()
}

/// OpenRouter's answer: TypeSafe's shape with its id, provider, dated
/// model, and priced usage.
fn openrouter_answer() -> String {
    let mut answer: Value = serde_json::from_str(TYPESAFE_ANSWER).unwrap();
    answer["model"] = json!("typesafe/jev-1.13-20260917");
    answer["id"] = json!("gen-or-test");
    answer["provider"] = json!("TypeSafe");
    answer["usage"]["cost"] = json!(0.0000198);
    answer.to_string()
}

const NO_CREDITS: &str = r#"{"error": {"code": "payment_required", "message": "Your organization has no available TypeSafe API credits."}}"#;

/// A client asking `typesafe` first, then each `(door name, url, naming)`.
fn failover_client(
    typesafe: &str,
    fallbacks: &[(&str, String, Naming)],
    cap: Option<Duration>,
) -> Result<Client, Box<dyn std::error::Error>> {
    benched_client(typesafe, fallbacks, cap, doors::BENCH)
}

/// [`failover_client`] with its own bench for a door refused for its key
/// or account.
fn benched_client(
    typesafe: &str,
    fallbacks: &[(&str, String, Naming)],
    cap: Option<Duration>,
    bench: Duration,
) -> Result<Client, Box<dyn std::error::Error>> {
    let mut failover = Failover::new(
        Door::new(
            doors::TYPESAFE_DOOR,
            typesafe,
            Naming::Canonical,
            ApiKey::new("ts-key"),
        ),
        fallbacks
            .iter()
            .enumerate()
            .map(|(at, (name, url, naming))| {
                Door::new(
                    *name,
                    url.clone(),
                    *naming,
                    ApiKey::new(format!("fallback-key-{at}")),
                )
            })
            .collect(),
    );
    failover = failover.bench(bench);
    if let Some(cap) = cap {
        failover = failover.primary_timeout(cap);
    }
    Ok(Client::new(
        Config::new()
            .exchange(doors::exchange(failover))
            .base_url(doors::TYPESAFE_DOOR)
            .default_model("jev-1.13.0")
            .timeout(Duration::from_secs(5))
            .retry(RetryPolicy {
                max_retries: 0,
                ..RetryPolicy::default()
            }),
    )?)
}

fn fixture_request() -> SystemOneRequest {
    let body: Value =
        serde_json::from_str(include_str!("fixtures/systemone-request.json")).unwrap();
    let decision = jev::DecisionRequest::from_value(body).unwrap();
    SystemOneRequest::from_decision(decision).model("jev-1.13.0")
}

#[tokio::test]
async fn no_credits_at_typesafe_falls_through_the_gateway_then_openrouter() -> Outcome {
    let (typesafe, typesafe_seen) = door(Behavior::Answer(402, NO_CREDITS.into())).await?;
    let (gateway, gateway_seen) = door(Behavior::Answer(
        503,
        r#"{"message": "No provider is available", "error_type": "internal_server_error"}"#.into(),
    ))
    .await?;
    let (openrouter, openrouter_seen) = door(Behavior::Answer(200, openrouter_answer())).await?;
    let client = failover_client(
        &typesafe,
        &[
            (
                doors::GATEWAY_DOOR,
                format!("{gateway}/typesafe/v1/systemone"),
                Naming::Gateway,
            ),
            (
                doors::OPENROUTER_DOOR,
                format!("{openrouter}/api/alpha/decisions"),
                Naming::OpenRouter,
            ),
        ],
        None,
    )?;
    let response = client.system_one(fixture_request()).await?;

    // Each door was asked once, under its own key, by its own name for Jev,
    // with the same state and questions.
    let (typesafe, gateway, openrouter) = (
        typesafe_seen.lock().await.clone(),
        gateway_seen.lock().await.clone(),
        openrouter_seen.lock().await.clone(),
    );
    assert_eq!((typesafe.len(), gateway.len(), openrouter.len()), (1, 1, 1));
    assert_eq!(typesafe[0].path, "/v1/systemone");
    assert_eq!(typesafe[0].bearer, "Bearer ts-key");
    assert_eq!(typesafe[0].body["model"], "jev-1.13.0");
    assert_eq!(gateway[0].path, "/typesafe/v1/systemone");
    assert_eq!(gateway[0].bearer, "Bearer fallback-key-0");
    assert_eq!(gateway[0].body["model"], "typesafe-ai/jev");
    assert_eq!(openrouter[0].path, "/api/alpha/decisions");
    assert_eq!(openrouter[0].bearer, "Bearer fallback-key-1");
    assert_eq!(openrouter[0].body["model"], "typesafe/jev-1.13");
    for seen in [&gateway[0], &openrouter[0]] {
        assert_eq!(seen.body["state"], typesafe[0].body["state"]);
        assert_eq!(seen.body["questions"], typesafe[0].body["questions"]);
    }

    // The answer reads as TypeSafe's would, and names the door that gave it.
    let typesafe_reading = jev::SystemOneResponse::decode(jev::RawResponse {
        status: 200,
        headers: Default::default(),
        bytes: TYPESAFE_ANSWER.as_bytes().to_vec(),
    })?;
    assert_eq!(response.answers_value(), typesafe_reading.answers_value());
    assert_eq!(response.model, "typesafe/jev-1.13-20260917");
    assert_eq!(
        response.service(),
        Some(json!({"door": doors::OPENROUTER_DOOR}))
    );
    assert_eq!(response.usage.cost, Some(0.0000198));
    // A client that fails over across its own doors is direct, not hosted.
    assert_eq!(client.service(), None);
    assert!(
        client
            .doors()
            .is_some_and(|doors| doors.contains(doors::GATEWAY_DOOR))
    );
    Ok(())
}

#[tokio::test]
async fn the_gateway_answers_first_and_openrouter_is_not_asked() -> Outcome {
    let (typesafe, _) = door(Behavior::Answer(402, NO_CREDITS.into())).await?;
    let (gateway, _) = door(Behavior::Answer(200, gateway_answer())).await?;
    let (openrouter, openrouter_seen) = door(Behavior::Answer(200, openrouter_answer())).await?;
    let client = failover_client(
        &typesafe,
        &[
            (
                doors::GATEWAY_DOOR,
                format!("{gateway}/typesafe/v1/systemone"),
                Naming::Gateway,
            ),
            (
                doors::OPENROUTER_DOOR,
                format!("{openrouter}/api/alpha/decisions"),
                Naming::OpenRouter,
            ),
        ],
        None,
    )?;
    let response = client.system_one(fixture_request()).await?;
    assert_eq!(response.model, "typesafe-ai/jev");
    assert_eq!(
        response.service(),
        Some(json!({"door": doors::GATEWAY_DOOR}))
    );
    assert_eq!(response.usage.cost, Some(0.000_019_78));
    assert_eq!(response.usage.input_tokens, Some(471));
    assert!(openrouter_seen.lock().await.is_empty());

    // The decision record names the gateway as the door that answered.
    let record = jev_record(&client, &response);
    assert_eq!(record["service"]["door"], doors::GATEWAY_DOOR);
    Ok(())
}

/// The fields a decision record reads off a response.
fn jev_record(client: &Client, response: &jev::SystemOneResponse) -> Value {
    json!({
        "door": client.base_url(),
        "service": response.service(),
        "model": response.model,
        "cost": response.usage.cost_usd(),
    })
}

#[tokio::test]
async fn a_refusal_of_the_question_never_fails_over() -> Outcome {
    let (typesafe, _) = door(Behavior::Answer(
        400,
        r#"{"error": {"code": "invalid_request", "message": "questions.x.type is not a question type"}}"#.into(),
    ))
    .await?;
    let (gateway, gateway_seen) = door(Behavior::Answer(200, gateway_answer())).await?;
    let client = failover_client(
        &typesafe,
        &[(
            doors::GATEWAY_DOOR,
            format!("{gateway}/typesafe/v1/systemone"),
            Naming::Gateway,
        )],
        None,
    )?;
    let error = client.system_one(fixture_request()).await.unwrap_err();
    let Error::Api(api) = &error else {
        panic!("expected TypeSafe's refusal, got {error}");
    };
    assert_eq!(api.status, 400);
    assert_eq!(
        error.refusal().map(|refusal| refusal.code),
        Some("invalid_request".into())
    );
    assert!(gateway_seen.lock().await.is_empty());
    Ok(())
}

#[tokio::test]
async fn a_doors_own_size_or_admission_limit_fails_over() -> Outcome {
    for (status, body) in [
        (
            413,
            r#"{"error": {"code": "not_admitted", "message": "the prompt is 20000 Clef tokens; this server admits 16384"}}"#,
        ),
        (
            400,
            r#"{"error": "question \"answer\": criteria must contain 2–26 candidates"}"#,
        ),
        (
            413,
            r#"{"error": "text and schema must not exceed 64 KiB"}"#,
        ),
    ] {
        let (typesafe, _) = door(Behavior::Answer(status, body.into())).await?;
        let (gateway, gateway_seen) = door(Behavior::Answer(200, gateway_answer())).await?;
        let client = failover_client(
            &typesafe,
            &[(
                doors::GATEWAY_DOOR,
                format!("{gateway}/typesafe/v1/systemone"),
                Naming::Gateway,
            )],
            None,
        )?;
        let response = client.system_one(fixture_request()).await?;
        assert_eq!(response.model, "typesafe-ai/jev", "{status} {body}");
        assert_eq!(gateway_seen.lock().await.len(), 1, "{status} {body}");
    }
    Ok(())
}

#[tokio::test]
async fn a_typesafe_answer_is_served_as_it_came() -> Outcome {
    let (typesafe, _) = door(Behavior::Answer(200, TYPESAFE_ANSWER.into())).await?;
    let (gateway, gateway_seen) = door(Behavior::Answer(200, gateway_answer())).await?;
    let client = failover_client(
        &typesafe,
        &[(
            doors::GATEWAY_DOOR,
            format!("{gateway}/typesafe/v1/systemone"),
            Naming::Gateway,
        )],
        None,
    )?;
    let response = client.system_one(fixture_request()).await?;
    assert_eq!(response.model, "jev-1.13.0");
    assert_eq!(response.service(), None);
    assert_eq!(response.raw().bytes, TYPESAFE_ANSWER.as_bytes());
    assert!(gateway_seen.lock().await.is_empty());
    Ok(())
}

#[tokio::test]
async fn an_unreachable_or_hung_typesafe_falls_through() -> Outcome {
    let (gateway, _) = door(Behavior::Answer(200, gateway_answer())).await?;
    let gateway_url = format!("{gateway}/typesafe/v1/systemone");

    let (dropped, _) = door(Behavior::Drop).await?;
    let client = failover_client(
        &dropped,
        &[(doors::GATEWAY_DOOR, gateway_url.clone(), Naming::Gateway)],
        None,
    )?;
    let response = client.system_one(fixture_request()).await?;
    assert_eq!(
        response.service(),
        Some(json!({"door": doors::GATEWAY_DOOR}))
    );

    let (hung, _) = door(Behavior::Slow(
        Duration::from_secs(3),
        200,
        TYPESAFE_ANSWER.into(),
    ))
    .await?;
    let client = failover_client(
        &hung,
        &[(doors::GATEWAY_DOOR, gateway_url, Naming::Gateway)],
        Some(Duration::from_millis(300)),
    )?;
    let response = client.system_one(fixture_request()).await?;
    assert_eq!(
        response.service(),
        Some(json!({"door": doors::GATEWAY_DOOR}))
    );
    Ok(())
}

#[tokio::test]
async fn when_no_fallback_answers_typesafes_refusal_stands() -> Outcome {
    let (typesafe, _) = door(Behavior::Answer(402, NO_CREDITS.into())).await?;
    let (gateway, _) = door(Behavior::Answer(
        401,
        r#"{"message": "Invalid API key", "error_type": "authentication_error"}"#.into(),
    ))
    .await?;
    let (openrouter, _) = door(Behavior::Answer(
        401,
        r#"{"error": {"code": 401, "message": "User not found."}}"#.into(),
    ))
    .await?;
    let client = failover_client(
        &typesafe,
        &[
            (
                doors::GATEWAY_DOOR,
                format!("{gateway}/typesafe/v1/systemone"),
                Naming::Gateway,
            ),
            (
                doors::OPENROUTER_DOOR,
                format!("{openrouter}/api/alpha/decisions"),
                Naming::OpenRouter,
            ),
        ],
        None,
    )?;
    let error = client.system_one(fixture_request()).await.unwrap_err();
    assert_eq!(
        error
            .refusal()
            .map(|refusal| (refusal.status, refusal.code)),
        Some((402, "payment_required".into()))
    );
    Ok(())
}

#[tokio::test]
async fn only_decisions_fail_over() -> Outcome {
    let (typesafe, typesafe_seen) = door(Behavior::Answer(402, NO_CREDITS.into())).await?;
    let (gateway, gateway_seen) = door(Behavior::Answer(200, r#"{"data": []}"#.into())).await?;
    let client = failover_client(
        &typesafe,
        &[(
            doors::GATEWAY_DOOR,
            format!("{gateway}/typesafe/v1/systemone"),
            Naming::Gateway,
        )],
        None,
    )?;
    assert!(
        client
            .models()
            .list(jev::ListOptions::default())
            .await
            .is_err()
    );
    assert_eq!(typesafe_seen.lock().await[0].path, "/v1/models");
    assert!(gateway_seen.lock().await.is_empty());
    Ok(())
}

#[tokio::test]
async fn a_door_that_cannot_pay_is_skipped_on_the_next_call() -> Outcome {
    let (typesafe, typesafe_seen) = door(Behavior::Answer(402, NO_CREDITS.into())).await?;
    let (gateway, gateway_seen) = door(Behavior::Answer(200, gateway_answer())).await?;
    let client = failover_client(
        &typesafe,
        &[(
            doors::GATEWAY_DOOR,
            format!("{gateway}/typesafe/v1/systemone"),
            Naming::Gateway,
        )],
        None,
    )?;
    for _ in 0..3 {
        let response = client.system_one(fixture_request()).await?;
        assert_eq!(
            response.service(),
            Some(json!({"door": doors::GATEWAY_DOOR}))
        );
    }
    // TypeSafe refused once for its account; the next calls went straight
    // to the gateway.
    assert_eq!(typesafe_seen.lock().await.len(), 1);
    assert_eq!(gateway_seen.lock().await.len(), 3);
    Ok(())
}

#[tokio::test]
async fn a_benched_door_is_asked_again_when_its_bench_ends() -> Outcome {
    let (typesafe, typesafe_seen) = door(Behavior::Answer(402, NO_CREDITS.into())).await?;
    let (gateway, _) = door(Behavior::Answer(200, gateway_answer())).await?;
    let client = benched_client(
        &typesafe,
        &[(
            doors::GATEWAY_DOOR,
            format!("{gateway}/typesafe/v1/systemone"),
            Naming::Gateway,
        )],
        None,
        Duration::from_millis(100),
    )?;
    client.system_one(fixture_request()).await?;
    client.system_one(fixture_request()).await?;
    assert_eq!(typesafe_seen.lock().await.len(), 1);
    tokio::time::sleep(Duration::from_millis(150)).await;
    client.system_one(fixture_request()).await?;
    assert_eq!(typesafe_seen.lock().await.len(), 2);
    Ok(())
}

#[tokio::test]
async fn a_benched_primarys_refusal_stands_when_no_fallback_answers() -> Outcome {
    let (typesafe, typesafe_seen) = door(Behavior::Answer(402, NO_CREDITS.into())).await?;
    let (gateway, gateway_seen) = door(Behavior::Answer(
        402,
        r#"{"message": "Insufficient funds", "error_type": "payment_required"}"#.into(),
    ))
    .await?;
    let client = failover_client(
        &typesafe,
        &[(
            doors::GATEWAY_DOOR,
            format!("{gateway}/typesafe/v1/systemone"),
            Naming::Gateway,
        )],
        None,
    )?;
    for _ in 0..2 {
        let error = client.system_one(fixture_request()).await.unwrap_err();
        assert_eq!(
            error
                .refusal()
                .map(|refusal| (refusal.status, refusal.code)),
            Some((402, "payment_required".into()))
        );
    }
    // Both doors were benched after the first call; the second asked
    // neither and TypeSafe's remembered refusal stood.
    assert_eq!(typesafe_seen.lock().await.len(), 1);
    assert_eq!(gateway_seen.lock().await.len(), 1);
    Ok(())
}

/// A client asking the gateway, then OpenRouter, then TypeSafe last: the
/// order a server holding every key uses.
fn gateway_first_client(
    typesafe: &str,
    gateway: &str,
    openrouter: &str,
) -> Result<Client, Box<dyn std::error::Error>> {
    let failover = Failover::new(
        Door::new(
            doors::TYPESAFE_DOOR,
            typesafe,
            Naming::Canonical,
            ApiKey::new("ts-key"),
        ),
        vec![
            Door::new(
                doors::GATEWAY_DOOR,
                format!("{gateway}/typesafe/v1/systemone"),
                Naming::Gateway,
                ApiKey::new("gateway-key"),
            ),
            Door::new(
                doors::OPENROUTER_DOOR,
                format!("{openrouter}/api/alpha/decisions"),
                Naming::OpenRouter,
                ApiKey::new("openrouter-key"),
            ),
        ],
    )
    .primary_last()
    .primary_timeout(Duration::from_secs(1));
    Ok(Client::new(
        Config::new()
            .exchange(doors::exchange(failover))
            .base_url(doors::TYPESAFE_DOOR)
            .default_model("jev-1.13.0")
            .timeout(Duration::from_secs(5))
            .retry(RetryPolicy {
                max_retries: 0,
                ..RetryPolicy::default()
            }),
    )?)
}

#[tokio::test]
async fn with_typesafe_last_the_gateway_answers_first() -> Outcome {
    let (typesafe, typesafe_seen) = door(Behavior::Answer(200, TYPESAFE_ANSWER.into())).await?;
    let (gateway, gateway_seen) = door(Behavior::Answer(200, gateway_answer())).await?;
    let (openrouter, openrouter_seen) = door(Behavior::Answer(200, openrouter_answer())).await?;
    let client = gateway_first_client(&typesafe, &gateway, &openrouter)?;
    let response = client.system_one(fixture_request()).await?;
    assert_eq!(
        response.service(),
        Some(json!({"door": doors::GATEWAY_DOOR}))
    );
    assert_eq!(gateway_seen.lock().await[0].bearer, "Bearer gateway-key");
    assert!(openrouter_seen.lock().await.is_empty());
    assert!(typesafe_seen.lock().await.is_empty());
    assert_eq!(
        client.doors().as_deref(),
        Some(
            "doors https://ai-gateway.vercel.sh → https://openrouter.ai → https://api.typesafe.ai"
        )
    );
    // Every route but a decision still goes to TypeSafe.
    let _ = client.models().list(jev::ListOptions::default()).await;
    assert_eq!(typesafe_seen.lock().await[0].path, "/v1/models");
    assert_eq!(gateway_seen.lock().await.len(), 1);
    Ok(())
}

#[tokio::test]
async fn with_typesafe_last_it_answers_when_the_others_cannot() -> Outcome {
    let (typesafe, typesafe_seen) = door(Behavior::Answer(200, TYPESAFE_ANSWER.into())).await?;
    let (gateway, gateway_seen) = door(Behavior::Answer(
        503,
        r#"{"message": "No provider is available", "error_type": "internal_server_error"}"#.into(),
    ))
    .await?;
    let (openrouter, openrouter_seen) = door(Behavior::Answer(
        402,
        r#"{"error": {"code": 402, "message": "Insufficient credits"}}"#.into(),
    ))
    .await?;
    let client = gateway_first_client(&typesafe, &gateway, &openrouter)?;
    for _ in 0..2 {
        let response = client.system_one(fixture_request()).await?;
        // TypeSafe's own answer, as it came: no fallback door named.
        assert_eq!(response.service(), None);
        assert_eq!(response.model, "jev-1.13.0");
    }
    let typesafe_seen = typesafe_seen.lock().await.clone();
    assert_eq!(typesafe_seen.len(), 2);
    assert_eq!(typesafe_seen[0].path, "/v1/systemone");
    assert_eq!(typesafe_seen[0].body["model"], "jev-1.13.0");
    // The gateway's 503 is asked again; OpenRouter's 402 benched it.
    assert_eq!(gateway_seen.lock().await.len(), 2);
    assert_eq!(openrouter_seen.lock().await.len(), 1);
    Ok(())
}

/// A carried door: another service (the hosted decision service) that
/// answers a decision as it came, adding its own `service` object, and
/// counts what it was handed.
#[derive(Debug, Default)]
struct Carrier {
    handed: std::sync::Mutex<Vec<jev::exchange::Call>>,
}

impl jev::exchange::Exchange for Carrier {
    fn exchange(&self, call: jev::exchange::Call) -> jev::exchange::Pending<'_> {
        self.handed.lock().unwrap().push(call);
        Box::pin(async {
            let mut answer: Value = serde_json::from_str(TYPESAFE_ANSWER).unwrap();
            answer["service"] =
                json!({"door": "https://ai-gateway.vercel.sh", "version": "decision-worker/test"});
            Ok(jev::exchange::Reply {
                status: 200,
                headers: vec![("content-type".into(), "application/json".into())],
                body: answer.to_string().into_bytes(),
            })
        })
    }

    fn service(&self) -> String {
        "hosted decision service w on wss://relay.test".to_string()
    }
}

#[tokio::test]
async fn a_key_that_cannot_pay_falls_through_to_a_carried_door_and_is_benched() -> Outcome {
    let (typesafe, typesafe_seen) = door(Behavior::Answer(402, NO_CREDITS.into())).await?;
    let carrier = Arc::new(Carrier::default());
    let failover = Failover::new(
        Door::new(
            doors::TYPESAFE_DOOR,
            typesafe.as_str(),
            Naming::Canonical,
            ApiKey::new("ts-key"),
        ),
        vec![Door::carried(
            "wss://relay.test",
            Arc::clone(&carrier) as Arc<dyn jev::exchange::Exchange>,
        )],
    );
    let client = Client::new(
        Config::new()
            .exchange(doors::exchange(failover))
            .base_url(doors::TYPESAFE_DOOR)
            .default_model("jev-1.13.0")
            .timeout(Duration::from_secs(5))
            .retry(RetryPolicy {
                max_retries: 0,
                ..RetryPolicy::default()
            }),
    )?;
    assert_eq!(
        client.doors().as_deref(),
        Some("doors https://api.typesafe.ai → wss://relay.test")
    );
    for _ in 0..3 {
        let response = client.system_one(fixture_request()).await?;
        // The carrier's own service object stands and names the carrier.
        assert_eq!(
            response.service(),
            Some(json!({
                "door": "https://ai-gateway.vercel.sh",
                "version": "decision-worker/test",
                "exchange": "hosted decision service w on wss://relay.test"
            }))
        );
    }
    // TypeSafe refused once for its account and was benched; every call
    // reached the carried door, as it came, with no key.
    assert_eq!(typesafe_seen.lock().await.len(), 1);
    let handed = carrier.handed.lock().unwrap();
    assert_eq!(handed.len(), 3);
    assert_eq!(handed[0].path, "/v1/systemone");
    let body: Value = serde_json::from_slice(handed[0].body.as_deref().unwrap())?;
    assert_eq!(body["model"], "jev-1.13.0");
    assert!(!format!("{handed:?}").contains("ts-key"));
    Ok(())
}

/// A person's own gateway key leading (BYOK, `model-access`): a primary
/// that names Jev the gateway's way takes the decision at its full URL with
/// `typesafe-ai/jev`, and OpenRouter after it on the person's own key.
#[tokio::test]
async fn a_gateway_primary_names_jev_its_own_way() -> Outcome {
    let (gateway, gateway_seen) = door(Behavior::Answer(402, NO_CREDITS.into())).await?;
    let (openrouter, openrouter_seen) = door(Behavior::Answer(200, openrouter_answer())).await?;
    let failover = Failover::new(
        Door::new(
            doors::GATEWAY_DOOR,
            format!("{gateway}/typesafe/v1/systemone"),
            Naming::Gateway,
            ApiKey::new("their-gateway-key"),
        ),
        vec![Door::new(
            doors::OPENROUTER_DOOR,
            format!("{openrouter}/api/alpha/decisions"),
            Naming::OpenRouter,
            ApiKey::new("their-openrouter-key"),
        )],
    );
    let client = Client::new(
        Config::new()
            .exchange(doors::exchange(failover))
            .base_url(doors::TYPESAFE_DOOR)
            .default_model("jev-1.13.0")
            .timeout(Duration::from_secs(5))
            .retry(RetryPolicy {
                max_retries: 0,
                ..RetryPolicy::default()
            }),
    )?;
    let response = client.system_one(fixture_request()).await?;
    assert_eq!(
        response.service(),
        Some(json!({"door": doors::OPENROUTER_DOOR}))
    );
    let gateway_seen = gateway_seen.lock().await;
    assert_eq!(gateway_seen[0].path, "/typesafe/v1/systemone");
    assert_eq!(gateway_seen[0].body["model"], "typesafe-ai/jev");
    assert_eq!(gateway_seen[0].bearer, "Bearer their-gateway-key");
    let openrouter_seen = openrouter_seen.lock().await;
    assert_eq!(openrouter_seen[0].body["model"], "typesafe/jev-1.13");
    assert_eq!(openrouter_seen[0].bearer, "Bearer their-openrouter-key");
    Ok(())
}
