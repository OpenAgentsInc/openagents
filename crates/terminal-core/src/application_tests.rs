use crate::{
    Application,
    context::Context,
    layout::Axis,
    pty::{Attachment, Event, Program, Sessions, Transport},
    smart::Draft,
};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
};

#[derive(Default)]
struct Fake {
    bridge: bool,
    output: Arc<Mutex<std::collections::VecDeque<Vec<u8>>>>,
    requests: Mutex<Vec<crate::bridge::Request>>,
    opened: AtomicUsize,
    input: Arc<Mutex<Vec<Vec<u8>>>>,
    closed: Arc<AtomicUsize>,
}
struct Pane {
    bridge: bool,
    output: Arc<Mutex<std::collections::VecDeque<Vec<u8>>>>,
    input: Arc<Mutex<Vec<Vec<u8>>>>,
    closed: Arc<AtomicUsize>,
}
impl Attachment for Pane {
    fn input(&self, bytes: &[u8]) {
        self.input.lock().unwrap().push(bytes.to_vec());
    }
    fn resize(&self, _: u16, _: u16) {}
    fn close(&self) {
        self.closed.fetch_add(1, Ordering::SeqCst);
    }
    fn poll(&mut self) -> Option<Event> {
        self.output.lock().unwrap().pop_front().map(Event::Output)
    }
    fn target(&self) -> Option<crate::proposals::Binding> {
        self.bridge.then(|| crate::proposals::Binding {
            terminal: "fixture-pane".into(),
            generation: "fixture-generation".into(),
            cwd: "/test/work".into(),
            shell_directory: None,
            context_digest: String::new(),
        })
    }
    fn directory(&self) -> Option<String> {
        None
    }
}
impl Transport for Fake {
    fn shell(&self) -> &Path {
        Path::new("/test/shell")
    }
    fn open(&self, _: &Program, _: u16, _: u16) -> Result<Box<dyn Attachment>, String> {
        self.opened.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(Pane {
            bridge: self.bridge,
            output: self.output.clone(),
            input: self.input.clone(),
            closed: self.closed.clone(),
        }))
    }
    fn shutdown(&self) {
        self.closed
            .fetch_add(self.opened.swap(0, Ordering::SeqCst), Ordering::SeqCst);
    }
    fn thread_program(&self) -> Option<Program> {
        None
    }
    fn resolve(&self, _: &str) -> Option<PathBuf> {
        None
    }
    fn request(
        &self,
        request: &crate::bridge::Request,
    ) -> Result<crate::bridge::Connection, String> {
        assert!(self.bridge, "A preview must not send a request");
        request.message().unwrap();
        self.requests.lock().unwrap().push(request.clone());
        let (sender, events) = mpsc::channel();
        sender
            .send(crate::bridge::Message::Attached(request.thread.clone()))
            .unwrap();
        if !request.text.starts_with("I ran `") {
            sender
                .send(crate::bridge::Message::Door("local".into()))
                .unwrap();
            sender
                .send(crate::bridge::Message::Answer(
                    "**It failed** because `2 + 2` is not 5 \u{2014} see [the test](src/lib.rs).\n{\"v\":1,\"commands\":[{\"command\":\"cargo test\",\"why\":\"rerun\"}]}".into(),
                ))
                .unwrap();
            sender
                .send(crate::bridge::Message::Proposal(
                    crate::proposals::Proposal {
                        thread: request.thread.clone(),
                        id: request.request.clone(),
                        revision: 1,
                        command: "cargo test".into(),
                        binding: request.binding.clone(),
                    },
                    crate::proposals::Effect::Ordinary,
                ))
                .unwrap();
        } else {
            // Duplicate delivery acknowledgments never repeat terminal input.
            sender
                .send(crate::bridge::Message::Attached(request.thread.clone()))
                .unwrap();
        }
        Ok(crate::bridge::Connection {
            events,
            process: Box::new(Ended),
        })
    }
    fn git_summary(&self, _: u64, _: String) -> mpsc::Receiver<(u64, String, String)> {
        mpsc::channel().1
    }
    fn open_link(&self, _: &str) -> Result<(), String> {
        Ok(())
    }
    fn clipboard(&self) -> Option<String> {
        None
    }
    fn copy(&self, _: &str) -> Result<(), String> {
        Ok(())
    }
}

