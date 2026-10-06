//! The bundled Jev tool, backed by the repository's TypeSafe SDK.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use jev::{Client, Config, ListOptions, Questions, RetryPolicy, SystemOneRequest};
use reqwest::header::{HeaderMap, HeaderValue};
use serde::Deserialize;
use serde_json::{Map, Value, json};

pub const TOOL_NAME: &str = "jev";
pub const DEFAULT_ENDPOINT: &str = jev::defaults::BASE_URL;
pub const GATEWAY_ENDPOINT: &str = "https://ai-gateway.vercel.sh/typesafe";
pub const GATEWAY_MODEL: &str = jev::doors::GATEWAY_MODEL;

const MAX_ARGUMENT_BYTES: usize = 128 * 1024;
const MAX_QUESTIONS: usize = 256;
const CALL_BUDGET: Duration = Duration::from_secs(30);
static REQUEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// The tool schema exposed when the Jev plugin is enabled and connected.
pub fn tool_definition() -> Value {
    let description = json!({"type": ["string", "object", "array", "null"]});
    let instructions = json!({"type": ["string", "object", "array"]});
    let question = |kind: &str, criteria: Value, required: Vec<&str>| {
        json!({
            "type": "object",
            "properties": {
                "type": {"type": "string", "enum": [kind]},
                "instructions": instructions,
                "criteria": criteria
            },
            "required": required,
            "additionalProperties": false
        })
    };
    json!({
        "type": "function",
        "function": {
            "name": TOOL_NAME,
            "description": "Ask Jev for typed semantic judgments over one state. Batch independent Noul (yes/no probability), Choice (one named option), and Score (ordered rubric) questions. Returns answers, probabilities, model, and usage; does not generate prose or execute actions.",
            "parameters": {
                "type": "object",
                "properties": {
                    "state": {
                        "type": ["string", "object", "array"],
                        "description": "The source material and relevant facts. Prefer named JSON fields. Each question independently reads this same state."
                    },
                    "questions": {
                        "type": "object",
                        "minProperties": 1,
                        "maxProperties": MAX_QUESTIONS,
                        "additionalProperties": {
                            "anyOf": [
                                question("noul", json!({
                                    "type": "object",
                                    "properties": {"true": description, "false": description},
                                    "additionalProperties": false
                                }), vec!["type", "instructions"]),
                                question("choice", json!({
                                    "type": "object",
                                    "minProperties": 1,
                                    "maxProperties": 255,
                                    "additionalProperties": description
                                }), vec!["type", "instructions", "criteria"]),
                                question("score", json!({
                                    "type": "array",
                                    "minItems": 2,
                                    "maxItems": 10,
                                    "items": description
                                }), vec!["type", "instructions", "criteria"])
                            ]
                        }
                    },
                    "model": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": 128,
                        "description": "Optional model ID for the configured Jev gateway. Omit to use the model selected in plugin settings."
                    }
                },
                "required": ["state", "questions"],
                "additionalProperties": false
            }
        }
    })
}

