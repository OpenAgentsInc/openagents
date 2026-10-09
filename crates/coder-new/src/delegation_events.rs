//! Child CLI delegation events travel separately from command output.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::time::Duration;

use serde_json::Value;

use crate::bundled_runtime::RuntimeEvent;

pub(crate) const CHANNEL_ENV: &str = "OPENAGENTS_CODER_EVENT_CHANNEL";
const FRAME_MAX: usize = 1024 * 1024;
const CLIENT_MAX: usize = 16;

pub(crate) trait Sink {
    fn emit(&self, event: RuntimeEvent);
}

impl<F: FnMut(RuntimeEvent)> Sink for RefCell<F> {
    fn emit(&self, event: RuntimeEvent) {
        (self.borrow_mut())(event);
    }
}

struct Client {
    stream: TcpStream,
    buffer: Vec<u8>,
    authenticated: bool,
}

type Scope = Vec<(String, String, String)>;

pub(crate) struct Bridge {
    listener: TcpListener,
    token: String,
    clients: Vec<Client>,
    active: BTreeMap<Vec<String>, (Scope, String)>,
}

impl Bridge {
    pub(crate) fn new() -> Result<Self, String> {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .map_err(|error| format!("Cannot observe child delegations: {error}"))?;
        listener
            .set_nonblocking(true)
            .map_err(|error| error.to_string())?;
        let mut random = [0u8; 32];
        getrandom::fill(&mut random).map_err(|error| error.to_string())?;
        let token = random.iter().map(|byte| format!("{byte:02x}")).collect();
        Ok(Self {
            listener,
            token,
            clients: Vec::new(),
            active: BTreeMap::new(),
        })
    }

    pub(crate) fn prepare(&self, command: &mut std::process::Command) {
        command.env(CHANNEL_ENV, self.endpoint());
    }

    pub(crate) fn endpoint(&self) -> String {
        format!(
            "{}/{}",
            self.listener.local_addr().expect("bound listener"),
            self.token
        )
    }