#[test]
fn injected_panes_keep_preview_input_local_and_end_on_shutdown() {
    let transport = Arc::new(Fake::default());
    let mut app = Application::new(Sessions(transport.clone()));
    app.toggle();
    app.ensure_started();
    let first = app.focus_id().unwrap();
    app.smart.draft = Some(Draft {
        pane: first,
        text: String::new(),
        scroll: 0,
        context: Context::default(),
    });
    app.paste("why\nthat failed");
    assert_eq!(app.smart.draft.as_ref().unwrap().text, "whythat failed");
    assert!(transport.input.lock().unwrap().is_empty());
    app.split(Axis::Columns, &Program::Shell);
    assert_ne!(app.focus_id(), Some(first));
    app.paste("ordinary shell input");
    assert_eq!(
        transport.input.lock().unwrap().as_slice(),
        &[b"ordinary shell input".to_vec()]
    );
    assert_eq!(app.smart.draft.as_ref().unwrap().text, "whythat failed");
    app.toggle();
    assert_eq!(app.panes(), 2);
    assert_eq!(transport.closed.load(Ordering::SeqCst), 0);
    app.toggle();
    app.shutdown();
    assert_eq!(transport.closed.load(Ordering::SeqCst), 2);
    assert_eq!(app.panes(), 0);
}

struct Ended;
impl crate::bridge::Process for Ended {
    fn ended(&mut self) -> Option<bool> {
        Some(true)
    }
}

#[test]
fn fixture_failure_request_typed_proposal_enter_and_result_stay_in_one_thread() {
    use crate::input::{KeyCode, Logical, NamedKey};
    let transport = Arc::new(Fake {
        bridge: true,
        ..Fake::default()
    });
    let mut app = Application::new(Sessions(transport.clone()));
    app.toggle();
    app.ensure_started();
    transport.output.lock().unwrap().push_back(b"\x1b]7;file:///test/work\x07\x1b]133;A\x07$ \x1b]133;B\x07cargo test\r\n\x1b]777;openagents;command;636172676f2074657374\x07\x1b]133;C\x07test failed: API_KEY=secret\r\n\x1b]133;D;1\x07\x1b]133;A\x07".to_vec());
    app.tick();
    let pane = app.focus_id().unwrap();
    assert_eq!(
        app.panes[&pane]
            .session
            .blocks
            .records
            .back()
            .unwrap()
            .status,
        Some(1)
    );
    app.copy_block();
    assert!(app.copied.as_ref().unwrap().contains("cargo test"));
    app.ask("why did that fail".into());
    assert!(transport.requests.lock().unwrap().is_empty());
    assert!(
        !app.smart
            .draft
            .as_ref()
            .unwrap()
            .context
            .preview()
            .contains("=secret")
    );
    let mut enter = crate::KeyIn {
        code: KeyCode::Enter,
        logical: Logical::Named(NamedKey::Enter),
        text: None,
        plain: None,
        pressed: true,
        repeat: false,
        synthetic: false,
    };
    app.key(&enter);
    enter.pressed = false;
    app.key(&enter);
    app.tick();
    let key = app.smart.pending.as_ref().unwrap().1.clone();
    assert!(transport.input.lock().unwrap().is_empty());
    assert_eq!(transport.requests.lock().unwrap().len(), 1);
    enter.pressed = true;
    app.key(&enter);
    assert_eq!(
        transport.input.lock().unwrap().as_slice(),
        &[b"cargo test\r".to_vec()]
    );
    transport.output.lock().unwrap().push_back(b"\x1b]777;openagents;command;636172676f2074657374\x07\x1b]133;C\x07test passed\r\n\x1b]133;D;0\x07\x1b]133;A\x07".to_vec());
    app.tick();
    app.tick();
    app.tick();
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].thread, requests[1].thread);
    assert!(!requests[1].new);
    assert_eq!(requests[1].context.blocks[0].status, Some(0));
    assert!(matches!(
        app.smart.book.entries[&key].phase,
        crate::proposals::Phase::Acknowledged { .. }
    ));
    assert_eq!(transport.input.lock().unwrap().len(), 1);
}

fn hex(text: &str) -> String {
    text.bytes().map(|b| format!("{b:02x}")).collect()
}

/// The hook's report of a prompt line: the first word's kind, then the buffer.
fn typed(kind: &str, line: &str) -> Vec<u8> {
    format!(
        "\x1b]777;openagents;word;{}\x07\x1b]777;openagents;buffer;{}\x07",
        hex(kind),
        hex(line)
    )
    .into_bytes()
}