/// Model guidance derived from the TypeSafe skill and current primitive docs.
pub fn instructions() -> &'static str {
    "The Jev plugin provides the jev tool for focused semantic judgments, including routing, ranking, extraction from supplied candidates, and checking claims against evidence. Keep exact lookups, calculations, known rules, and execution in ordinary code. Jev returns typed answers and probabilities, not generated text or reasoning explanations.\n\
     Send state with the source material, relevant identities, relationships, policies, and current facts. Prefer a JSON object with descriptive fields; a string suits one passage. Reference nested state in instructions with paths such as `ticket.messages[0].text`. Each call is complete and independent; Jev receives no chat history unless you include it in state.\n\
     Supply questions as an object keyed by caller-chosen IDs. IDs are not visible to Jev, so each question's instructions must state its complete meaning. Ask one coherent judgment per question. Put the material in state, the judgment in instructions, and answer descriptions in criteria. Instructions and criterion descriptions can be strings, objects, or arrays.\n\
     Use {\"type\":\"noul\",\"instructions\":\"Does the customer request a refund?\"} for a condition's yes probability from 0 to 1. Optional criteria can describe true and false. A Noul near 0.5 means yes and no are similarly probable, not medium intensity.\n\
     Use choice with criteria mapping 1–255 named options to their descriptions. Include an other or no-match option when appropriate; the model cannot select an omitted candidate. Read choice, probabilities, and confidence. Use separate Nouls when several labels may independently apply.\n\
     Use score with criteria containing 2–10 ordered descriptions of concrete situations, lowest to highest. Each level must stand on its own; avoid numeric-only levels or references to neighboring levels. Read score as a probability-weighted position from 0 to the last level index, alongside probabilities, legend, and confidence.\n\
     Batch independent questions over the same state in one call, including speculative questions with explicit premises. They cannot see each other's answers. Use a later call only when prior answers are needed to obtain evidence or construct new questions. Extra questions consume tokens. This tool accepts at most 256 questions and 128 KiB of arguments per call.\n\
     Omit model to use the model selected in plugin settings: jev-latest for TypeSafe direct, or typesafe-ai/jev for Vercel AI Gateway. An explicit model must be available at the configured gateway. Credentials and endpoint are supplied by plugin settings; never put an API key in tool arguments. Probabilities are judgments, not proof or permission. Choice and Score confidence measures distribution concentration. Keep action policy and evaluated thresholds in code, and report uncertainty when evidence is insufficient. API failures are not judgments."
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Arguments {
    state: Value,
    questions: Map<String, Value>,
    model: Option<String>,
}

/// Ask the SDK's System One endpoint using credentials supplied by the host.
pub async fn execute(api_key: &str, endpoint: &str, arguments: Value) -> Result<Value, String> {
    let request = request(arguments, api_key)?;
    let client = client(api_key, endpoint)?;
    let response = client
        .system_one(request)
        .await
        .map_err(|error| error_message(error, api_key))?;
    let mut result = json!({
        "model": response.model,
        "answers": response.answers_value(),
        "usage": response.usage,
        "request_id": response.request_id()
    });
    redact_value(&mut result, api_key);
    Ok(result)
}

/// Check a credential by listing its models without requesting inference.
pub async fn test_key(api_key: &str, endpoint: &str) -> Result<Vec<String>, String> {
    test_key_for_model(api_key, endpoint, default_model(endpoint)).await
}

/// Check the selected connection without requesting a decision.
pub async fn test_key_for_model(
    api_key: &str,
    endpoint: &str,
    model: &str,
) -> Result<Vec<String>, String> {
    let client = configured_client(api_key, endpoint, model)?;
    let models = client
        .models()
        .list(ListOptions::new())
        .await
        .map_err(|error| error_message(error, api_key))?;
    Ok(models
        .into_iter()
        .map(|model| safe_text(&model.name, api_key, 128))
        .collect())
}

fn request(arguments: Value, api_key: &str) -> Result<SystemOneRequest, String> {
    let bytes = serde_json::to_vec(&arguments).map_err(|_| "Jev arguments must be JSON.")?;
    if bytes.len() > MAX_ARGUMENT_BYTES {
        return Err("Jev arguments exceed 128 KiB.".into());
    }
    if !api_key.is_empty() && String::from_utf8_lossy(&bytes).contains(api_key) {
        return Err("Keep the Jev API key in plugin settings, outside tool arguments.".into());
    }
    let arguments: Arguments = serde_json::from_value(arguments).map_err(
        |_| "Jev requires state and a questions object, with an optional model; no other fields.",
    )?;
    if !matches!(
        arguments.state,
        Value::String(_) | Value::Object(_) | Value::Array(_)
    ) {
        return Err("Jev state must be text, a JSON object, or an array.".into());
    }
    if arguments.questions.len() > MAX_QUESTIONS {
        return Err("Jev accepts at most 256 questions per tool call.".into());
    }
    for (id, question) in &arguments.questions {
        validate_question(id, question)?;
    }
    let questions = Questions::from_map(arguments.questions);
    questions
        .validate()
        .map_err(|error| error_message(error, api_key))?;
    let mut request = SystemOneRequest::new(arguments.state, questions);
    if let Some(model) = arguments.model {
        if model.is_empty() || model.len() > 128 || !model.chars().all(model_character) {
            return Err("The Jev model ID must use 1–128 letters, digits, dots, dashes, underscores, colons, or slashes.".into());
        }
        request = request.model(model);
    }
    let mut headers = HeaderMap::new();
    headers.insert(
        "idempotency-key",
        HeaderValue::from_str(&request_id()).map_err(|_| "The Jev request ID is invalid.")?,
    );
    Ok(request.headers(headers))
}

