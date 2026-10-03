//! A local Responses door backed by the operator's signed-in coding engine.
//! Login credentials stay outside the eval sandbox. Each call runs without
//! tools in a scratch directory and validates its structured result.

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
};
use coder_delegate::delegate::{Agent, Credential};
use serde_json::{Value, json};
use std::io::Read;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
struct Engine {
    agent: Agent,
    binary: PathBuf,
}

pub(crate) struct Bridge {
    pub(crate) url: String,
    pub(crate) key: String,
    pub(crate) model: String,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Drop for Bridge {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
impl Bridge {
    pub(crate) fn discover() -> Result<Self, String> {
        for agent in [Agent::ClaudeCode, Agent::Codex] {
            let (binary, credential) =
                coder_delegate::delegate::resolve(agent, |name| std::env::var(name).ok());
            // Claude can keep its login in the macOS keychain, with no credentials file.
            let logged_in = credential != Credential::Missing
                || binary.as_ref().is_some_and(|binary| {
                    if agent != Agent::ClaudeCode {
                        return false;
                    }
                    // The interview's runner can call discovery from inside a runtime.
                    let binary = binary.clone();
                    let Ok(ended) = std::thread::spawn(move || {
                        crate::runtime().block_on(
                            supervise::Job::new(binary.as_os_str())
                                .args(["auth", "status", "--json"])
                                .bounded(supervise::Limits::within(Duration::from_secs(10)))
                                .run(),
                        )
                    })
                    .join() else {
                        return false;
                    };
                    ended.ending.success()
                        && serde_json::from_str::<Value>(&ended.stdout.text)
                            .is_ok_and(|status| status["loggedIn"] == true)
                });
            if let Some(binary) = binary
                && logged_in
            {
                return Self::start(Engine { agent, binary });
            }
        }
        Err("Sign in to Claude Code or Codex, or add a provider key with `openagents settings provider-key set openrouter`.".into())
    }
    fn start(engine: Engine) -> Result<Self, String> {
        let key = ext_eval::proxy::random_token().map_err(|e| e.to_string())?;
        let model = engine.agent.word().to_string();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let url = format!(
            "http://{}",
            listener.local_addr().map_err(|e| e.to_string())?
        );
        let state = Arc::new((engine, key.clone()));
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;
        let thread = std::thread::spawn(move || {
            runtime.block_on(async move {
                let listener =
                    tokio::net::TcpListener::from_std(listener).expect("bridge listener");
                let app = Router::new()
                    .route("/v1/responses", post(respond))
                    .with_state(state);
                let _ = axum::serve(listener, app)
                    .with_graceful_shutdown(async {
                        let _ = stopped.await;
                    })
                    .await;
            })
        });
        Ok(Self {
            url,
            key,
            model,
            stop: Some(stop),
            thread: Some(thread),
        })
    }
}
async fn respond(
    State(state): State<Arc<(Engine, String)>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    if headers.get("authorization").and_then(|h| h.to_str().ok())
        != Some(format!("Bearer {}", state.1).as_str())
    {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    match state.0.answer(&body).await {
        Ok(text) => {
            let delta = json!({"type":"response.output_text.delta", "delta":text});
            let done = json!({"type":"response.completed", "response":{}});
            if body["stream"] == true {
                (
                    [("content-type", "text/event-stream")],
                    format!("data: {delta}\n\ndata: {done}\n\n"),
                )
                    .into_response()
            } else {
                Json(json!({"output": [{"type": "message", "content": [{"type": "output_text", "text": text}]}]})).into_response()
            }
        }
        Err(error) => (StatusCode::BAD_GATEWAY, Json(json!({"error":error}))).into_response(),
    }
}
fn contract(body: &Value) -> Result<(Value, bool), String> {
    if let Some(schema) = body.pointer("/text/format/schema") {
        return Ok((schema.clone(), false));
    }
    let instructions = body["instructions"].as_str().unwrap_or_default();
    if let Some((_, tail)) = instructions.split_once("## JSON schema\n") {
        let mut values = serde_json::Deserializer::from_str(tail).into_iter::<Value>();
        return values
            .next()
            .ok_or("missing proposal schema")?
            .map(|v| (v, false))
            .map_err(|e| e.to_string());
    }
    Ok((
        json!({"type":"object", "properties":{"answer":{"type":"string"}}, "required":["answer"], "additionalProperties":false}),
        true,
    ))
}
// Codex requires closed objects and every property in required. Optional
// proposal fields remain nullable, so the interview can still omit a change.
fn strict_schema(schema: &mut Value) {
    match schema {
        Value::Object(object) => {
            for value in object.values_mut() {
                strict_schema(value);
            }
            if let Some(value) = object.remove("const") {
                object.insert("enum".into(), json!([value]));
            }
            if let Some(one_of) = object.remove("oneOf") {
                object.insert("anyOf".into(), one_of);
            }
            if let Some(Value::Object(properties)) = object.get("properties") {
                let required: Vec<Value> = properties.keys().map(|key| json!(key)).collect();
                object.insert("required".into(), json!(required));
                object.insert("additionalProperties".into(), json!(false));
            }
        }
        Value::Array(values) => {
            for value in values {
                strict_schema(value);
            }
        }
        _ => {}
    }
}
impl Engine {
    async fn answer(&self, body: &Value) -> Result<String, String> {
        let (mut schema, wrapped) = contract(body)?;
        if self.agent == Agent::Codex {
            strict_schema(&mut schema);
        }
        let validator = jsonschema::validator_for(&schema).map_err(|e| e.to_string())?;
        let scratch = tempfile::tempdir().map_err(|e| e.to_string())?;
        let schema_path = scratch.path().join("schema.json");
        let output_path = scratch.path().join("answer.json");
        std::fs::write(&schema_path, schema.to_string()).map_err(|e| e.to_string())?;
        let mut prompt = format!(
            "{}\n\nConversation: {}\n\nReturn JSON matching this schema: {schema}. {}",
            body["instructions"].as_str().unwrap_or_default(),
            body["input"],
            if wrapped {
                "Put your complete reply in answer, including any JSON plan the instructions request."
            } else {
                ""
            }
        );
        for attempt in 0..2 {
            // Never read a last-message file left by an earlier attempt.
            let _ = std::fs::remove_file(&output_path);
            let mut command = std::process::Command::new(&self.binary);
            command
                .current_dir(scratch.path())
                .env("ENABLE_CLAUDEAI_MCP_SERVERS", "false");
            match self.agent {
                Agent::ClaudeCode => {
                    command
                        .args([
                            "-p",
                            "--output-format",
                            "json",
                            "--tools",
                            "",
                            "--strict-mcp-config",
                            "--mcp-config",
                            "{\"mcpServers\":{}}",
                            "--no-session-persistence",
                        ])
                        .arg(&prompt);
                }
                _ => {
                    command
                        .args([
                            "exec",
                            "--sandbox",
                            "read-only",
                            "--skip-git-repo-check",
                            "--ephemeral",
                            "--output-schema",
                        ])
                        .arg(&schema_path)
                        .arg("--output-last-message")
                        .arg(&output_path)
                        .arg(&prompt);
                }
            }
            let ended = supervise::Job::from_command(command)
                .bounded(supervise::Limits::within(Duration::from_secs(180)).keeping(1024 * 1024))
                .run()
                .await;
            if !ended.ending.success() || ended.truncated() {
                return Err(format!(
                    "{} structured call {}",
                    self.agent.word(),
                    ended.ending
                ));
            }
            let output = if self.agent == Agent::Codex {
                let mut output = String::new();
                std::fs::File::open(&output_path)
                    .map_err(|e| e.to_string())?
                    .take(1024 * 1024 + 1)
                    .read_to_string(&mut output)
                    .map_err(|e| e.to_string())?;
                if output.len() > 1024 * 1024 {
                    return Err("the engine result exceeds 1 MiB".into());
                }
                output
            } else {
                ended.stdout.text
            };
            let parsed = decode(self.agent, &output).and_then(|v| {
                validator.validate(&v).map_err(|e| e.to_string())?;
                Ok(v)
            });
            match parsed {
                Ok(value) => {
                    return if wrapped {
                        value["answer"]
                            .as_str()
                            .map(str::to_string)
                            .ok_or("missing answer".into())
                    } else {
                        Ok(value.to_string())
                    };
                }
                Err(error) if attempt == 0 => prompt.push_str(&format!(
                    "\nYour previous output failed validation: {error}. Return only corrected JSON."
                )),
                Err(error) => {
                    return Err(format!(
                        "{} returned invalid JSON after one retry: {error}",
                        self.agent.word()
                    ));
                }
            }
        }
        unreachable!()
    }
}
fn decode(agent: Agent, output: &str) -> Result<Value, String> {
    let value: Value = serde_json::from_str(output).map_err(|e| e.to_string())?;
    if agent == Agent::ClaudeCode {
        if value["is_error"] == true {
            return Err("Claude Code reported an error".into());
        }
        if let Some(structured) = value.get("structured_output") {
            return Ok(structured.clone());
        }
        return serde_json::from_str(value["result"].as_str().ok_or("Claude result is missing")?)
            .map_err(|e| e.to_string());
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    const CLAUDE: &str = include_str!("../fixtures/eval-engine/claude.json");
    const CODEX: &str = include_str!("../fixtures/eval-engine/codex.json");

    fn request() -> Value {
        let schema = json!({"type":"object","properties":{"say":{"type":"string"}},"required":["say"],"additionalProperties":false});
        json!({"instructions":format!("Interview\n## JSON schema\n{schema}\n\n## State\n{{}}"),"input":[],"stream":true})
    }
    #[test]
    fn recorded_results_and_schemas() {
        assert_eq!(
            decode(Agent::ClaudeCode, CLAUDE).unwrap(),
            decode(Agent::Codex, CODEX).unwrap()
        );
        assert!(decode(Agent::ClaudeCode, r#"{"is_error":true,"result":"no"}"#).is_err());
        assert!(decode(Agent::Codex, "Here are your tests.").is_err());
        let (schema, wrapped) = contract(&request()).unwrap();
        assert!(!wrapped);
        assert!(
            jsonschema::validator_for(&schema)
                .unwrap()
                .is_valid(&decode(Agent::Codex, CODEX).unwrap())
        );
    }
    #[cfg(unix)]
    fn fake(agent: Agent, fail_twice: bool) -> (tempfile::TempDir, Engine) {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("engine");
        let fixture = if agent == Agent::ClaudeCode {
            CLAUDE
        } else {
            CODEX
        };
        let flags = if agent == Agent::ClaudeCode {
            "case \"$*\" in *'--output-format json'*'--tools'*'--no-session-persistence'*) ;; *) exit 3 ;; esac"
        } else {
            "[ \"$1\" = exec ] || exit 3\ncase \"$*\" in *'--output-schema'*) ;; *) exit 3 ;; esac\nwhile [ \"$1\" != --output-schema ]; do shift; done\n[ -f \"$2\" ] || exit 4"
        };
        let destination = if agent == Agent::Codex {
            "while [ \"$1\" != --output-last-message ]; do shift; done\nexec > \"$2\""
        } else {
            ""
        };
        let script = format!(
            "#!/bin/sh\n{flags}\n{destination}\nprintf '%s\\n' \"$*\" >> '{}'\nif [ ! -f '{}' ]; then touch '{}'; printf '%s\\n' 'prose instead of JSON'; elif {}; then printf '%s\\n' '{{\"say\":9}}'; else printf '%s\\n' '{}'; fi\n",
            dir.path().join("calls").display(),
            dir.path().join("called").display(),
            dir.path().join("called").display(),
            if fail_twice { "true" } else { "false" },
            fixture.trim()
        );
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        (
            dir,
            Engine {
                agent,
                binary: path,
            },
        )
    }
    #[tokio::test]
    #[cfg(unix)]
    async fn both_engines_retry_once_with_the_parse_error() {
        for agent in [Agent::ClaudeCode, Agent::Codex] {
            let (dir, engine) = fake(agent, false);
            let result = engine.answer(&request()).await.unwrap();
            assert_eq!(
                serde_json::from_str::<Value>(&result).unwrap()["say"],
                "We help you check the plugin."
            );
            let calls = std::fs::read_to_string(dir.path().join("calls")).unwrap();
            assert_eq!(calls.matches("Return JSON matching").count(), 2);
            assert!(calls.contains("failed validation: expected value"));
            let (dir, engine) = fake(agent, true);
            assert!(
                engine
                    .answer(&request())
                    .await
                    .unwrap_err()
                    .contains("after one retry")
            );
            assert_eq!(
                std::fs::read_to_string(dir.path().join("calls"))
                    .unwrap()
                    .matches("Return JSON matching")
                    .count(),
                2
            );
        }
    }
    #[test]
    #[cfg(unix)]
    fn responses_boundary_authenticates_and_streams_validated_output() {
        let (_dir, engine) = fake(Agent::ClaudeCode, false);
        let bridge = Bridge::start(engine).unwrap();
        let client = reqwest::blocking::Client::new();
        let url = format!("{}/v1/responses", bridge.url);
        assert_eq!(
            client.post(&url).json(&request()).send().unwrap().status(),
            401
        );
        let text = client
            .post(&url)
            .bearer_auth(&bridge.key)
            .json(&request())
            .send()
            .unwrap()
            .error_for_status()
            .unwrap()
            .text()
            .unwrap();
        assert!(text.contains("response.output_text.delta"));
        assert!(text.contains("response.completed"));
        assert!(text.contains("We help you check the plugin."));
    }
    #[test]
    fn every_interview_contract_is_valid_and_rejects_wrong_fields() {
        use ext_eval::author::{Need, proposal};
        for need in [
            Need::Tool { change: None },
            Need::Tests { change: None },
            Need::Checks { change: None },
            Need::Fix {
                change: "fix".into(),
            },
            Need::Read,
            Need::Say {
                message: "ready".into(),
            },
        ] {
            let mut schema = proposal::schema(&need);
            let validator = jsonschema::validator_for(&schema).unwrap();
            assert!(!validator.is_valid(&json!({})));
            assert!(!validator.is_valid(&json!({"say": 9})));
            strict_schema(&mut schema);
            jsonschema::validator_for(&schema).unwrap();
            assert_eq!(schema["additionalProperties"], false);
            assert_eq!(
                schema["required"].as_array().unwrap().len(),
                schema["properties"].as_object().unwrap().len()
            );
        }
    }
    #[tokio::test]
    #[cfg(unix)]
    async fn both_engines_retry_schema_errors_with_the_validation_error() {
        for agent in [Agent::ClaudeCode, Agent::Codex] {
            let (dir, engine) = fake(agent, false);
            let wrong = if agent == Agent::ClaudeCode {
                json!({"type":"result", "is_error":false, "result":"{\"say\":9}"}).to_string()
            } else {
                json!({"say":9}).to_string()
            };
            let script = std::fs::read_to_string(&engine.binary).unwrap();
            std::fs::write(
                &engine.binary,
                script.replace("prose instead of JSON", &wrong),
            )
            .unwrap();
            let result = engine.answer(&request()).await.unwrap();
            assert_eq!(
                serde_json::from_str::<Value>(&result).unwrap()["say"],
                "We help you check the plugin."
            );
            let calls = std::fs::read_to_string(dir.path().join("calls")).unwrap();
            assert_eq!(calls.matches("Return JSON matching").count(), 2);
            assert!(calls.contains("failed validation:"));
            assert!(calls.contains("string"), "{calls}");
        }
    }
    #[tokio::test]
    #[cfg(unix)]
    async fn run_replies_preserve_plain_answers_and_coder_plans() {
        let plan = r#"{"v":1,"commands":[{"command":"pwd","why":"check the workspace"}]}"#;
        for agent in [Agent::ClaudeCode, Agent::Codex] {
            for answer in ["Welcome aboard!", plan] {
                let (_dir, engine) = fake(agent, false);
                let wrapped = json!({"answer": answer}).to_string();
                let recorded = if agent == Agent::ClaudeCode {
                    json!({"type":"result", "is_error":false, "result": wrapped}).to_string()
                } else {
                    wrapped
                };
                let fixture = if agent == Agent::ClaudeCode {
                    CLAUDE
                } else {
                    CODEX
                };
                let script = std::fs::read_to_string(&engine.binary).unwrap();
                std::fs::write(&engine.binary, script.replace(fixture.trim(), &recorded)).unwrap();
                let request = json!({"instructions":"Answer the task or propose a Coder JSON plan.",
                                     "input":[], "stream":false});
                let reply = engine.answer(&request).await.unwrap();
                assert_eq!(reply, answer);
                if answer == plan {
                    let parsed = coder::Plan::read(&reply).unwrap();
                    assert_eq!(parsed.proposals[0].command, "pwd");
                }
            }
        }
    }
}
