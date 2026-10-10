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
    let instructions = json!({
        "type": ["string", "object", "array"],
        "description": "The complete judgment to make about state. Write a specific question; question IDs are not visible to Jev."
    });
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
            "description": "Evaluate state at the configured Jev /v1/systemone endpoint. Batch Noul (yes/no probability), Choice (one named option), and Score (ordered rubric) in one questions object; these are question types, not separate endpoints. Pass only state, questions, and optional model. Returns typed answers, probabilities, model, and usage.",
            "parameters": {
                "type": "object",
                "properties": {
                    "state": {
                        "type": ["string", "object", "array"],
                        "description": "The source material and relevant facts. Prefer named JSON fields. Each question independently reads this same state."
                    },
                    "questions": {
                        "type": "object",
                        "description": "A non-empty map from question IDs to question objects. Mix noul, choice, and score in this map; do not use a questions array.",
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
     This tool calls only the /v1/systemone evaluation endpoint at the gateway configured in /plugins. Noul, Choice, and Score are question types, not different endpoints. To try all three, put them in one questions object. Pass only state, questions, and optional model; do not pass endpoint, url, method, action, headers, or credentials. Other API operations are not exposed by this tool.\n\
     Send state with the source material, relevant identities, relationships, policies, and current facts. Prefer a JSON object with descriptive fields; a string suits one passage. Reference nested state in instructions with paths such as `ticket.messages[0].text`. Each call is complete and independent; Jev receives no chat history unless you include it in state.\n\
     Supply questions as an object keyed by caller-chosen IDs. IDs are not visible to Jev, so each question's instructions must state its complete meaning. Ask one coherent judgment per question. Put the material in state, the judgment in instructions, and answer descriptions in criteria. Instructions and criterion descriptions can be strings, objects, or arrays.\n\
     Use {\"type\":\"noul\",\"instructions\":\"Does the customer request a refund?\"} for a condition's yes probability from 0 to 1. Optional criteria can describe true and false. A Noul near 0.5 means yes and no are similarly probable, not medium intensity.\n\
     Use choice with criteria mapping 1–255 named options to their descriptions. Include an other or no-match option when appropriate; the model cannot select an omitted candidate. Read choice, probabilities, and confidence. Use separate Nouls when several labels may independently apply.\n\
     Use score with criteria containing 2–10 ordered descriptions of concrete situations, lowest to highest. Each level must stand on its own; avoid numeric-only levels or references to neighboring levels. Read score as a probability-weighted position from 0 to the last level index, alongside probabilities, legend, and confidence.\n\
     Batch independent questions over the same state in one call, including speculative questions with explicit premises. They cannot see each other's answers. Use a later call only when prior answers are needed to obtain evidence or construct new questions. Extra questions consume tokens. This tool accepts at most 256 questions and 128 KiB of arguments per call.\n\
     Complete example using all three types: {\"state\":{\"message\":\"Please refund the duplicate charge.\"},\"questions\":{\"refund\":{\"type\":\"noul\",\"instructions\":\"Does `message` request a refund?\"},\"team\":{\"type\":\"choice\",\"instructions\":\"Which team handles `message`?\",\"criteria\":{\"billing\":\"Payments and refunds\",\"other\":\"Anything else\"}},\"urgency\":{\"type\":\"score\",\"instructions\":\"How urgent is `message`?\",\"criteria\":[\"Routine inquiry\",\"Immediate interruption needed\"]}}}. The yes/no type is noul, not boolean. If a tool result reports invalid arguments, fix the indicated field and retry; do not treat it as a decision result.\n\
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
    send(&client, request, api_key).await
}

/// Ask Jev through a client that holds no key of this computer's, such as
/// the hosted decision service ([`keyless_client`]).
pub async fn execute_keyless(client: &Client, arguments: Value) -> Result<Value, String> {
    let request = request(arguments, "")?;
    send(client, request, "").await
}

async fn send(client: &Client, request: SystemOneRequest, api_key: &str) -> Result<Value, String> {
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
    validate_arguments(&arguments)?;
    let arguments: Arguments = serde_json::from_value(arguments).map_err(
        |_| "Jev arguments must contain state, a questions object, and an optional model ID.",
    )?;
    for (index, (id, question)) in arguments.questions.iter().enumerate() {
        validate_question(id, question)
            .map_err(|error| format!("Jev questions entry {}: {error}", index + 1))?;
    }
    let questions = Questions::from_map(arguments.questions);
    questions
        .validate()
        .map_err(|_| "Jev questions are invalid. Use noul, choice, or score questions with instructions and the required criteria.")?;
    let mut request = SystemOneRequest::new(arguments.state, questions);
    if let Some(model) = arguments.model {
        request = request.model(model);
    }
    let mut headers = HeaderMap::new();
    headers.insert(
        "idempotency-key",
        HeaderValue::from_str(&request_id()).map_err(|_| "The Jev request ID is invalid.")?,
    );
    Ok(request.headers(headers))
}

fn validate_arguments(arguments: &Value) -> Result<(), String> {
    let object = arguments.as_object().ok_or(
        "Jev arguments must be an object. Use {\"state\":\"text to evaluate\",\"questions\":{\"check\":{\"type\":\"noul\",\"instructions\":\"Is the text a refund request?\"}}}.",
    )?;
    if object
        .keys()
        .any(|field| !matches!(field.as_str(), "state" | "questions" | "model"))
    {
        return Err("Jev arguments contain an unsupported top-level field. Use state and questions, with optional model and no other fields. The endpoint comes from /plugins; remove endpoint, URL, method, action, headers, and other extra fields, then retry.".into());
    }
    let state = object
        .get("state")
        .ok_or("Missing Jev field state. Add the text, JSON object, or array to evaluate.")?;
    if !description(state) {
        return Err("Jev field state must be text, a JSON object, or an array. Wrap numbers and booleans in an object.".into());
    }
    let questions = object.get("questions").ok_or("Missing Jev field questions. Add a question-ID map, for example {\"check\":{\"type\":\"noul\",\"instructions\":\"Is the state a refund request?\"}}.")?;
    let questions = questions.as_object().ok_or("Jev field questions must be an object mapping question IDs to question objects, not an array. Example: {\"check\":{\"type\":\"noul\",\"instructions\":\"Is the state a refund request?\"}}.")?;
    if questions.is_empty() {
        return Err("Jev field questions is empty. Add at least one noul, choice, or score question with instructions.".into());
    }
    if questions.len() > MAX_QUESTIONS {
        return Err("Jev accepts at most 256 questions per tool call. Split the questions into smaller batches.".into());
    }
    if let Some(model) = object.get("model") {
        let model = model.as_str().ok_or("Jev field model must be a model ID string. Omit model to use plugin settings; do not pass null.")?;
        if model.is_empty() || model.len() > 128 || !model.chars().all(model_character) {
            return Err("The Jev model ID must use 1–128 letters, digits, dots, dashes, underscores, colons, or slashes. Omit model to use plugin settings.".into());
        }
    }
    Ok(())
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
                let criteria = criteria.as_object().ok_or(
                    "Noul criteria must be an object with optional true and false descriptions.",
                )?;
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
                || criteria.len() > 255
                || criteria
                    .iter()
                    .any(|(key, value)| key.is_empty() || !nullable_description(value))
            {
                return Err("Choice criteria must contain 1–255 named options, with text, object, array, or null descriptions.".into());
            }
        }
        Some("score") => {
            let criteria = object
                .get("criteria")
                .and_then(Value::as_array)
                .ok_or("Score criteria must be an ordered array of descriptions.")?;
            if !(2..=10).contains(&criteria.len()) {
                return Err("Score criteria must contain 2–10 ordered level descriptions. Add or remove levels, then retry.".into());
            }
            if criteria.iter().any(|value| !nullable_description(value)) {
                return Err("Score levels must be text, objects, arrays, or null.".into());
            }
        }
        _ => return Err(
            "Jev question type must be noul, choice, or score. Use noul for a boolean question."
                .into(),
        ),
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

/// Whether Jev works here with no key saved: the OpenAgents hosted decision
/// service answers unless `OPENAGENTS_JEV_HOSTED=off` turns it off.
pub fn keyless_available(env: &dyn Fn(&str) -> Option<String>) -> bool {
    !env(jev_hosted::HOSTED_VAR).is_some_and(|value| value.trim() == "off")
}

/// Jev when no key is saved in `/plugins`, through the resolver every Jev
/// caller shares (`jev_hosted::resolve`): this computer's TypeSafe key
/// (`TYPESAFE_API_KEY`, else `api_key` in `~/.openagents/jev.json`), else the
/// OpenAgents hosted decision service, which needs no key. `dir` is
/// `~/.openagents`. A saved key never reaches here: it always wins.
///
/// # Errors
///
/// Why this computer has no Jev, in one sentence that carries no key.
pub fn keyless_client(
    env: &dyn Fn(&str) -> Option<String>,
    dir: &std::path::Path,
    model: &str,
) -> Result<Client, String> {
    let door = jev_hosted::Door {
        url: jev_hosted::DOOR,
        model,
    };
    jev_hosted::resolve(env, dir, &door, &|config| {
        config.retry(RetryPolicy {
            budget: Some(CALL_BUDGET),
            ..RetryPolicy::default()
        })
    })
    .map(|resolved| resolved.client)
    .map_err(|why| format!("Jev is unavailable: {why}"))
}

/// The model a keyless call asks for: the chosen model on TypeSafe's door,
/// else Jev's default (the hosted service fronts TypeSafe's door only).
pub fn keyless_model<'a>(endpoint: &str, model: &'a str) -> &'a str {
    if endpoint.trim_end_matches('/') == DEFAULT_ENDPOINT.trim_end_matches('/') {
        model
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
    fn no_key_resolves_to_the_hosted_service_and_a_key_here_wins() {
        let home = tempfile::tempdir().unwrap();
        let none = |_: &str| None;
        assert!(keyless_available(&none));
        // No key anywhere: the hosted decision service, signed by a key
        // made on first use, and no TypeSafe key on this computer.
        let client = keyless_client(&none, home.path(), jev::defaults::MODEL).unwrap();
        assert_eq!(jev_hosted::via(&client), "hosted");
        assert!(home.path().join(jev_hosted::KEY_FILE).exists());
        // This computer's own TypeSafe key is used before the hosted service.
        let own = |name: &str| (name == "TYPESAFE_API_KEY").then(|| "fixture-own-key".into());
        let client = keyless_client(&own, home.path(), jev::defaults::MODEL).unwrap();
        assert_eq!(jev_hosted::via(&client), "direct");
        // Turned off on this computer: no Jev, and the reason, never a key.
        let off = |name: &str| (name == jev_hosted::HOSTED_VAR).then(|| "off".into());
        assert!(!keyless_available(&off));
        let error = keyless_client(&off, home.path(), jev::defaults::MODEL).unwrap_err();
        assert!(error.starts_with("Jev is unavailable"), "{error}");
        // The hosted service fronts TypeSafe's door, so another door's model
        // falls back to Jev's default.
        assert_eq!(keyless_model(DEFAULT_ENDPOINT, "jev-1.13.0"), "jev-1.13.0");
        assert_eq!(
            keyless_model(GATEWAY_ENDPOINT, GATEWAY_MODEL),
            jev::defaults::MODEL
        );
    }

    #[test]
    fn a_saved_key_wins_over_the_built_in_service() {
        let settings = crate::plugin_tools::ExecutionSettings {
            prompt_inbox: None,
            fleet: None,
            connections: None,
            boat: Default::default(),
            gce: crate::cloud_settings::Configuration::gce(),
            cloud_root: "fixture-state".into(),
            remote_targets: Default::default(),
            microcoder: false,
            cli: false,
            acp: false,
            jev_enabled: true,
            jev_key: Some(model_access::ApiKey::new("fixture-saved-key")),
            redaction_keys: vec![],
            jev_model: jev::defaults::MODEL.into(),
            jev_endpoint: DEFAULT_ENDPOINT.into(),
            agents: vec![],
            cwd: std::path::PathBuf::from("/unused"),
            instructions: None,
            shell: false,
            brainstorm: None,
            disclosure_desk: None,
            memory: None,
        };
        let client = settings.jev_client().unwrap().unwrap();
        assert_eq!(jev_hosted::via(&client), "direct");
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
    fn unsupported_endpoint_error_explains_scope_and_accepts_the_repaired_call() {
        let mut call = arguments();
        call["endpoint"] = json!("https://private-route.invalid/owner-data");
        let error = request(call.clone(), "fixture-token").unwrap_err();
        assert!(error.contains("unsupported top-level field"));
        assert!(error.contains("no other fields"));
        assert!(error.contains("endpoint comes from /plugins"));
        assert!(error.contains("then retry"));
        assert!(!error.contains("private-route"));
        assert!(!error.contains("owner-data"));
        assert!(error.len() <= 512);
        call.as_object_mut().unwrap().remove("endpoint");
        assert!(request(call, "fixture-token").is_ok());
    }

    #[test]
    fn question_shape_errors_identify_the_field_and_allow_corrections() {
        let mut call = arguments();
        let questions = call["questions"].take();
        call["questions"] = json!([questions.clone()]);
        let error = request(call.clone(), "fixture-token").unwrap_err();
        assert!(error.contains("field questions"));
        assert!(error.contains("not an array"));
        assert!(error.contains("\"type\":\"noul\""));
        call["questions"] = questions;
        assert!(request(call.clone(), "fixture-token").is_ok());

        call["questions"]["refund"]["type"] = json!("boolean");
        let error = request(call.clone(), "fixture-token").unwrap_err();
        assert!(error.contains("questions entry 1"));
        assert!(error.contains("Use noul for a boolean question"));
        call["questions"]["refund"]["type"] = json!("noul");
        assert!(request(call.clone(), "fixture-token").is_ok());

        call["model"] = Value::Null;
        let error = request(call.clone(), "fixture-token").unwrap_err();
        assert!(error.contains("field model"));
        assert!(error.contains("Omit model to use plugin settings"));
        call.as_object_mut().unwrap().remove("model");
        assert!(request(call, "fixture-token").is_ok());
    }

    #[test]
    fn local_validation_feedback_never_echoes_arbitrary_names_or_values() {
        let private_name = "private-input-".repeat(256);
        let mut unknown = arguments();
        unknown[&private_name] = json!("private-input-value");
        let error = request(unknown, "fixture-token").unwrap_err();
        assert!(!error.contains("private-input"));
        assert!(error.len() <= 512);
        for question in [
            json!({"type":"noul", "instructions":"text", "private-field":"private-value"}),
            json!({"type":"private-type", "instructions":"text"}),
            json!({"type":"choice", "instructions":"text", "criteria":{"private-option":true}}),
        ] {
            let call =
                json!({"state":"private-state", "questions":{ "private-question-id":question }});
            let error = request(call, "fixture-token").unwrap_err();
            assert!(error.contains("questions entry 1"));
            assert!(!error.contains("private-"));
            assert!(error.len() <= 512);
        }
    }

    #[test]
    fn guidance_gives_a_valid_complete_call_batching_all_three_question_types() {
        let example = instructions()
            .split_once("Complete example using all three types: ")
            .unwrap()
            .1
            .split_once(". The yes/no type")
            .unwrap()
            .0;
        let example: Value = serde_json::from_str(example).unwrap();
        let questions = example["questions"].as_object().unwrap();
        assert_eq!(questions.len(), 3);
        for kind in ["noul", "choice", "score"] {
            assert!(questions.values().any(|question| question["type"] == kind));
        }
        assert!(request(example, "fixture-token").is_ok());
        assert!(instructions().contains("Other API operations are not exposed"));
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
                prompt_inbox: None,
                fleet: None,
                connections: None,
                boat: Default::default(),
                gce: crate::cloud_settings::Configuration::gce(),
                cloud_root: "fixture-state".into(),
                remote_targets: Default::default(),
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
                instructions: None,
                shell: false,
                brainstorm: None,
                disclosure_desk: None,
                memory: None,
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