fn validate_question(id: &str, question: &Value) -> Result<(), String> {
    if id.is_empty() || id.len() > 128 || id.chars().any(char::is_control) {
        return Err("Jev question IDs must use 1–128 bytes without control characters.".into());
    }
    let object = question
        .as_object()
        .ok_or("Each Jev question must be an object.")?;
    if object
        .keys()
        .any(|key| !matches!(key.as_str(), "type" | "instructions" | "criteria"))
    {
        return Err("A Jev question accepts type, instructions, and criteria only.".into());
    }
    if !object.get("instructions").is_some_and(description) {
        return Err("Each Jev question needs instructions as text, an object, or an array.".into());
    }
    match object.get("type").and_then(Value::as_str) {
        Some("noul") => {
            if let Some(criteria) = object.get("criteria") {
                let criteria = criteria
                    .as_object()
                    .ok_or("Noul criteria must be an object with true and false descriptions.")?;
                if criteria.iter().any(|(key, value)| {
                    !matches!(key.as_str(), "true" | "false") || !nullable_description(value)
                }) {
                    return Err("Noul criteria accepts true and false descriptions only.".into());
                }
            }
        }
        Some("choice") => {
            let criteria = object
                .get("criteria")
                .and_then(Value::as_object)
                .ok_or("Choice criteria must map named options to descriptions.")?;
            if criteria.is_empty()
                || criteria
                    .iter()
                    .any(|(key, value)| key.is_empty() || !nullable_description(value))
            {
                return Err("Choice needs at least one named option, with text, object, array, or null descriptions.".into());
            }
        }
        Some("score") => {
            let criteria = object
                .get("criteria")
                .and_then(Value::as_array)
                .ok_or("Score criteria must be an ordered array of descriptions.")?;
            if criteria.iter().any(|value| !nullable_description(value)) {
                return Err("Score levels must be text, objects, arrays, or null.".into());
            }
        }
        _ => return Err("Jev question type must be noul, choice, or score.".into()),
    }
    Ok(())
}

fn description(value: &Value) -> bool {
    matches!(value, Value::String(_) | Value::Object(_) | Value::Array(_))
}

fn nullable_description(value: &Value) -> bool {
    value.is_null() || description(value)
}

fn model_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_' | ':' | '/')
}

fn client(api_key: &str, endpoint: &str) -> Result<Client, String> {
    configured_client(api_key, endpoint, default_model(endpoint))
}

fn default_model(endpoint: &str) -> &'static str {
    if endpoint.trim_end_matches('/') == GATEWAY_ENDPOINT {
        GATEWAY_MODEL
    } else {
        jev::defaults::MODEL
    }
}

/// Build the SDK client shared by bundled tools with the selected Jev model.
pub fn configured_client(api_key: &str, endpoint: &str, model: &str) -> Result<Client, String> {
    if api_key.trim().is_empty() {
        return Err("Connect a Jev API key in /plugins to use the Jev tool.".into());
    }
    if model.is_empty() || model.len() > 128 || !model.chars().all(model_character) {
        return Err("Enter a valid Jev model ID using at most 128 bytes.".into());
    }
    let url = reqwest::Url::parse(endpoint).map_err(|_| "The Jev endpoint must be an HTTP URL.")?;
    let loopback = url.host_str().is_some_and(|host| {
        host.trim_matches(['[', ']'])
            .parse::<std::net::IpAddr>()
            .is_ok_and(|address| address.is_loopback())
    });
    if endpoint.len() > 2048
        || url.host_str().is_none()
        || !(url.scheme() == "https" || url.scheme() == "http" && loopback)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("The Jev endpoint must use HTTPS, or HTTP on a loopback IP, without credentials, query, or fragment.".into());
    }
    let http = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "The Jev HTTP client could not start.")?;
    Client::new(
        Config::new()
            .api_key(api_key)
            .base_url(endpoint)
            .default_model(model)
            .retry(RetryPolicy {
                budget: Some(CALL_BUDGET),
                ..RetryPolicy::default()
            })
            .http_client(http),
    )
    .map_err(|error| error_message(error, api_key))
}