#[test]
fn one_caret_routes_enter_to_the_shell_or_a_request() {
    use crate::input::{KeyCode, Logical, NamedKey};
    let transport = Arc::new(Fake {
        bridge: true,
        ..Fake::default()
    });
    let mut app = Application::new(Sessions(transport.clone()));
    app.toggle();
    app.ensure_started();
    let enter = crate::KeyIn {
        code: KeyCode::Enter,
        logical: Logical::Named(NamedKey::Enter),
        text: None,
        plain: None,
        pressed: true,
        repeat: false,
        synthetic: false,
    };
    let release = crate::KeyIn {
        pressed: false,
        ..enter.clone()
    };
    let output = |bytes: Vec<u8>| transport.output.lock().unwrap().push_back(bytes);
    let last = || transport.input.lock().unwrap().last().cloned().unwrap();
    output(b"\x1b]7;file:///test/work\x07\x1b]133;A\x07$ \x1b]133;B\x07".to_vec());
    output(typed("command", "git status"));
    app.tick();
    let pane = app.focus_id().unwrap();
    assert_eq!(app.routing(pane).unwrap().label(), "shell");
    app.key(&enter);
    app.key(&release);
    assert_eq!(last(), b"\r");

    // Prose goes to the hook's ask widget, which hands the line back as a request.
    output(typed("none", "why did that fail"));
    app.tick();
    assert_eq!(app.routing(pane).unwrap().label(), "ask");
    app.key(&enter);
    app.key(&release);
    assert_eq!(last(), crate::smart::ASK_KEY);
    output(
        format!(
            "\x1b]777;openagents;request;{}\x07",
            hex("why did that fail")
        )
        .into_bytes(),
    );
    app.tick();
    assert_eq!(app.smart.draft.as_ref().unwrap().text, "why did that fail");
    assert!(transport.requests.lock().unwrap().is_empty());

    // Up undoes the routing: the line returns to the prompt as a command.
    let up = crate::KeyIn {
        code: KeyCode::ArrowUp,
        logical: Logical::Named(NamedKey::ArrowUp),
        ..enter.clone()
    };
    app.key(&up);
    assert!(app.smart.draft.is_none());
    assert_eq!(last(), b"why did that fail");
    output(typed("none", "why did that fail"));
    app.tick();
    assert_eq!(app.routing(pane).unwrap().label(), "shell");
    app.key(&enter);
    app.key(&release);
    assert_eq!(last(), b"\r");

    // A command the shell did not find offers a request on the empty prompt.
    output(
        format!(
            "\x1b]777;openagents;command;{}\x07\x1b]133;C\x07zsh: command not found: gti\r\n\x1b]133;D;127\x07\x1b]133;A\x07$ \x1b]133;B\x07",
            hex("gti")
        )
        .into_bytes(),
    );
    output(typed("", ""));
    app.tick();
    assert!(app.notice.as_ref().unwrap().contains("not a command"));
    app.key(&enter);
    app.key(&release);
    let draft = app.smart.draft.as_ref().unwrap();
    assert_eq!(draft.text, "gti");
    assert_eq!(draft.context.blocks[0].status, Some(127));
    assert!(transport.requests.lock().unwrap().is_empty());
}

fn typing(app: &mut Application, text: &str) {
    use crate::input::{KeyCode, Logical};
    for c in text.chars() {
        app.key(&crate::KeyIn {
            code: KeyCode::KeyA,
            logical: Logical::Character(c.to_string()),
            text: Some(c.to_string()),
            plain: None,
            pressed: true,
            repeat: false,
            synthetic: false,
        });
    }
}

fn press(app: &mut Application, code: crate::input::KeyCode, named: crate::input::NamedKey) {
    use crate::input::Logical;
    let mut key = crate::KeyIn {
        code,
        logical: Logical::Named(named),
        text: None,
        plain: None,
        pressed: true,
        repeat: false,
        synthetic: false,
    };
    app.key(&key);
    key.pressed = false;
    app.key(&key);
}