    pub(crate) fn drain(&mut self, sink: &dyn Sink, keys: &[model_access::ApiKey]) {
        for _ in 0..CLIENT_MAX {
            let Ok((stream, _)) = self.listener.accept() else {
                break;
            };
            if self.clients.len() < CLIENT_MAX && stream.set_nonblocking(true).is_ok() {
                self.clients.push(Client {
                    stream,
                    buffer: Vec::new(),
                    authenticated: false,
                });
            }
        }
        let mut received = Vec::new();
        self.clients.retain_mut(|client| {
            let mut alive = true;
            let mut bytes = [0u8; 8192];
            let mut read = 0;
            while read < FRAME_MAX {
                match client.stream.read(&mut bytes) {
                    Ok(0) => {
                        alive = false;
                        break;
                    }
                    Ok(count) => {
                        client.buffer.extend_from_slice(&bytes[..count]);
                        read += count;
                        if client.buffer.len() > FRAME_MAX {
                            return false;
                        }
                        while let Some(end) = client.buffer.iter().position(|byte| *byte == b'\n') {
                            let line = client.buffer.drain(..=end).collect::<Vec<_>>();
                            if !client.authenticated {
                                if &line[..end] != self.token.as_bytes() {
                                    return false;
                                }
                                client.authenticated = true;
                            } else if let Ok(value) = serde_json::from_slice::<Value>(&line) {
                                if value["event"] == "delegation" {
                                    if let Some(event) = decode(&value, 0) {
                                        received.push(event);
                                    }
                                }
                            }
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(_) => {
                        alive = false;
                        break;
                    }
                }
            }
            alive
        });
        for mut event in received {
            event.redact(keys);
            self.track(&event, &mut Vec::new());
            sink.emit(event);
        }
    }

    fn track(&mut self, event: &RuntimeEvent, scope: &mut Scope) {
        match event {
            RuntimeEvent::Delegation {
                id,
                name,
                task,
                event,
            } => {
                scope.push((id.clone(), name.clone(), task.clone()));
                self.track(event, scope);
                scope.pop();
            }
            RuntimeEvent::Tool { name, running, .. }
                if matches!(name.as_str(), "acp_subagent" | "microcoder") && !scope.is_empty() =>
            {
                let id = scope.iter().map(|(id, _, _)| id.clone()).collect();
                if *running {
                    self.active.insert(id, (scope.clone(), name.clone()));
                } else {
                    self.active.remove(&id);
                }
            }
            _ => {}
        }
    }

    pub(crate) fn finish(&mut self, sink: &dyn Sink, keys: &[model_access::ApiKey]) {
        self.drain(sink, keys);
        for (_, (scope, tool)) in std::mem::take(&mut self.active) {
            let mut event = RuntimeEvent::Tool {
                name: tool,
                input: Value::Null,
                output: serde_json::json!({"error":"The command ended before the delegation returned."}),
                running: false,
            };
            for (id, name, task) in scope.into_iter().rev() {
                event = RuntimeEvent::Delegation {
                    id,
                    name,
                    task,
                    event: Box::new(event),
                };
            }
            sink.emit(event);
        }
    }
}

pub(crate) struct Publisher(Option<TcpStream>);

impl Publisher {
    pub(crate) fn connect(endpoint: Option<&str>) -> Self {
        let connect = || -> Option<TcpStream> {
            let (address, token) = endpoint?.split_once('/')?;
            let address: SocketAddr = address.parse().ok()?;
            if !address.ip().is_loopback() || token.len() != 64 {
                return None;
            }
            let mut stream =
                TcpStream::connect_timeout(&address, Duration::from_millis(100)).ok()?;
            stream.set_nodelay(true).ok()?;
            stream
                .set_write_timeout(Some(Duration::from_millis(500)))
                .ok()?;
            writeln!(stream, "{token}").ok()?;
            Some(stream)
        };
        Self(connect())
    }

    pub(crate) fn send(&mut self, value: &Value) {
        if value["event"] != "delegation" {
            return;
        }
        let Ok(mut bytes) = serde_json::to_vec(value) else {
            return;
        };
        if bytes.len() >= FRAME_MAX {
            return;
        }
        bytes.push(b'\n');
        if self
            .0
            .as_mut()
            .is_some_and(|stream| stream.write_all(&bytes).is_err())
        {
            self.0 = None;
        }
    }
}

pub(crate) fn decode(value: &Value, depth: usize) -> Option<RuntimeEvent> {
    if depth > 16 {
        return None;
    }
    let word = |name| value.get(name)?.as_str().map(str::to_owned);
    Some(match value["event"].as_str()? {
        "delegation" => RuntimeEvent::Delegation {
            id: word("id")?,
            name: word("name")?,
            task: word("task")?,
            event: Box::new(decode(value.get("update")?, depth + 1)?),
        },
        "delta" => RuntimeEvent::Text(word("text")?),
        "model" => RuntimeEvent::Model(word("model")?),
        "usage" => RuntimeEvent::Tokens(value.get("tokens")?.as_u64()?),
        "progress" => RuntimeEvent::Progress {
            step: usize::try_from(value.get("step")?.as_u64()?).ok()?,
            complete: value
                .get("complete")
                .and_then(Value::as_f64)
                .filter(|complete| complete.is_finite()),
        },
        "tool" => RuntimeEvent::Tool {
            name: word("name")?,
            input: value.get("input")?.clone(),
            output: value.get("output")?.clone(),
            running: value.get("running")?.as_bool()?,
        },
        _ => return None,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{App, Mode, live::Update};
    use serde_json::json;
    use std::path::Path;
    use std::sync::{Arc, atomic::AtomicBool};

    #[test]
    fn child_cli_fixture() {
        let Ok(root) = std::env::var("CODER_DELEGATION_TEST_ROOT") else {
            return;
        };
        let root = std::path::PathBuf::from(root);
        let context = crate::programmatic::Context {
            root: root.join("state"),
            cwd: root.clone(),
            environment: [
                (
                    "HOME".into(),
                    root.join("home").to_string_lossy().into_owned(),
                ),
                ("PATH".into(), root.to_string_lossy().into_owned()),
                (
                    "CODER_ONE_CODEX_BIN".into(),
                    root.join("codex").to_string_lossy().into_owned(),
                ),
                (CHANNEL_ENV.into(), std::env::var(CHANNEL_ENV).unwrap()),
            ]
            .into(),
            input: None,
            canceled: None,
            approvals: None,
        };
        crate::programmatic::execute(
            &[
                "delegate",
                "codex",
                "--task",
                "Review the fixture",
                "--session",
                "cli-fixture",
            ]
            .map(str::to_owned),
            &context,
            &mut |value| println!("{value}"),
        )
        .unwrap();
    }

    #[cfg(unix)]
    pub(crate) fn fixture(root: &Path) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let codex = root.join("codex");
        std::fs::write(&codex, r#"#!/bin/sh
printf '%s\n' '{"type":"thread.started","thread_id":"fixture","model":"fixture/codex"}'
printf '%s\n' '{"type":"item.started","item":{"type":"command_execution","command":"fixture child command"}}'
/bin/sleep 0.15
printf '%s\n' '{"type":"item.completed","item":{"type":"command_execution","command":"fixture child command","aggregated_output":"fixture child output","exit_code":0}}'
printf '%s\n' '{"type":"item.completed","item":{"type":"agent_message","text":"Fixture child reply."}}'
printf '%s\n' '{"type":"turn.completed","usage":{"input_tokens":11,"output_tokens":7}}'
"#).unwrap();
        std::fs::set_permissions(&codex, std::fs::Permissions::from_mode(0o700)).unwrap();
        let cli = root.join("openagents");
        let word = crate::bundled_runtime::shell_word;
        std::fs::write(&cli, format!(
            "#!/bin/sh\nexport CODER_DELEGATION_TEST_ROOT={}\nexec {} --exact delegation_events::tests::child_cli_fixture --nocapture\n",
            word(&root.to_string_lossy()), word(&std::env::current_exe().unwrap().to_string_lossy()),
        )).unwrap();
        std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o700)).unwrap();
        cli
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn redirected_cli_delegation_creates_a_live_rail_and_retained_child_chat() {
        let root = tempfile::tempdir().unwrap();
        let cli = fixture(root.path());
        let mut app = App::default();
        app.set_mode(Mode::Live);
        app.live.busy = true;
        let mut saw_running = false;
        let command = format!(
            "{} --json coder delegate codex --task 'Review the fixture' > delegated.jsonl 2>&1",
            crate::bundled_runtime::shell_word(&cli.to_string_lossy())
        );
        let result = crate::bundled_runtime::run_command(
            &command,
            root.path(),
            &[],
            &Arc::new(AtomicBool::new(false)),
            &mut |event| {
                let RuntimeEvent::Delegation {
                    id,
                    name,
                    task,
                    event,
                } = event
                else {
                    panic!("expected a child event")
                };
                app.apply_update(Update::Delegation {
                    id: app.request_id,
                    delegation: id,
                    name,
                    task,
                    event: *event,
                });
                if app.delegations[0].running {
                    saw_running = true;
                    let mut terminal =
                        ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24)).unwrap();
                    terminal
                        .draw(|frame| crate::ui::render(frame, &mut app))
                        .unwrap();
                    let buffer = terminal.backend().buffer();
                    let rail: String = (0..80).map(|x| buffer[(x, 22)].symbol()).collect();
                    assert!(
                        rail.contains("Codex"),
                        "Missing subagent below the composer: {rail}"
                    );
                }
            },
        )
        .await
        .unwrap();
        assert_eq!(result["exit"], 0, "{result}");
        assert_eq!(result["output"], "");
        assert!(
            std::fs::read_to_string(root.path().join("delegated.jsonl"))
                .unwrap()
                .contains("delegation")
        );
        assert!(saw_running);
        assert_eq!(app.delegations.len(), 1);
        let child = &app.delegations[0];
        assert!(!child.running && !child.chat.busy);
        assert_eq!(child.chat.tokens, 18);
        assert!(child.chat.entries.iter().any(|entry| matches!(entry, crate::live::Entry::Assistant { text, model, .. } if text == "Fixture child reply." && model.as_deref() == Some("fixture/codex"))));
        assert!(child.chat.entries.iter().any(|entry| matches!(entry, crate::live::Entry::Tool { name, running: false, .. } if name == "Run")));
        let document = crate::trajectory::main_document(&app, root.path());
        let mut restored = App::default();
        crate::trajectory::restore_app(&mut restored, &document).unwrap();
        assert_eq!(restored.delegations.len(), 1);
        assert_eq!(restored.delegations[0].name, "Codex");
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn structured_cli_delegation_uses_the_same_child_event_stream() {
        let root = tempfile::tempdir().unwrap();
        let cli = fixture(root.path());
        let mut events = Vec::new();
        let result = crate::bundled_runtime::cli_at(
            &cli,
            &["coder".into(), "delegate".into(), "codex".into()],
            root.path(),
            &Arc::new(AtomicBool::new(false)),
            &mut |event| events.push(event),
        )
        .await
        .unwrap();
        assert_eq!(result["exit"], 0, "{result}");
        assert!(events.iter().any(|event| matches!(event, RuntimeEvent::Delegation { name, event, .. } if name == "Codex" && matches!(event.as_ref(), RuntimeEvent::Tool { name, running: true, .. } if name == "acp_subagent"))));
        assert!(events.iter().any(|event| matches!(event, RuntimeEvent::Delegation { event, .. } if matches!(event.as_ref(), RuntimeEvent::Tool { running: false, output, .. } if output["tokens"] == 18))));
    }

    #[test]
    fn channel_rejects_wrong_tokens_and_closes_unfinished_redacted_children() {
        let mut bridge = Bridge::new().unwrap();
        let events = RefCell::new(Vec::new());
        let sink = RefCell::new(|event| events.borrow_mut().push(event));
        // Loopback delivery is asynchronous: poll the bridge until a condition
        // holds instead of assuming one drain sees bytes written just before.
        let wait = |bridge: &mut Bridge, done: &dyn Fn(&Bridge) -> bool| {
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            while !done(bridge) {
                assert!(
                    std::time::Instant::now() < deadline,
                    "bridge never delivered"
                );
                bridge.drain(&sink, &[model_access::ApiKey::new("credential")]);
                std::thread::sleep(Duration::from_millis(2));
            }
        };
        let mut value = json!({"event":"delegation","id":"fixture","name":"Codex","task":"credential","update":{"event":"tool","name":"acp_subagent","input":{},"output":null,"running":true}});
        let wrong = bridge.endpoint().replace(&bridge.token, &"0".repeat(64));
        Publisher::connect(Some(&wrong)).send(&value);
        bridge.drain(&sink, &[]);
        assert!(bridge.active.is_empty());
        let mut publisher = Publisher::connect(Some(&bridge.endpoint()));
        publisher.send(&value);
        value["event"] = json!("tool");
        publisher.send(&value);
        wait(&mut bridge, &|bridge| !bridge.active.is_empty());
        bridge.finish(&sink, &[model_access::ApiKey::new("credential")]);
        drop(sink);
        let events = events.into_inner();
        assert_eq!(events.len(), 2);
        assert!(matches!(&events[0], RuntimeEvent::Delegation {task,..} if task == "[redacted]"));
        assert!(
            matches!(&events[1], RuntimeEvent::Delegation {event,..} if matches!(event.as_ref(), RuntimeEvent::Tool {running:false,output,..} if output["error"].is_string()))
        );
    }
}
