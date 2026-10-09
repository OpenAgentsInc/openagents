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
    pub reply_started_at: Option<std::time::Instant>,
    pub busy: bool,
    pub notice: Option<String>,
    pub tokens: u64,
    /// Standing instructions the model reads as system instructions every
    /// turn (`coder chat --instructions`). They are never an entry, so the
    /// transcript, a follower, and an export never show them.
    pub instructions: Option<String>,
    pub(crate) cache: crate::ui::TranscriptCache,
}

#[derive(Clone, PartialEq)]
pub enum Entry {
    User(String),
    Assistant {
        text: String,
        model: Option<String>,
        elapsed_ms: Option<u64>,
    },
    Tool {
        name: String,
        input: serde_json::Value,
        output: serde_json::Value,
        running: bool,
    },
    Delegation {
        id: String,
        name: String,
        task: String,
        running: bool,
        output: serde_json::Value,
        /// The run's latest step and Jev's estimate of how much is done,
        /// while a Microcoder run reports them. Never a budget.
        progress: Option<(usize, Option<f64>)>,
    },
}

pub struct Delegation {
    pub id: String,
    pub name: String,
    pub task: String,
    pub chat: Chat,
    pub started_at: u64,
    pub elapsed_seconds: u64,
    pub running: bool,
    pub(crate) draft: crate::Draft,
    pub(crate) scroll: u16,
}

impl Chat {
    pub fn reply_elapsed_ms(&self) -> Option<u64> {
        self.reply_started_at
            .map(|started| started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64)
    }

    pub fn layout_builds(&self) -> usize {
        self.cache.builds
    }
    pub fn finish_partial(&mut self) {
        if !self.partial.is_empty() {
            self.entries.push(Entry::Assistant {
                elapsed_ms: self.reply_elapsed_ms(),
                text: std::mem::take(&mut self.partial),
                model: self.partial_model.take(),
            });
        }
    }

    pub fn stop_tools(&mut self, reason: &str) {
        for entry in &mut self.entries {
            if let Entry::Tool {
                running, output, ..
            } = entry
            {
                if *running {
                    *running = false;
                    *output = serde_json::json!({"error":reason});
                }
            }
        }
    }

    pub fn tool(
        &mut self,
        name: String,
        input: serde_json::Value,
        output: serde_json::Value,
        running: bool,
    ) {
        self.finish_partial();
        if let Some(Entry::Tool { input: previous_input, output: previous_output, running: previous_running, .. }) = self.entries.iter_mut().rev().find(|entry| {
            matches!(entry, Entry::Tool { name: previous, input: previous_input, running: true, .. } if previous == &name && (input.is_null() || previous_input == &input))
        }) {
            if !input.is_null() { *previous_input = input; }
            *previous_output = output;
            *previous_running = running;
        } else {
            self.entries.push(Entry::Tool { name, input, output, running });
        }
    }