#[test]
fn the_sheet_runs_commands_asks_questions_and_confirms_proposals() {
    use crate::input::{KeyCode, NamedKey};
    use crate::paper::KEYS;
    let transport = Arc::new(Fake {
        bridge: true,
        ..Fake::default()
    });
    let mut app = Application::new(Sessions(transport.clone()));
    app.paper.on = true;
    app.toggle();
    app.ensure_started();
    let sheet = |app: &mut Application| app.paper_sheet(120, 40, "12:00:00", "0.50");
    let output = |bytes: &[u8]| transport.output.lock().unwrap().push_back(bytes.to_vec());
    let table = hex("p:/nonexistent\na:ll\nf:");
    output(format!("\x1b]7;file:///test/work\x07\x1b]777;openagents;table;{table}\x07\x1b]133;A\x07$ \x1b]133;B\x07").as_bytes());
    app.tick();

    // The status area, the transcript frame, and the key strip are always there.
    let idle = sheet(&mut app);
    assert_eq!(idle.rows.len(), 40);
    for row in 0..40 {
        let text = idle.row_text(row);
        assert_eq!(text.len(), 120, "row {row}: {text:?}");
        assert!(text.chars().all(|c| (' '..='~').contains(&c)), "row {row}");
    }
    assert!(idle.row_text(1).starts_with("| DIR /test/work  GIT"));
    assert!(idle.row_text(1).contains("EXIT -"));
    assert!(
        idle.row_text(2)
            .starts_with("| REQUEST idle  QUEUE 0  PENDING 0")
    );
    assert!(idle.row_text(3).starts_with("| CONTEXT"));
    assert!(
        idle.row_text(39)
            .starts_with(KEYS.split("  PGUP").next().unwrap())
    );
    assert!(idle.row_text(37).starts_with("| SHELL > "));
    assert_eq!(app.paper.grid, (31, 114));

    // A command line goes to the shell as typed; `cargo` resolves nowhere
    // here, but its argument shape keeps it a command.
    typing(&mut app, "ll -la");
    assert_eq!(app.paper_route(), Some(crate::route::Route::Shell));
    press(&mut app, KeyCode::Enter, NamedKey::Enter);
    assert_eq!(transport.input.lock().unwrap().last().unwrap(), b"ll -la\r");
    output(format!("\x1b]777;openagents;command;{}\x07\x1b]133;C\x07\x1b[31mtest addition ... FAILED\x1b[0m \u{2717}\r\nthread panicked: \u{201c}2 + 2\u{201d}\r\n\x1b]133;D;101\x07\x1b]133;A\x07$ \x1b]133;B\x07", hex("cargo test")).as_bytes());
    app.tick();
    let failed = sheet(&mut app).text();
    assert!(failed.contains("$ cargo test"));
    assert!(failed.contains("test addition ... FAILED x"));
    assert!(failed.contains("thread panicked: \"2 + 2\""));
    assert!(failed.contains("[exit 101"));
    assert!(failed.contains("EXIT 101"));
    assert!(failed.contains("CONTEXT block 1 `cargo test` exit 101 (F2 detaches)"));

    // A question goes to OpenAgents without a preview or a second key.
    typing(&mut app, "why did that fail");
    assert_eq!(
        sheet(&mut app).row_text(37),
        format!("| ASK   > why did that fail{} |", " ".repeat(116 - 25))
    );
    press(&mut app, KeyCode::Enter, NamedKey::Enter);
    app.tick();
    {
        let requests = transport.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        let message = requests[0].message().unwrap();
        assert!(message.starts_with("why did that fail\n\nAttached from my terminal:"));
        for leaked in ["JSON", "schema", "pending for my Enter", "Do not execute"] {
            assert!(!message.contains(leaked), "{leaked}");
        }
    }
    let asked = sheet(&mut app).text();
    assert!(asked.contains("ASK: why did that fail"));
    assert!(
        asked.contains("OPENAGENTS: It failed because 2 + 2 is not 5 - see the test (src/lib.rs).")
    );
    for leaked in ["**", "`2", "{\"v\"", "commands", "\u{2014}"] {
        assert!(!asked.contains(leaked), "{leaked}");
    }
    assert!(asked.contains("PROPOSED: cargo test   [ENTER] confirm  [ESC] reject"));
    assert!(asked.contains("| CONFIRM? cargo test   ENTER confirms and runs it, ESC rejects it"));
    assert!(asked.contains("PENDING 1"));
    assert!(asked.contains("DOOR local"));

    // REJECT leaves the shell alone.
    let sent = transport.input.lock().unwrap().len();
    press(&mut app, KeyCode::Escape, NamedKey::Escape);
    assert!(app.smart.pending.is_none());
    assert_eq!(transport.input.lock().unwrap().len(), sent);
    assert!(sheet(&mut app).text().contains("REJECTED: cargo test"));

    // Asking again offers it again; CONFIRM runs it in the shell.
    typing(&mut app, "how do I fix it");
    press(&mut app, KeyCode::Enter, NamedKey::Enter);
    app.tick();
    assert!(app.smart.pending.is_some());
    press(&mut app, KeyCode::Enter, NamedKey::Enter);
    assert_eq!(
        transport.input.lock().unwrap().last().unwrap(),
        b"cargo test\r"
    );
    assert!(sheet(&mut app).text().contains("CONFIRMED: cargo test"));
}

#[test]
fn a_second_question_queues_instead_of_blocking() {
    let transport = Arc::new(Fake {
        bridge: true,
        ..Fake::default()
    });
    let mut app = Application::new(Sessions(transport.clone()));
    app.paper.on = true;
    app.toggle();
    app.ensure_started();
    transport
        .output
        .lock()
        .unwrap()
        .push_back(b"\x1b]7;file:///test/work\x07\x1b]133;A\x07$ \x1b]133;B\x07".to_vec());
    app.tick();
    app.paper_ask("why is the sky blue");
    app.paper_ask("and the sea");
    assert_eq!(app.paper.queue.len(), 1);
    let text = app.paper_sheet(120, 40, "12:00:00", "0.50").text();
    assert!(text.contains("QUEUE 1"));
    assert!(!text.contains("Finish or dismiss"));
}
