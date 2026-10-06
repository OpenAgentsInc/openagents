//! Background provider and plugin work, separate from demo fixtures.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

use model_access::ApiKey;
use openrouter::{Message, Streamed};
use tokio::sync::oneshot;

use crate::provider::{KeyInfo, Provider};
use crate::{bundled_runtime::RuntimeEvent, plugin_tools::ExecutionSettings};

#[derive(Default)]
pub struct Chat {
    pub entries: Vec<Entry>,
    pub partial: String,
    pub partial_model: Option<String>,
    pub busy: bool,
    pub notice: Option<String>,
    pub tokens: u64,
}

pub enum Entry {
    User(String),
    Assistant {
        text: String,
        model: Option<String>,
    },
    Tool {
        name: String,
        input: serde_json::Value,
        output: serde_json::Value,
        running: bool,
    },
}

impl Chat {
    pub fn messages(&self) -> Vec<Message> {
        self.entries
            .iter()
            .map(|entry| match entry {
                Entry::User(text) => Message::user(text.clone()),
                Entry::Assistant { text, .. } => Message {
                    role: "assistant".into(),
                    content: text.clone(),
                },
                Entry::Tool {
                    name,
                    output,
                    running,
                    ..
                } => Message {
                    role: "assistant".into(),
                    content: format!(
                        "Tool observation from {name} ({}): {output}",
                        if *running { "running" } else { "finished" }
                    ),
                },
            })
            .collect()
    }
}