    pub fn messages(&self) -> Vec<Message> {
        let newest_brainstorm = self.entries.iter().rposition(|entry| matches!(entry,
            Entry::Tool { name, output, running: false, .. } if crate::brainstorm::is_tool(name) && (output.get("observation").is_some() || output.get("error").is_some())));
        self.entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                if let Entry::Tool {
                    name,
                    output,
                    running: false,
                    ..
                } = entry
                {
                    if crate::brainstorm::is_tool(name) {
                        if newest_brainstorm != Some(index) {
                            return None;
                        }
                        return crate::brainstorm::context(output).map(|content| Message {
                            role: "assistant".into(),
                            content,
                        });
                    }
                }
                Some(match entry {
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
                    Entry::Delegation {
                        name,
                        task,
                        running,
                        output,
                        ..
                    } => Message {
                        role: "assistant".into(),
                        content: format!(
                            "Delegation to {name}: {task} ({}): {output}",
                            if *running { "running" } else { "finished" }
                        ),
                    },
                })
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
    Brainstorm {
        job: crate::brainstorm::Job,
    },
    Check,
    CheckJev {
        endpoint: String,
        model: String,
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
    Delegate {
        delegation: String,
        name: String,
        tool: String,
        arguments: serde_json::Value,
        execution: ExecutionSettings,
        model: String,
        options: crate::models::GenerationOptions,
    },
}

pub enum Update {
    BrainstormFinished {
        id: u64,
        generation: u64,
        result: Result<crate::brainstorm::Outcome, brainstorm_client::Error>,
    },
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
    Delegation {
        id: u64,
        delegation: String,
        name: String,
        task: String,
        event: RuntimeEvent,
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
            | Self::BrainstormFinished { id, .. }
            | Self::CheckedJev { id, .. }
            | Self::Tool { id, .. }
            | Self::Delegation { id, .. }
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
                                | Update::BrainstormFinished { .. }
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
                            app.apply_update(if let Some(job) = &app.brainstorm_job {
                                Update::BrainstormFinished {
                                    id: *id,
                                    generation: job.generation,
                                    result: Err(brainstorm_client::Error::Transport),
                                }
                            } else if app.checking_jev {
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
        app.poll_disclosure();
        app.process_prompt_queue();
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
    let request = match request {
        Request {
            id,
            kind: Work::Brainstorm { job },
            ..
        } => {
            run_brainstorm(id, job, sender, canceled);
            return;
        }
        request => request,
    };
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
        let mut event_callback = |event| match event {
            RuntimeEvent::Tool { name, input, output, running } => {
                let _ = sender.send(Update::Tool { id, name, input, output, running });
            },
            RuntimeEvent::Delegation { id: delegation, name, task, event } => {
                let _ = sender.send(Update::Delegation { id, delegation, name, task, event: *event });
            },
            _ => {},
        };
        let work = async {
            let update = match request.kind {
                Work::Delegate { delegation, name, tool, arguments, mut execution, model, options } => {
                    if !request.key.expose().is_empty() {
                        execution.redaction_keys.push(model_access::ApiKey::new(request.key.expose()));
                    }
                    let task = arguments["task"].as_str().or(arguments["message"].as_str()).unwrap_or_default().to_owned();
                    let cloud_job=if matches!(tool.as_str(),"boat_job"|"gce_job"){arguments["job"].as_str().map(str::to_owned)}else{None};
                    let mut events = |event| {
                        let event=match event{RuntimeEvent::Delegation{id,event,..} if cloud_job.as_deref()==Some(id.as_str())=>*event,event=>event};
                        event_callback(RuntimeEvent::Delegation {id:delegation.clone(),name:name.clone(),task:task.clone(),event:Box::new(event)});
                    };
                    events(RuntimeEvent::Tool { name: tool.clone(), input: arguments.clone(), output: serde_json::Value::Null, running: true });
                    let provider = if request.key.expose().is_empty() { None } else {
                        openrouter::Client::new(openrouter::Config::new(openrouter::ApiKey::new(request.key.expose()))).ok().map(|client| crate::plugin_tools::GenerationProvider { client, model, effort: options.reasoning })
                    };
                    let result = execution.execute(&tool, arguments, provider, &cancel, &mut events).await;
                    let output = match &result { Ok(value) => value.clone(), Err(error) => serde_json::json!({"error":error}) };
                    events(RuntimeEvent::Tool { name: tool, input: serde_json::Value::Null, output, running: false });
                    Update::Finished { id, result: result.map(|value| Streamed { text: value["reply"].as_str().unwrap_or_default().into(), model: value["model"].as_str().unwrap_or_default().into(), ..Streamed::default() }) }
                },
                Work::CheckJev { endpoint, model } => Update::CheckedJev {
                    id,
                    result: crate::jev_plugin::test_key_for_model(request.key.expose(), &endpoint, &model).await,
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
                            RuntimeEvent::Delegation { .. } | RuntimeEvent::Tokens(_) | RuntimeEvent::Progress { .. } => event_callback(event),
                        }
                    };
                    let result = async {
                        let task = local_task(&messages, &execution)?;
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
                        if !matches!(result["outcome"]["ending"]["reason"].as_str(), Some("finished" | "tests_held" | "checks_passed" | "asked")) {
                            return Err(format!("The coding loop stopped: {}.", result["outcome"]));
                        }
                        if reply.text.is_empty() {
                            return Err("The coding loop stopped before producing a reply. Inspect the tool results or retry with a smaller task.".into());
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

fn run_brainstorm(
    id: u64,
    job: crate::brainstorm::Job,
    sender: mpsc::Sender<Update>,
    canceled: oneshot::Receiver<()>,
) {
    let generation = job.generation;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build();
    let result = match runtime {
        Err(_) => Err(brainstorm_client::Error::Transport),
        Ok(runtime) => runtime.block_on(async {
            let work = job.run();
            tokio::pin!(work);
            tokio::select! {
                biased;
                _ = canceled => {
                    job.cancellation.cancel();
                    Err(brainstorm_client::Error::Cancelled)
                }
                result = &mut work => result,
            }
        }),
    };
    let _ = sender.send(Update::BrainstormFinished {
        id,
        generation,
        result,
    });
}

/// Project the same live messages, including bounded observations, into the local route.
fn local_conversation(messages: &[Message], limit: usize) -> String {
    let mut context = Vec::new();
    let mut bytes = 0;
    for message in messages.iter().rev() {
        let row = format!("{}: {}", message.role, message.content);
        let separator = usize::from(!context.is_empty()) * 2;
        if bytes + row.len() + separator > limit {
            break;
        }
        bytes += row.len() + separator;
        context.push(row);
    }
    context.reverse();
    context.join("\n\n")
}

pub(crate) fn local_task(
    messages: &[Message],
    execution: &ExecutionSettings,
) -> Result<String, String> {
    const LIMIT: usize = 56 * 1024;
    let prefix = execution.instructions.as_deref().filter(|text| !text.trim().is_empty())
        .map(|standing| format!("Standing instructions (from the host, not the user):\n{standing}\n\nThe conversation:\n")).unwrap_or_default();
    let suffix = if execution.cli {
        "\n\nThe bundled OpenAgents CLI is enabled for requested CLI work: openagents --json with an argument array's equivalent syntax. Answer questions directly from what you know and from read-only commands; read a command group's --help only when you need a command you do not know. Follow the user's authorization for effects."
    } else {
        ""
    };
    let allowance = LIMIT
        .checked_sub(prefix.len().saturating_add(suffix.len()))
        .ok_or("The host instructions exceed the local route's context allowance.")?;
    let task = execution.redact_text(&format!(
        "{prefix}{}{suffix}",
        local_conversation(messages, allowance)
    ));
    if task.len() > LIMIT {
        return Err("The redacted context exceeds the local route's allowance.".into());
    }
    Ok(task)
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
            Entry::Assistant { text, model, .. }
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