fn request_id() -> String {
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = REQUEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("coder-new-jev-{}-{time:x}-{sequence:x}", std::process::id())
}

fn error_message(error: jev::Error, api_key: &str) -> String {
    safe_text(&format!("Jev: {error}"), api_key, 600)
}

fn safe_text(text: &str, api_key: &str, limit: usize) -> String {
    let text = if api_key.is_empty() {
        text.to_string()
    } else {
        text.replace(api_key, "[redacted]")
    };
    text.chars()
        .filter(|character| !character.is_control())
        .take(limit)
        .collect()
}

fn redact_value(value: &mut Value, api_key: &str) {
    match value {
        Value::String(text) => {
            if !api_key.is_empty() {
                *text = text.replace(api_key, "[redacted]");
            }
        }
        Value::Array(values) => {
            for value in values {
                redact_value(value, api_key);
            }
        }
        Value::Object(object) => {
            let old = std::mem::take(object);
            for (key, mut value) in old {
                redact_value(&mut value, api_key);
                object.insert(safe_text(&key, api_key, 1024), value);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::thread::{self, JoinHandle};

    use super::*;

    fn arguments() -> Value {
        json!({
            "state": {"message": "The refund still has not arrived."},
            "questions": {
                "refund": {"type": "noul", "instructions": "Does the customer ask about a refund?"},
                "team": {"type": "choice", "instructions": "Which team handles this message?", "criteria": {"billing": "Payments and refunds", "other": "Anything else"}},
                "urgency": {"type": "score", "instructions": "How urgent is the message?", "criteria": ["Routine inquiry", "Immediate interruption needed"]}
            }
        })
    }

    fn server(status: u16, body: Value) -> (String, JoinHandle<(String, Value)>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let handle = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut reader = BufReader::new(socket.try_clone().unwrap());
            let mut head = String::new();
            let mut length = 0;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                head.push_str(&line);
                if line == "\r\n" {
                    break;
                }
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = value.trim().parse::<usize>().unwrap();
                }
            }
            let mut bytes = vec![0; length];
            reader.read_exact(&mut bytes).unwrap();
            let request = if bytes.is_empty() {
                Value::Null
            } else {
                serde_json::from_slice(&bytes).unwrap()
            };
            let body = body.to_string();
            write!(socket, "HTTP/1.1 {status} Fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\nX-Typesafe-Request-Id: jev-fixture\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            socket.flush().unwrap();
            (head, request)
        });
        (endpoint, handle)
    }

    #[test]
    fn rejects_unknown_fields_and_invalid_question_shapes_before_dispatch() {
        let mut unknown = arguments();
        unknown["api_key"] = json!("not-a-credential");
        assert!(
            request(unknown, "fixture-token")
                .unwrap_err()
                .contains("no other fields")
        );
        let mut unknown_question = arguments();
        unknown_question["questions"]["refund"]["endpoint"] = json!("https://example.invalid");
        assert!(request(unknown_question, "fixture-token").is_err());
        for invalid in [
            json!({"type": "chat", "instructions": "Write an explanation"}),
            json!({"type": "choice", "instructions": "Choose", "criteria": []}),
            json!({"type": "score", "instructions": "Rate", "criteria": ["Only one"]}),
            json!({"type": "noul", "instructions": "Judge", "criteria": {"maybe": "Uncertain"}}),
        ] {
            let mut arguments = arguments();
            arguments["questions"]["refund"] = invalid;
            assert!(request(arguments, "fixture-token").is_err());
        }
    }

    #[test]
    fn enforces_sdk_option_bounds_and_host_request_bounds() {
        let mut too_many = arguments();
        too_many["questions"]["team"]["criteria"] =
            Value::Object((0..256).map(|i| (i.to_string(), Value::Null)).collect());
        assert!(
            request(too_many, "fixture-token")
                .unwrap_err()
                .contains("255")
        );
        let mut too_many_levels = arguments();
        too_many_levels["questions"]["urgency"]["criteria"] = json!(vec!["level"; 11]);
        assert!(
            request(too_many_levels, "fixture-token")
                .unwrap_err()
                .contains("10")
        );
        let mut too_big = arguments();
        too_big["state"] = json!("x".repeat(MAX_ARGUMENT_BYTES));
        assert!(
            request(too_big, "fixture-token")
                .unwrap_err()
                .contains("128 KiB")
        );
        let mut leaked_key = arguments();
        leaked_key["state"] = json!("fixture-token");
        assert!(
            request(leaked_key, "fixture-token")
                .unwrap_err()
                .contains("plugin settings")
        );
        assert!(request(json!({"state": "text", "questions": {}}), "fixture-token").is_err());
    }

    #[test]
    fn definition_and_guidance_exclude_credentials_and_cover_typed_judgments() {
        let definition = tool_definition();
        assert_eq!(definition["function"]["name"], TOOL_NAME);
        assert!(
            definition["function"]["parameters"]["properties"]
                .get("api_key")
                .is_none()
        );
        assert!(instructions().contains("Batch independent questions"));
        assert!(instructions().contains("not proof or permission"));
        assert!(instructions().contains("jev-latest"));
        assert_eq!(DEFAULT_ENDPOINT, "https://api.typesafe.ai");
    }

    #[tokio::test]
    async fn sdk_returns_typed_answers_and_sends_credentials_only_as_a_header() {
        let (endpoint, handle) = server(
            200,
            json!({
                "model": "jev-fixture", "answers": {
                    "refund": {"type": "noul", "noul": 0.9},
                    "team": {"type": "choice", "choice": "billing", "confidence": 0.8, "probabilities": {"billing": 0.9, "other": 0.1}},
                    "urgency": {"type": "score", "score": 0.2, "confidence": 0.6, "probabilities": {"0": 0.8, "1": 0.2}, "legend": {"0": "Routine inquiry", "1": "Immediate interruption needed"}}
                }, "usage": {"input_tokens": 42, "output_tokens": 7}
            }),
        );
        let response = execute("fixture-token", &endpoint, arguments())
            .await
            .unwrap();
        assert_eq!(response["answers"]["refund"]["noul"], 0.9);
        assert_eq!(response["answers"]["team"]["choice"], "billing");
        assert_eq!(response["usage"]["input_tokens"], 42);
        assert_eq!(response["request_id"], "jev-fixture");
        let (head, request) = handle.join().unwrap();
        assert!(head.starts_with("POST /v1/systemone "));
        assert!(
            head.to_ascii_lowercase()
                .contains("authorization: bearer fixture-token")
        );
        assert!(
            head.to_ascii_lowercase()
                .contains("idempotency-key: coder-new-jev-")
        );
        assert!(head.to_ascii_lowercase().contains("x-attempt: 1"));
        assert_eq!(request["model"], jev::defaults::MODEL);
        assert!(!request.to_string().contains("fixture-token"));
        assert!(!response.to_string().contains("fixture-token"));
    }

    #[tokio::test]
    async fn key_check_uses_model_listing_without_inference() {
        let (endpoint, handle) = server(
            200,
            json!({"models": [{"name": "jev-fixture", "description": "Fixture model", "release_date": "2026-10-06"}]}),
        );
        assert_eq!(
            test_key("fixture-token", &endpoint).await.unwrap(),
            ["jev-fixture"]
        );
        let (head, body) = handle.join().unwrap();
        assert!(head.starts_with("GET /v1/models "));
        assert!(body.is_null());
    }

    #[tokio::test]
    async fn gateway_key_check_preserves_the_typesafe_path_prefix() {
        let (endpoint, handle) = server(
            200,
            json!({"models": [{"name": GATEWAY_MODEL, "description": "Jev", "release_date": "2026-09-15"}]}),
        );
        let models = test_key_for_model(
            "fixture-gateway-key",
            &format!("{endpoint}/typesafe"),
            GATEWAY_MODEL,
        )
        .await
        .unwrap();
        assert_eq!(models, [GATEWAY_MODEL]);
        let (head, body) = handle.join().unwrap();
        assert!(head.starts_with("GET /typesafe/v1/models "));
        assert!(
            head.to_ascii_lowercase()
                .contains("authorization: bearer fixture-gateway-key")
        );
        assert!(body.is_null());
        assert_eq!(default_model(GATEWAY_ENDPOINT), GATEWAY_MODEL);
    }

    #[tokio::test]
    async fn gateway_settings_reach_the_chat_tool_and_microcoder_judge() {
        for chat_tool in [true, false] {
            let (endpoint, handle) = server(
                200,
                json!({
                    "model": GATEWAY_MODEL,
                    "answers": {
                        "refund": {"type": "noul", "noul": 0.9},
                        "team": {"type": "choice", "choice": "billing", "confidence": 0.8, "probabilities": {"billing": 0.9, "other": 0.1}},
                        "urgency": {"type": "score", "score": 0.2, "confidence": 0.6, "probabilities": {"0": 0.8, "1": 0.2}, "legend": {"0": "Routine inquiry", "1": "Immediate interruption needed"}}
                    },
                    "usage": {"input_tokens": 42, "output_tokens": 7},
                    "provider_metadata": {"gateway": {"cost": "0.00000168"}}
                }),
            );
            let settings = crate::plugin_tools::ExecutionSettings {
                microcoder: false,
                cli: false,
                acp: false,
                jev_enabled: true,
                jev_key: Some(model_access::ApiKey::new("fixture-gateway-key")),
                redaction_keys: vec![],
                jev_model: GATEWAY_MODEL.into(),
                jev_endpoint: format!("{endpoint}/typesafe"),
                agents: vec![],
                cwd: std::path::PathBuf::from("/unused"),
            };
            let result = if chat_tool {
                settings
                    .execute(
                        TOOL_NAME,
                        arguments(),
                        None,
                        &std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
                        &mut |_| {},
                    )
                    .await
                    .unwrap()
            } else {
                let response = settings
                    .jev_client()
                    .unwrap()
                    .unwrap()
                    .system_one(request(arguments(), "fixture-gateway-key").unwrap())
                    .await
                    .unwrap();
                json!({"model": response.model, "answers": response.answers_value(), "usage": response.usage})
            };
            assert_eq!(result["model"], GATEWAY_MODEL);
            assert_eq!(result["answers"]["refund"]["noul"], 0.9);
            assert_eq!(result["usage"]["input_tokens"], 42);
            assert!(!result.to_string().contains("fixture-gateway-key"));
            let (head, body) = handle.join().unwrap();
            assert!(head.starts_with("POST /typesafe/v1/systemone "));
            assert!(
                head.to_ascii_lowercase()
                    .contains("authorization: bearer fixture-gateway-key")
            );
            assert_eq!(body["model"], GATEWAY_MODEL);
            assert_eq!(body["questions"]["refund"]["type"], "noul");
            assert!(!body.to_string().contains("fixture-gateway-key"));
        }
    }

    #[tokio::test]
    async fn service_errors_redact_credentials_and_preserve_failure_identity() {
        let (endpoint, handle) = server(
            401,
            json!({"error": {"code": "unauthenticated", "message": "Rejected fixture-token"}}),
        );
        let error = execute("fixture-token", &endpoint, arguments())
            .await
            .unwrap_err();
        assert!(error.contains("401"));
        assert!(error.contains("jev-fixture"));
        assert!(error.contains("[redacted]"));
        assert!(!error.contains("fixture-token"));
        handle.join().unwrap();
    }

    #[tokio::test]
    async fn missing_key_and_unsafe_endpoint_are_rejected_without_network() {
        assert!(
            configured_client("fixture-token", "http://[::1]:9090/typesafe", GATEWAY_MODEL).is_ok()
        );
        assert!(
            execute("", DEFAULT_ENDPOINT, arguments())
                .await
                .unwrap_err()
                .contains("Connect a Jev API key")
        );
        for endpoint in [
            "http://example.invalid",
            "https://user:password@example.invalid",
            "https://example.invalid?key=x",
            "https://example.invalid#fragment",
        ] {
            assert!(
                execute("fixture-token", endpoint, arguments())
                    .await
                    .unwrap_err()
                    .contains("HTTPS")
            );
        }
    }
}