pub(crate) fn model_slug(model: &str) -> Option<String> {
    (!model.is_empty()
        && model.len() <= 1024
        && model
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-/:".contains(&byte)))
    .then(|| model.to_owned())
}

pub struct Request {
    pub id: u64,
    pub key: ApiKey,
    pub kind: Work,
}

pub enum Work {
    Check,
    CheckJev {
        endpoint: String,
    },
    Chat {
        model: String,
        options: crate::models::GenerationOptions,
        messages: Vec<Message>,
        execution: ExecutionSettings,
    },
    Microcoder {
        messages: Vec<Message>,
        execution: ExecutionSettings,
    },
}

pub enum Update {
    Checked {
        id: u64,
        result: Result<KeyInfo, String>,
    },
    CheckedJev {
        id: u64,
        result: Result<Vec<String>, String>,
    },
    Tool {
        id: u64,
        name: String,
        input: serde_json::Value,
        output: serde_json::Value,
        running: bool,
    },
    Delta {
        id: u64,
        text: String,
    },
    Model {
        id: u64,
        model: String,
    },
    Finished {
        id: u64,
        result: Result<Streamed, String>,
    },
}

impl Update {
    pub fn id(&self) -> u64 {
        match self {
            Self::Checked { id, .. }
            | Self::CheckedJev { id, .. }
            | Self::Tool { id, .. }
            | Self::Delta { id, .. }
            | Self::Model { id, .. }
            | Self::Finished { id, .. } => *id,
        }
    }
}

/// One active request. Cancellation also waits for child process cleanup.
#[derive(Default)]
pub struct Background {
    active: Option<(u64, oneshot::Sender<()>, mpsc::Receiver<Update>)>,
}

impl Background {
    pub fn sync(&mut self, app: &mut crate::App) {
        if self
            .active
            .as_ref()
            .is_some_and(|(id, _, _)| *id != app.request_id)
        {
            self.cancel();
        }
        if let Some(request) = app.request.take() {
            self.cancel();
            let id = request.id;
            let (sender, receiver) = mpsc::channel();
            let (cancel, canceled) = oneshot::channel();
            std::thread::spawn(move || run(request, sender, canceled));
            self.active = Some((id, cancel, receiver));
        }
        let mut finished = false;
        if let Some((id, _, receiver)) = &self.active {
            loop {
                match receiver.try_recv() {
                    Ok(update) => {
                        finished |= matches!(
                            update,
                            Update::Checked { .. }
                                | Update::CheckedJev { .. }
                                | Update::Finished { .. }
                        );
                        app.apply_update(update);
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        if !finished {
                            let error =
                                "The chat worker stopped before completing the request.".into();
                            app.apply_update(if app.checking_jev {
                                Update::CheckedJev {
                                    id: *id,
                                    result: Err(error),
                                }
                            } else if app.checking_key {
                                Update::Checked {
                                    id: *id,
                                    result: Err(error),
                                }
                            } else {
                                Update::Finished {
                                    id: *id,
                                    result: Err(error),
                                }
                            });
                        }
                        finished = true;
                        break;
                    }
                }
            }
        }
        if finished {
            self.active = None;
        }
    }

    fn cancel(&mut self) {
        if let Some((_, sender, _)) = self.active.take() {
            let _ = sender.send(());
        }
    }
}

impl Drop for Background {
    fn drop(&mut self) {
        self.cancel();
    }
}

fn run(request: Request, sender: mpsc::Sender<Update>, canceled: oneshot::Receiver<()>) {
    run_with_provider(request, sender, canceled, Provider::new);
}

fn run_with_provider(
    request: Request,
    sender: mpsc::Sender<Update>,
    canceled: oneshot::Receiver<()>,
    create: impl FnOnce(openrouter::ApiKey) -> Result<Provider, String>,
) {
    let id = request.id;
    let checking = match request.kind {
        Work::Check => 1,
        Work::CheckJev { .. } => 2,
        _ => 0,
    };
    let result = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build();
    let Ok(runtime) = result else {
        failure(
            id,
            checking,
            &sender,
            "The chat worker could not start.".into(),
        );
        return;
    };
    runtime.block_on(async move {
        let cancel = Arc::new(AtomicBool::new(false));
        let mut text_callback = |text: &str| {
            let _ = sender.send(Update::Delta { id, text: text.into() });
        };
        let mut model_callback = |model: &str| {
            let _ = sender.send(Update::Model { id, model: model.into() });
        };
        let mut event_callback = |event| {
            if let RuntimeEvent::Tool { name, input, output, running } = event {
                let _ = sender.send(Update::Tool { id, name, input, output, running });
            }
        };
        let work = async {
            let update = match request.kind {
                Work::CheckJev { endpoint } => Update::CheckedJev {
                    id,
                    result: crate::jev_plugin::test_key(request.key.expose(), &endpoint).await,
                },
                Work::Microcoder { messages, execution } => {
                    let mut events = |event| {
                        match event {
                            RuntimeEvent::Text(text) => text_callback(&execution.redact_text(&text)),
                            RuntimeEvent::Model(model) => model_callback(&model),
                            RuntimeEvent::Tool { name, mut input, mut output, running } => {
                                execution.redact(&mut input);
                                execution.redact(&mut output);
                                event_callback(RuntimeEvent::Tool { name, input, output, running });
                            }
                        }
                    };
                    let result = async {
                        let mut context = Vec::new();
                        let mut bytes = 0;
                        for message in messages.iter().rev() {
                            let row = format!("{}: {}", message.role, message.content);
                            if bytes + row.len() > 56 * 1024 {
                                break;
                            }
                            bytes += row.len();
                            context.push(row);
                        }
                        context.reverse();
                        let mut task = context.join("\n\n");
                        if execution.cli {
                            task.push_str("\n\nThe bundled OpenAgents CLI is enabled. Use openagents --help to discover commands, and openagents --json with an argument array's equivalent syntax for requested CLI work. Follow the user's authorization for effects.");
                        }
                        let task = execution.redact_text(&task);
                        let client = execution.jev_client()?;
                        let result = crate::bundled_runtime::microcoder_local(
                            &task, &execution.cwd, client, &execution.redaction_keys,
                            &cancel, &mut events,
                        ).await?;
                        let mut reply = Streamed {
                            text: execution.redact_text(result["reply"].as_str().unwrap_or_default()),
                            model: result["model"].as_str().unwrap_or_default().into(),
                            ..Streamed::default()
                        };
                        reply.usage.total_tokens = result["tokens"].as_u64().unwrap_or_default();
                        if !matches!(result["outcome"]["reason"].as_str(), Some("finished" | "tests_held" | "checks_passed")) {
                            return Err(format!("Microcoder stopped: {}.", result["outcome"]));
                        }
                        if reply.text.is_empty() {
                            return Err("Microcoder stopped before producing a reply. Inspect the tool results or retry with a smaller task.".into());
                        }
                        Ok(reply)
                    }.await;
                    Update::Finished { id, result }
                }
                kind => {
                    let provider = match create(openrouter::ApiKey::new(request.key.expose())) {
                        Ok(provider) => provider,
                        Err(error) => {
                            failure(id, checking, &sender, error);
                            return;
                        }
                    };
                    match kind {
                        Work::Check => Update::Checked { id, result: provider.check().await },
                        Work::Chat { model, options, messages, execution } => Update::Finished {
                            id,
                            result: provider.chat_with_plugins(
                                &model, &options, messages, &execution,
                                &mut text_callback, &mut model_callback,
                                &mut event_callback, &cancel,
                            ).await,
                        },
                        _ => unreachable!(),
                    }
                }
            };
            let _ = sender.send(update);
        };
        tokio::pin!(work);
        tokio::select! {
            _ = canceled => {
                cancel.store(true, Ordering::Relaxed);
                // Process-backed tools need time to cancel and reap their group.
                if checking == 0 {
                    let _ = tokio::time::timeout(std::time::Duration::from_secs(5), &mut work).await;
                }
            },
            _ = &mut work => {}
        }
    });
}

fn failure(id: u64, checking: u8, sender: &mpsc::Sender<Update>, error: String) {
    let update = match checking {
        1 => Update::Checked {
            id,
            result: Err(error),
        },
        2 => Update::CheckedJev {
            id,
            result: Err(error),
        },
        _ => Update::Finished {
            id,
            result: Err(error),
        },
    };
    let _ = sender.send(update);
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        thread::{self, JoinHandle},
        time::{Duration, Instant},
    };

    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    use serde_json::{Value, json};

    use super::*;
    use crate::{App, Mode, Screen, plugins::Connection};

    const FIXTURE_TOKEN: &str = "local-worker-fixture-token";
    const TEST_TIMEOUT: Duration = Duration::from_secs(3);

    fn key(app: &mut App, code: KeyCode) {
        assert!(app.handle(Event::Key(KeyEvent::new(code, KeyModifiers::NONE))));
    }

    fn configured_app() -> App {
        let mut app = App::default();
        app.messages.push("demo-only-message".into());
        assert!(app.handle(Event::Paste("demo-only-draft".into())));
        app.set_mode(Mode::Live);
        key(&mut app, KeyCode::F(2));
        key(&mut app, KeyCode::Enter);
        assert!(app.handle(Event::Paste(FIXTURE_TOKEN.into())));
        key(&mut app, KeyCode::Enter);
        key(&mut app, KeyCode::Home);
        for _ in 0..app.plugins.field(false).0.chars().count() {
            key(&mut app, KeyCode::Delete);
        }
        assert!(app.handle(Event::Paste("fixture/model".into())));
        key(&mut app, KeyCode::Enter);
        key(&mut app, KeyCode::Enter);
        assert!(app.plugins.key_configured);
        assert!(app.checking_key);
        assert!(matches!(
            app.request.as_ref().map(|request| &request.kind),
            Some(Work::Check)
        ));
        app
    }

    fn fixture_listener() -> (TcpListener, String) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}/api/v1", listener.local_addr().unwrap());
        (listener, base)
    }

    fn accept(listener: &TcpListener) -> TcpStream {
        let deadline = Instant::now() + TEST_TIMEOUT;
        loop {
            match listener.accept() {
                Ok((socket, _)) => {
                    socket.set_nonblocking(false).unwrap();
                    socket.set_read_timeout(Some(TEST_TIMEOUT)).unwrap();
                    socket.set_write_timeout(Some(TEST_TIMEOUT)).unwrap();
                    return socket;
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(
                        Instant::now() < deadline,
                        "the worker did not contact the fixture"
                    );
                    thread::sleep(Duration::from_millis(5));
                }
                Err(error) => panic!("fixture accept failed: {error}"),
            }
        }
    }

    fn read_request(socket: &mut TcpStream) -> String {
        let mut bytes = Vec::new();
        let mut buffer = [0_u8; 4096];
        loop {
            let count = socket.read(&mut buffer).unwrap();
            assert!(count > 0, "the worker closed before sending its request");
            bytes.extend_from_slice(&buffer[..count]);
            assert!(
                bytes.len() < 64 * 1024,
                "fixture request exceeded its bound"
            );
            if let Some(header_end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                let header = String::from_utf8_lossy(&bytes[..header_end]);
                let length = header
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().unwrap())
                    })
                    .unwrap_or(0);
                if bytes.len() >= header_end + 4 + length {
                    return String::from_utf8(bytes).unwrap();
                }
            }
        }
    }

    fn authenticated(request: &str) {
        assert!(request.lines().any(|line| {
            line.split_once(':').is_some_and(|(name, value)| {
                name.eq_ignore_ascii_case("authorization")
                    && value.trim() == format!("Bearer {FIXTURE_TOKEN}")
            })
        }));
    }

    fn headers(socket: &mut TcpStream, content_type: &str, body_length: usize) {
        write!(
            socket,
            "HTTP/1.1 200 OK\r\ncontent-type: {content_type}\r\ncontent-length: {body_length}\r\nconnection: close\r\n\r\n"
        )
        .unwrap();
    }

    fn start(app: &mut App, base: &str, background: &mut Background) -> JoinHandle<()> {
        let request = app.request.take().expect("the UI queued work");
        let id = request.id;
        let base = base.to_owned();
        let (sender, receiver) = mpsc::channel();
        let (cancel, canceled) = oneshot::channel();
        let worker = thread::spawn(move || {
            run_with_provider(request, sender, canceled, |key| {
                Provider::with_base(key, &base)
            });
        });
        assert!(background.active.is_none());
        background.active = Some((id, cancel, receiver));
        worker
    }

    fn pump(app: &mut App, background: &mut Background, done: impl Fn(&App) -> bool) {
        let deadline = Instant::now() + TEST_TIMEOUT;
        loop {
            background.sync(app);
            if done(app) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "the loopback worker did not finish its phase"
            );
            thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn saved_key_and_live_prompt_reach_the_worker_and_stream_back_to_the_app() {
        let (listener, base) = fixture_listener();
        let (finish, finish_rx) = mpsc::channel();
        let server = thread::spawn(move || {
            let mut socket = accept(&listener);
            let check = read_request(&mut socket);
            authenticated(&check);
            assert!(check.starts_with("GET /api/v1/key HTTP/1.1"));
            let key_body = r#"{"data":{"label":"local-worker-fixture-token","limit_remaining":0}}"#;
            headers(&mut socket, "application/json", key_body.len());
            socket.write_all(key_body.as_bytes()).unwrap();
            drop(socket);

            let mut socket = accept(&listener);
            let chat = read_request(&mut socket);
            authenticated(&chat);
            let first = "data: {\"model\":\"fixture/model\",\"choices\":[{\"delta\":{\"content\":\"Loopback \"}}]}\n\n";
            let last = concat!(
                "data: {\"choices\":[{\"delta\":{\"content\":\"reply\"},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":3,\"completion_tokens\":2,\"total_tokens\":5,\"cost\":0.002}}\n\n",
                "data: [DONE]\n\n"
            );
            headers(&mut socket, "text/event-stream", first.len() + last.len());
            socket.write_all(first.as_bytes()).unwrap();
            socket.flush().unwrap();
            finish_rx.recv_timeout(TEST_TIMEOUT).unwrap();
            socket.write_all(last.as_bytes()).unwrap();
            chat
        });

        let mut app = configured_app();
        let mut background = Background::default();
        let check_worker = start(&mut app, &base, &mut background);
        pump(&mut app, &mut background, |app| {
            matches!(app.plugins.connection, Connection::Verified)
        });
        check_worker.join().unwrap();
        assert!(!app.checking_key);
        assert!(background.active.is_none());
        assert_eq!(
            app.plugins.connection_label(),
            "OpenRouter API key verified"
        );

        if !app.plugins.enabled {
            key(&mut app, KeyCode::Char(' '));
        }
        assert!(app.plugins.enabled);
        key(&mut app, KeyCode::Esc);
        assert!(app.screen == Screen::Conversation);
        assert!(app.handle(Event::Paste("Loopback prompt".into())));
        key(&mut app, KeyCode::Enter);
        let chat_worker = start(&mut app, &base, &mut background);
        pump(&mut app, &mut background, |app| {
            app.live.partial == "Loopback "
        });
        assert!(app.live.busy);
        assert_eq!(app.live.partial_model.as_deref(), Some("fixture/model"));
        assert!(background.active.is_some());
        assert_eq!(app.live.entries.len(), 1);
        assert!(matches!(&app.live.entries[0], Entry::User(text) if text == "Loopback prompt"));
        finish.send(()).unwrap();
        pump(&mut app, &mut background, |app| !app.live.busy);
        chat_worker.join().unwrap();
        assert!(app.live.partial.is_empty());
        assert!(app.live.partial_model.is_none());
        assert!(app.live.notice.is_none());
        assert_eq!(app.live.tokens, 5);
        assert!(matches!(
            &app.live.entries[1],
            Entry::Assistant { text, model }
                if text == "Loopback reply" && model.as_deref() == Some("fixture/model")
        ));

        let request = server.join().unwrap();
        assert!(request.starts_with("POST /api/v1/chat/completions HTTP/1.1"));
        let body: Value = serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
        assert_eq!(body["model"], "fixture/model");
        assert_eq!(body["stream"], true);
        assert_eq!(
            body["messages"].as_array().unwrap().last().unwrap(),
            &json!({"role":"user","content":"Loopback prompt"})
        );
        assert!(!request.contains("demo-only"));
        assert!(!crate::snapshot::svg(&mut app, 110, 36).contains(FIXTURE_TOKEN));
    }

    #[test]
    fn canceling_a_key_check_closes_the_worker_and_refuses_late_verification() {
        let (listener, base) = fixture_listener();
        let (arrived, arrival) = mpsc::channel();
        let server = thread::spawn(move || {
            let mut socket = accept(&listener);
            let request = read_request(&mut socket);
            authenticated(&request);
            assert!(request.starts_with("GET /api/v1/key HTTP/1.1"));
            arrived.send(()).unwrap();
            let mut byte = [0_u8; 1];
            match socket.read(&mut byte) {
                Ok(0) => true,
                Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => true,
                _ => false,
            }
        });
        let mut app = configured_app();
        let id = app.request_id;
        let mut background = Background::default();
        let worker = start(&mut app, &base, &mut background);
        arrival.recv_timeout(TEST_TIMEOUT).unwrap();
        app.cancel_request();
        background.sync(&mut app);
        worker.join().unwrap();
        assert!(
            server.join().unwrap(),
            "the canceled HTTP connection remained open"
        );
        assert!(background.active.is_none());
        assert!(!app.checking_key);
        app.apply_update(Update::Checked {
            id,
            result: Ok(KeyInfo {
                status: "Verified",
                limit_remaining: Some(1.0),
            }),
        });
        assert!(matches!(app.plugins.connection, Connection::Unchecked));
    }
}
