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
    /// What the shared client answers to reading a thread.
    thread: Mutex<Option<Vec<u8>>>,
    /// Every thread read asked for.
    reads: Mutex<Vec<String>>,
    /// What the task owner answers to viewing a run: stdout and stderr.
    run: Mutex<Option<(Vec<u8>, Vec<u8>)>>,
    /// Every run read asked for.
    run_reads: Mutex<Vec<String>>,
    /// The task status a command's receipt names; none leaves the outcome
    /// unknown.
    receipt: Mutex<Option<String>>,
    /// Every task command sent: its verb and exact bytes.
    commands: Mutex<Vec<(String, Vec<u8>)>>,
    /// What the task owner answers to reading a retained file, by path.
    artifacts: Mutex<std::collections::BTreeMap<String, (Vec<u8>, Vec<u8>)>>,
    /// Every retained file read asked for.
    artifact_reads: Mutex<Vec<String>>,
    /// The host's background rules, as `background list` answers them.
    rules: Mutex<Option<serde_json::Value>>,
    /// Every pause or resume sent: its verb and rule.
    rule_commands: Mutex<Vec<(String, String)>>,
    /// What `plugin test studies` answers, and every directory listed.
    studies: Mutex<Option<serde_json::Value>>,
    study_lists: Mutex<Vec<String>>,
    /// What `plugin test show` answers, by results directory, and every
    /// directory read.
    study: Mutex<std::collections::BTreeMap<String, serde_json::Value>>,
    study_reads: Mutex<Vec<String>>,
    /// What `plugin inspect` answers.
    components: Mutex<Option<serde_json::Value>>,
    /// Every `plugin use` sent: plugin, version, digest, request, and
    /// workspace. The fake owner runs a request once and follows it after.
    uses: Mutex<Vec<[String; 5]>>,
    /// Knowledge entries `kb show` answers, by ID, and what `kb search`
    /// lists (possibly older versions); the studio's goals.
    entries: Mutex<std::collections::BTreeMap<String, serde_json::Value>>,
    hits: Mutex<Vec<serde_json::Value>>,
    goals: Mutex<Option<serde_json::Value>>,
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
    fn read_thread(&self, thread: &str) -> mpsc::Receiver<crate::thread::Read> {
        self.reads.lock().unwrap().push(thread.to_owned());
        let (sender, receiver) = mpsc::channel();
        let answer = self.thread.lock().unwrap().clone().unwrap_or_default();
        sender.send(crate::thread::decode(&answer, thread)).unwrap();
        receiver
    }
    fn read_run(&self, task: &str) -> mpsc::Receiver<crate::run::Read> {
        self.run_reads.lock().unwrap().push(task.to_owned());
        let (sender, receiver) = mpsc::channel();
        let (stdout, stderr) = self.run.lock().unwrap().clone().unwrap_or_default();
        sender
            .send(crate::run::decode(&stdout, &stderr, task))
            .unwrap();
        receiver
    }
    fn read_artifact(
        &self,
        _: &str,
        path: &str,
        digest: &str,
    ) -> mpsc::Receiver<crate::files::Read> {
        self.artifact_reads.lock().unwrap().push(path.to_owned());
        let (stdout, stderr) = self
            .artifacts
            .lock()
            .unwrap()
            .get(path)
            .cloned()
            .unwrap_or_default();
        let (sender, receiver) = mpsc::channel();
        sender
            .send(crate::files::decode(&stdout, &stderr, path, digest))
            .unwrap();
        receiver
    }
    fn read_rules(&self) -> mpsc::Receiver<crate::rules::Read> {
        let (sender, receiver) = mpsc::channel();
        let answer = match self.rules.lock().unwrap().clone() {
            Some(rules) => crate::rules::decode(rules.to_string().as_bytes(), b""),
            None => crate::rules::decode(b"", br#"{"error":"the host's rules store is locked"}"#),
        };
        sender.send(answer).unwrap();
        receiver
    }
    fn read_studies(&self, root: &str) -> mpsc::Receiver<crate::gym::ListRead> {
        self.study_lists.lock().unwrap().push(root.to_owned());
        let (sender, receiver) = mpsc::channel();
        let answer = match self.studies.lock().unwrap().clone() {
            Some(studies) => crate::gym::decode_list(studies.to_string().as_bytes(), b""),
            None => crate::gym::decode_list(b"", br#"{"error":"openagents is not installed"}"#),
        };
        sender.send(answer).unwrap();
        receiver
    }
    fn read_components(&self, _root: &str) -> mpsc::Receiver<crate::gym::ComponentsRead> {
        let (sender, receiver) = mpsc::channel();
        let answer = match self.components.lock().unwrap().clone() {
            Some(held) => crate::gym::decode_components(held.to_string().as_bytes(), b""),
            None => crate::gym::decode_components(b"", br#"{"error":"no plugins store"}"#),
        };
        sender.send(answer).unwrap();
        receiver
    }
    fn plugin_use(
        &self,
        id: &str,
        version: &str,
        digest: &str,
        request: &str,
        workspace: &str,
    ) -> mpsc::Receiver<crate::gym::UseRead> {
        let mut uses = self.uses.lock().unwrap();
        let again = uses.iter().any(|terms| terms[3] == request);
        uses.push([id, version, digest, request, workspace].map(str::to_owned));
        let mut components = self.components.lock().unwrap();
        if !again && let Some(held) = components.as_mut() {
            held["plugins"][0]["runs"] = serde_json::json!([{
                "request": "use-8d1b6c07a651cae1", "thread": "terminal", "state": "completed",
                "version": version, "this_release": true, "check": "verified",
                "cost_microusd": 0, "wall_ms": 40,
                "outputs": [{"digest": format!("sha256:{}", "c".repeat(64)),
                             "path": "/home/route-artifacts/c", "state": "retained"}],
            }]);
        }
        let answer = serde_json::json!({
            "dispatched": if again { "followed" } else { "ran" },
            "text": "Action items (5)\n\n1. Ana: send the revised budget to finance, by Friday \
                     (notes/standup.md line 6)",
        });
        let (sender, receiver) = mpsc::channel();
        sender
            .send(crate::gym::decode_use(answer.to_string().as_bytes(), b""))
            .unwrap();
        receiver
    }
    fn search_knowledge(&self, _query: &str) -> mpsc::Receiver<crate::knowledge::HitsRead> {
        let hits: Vec<serde_json::Value> = self
            .hits
            .lock()
            .unwrap()
            .iter()
            .map(|entry| serde_json::json!({"id": entry["id"], "score": 1.0, "entry": entry}))
            .collect();
        let answer = serde_json::json!({"query": "budget", "hits": hits}).to_string();
        let (sender, receiver) = mpsc::channel();
        sender
            .send(crate::knowledge::decode_hits(answer.as_bytes(), b""))
            .unwrap();
        receiver
    }
    fn read_entry(&self, id: &str) -> mpsc::Receiver<crate::knowledge::ShownRead> {
        let answer = match self.entries.lock().unwrap().get(id) {
            Some(entry) => serde_json::json!({"entry": entry, "pending": null}).to_string(),
            None => r#"{"error":"no entry"}"#.to_owned(),
        };
        let (sender, receiver) = mpsc::channel();
        sender
            .send(crate::knowledge::decode_shown(answer.as_bytes(), b"", id))
            .unwrap();
        receiver
    }
    fn read_goals(&self) -> mpsc::Receiver<crate::knowledge::GoalsRead> {
        let answer = self.goals.lock().unwrap().clone().map_or_else(
            || r#"{"error":"no studio"}"#.to_owned(),
            |goals| goals.to_string(),
        );
        let (sender, receiver) = mpsc::channel();
        sender
            .send(crate::knowledge::decode_goals(answer.as_bytes(), b""))
            .unwrap();
        receiver
    }
    fn read_study(&self, dir: &str) -> mpsc::Receiver<crate::gym::Read> {
        self.study_reads.lock().unwrap().push(dir.to_owned());
        let (sender, receiver) = mpsc::channel();
        let answer = match self.study.lock().unwrap().get(dir) {
            Some(study) => crate::gym::decode(study.to_string().as_bytes(), b"", dir),
            None => crate::gym::decode(
                br#"{"error":"report.json: No such file or directory"}"#,
                b"",
                dir,
            ),
        };
        sender.send(answer).unwrap();
        receiver
    }
    fn rule_command(&self, verb: &str, id: &str) -> mpsc::Receiver<Result<(), String>> {
        self.rule_commands
            .lock()
            .unwrap()
            .push((verb.to_owned(), id.to_owned()));
        // The host saves the rule; the next list shows it.
        let mut rules = self.rules.lock().unwrap();
        let answer = match rules.as_mut().and_then(|rules| {
            rules["rules"]
                .as_array_mut()?
                .iter_mut()
                .find(|rule| rule["id"] == id)
        }) {
            Some(rule) if id != "locked" => {
                rule["enabled"] = serde_json::json!(verb == "resume");
                let saved = serde_json::json!({"rule": {"id": id}}).to_string();
                crate::rules::decode_change(saved.as_bytes(), b"", id)
            }
            _ => crate::rules::decode_change(
                br#"{"error":"the rule is managed by its plugin"}"#,
                b"",
                id,
            ),
        };
        let (sender, receiver) = mpsc::channel();
        sender.send(answer).unwrap();
        receiver
    }
    fn task_command(&self, verb: &str, bytes: &[u8]) -> mpsc::Receiver<crate::run::Sent> {
        self.commands
            .lock()
            .unwrap()
            .push((verb.to_owned(), bytes.to_vec()));
        let command: serde_json::Value = serde_json::from_slice(bytes).unwrap();
        let (sender, receiver) = mpsc::channel();
        let answer = match self.receipt.lock().unwrap().clone() {
            Some(status) => {
                let receipt = serde_json::json!({
                    "command_id": command["command_id"],
                    "task_id": command["task_id"],
                    "status": status,
                });
                crate::run::decode_receipt(
                    receipt.to_string().as_bytes(),
                    b"",
                    command["command_id"].as_str().unwrap(),
                )
            }
            None => crate::run::Sent::Unknown("the task owner did not answer in time".into()),
        };
        sender.send(answer).unwrap();
        receiver
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

#[test]
fn an_agent_drives_its_own_pane_until_a_key_takes_it_back() {
    use crate::input::{KeyCode, Logical};
    let transport = Arc::new(Fake::default());
    let mut app = Application::new(Sessions(transport.clone()));
    let pane = app.open_typist("ada").unwrap();
    // The pane shows, but the world keeps the keyboard.
    assert!(app.open && !app.focused && !app.paper.on);
    assert_eq!(app.panes[&pane].typist.as_deref(), Some("ada"));
    app.send_to(pane, b"cargo test -p atif\r");
    assert_eq!(
        transport.input.lock().unwrap().as_slice(),
        &[b"cargo test -p atif\r".to_vec()]
    );
    // The command's block finishes with its status.
    transport.output.lock().unwrap().push_back(b"\x1b]133;A\x07$ \x1b]133;B\x07cargo test -p atif\r\n\x1b]777;openagents;command;636172676f2074657374202d702061746966\x07\x1b]133;C\x07test result: ok. 31 passed\r\n\x1b]133;D;0\x07\x1b]133;A\x07".to_vec());
    app.tick();
    let block = app.panes[&pane].session.blocks.records.back().unwrap();
    assert_eq!(block.status, Some(0));
    assert!(block.output.contains("31 passed"));
    // A modifier alone takes nothing back; a real key does, and still types.
    app.focused = true;
    let mut key = crate::KeyIn {
        code: KeyCode::ShiftLeft,
        logical: Logical::Character("".into()),
        text: None,
        plain: None,
        pressed: true,
        repeat: false,
        synthetic: false,
    };
    assert!(app.key(&key));
    assert_eq!(app.panes[&pane].typist.as_deref(), Some("ada"));
    key.code = KeyCode::KeyQ;
    key.logical = Logical::Character("q".into());
    key.text = Some("q".into());
    assert!(app.key(&key));
    assert!(app.panes[&pane].typist.is_none());
    assert!(app.panes[&pane].taken_back);
    assert!(app.show_pane(pane));
    assert!(!app.show_pane(pane + 100));
}

#[test]
fn a_multiline_clipboard_paste_waits_for_enter_at_a_plain_prompt() {
    use crate::input::{KeyCode, NamedKey};
    let transport = Arc::new(Fake::default());
    let mut app = Application::new(Sessions(transport.clone()));
    app.toggle();
    app.ensure_started();
    app.paper.on = false;
    let sent = || transport.input.lock().unwrap().concat();
    // One line goes at once, and a paste past the bound is refused whole.
    app.paste_clipboard("echo one");
    assert_eq!(sent(), b"echo one");
    transport.input.lock().unwrap().clear();
    app.paste_clipboard(&"x\n".repeat(crate::paste::MAX_PASTE));
    assert!(sent().is_empty() && app.paste_hold.is_none());
    // Two lines wait, showing their count; Escape sends nothing.
    app.paste_clipboard("rm -rf build\r\nls é\r\n");
    assert!(sent().is_empty());
    assert_eq!(
        app.paste_prompt().as_deref(),
        Some("Paste 2 lines? Enter sends them; Escape cancels.")
    );
    press(&mut app, KeyCode::Escape, NamedKey::Escape);
    assert!(sent().is_empty() && app.paste_hold.is_none());
    // Other keys, a repeated or synthetic Enter, and an agent's press do
    // not send it.
    app.paste_clipboard("rm -rf build\r\nls é\r\n");
    typing(&mut app, "y");
    let mut enter = crate::KeyIn {
        code: KeyCode::Enter,
        logical: crate::input::Logical::Named(NamedKey::Enter),
        text: None,
        plain: None,
        pressed: true,
        repeat: true,
        synthetic: false,
    };
    app.key(&enter);
    enter.repeat = false;
    enter.synthetic = true;
    app.key(&enter);
    assert!(
        app.apply(&crate::control::Request::Press {
            name: "enter".into()
        })
        .is_err()
    );
    assert!(sent().is_empty());
    // Enter sends the exact text once; the next Enter is only a key.
    press(&mut app, KeyCode::Enter, NamedKey::Enter);
    assert_eq!(sent(), "rm -rf build\rls é\r".as_bytes());
    assert!(app.paste_hold.is_none() && app.paste_prompt().is_none());
    press(&mut app, KeyCode::Enter, NamedKey::Enter);
    assert_eq!(sent(), "rm -rf build\rls é\r\r".as_bytes());
}

#[test]
fn negotiated_bracketed_paste_and_full_screen_programs_take_a_paste_at_once() {
    let transport = Arc::new(Fake::default());
    let mut app = Application::new(Sessions(transport.clone()));
    app.toggle();
    app.ensure_started();
    app.paper.on = false;
    transport
        .output
        .lock()
        .unwrap()
        .push_back(b"\x1b[?2004h".to_vec());
    app.tick();
    app.paste_clipboard("one\ntwo\n");
    assert!(app.paste_hold.is_none());
    assert_eq!(
        transport.input.lock().unwrap().concat(),
        b"\x1b[200~one\rtwo\r\x1b[201~"
    );
    transport.input.lock().unwrap().clear();
    transport
        .output
        .lock()
        .unwrap()
        .push_back(b"\x1b[?2004l\x1b[?1049h".to_vec());
    app.tick();
    app.paste_clipboard("one\ntwo");
    assert!(app.paste_hold.is_none());
    assert_eq!(transport.input.lock().unwrap().concat(), b"one\rtwo");
    // The control socket's text is an agent's input, never held.
    transport.input.lock().unwrap().clear();
    transport
        .output
        .lock()
        .unwrap()
        .push_back(b"\x1b[?1049l".to_vec());
    app.tick();
    app.apply(&crate::control::Request::Send {
        text: "one\ntwo".into(),
    })
    .unwrap();
    assert_eq!(transport.input.lock().unwrap().concat(), b"one\rtwo");
}

#[cfg(unix)]
#[test]
fn a_missing_command_offers_corrections_that_run_nothing() {
    use std::os::unix::fs::PermissionsExt;
    let bin = tempfile::tempdir().unwrap();
    let git = bin.path().join("git");
    std::fs::write(&git, "").unwrap();
    std::fs::set_permissions(&git, std::fs::Permissions::from_mode(0o755)).unwrap();
    let transport = Arc::new(Fake::default());
    let mut app = Application::new(Sessions(transport.clone()));
    app.toggle();
    app.ensure_started();
    app.paper.on = false;
    let table = format!("p:{}\na:\nf:", bin.path().display());
    transport.output.lock().unwrap().push_back(
        format!(
            "\x1b]777;openagents;table;{}\x07\x1b]133;A\x07$ \x1b]133;B\x07gti status\r\n\x1b]777;openagents;command;{}\x07\x1b]133;C\x07gti: command not found\r\n\x1b]133;D;127\x07\x1b]133;A\x07$ ",
            hex(&table),
            hex("gti status")
        )
        .into_bytes(),
    );
    app.tick();
    let notice = app.notice.clone().unwrap_or_default();
    assert!(
        notice.contains("types `git status` without running it"),
        "{notice}"
    );
    // Typed at the prompt, without Enter; again, it replaces itself.
    app.type_correction();
    assert_eq!(transport.input.lock().unwrap().concat(), b"git status");
    transport.input.lock().unwrap().clear();
    app.type_correction();
    let mut again = vec![0x7f; "git status".len()];
    again.extend_from_slice(b"git status");
    assert_eq!(transport.input.lock().unwrap().concat(), again);
    // The sheet puts it on the input line, and sends nothing.
    transport.input.lock().unwrap().clear();
    app.paper.on = true;
    press(
        &mut app,
        // The sheet reads the logical key; the code has no F7.
        crate::input::KeyCode::F1,
        crate::input::NamedKey::F7,
    );
    assert_eq!(app.paper.input, "git status");
    assert!(transport.input.lock().unwrap().is_empty());
}

#[test]
fn block_search_selects_matches_and_sends_nothing_to_the_shell() {
    use crate::input::{KeyCode, NamedKey};
    let transport = Arc::new(Fake::default());
    let mut app = Application::new(Sessions(transport.clone()));
    app.toggle();
    app.ensure_started();
    app.paper.on = false;
    let run = |cwd: &str, command: &str, output: &str, status: i32| {
        format!(
            "\x1b]7;file://localhost{cwd}\x07\x1b]133;A\x07$ \x1b]133;B\x07{command}\r\n\x1b]777;openagents;command;{}\x07\x1b]133;C\x07{output}\r\n\x1b]133;D;{status}\x07",
            hex(command)
        )
    };
    let output = [
        run("/work/app", "cargo test", "1 failed", 101),
        run("/work/lib", "cargo test", "ok", 0),
        run("/work/app", "ls", "Cargo.toml", 0),
        "\x1b]133;A\x07$ ".to_owned(),
    ]
    .concat();
    transport
        .output
        .lock()
        .unwrap()
        .push_back(output.into_bytes());
    app.tick();
    let pane = app.focus_id().unwrap();
    assert_eq!(app.panes[&pane].session.blocks.records.len(), 3);
    app.open_find();
    typing(&mut app, "cargo test dir:app");
    assert_eq!(app.smart.selected, Some((pane, 1)));
    assert!(
        app.find_prompt().unwrap().contains("1 of 1"),
        "{:?}",
        app.find_prompt()
    );
    // Clearing the directory filter: the newest match first, then older.
    for _ in 0.."dir:app".len() {
        press(&mut app, KeyCode::Backspace, NamedKey::Backspace);
    }
    assert_eq!(app.smart.selected, Some((pane, 2)));
    press(&mut app, KeyCode::ArrowDown, NamedKey::ArrowDown);
    assert_eq!(app.smart.selected, Some((pane, 1)));
    press(&mut app, KeyCode::ArrowUp, NamedKey::ArrowUp);
    assert_eq!(app.smart.selected, Some((pane, 2)));
    // Enter moves to the next match; it never reaches the shell.
    press(&mut app, KeyCode::Enter, NamedKey::Enter);
    assert_eq!(app.smart.selected, Some((pane, 1)));
    assert!(transport.input.lock().unwrap().is_empty());
    press(&mut app, KeyCode::Escape, NamedKey::Escape);
    assert!(app.find.is_none());
    assert_eq!(app.smart.selected, Some((pane, 1)));
    // The selected block's own actions still apply.
    app.copy_block();
    assert_eq!(app.copied.as_deref(), Some("$ cargo test\n1 failed"));
}

#[test]
fn a_full_pane_searches_within_a_frame() {
    let mut blocks = crate::blocks::Blocks::default();
    let output = "ordinary build output line\n".repeat(crate::blocks::MAX_OUTPUT / 27);
    for id in 1..=crate::blocks::MAX_BLOCKS as u64 {
        blocks.records.push_back(crate::blocks::Block {
            id,
            command: format!("cargo build -p crate{id}"),
            cwd: Some("/work".into()),
            start: crate::blocks::Position { line: id, col: 0 },
            end: Some(crate::blocks::Position { line: id, col: 1 }),
            status: Some(0),
            started_ms: 0,
            elapsed_ms: None,
            output: output.clone(),
            truncated: false,
            collapsed: false,
            alternate: false,
        });
    }
    let query = crate::search::Query::parse("needle status:fail");
    let started = std::time::Instant::now();
    let found = crate::search::search(&blocks, &query);
    let elapsed = started.elapsed();
    assert!(found.ids.is_empty());
    eprintln!(
        "searched {} blocks of {} bytes in {elapsed:?}",
        blocks.records.len(),
        output.len()
    );
    // The worst case: every block full and every word checked.
    let query = crate::search::Query::parse("needle");
    let started = std::time::Instant::now();
    let _ = crate::search::search(&blocks, &query);
    let worst = started.elapsed();
    eprintln!("worst case {worst:?}");
    assert!(worst < std::time::Duration::from_millis(250), "{worst:?}");
}

#[test]
fn a_mount_answers_workbench_intents_for_its_own_panes_only() {
    use crate::control::Request;
    use workbench::{Action, Host, Intent, Kind, ResourceRef};
    let transport = Arc::new(Fake::default());
    let mut app = Application::new(Sessions(transport.clone()));
    app.toggle();
    app.ensure_started();
    let first = app.focus_id().unwrap();
    app.split(Axis::Columns, &Program::Shell);
    let reference = app.resource(first).unwrap();
    reference.check().unwrap();
    let status = app.status();
    assert_eq!(
        status["panes"][0]["resource"],
        serde_json::to_value(&reference).unwrap()
    );
    assert_eq!(status["directory"]["capabilities"][0]["kind"], "terminal");
    let resolve = |app: &mut Application, n: u8, target: ResourceRef| {
        let intent = Intent::new(format!("{n:02x}").repeat(32), target, Action::Open);
        app.apply(&Request::Resolve { intent }).unwrap()["state"]["kind"].clone()
    };

    // The same contract every surface sends: open focuses the pane.
    assert_ne!(app.focus_id(), Some(first));
    assert_eq!(resolve(&mut app, 1, reference.clone()), "opened");
    assert_eq!(app.focus_id(), Some(first));

    // A pane from an earlier run is lost, and nothing starts in its place.
    let opened = transport.opened.load(Ordering::SeqCst);
    let mut earlier = reference.clone();
    earlier.generation = Some("0".repeat(64));
    assert_eq!(resolve(&mut app, 2, earlier), "lost");
    let mut gone = reference.clone();
    gone.id = "999".into();
    assert_eq!(resolve(&mut app, 3, gone), "closed");
    assert_eq!(transport.opened.load(Ordering::SeqCst), opened);
    assert_eq!(app.panes(), 2);

    // Kinds a mount does not own are unsupported, and another owner's
    // references are refused rather than answered.
    let local = Host::Local {
        instance: app.instance.clone(),
    };
    let thread = ResourceRef::new(Kind::Thread, local, "t1");
    assert_eq!(resolve(&mut app, 4, thread), "unsupported");
    let mut other = reference;
    other.host = Host::Local {
        instance: "1".repeat(64),
    };
    let intent = Intent::new("05".repeat(32), other, Action::Open);
    let refused = app.apply(&Request::Resolve { intent }).unwrap_err();
    assert!(refused.starts_with("identity_mismatch"), "{refused}");
    app.shutdown();
}

/// A fixture thread owner: ready, archived (read-only), and revoked
/// threads, by turn count.
struct FixtureThreads;

impl workbench::pane::PaneAdapter for FixtureThreads {
    fn kind(&self) -> workbench::pane::PaneKind {
        workbench::pane::PaneKind::Thread
    }

    fn describe(&self, subject: &workbench::pane::Subject) -> workbench::pane::Description {
        use workbench::Revision;
        use workbench::pane::{Description, PaneState};
        let (turns, actions) = match subject.id() {
            "ready" => (5, vec!["reply".to_owned()]),
            "archived" => (2, Vec::new()),
            "revoked" => {
                return Description {
                    state: PaneState::Revoked,
                    title: "A thread".into(),
                    detail: String::new(),
                    actions: vec!["reply".into()],
                };
            }
            _ => return Description::only(PaneState::Missing, "No such thread"),
        };
        if let Some(asked) = subject.revision()
            && *asked != Revision::Counter(turns)
        {
            return Description::only(
                PaneState::Stale {
                    current: Some(Revision::Counter(turns)),
                },
                "A thread",
            );
        }
        Description {
            state: PaneState::Ready,
            title: format!("Thread {}", subject.id()),
            detail: format!("{turns} turns"),
            actions,
        }
    }
}

#[test]
fn every_mount_resolves_product_panes_the_same_way() {
    use crate::control::Request;
    use workbench::pane::{PaneKind, PaneState, Subject, View};
    use workbench::{Host, Kind, ResourceRef, Revision};
    let host = Host::Paired {
        key: "ef".repeat(32),
    };
    let thread = |id: &str, revision: Option<u64>| {
        let mut resource = ResourceRef::new(Kind::Thread, host.clone(), id);
        resource.revision = revision.map(Revision::Counter);
        Subject::Resource { resource }
    };
    let fixtures = vec![
        (PaneKind::Thread, thread("ready", Some(5))),
        (PaneKind::Thread, thread("archived", None)),
        (PaneKind::Thread, thread("revoked", None)),
        (PaneKind::Thread, thread("ready", Some(3))),
        (PaneKind::Thread, thread("gone", None)),
        (
            PaneKind::Run,
            Subject::Resource {
                resource: ResourceRef::new(Kind::Run, host.clone(), "task-9"),
            },
        ),
        (
            PaneKind::Knowledge,
            Subject::Record {
                host: host.clone(),
                id: "kb-1".into(),
                revision: None,
            },
        ),
    ];
    // The Grid's overlay and the standalone window each mount an
    // application with the same adapters.
    let mount = || {
        let mut app = Application::new(Sessions(Arc::new(Fake::default())));
        let products = std::mem::take(&mut app.products.panes);
        app.products.panes = products.adapter(Box::new(FixtureThreads));
        app
    };
    let (mut grid, mut window) = (mount(), mount());
    let mut states = Vec::new();
    for (pane, subject) in &fixtures {
        let request = Request::Pane {
            pane: *pane,
            subject: subject.clone(),
        };
        let shown = grid.apply(&request).unwrap();
        assert_eq!(shown, window.apply(&request).unwrap());
        let descriptor: workbench::pane::PaneDescriptor = serde_json::from_value(shown).unwrap();
        descriptor.check().unwrap();
        assert_eq!(&descriptor.subject, subject);
        states.push((descriptor.state, descriptor.actions));
    }
    assert_eq!(states[0], (PaneState::Ready, vec!["reply".to_owned()]));
    // Read-only and revoked panes offer no mutation.
    assert_eq!(states[1], (PaneState::Ready, Vec::new()));
    assert_eq!(states[2], (PaneState::Revoked, Vec::new()));
    // A stale reference names the current turn count and creates nothing.
    assert_eq!(
        states[3],
        (
            PaneState::Stale {
                current: Some(Revision::Counter(5))
            },
            Vec::new()
        )
    );
    assert_eq!(states[4], (PaneState::Missing, Vec::new()));
    // Without an adapter: the declared TTY view, or the label alone.
    assert_eq!(
        states[5].0,
        PaneState::Fallback {
            view: View::Tty {
                command: ["coder", "task", "show", "task-9"]
                    .map(str::to_owned)
                    .to_vec()
            }
        }
    );
    assert_eq!(states[6].0, PaneState::Fallback { view: View::Label });
    // Opening the same thread again refreshes its pane rather than adding
    // one, and the status lists the open panes with the focus.
    assert_eq!(grid.products.open.len(), 6);
    let status = grid.status();
    assert_eq!(status["products"]["panes"].as_array().unwrap().len(), 6);
    assert_eq!(status["products"], window.status()["products"]);
    // A subject that does not fit its pane is refused.
    let wrong = Request::Pane {
        pane: PaneKind::Knowledge,
        subject: thread("ready", None),
    };
    assert!(grid.apply(&wrong).is_err());
    assert!(grid.products.close(0));
    assert_eq!(grid.products.open.len(), 5);
}

#[test]
fn the_thread_page_reads_the_sheets_conversation_and_never_sends_on_open() {
    use crate::input::{KeyCode, NamedKey};
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
    let sheet = |app: &mut Application| app.paper_sheet(120, 40, "12:00:00", "0.50");
    let f4 = |app: &mut Application| press(app, KeyCode::Unidentified, NamedKey::F4);

    // Before any question there is no conversation, and nothing is read.
    f4(&mut app);
    assert!(!app.paper.thread.open);
    assert!(sheet(&mut app).text().contains("LAST No conversation yet"));
    assert!(transport.reads.lock().unwrap().is_empty());

    // A question attaches the sheet to its thread.
    app.paper_ask("why did that fail");
    app.tick();
    // The fixture answer proposes a command; REJECT it.
    press(&mut app, KeyCode::Escape, NamedKey::Escape);
    assert!(app.smart.pending.is_none());
    let thread = transport.requests.lock().unwrap()[0].thread.clone();
    let answer = |busy: bool| {
        serde_json::json!({
            "thread": thread,
            "title": "Why the test failed",
            "backend": "local",
            "busy": busy,
            "failure": null,
            "coder": null,
            "turns": [
                {"role": "user", "text": "why did that fail", "request": "r1"},
                {"role": "assistant", "text": "**It failed** because `2 + 2` is not 5 \u{2014} see [the test](src/lib.rs).\n{\"v\":1,\"commands\":[]}"},
            ],
            "coder_turns": [],
        })
        .to_string()
        .into_bytes()
    };
    *transport.thread.lock().unwrap() = Some(answer(true));

    // F4 shows that thread natively, plainly, with its state and the TTY
    // command for the same thread ID; opening it sends nothing.
    f4(&mut app);
    assert!(app.paper.thread.open);
    app.tick();
    let page = sheet(&mut app);
    let text = page.text();
    for row in 0..40 {
        let row_text = page.row_text(row);
        assert_eq!(row_text.len(), 120, "row {row}: {row_text:?}");
        assert!(
            row_text.chars().all(|c| (' '..='~').contains(&c)),
            "row {row}"
        );
    }
    assert!(text.contains("THREAD Why the test failed  2 turns  [reply arriving]"));
    assert!(text.contains(&format!("TTY: openagents chat read --thread {thread}")));
    assert!(text.contains("YOU: why did that fail"));
    assert!(
        text.contains("OPENAGENTS: It failed because 2 + 2 is not 5 - see the test (src/lib.rs).")
    );
    assert!(text.contains("[a reply is arriving]"));
    for leaked in ["**", "{\"v\"", "\u{2014}"] {
        assert!(!text.contains(leaked), "{leaked}");
    }
    assert!(page.row_text(37).starts_with("| REPLY > "));
    assert_eq!(transport.requests.lock().unwrap().len(), 1);
    assert_eq!(transport.reads.lock().unwrap().as_slice(), [thread.clone()]);

    // REJECT returns to the transcript; reopening reads the same thread
    // again and still sends nothing.
    *transport.thread.lock().unwrap() = Some(answer(false));
    press(&mut app, KeyCode::Escape, NamedKey::Escape);
    assert!(!app.paper.thread.open);
    assert!(sheet(&mut app).text().contains("ASK: why did that fail"));
    f4(&mut app);
    app.tick();
    assert!(sheet(&mut app).text().contains("2 turns  [current]"));
    assert_eq!(transport.reads.lock().unwrap().len(), 2);
    assert_eq!(transport.requests.lock().unwrap().len(), 1);

    // On the page, ENTER sends the line once, to the same thread.
    typing(&mut app, "and the sea");
    press(&mut app, KeyCode::Enter, NamedKey::Enter);
    app.tick();
    {
        let requests = transport.requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[1].thread, thread);
        assert!(!requests[1].new);
        assert_eq!(requests[1].text, "and the sea");
    }

    // A thread the client does not keep is missing, and one it cannot
    // read is unavailable; neither creates or sends anything.
    *transport.thread.lock().unwrap() = Some(br#"{"error":"Thread not found."}"#.to_vec());
    f4(&mut app);
    f4(&mut app);
    app.tick();
    let text = sheet(&mut app).text();
    assert!(text.contains("[missing]"));
    assert!(text.contains("This thread is not on this computer. Nothing was created"));
    *transport.thread.lock().unwrap() = Some(b"not json".to_vec());
    f4(&mut app);
    f4(&mut app);
    app.tick();
    let text = sheet(&mut app).text();
    assert!(text.contains("[unavailable]"));
    assert!(
        text.contains("The thread can't be read now: the chat client's answer was not readable.")
    );
    assert_eq!(transport.requests.lock().unwrap().len(), 2);
}

#[test]
fn a_thread_read_answers_only_for_the_thread_asked() {
    use crate::thread::{Unread, decode};
    let ok = br#"{"thread":"t1","title":"T","busy":false,"failure":null,"turns":[{"role":"user","text":"hi"}],"coder_turns":[]}"#;
    let thread = decode(ok, "t1").unwrap();
    assert_eq!(thread.turns.len(), 1);
    assert!(
        matches!(decode(ok, "t2"), Err(Unread::Unavailable(why)) if why.contains("another thread"))
    );
    assert_eq!(
        decode(br#"{"error":"openagents chat: Thread not found."}"#, "t1"),
        Err(Unread::Missing)
    );
    assert!(matches!(decode(b"", "t1"), Err(Unread::Unavailable(_))));
    assert!(matches!(
        decode(&vec![b' '; crate::thread::READ_MAX + 1], "t1"),
        Err(Unread::Unavailable(why)) if why.contains("too large")
    ));
}

/// A run as `openagents --json task view` answers it.
fn run_view(task: &str, adapter: &str, status: &str, evidence: serde_json::Value) -> Vec<u8> {
    let ended = !matches!(status, "running" | "queued" | "cancel_requested");
    serde_json::json!({
        "schema": "openagents.coder.task-view.v1",
        "task": {
            "task_id": task,
            "revision": 3,
            "intent": {
                "title": "Greet the studio",
                "prompt": "Greet with Hello, studio",
                "workspace": {"path": "/test/work", "source_revision": null},
                "configuration": {"adapter": adapter, "model": "gpt-6-luna"},
            },
            "intent_digest": "d",
            "status": status,
            "execution": if ended { "finished" } else { "running" },
            "checks": "not_run",
            "cancellation_reason": null,
            "run": if ended {
                serde_json::json!({"epoch": 1, "result": {"ending": "completed", "exit_code": 0,
                    "stop_requested": false, "group_clear": true, "elapsed_ms": 4200,
                    "trace_digest": "t", "candidate_snapshot": null, "artifact_file": null,
                    "artifact_digest": null, "output_incomplete": false, "cost_status": "unknown"}})
            } else {
                serde_json::json!({"epoch": 1, "result": null})
            },
        },
        "evidence": evidence,
        "artifacts": null,
        "artifact_error": null,
        "artifact_faults": [],
        "verification": "not_run",
        "integration": "not_attempted",
        "cost_usd": null,
        "cost_status": "unknown",
    })
    .to_string()
    .into_bytes()
}

#[test]
fn the_run_page_shows_recorded_children_and_steers_and_cancels_the_original_task() {
    use crate::input::{KeyCode, NamedKey};
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
    let sheet = |app: &mut Application| app.paper_sheet(120, 40, "12:00:00", "0.50");
    let key = |app: &mut Application, named: NamedKey| press(app, KeyCode::Unidentified, named);

    // F9 before any question names no run, and reads nothing.
    key(&mut app, NamedKey::F9);
    assert!(!app.paper.run.open);
    assert!(transport.run_reads.lock().unwrap().is_empty());

    app.paper_ask("greet the studio");
    app.tick();
    press(&mut app, KeyCode::Escape, NamedKey::Escape);
    let thread = transport.requests.lock().unwrap()[0].thread.clone();
    let linked = |host: &str| {
        serde_json::json!({
            "thread": thread, "title": "Greeting", "busy": false, "failure": null,
            "turns": [{"role": "user", "text": "greet the studio"}],
            "coder": {"host": host, "task": "task-1"}, "coder_turns": [],
        })
        .to_string()
        .into_bytes()
    };
    *transport.thread.lock().unwrap() = Some(linked("local"));
    let spawning = serde_json::json!({
        "state": "unsealed", "total_steps": 3, "more_available": false, "faults": [],
        "steps": [
            {"at": 1, "source": "user", "message": "Greet with Hello, studio"},
            {"at": 2, "source": "agent", "message": "Splitting the work.",
             "call": {"id": "c1", "name": "spawn_agent", "arguments": {"task": "docs"},
                      "output": "", "outcome": "completed", "milliseconds": 5,
                      "extra": {"subagent_trajectory_ref": [{"session_id": "child-7"}]}}},
            {"at": 3, "source": "agent", "message": "**Wrote** `greeting.txt`.",
             "call": {"id": "c2", "name": "shell", "arguments": {"command": "ls"},
                      "output": "greeting.txt", "outcome": "completed", "milliseconds": 5}},
        ],
    });
    *transport.run.lock().unwrap() = Some((
        run_view(
            "task-1",
            "microcoder-repository",
            "running",
            spawning.clone(),
        ),
        Vec::new(),
    ));

    // F9 reads the conversation for its run first, then shows the run.
    key(&mut app, NamedKey::F9);
    assert!(app.paper.thread.open && !app.paper.run.open);
    app.tick();
    assert!(
        sheet(&mut app)
            .text()
            .contains("RUN task-1 on local: F9 shows it")
    );
    key(&mut app, NamedKey::F9);
    assert!(app.paper.run.open);
    app.tick();
    let page = sheet(&mut app);
    let text = page.text();
    for row in 0..40 {
        let row_text = page.row_text(row);
        assert_eq!(row_text.len(), 120, "row {row}: {row_text:?}");
        assert!(
            row_text.chars().all(|c| (' '..='~').contains(&c)),
            "row {row}"
        );
    }
    assert!(text.contains("RUN Greet the studio  (task-1, revision 3)"));
    assert!(text.contains(
        "STATUS running  EXECUTION running  CHECKS not run  ENGINE microcoder-repository gpt-6-luna"
    ));
    assert!(text.contains("CHILD at step 2: session child-7"));
    assert!(text.contains("TOOL spawn_agent (completed)"));
    assert!(text.contains("3 AGENT: Wrote greeting.txt."));
    assert!(text.contains("CONTROLS ENTER steers with the line, F7 cancels the run"));
    assert!(page.row_text(37).starts_with("| STEER > "));
    assert!(page.row_text(39).starts_with(crate::paper::RUN_KEYS));
    assert_eq!(transport.run_reads.lock().unwrap().as_slice(), ["task-1"]);

    // F7 arms the cancel; REJECT drops it and sends nothing.
    key(&mut app, NamedKey::F7);
    assert!(
        sheet(&mut app)
            .row_text(37)
            .starts_with("| CONFIRM? cancel run task-1")
    );
    press(&mut app, KeyCode::Escape, NamedKey::Escape);
    assert!(app.paper.run.open && app.paper.run.command.is_none());
    assert!(transport.commands.lock().unwrap().is_empty());

    // CONFIRM sends it once, to the original task at the revision read.
    *transport.receipt.lock().unwrap() = Some("cancel_requested".into());
    key(&mut app, NamedKey::F7);
    press(&mut app, KeyCode::Enter, NamedKey::Enter);
    {
        let commands = transport.commands.lock().unwrap();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].0, "cancel");
        let command: serde_json::Value = serde_json::from_slice(&commands[0].1).unwrap();
        assert_eq!(command["schema"], crate::run::COMMAND_SCHEMA);
        assert_eq!(command["task_id"], "task-1");
        assert_eq!(command["expected_revision"], 3);
        assert_eq!(command["action"]["type"], "cancel");
    }
    *transport.run.lock().unwrap() = Some((
        run_view(
            "task-1",
            "microcoder-repository",
            "cancel_requested",
            spawning.clone(),
        ),
        Vec::new(),
    ));
    app.tick();
    app.tick();
    let text = sheet(&mut app).text();
    assert!(text.contains("accepted; the task is cancel requested"));
    assert!(text.contains("CANCEL requested; the run has not acknowledged it yet"));

    // A steer whose outcome is unknown is sent again as the same command.
    *transport.receipt.lock().unwrap() = None;
    typing(&mut app, "use the plain greeting");
    press(&mut app, KeyCode::Enter, NamedKey::Enter);
    assert!(
        sheet(&mut app)
            .row_text(37)
            .starts_with("| CONFIRM? steer run task-1")
    );
    press(&mut app, KeyCode::Enter, NamedKey::Enter);
    app.tick();
    assert!(sheet(&mut app).text().contains("outcome unknown"));
    *transport.receipt.lock().unwrap() = Some("running".into());
    press(&mut app, KeyCode::Enter, NamedKey::Enter);
    {
        let commands = transport.commands.lock().unwrap();
        assert_eq!(commands.len(), 3);
        assert_eq!(commands[1].0, "correct");
        assert_eq!(commands[1], commands[2]);
        let command: serde_json::Value = serde_json::from_slice(&commands[1].1).unwrap();
        assert_eq!(command["action"]["prompt"], "use the plain greeting");
    }

    // REJECT returns to the conversation; reopening only reads.
    app.tick();
    press(&mut app, KeyCode::Escape, NamedKey::Escape);
    assert!(!app.paper.run.open && app.paper.thread.open);
    key(&mut app, NamedKey::F9);
    app.tick();
    assert_eq!(transport.commands.lock().unwrap().len(), 3);
    assert_eq!(transport.requests.lock().unwrap().len(), 1);

    // Another adapter's ended run with a missing trace: the linkage is
    // unknown, and no control is offered.
    *transport.run.lock().unwrap() = Some((
        run_view(
            "task-1",
            "opencode",
            "finished",
            serde_json::json!({"state": "missing", "total_steps": 0, "more_available": false, "steps": []}),
        ),
        Vec::new(),
    ));
    key(&mut app, NamedKey::F9);
    key(&mut app, NamedKey::F9);
    app.tick();
    let text = sheet(&mut app).text();
    assert!(text.contains("ENGINE opencode gpt-6-luna"));
    assert!(text.contains("CHILDREN unknown: the trace is missing"));
    assert!(text.contains("ENDED completed  exit 0  4.2 s"));
    assert!(text.contains("CONTROLS none"));
    key(&mut app, NamedKey::F7);
    assert!(
        app.paper
            .run
            .command
            .as_ref()
            .is_none_or(|c| c.verb == "correct")
    );
    assert!(
        sheet(&mut app)
            .text()
            .contains("LAST This run offers no controls here.")
    );
    assert_eq!(transport.commands.lock().unwrap().len(), 3);

    // A run the task store does not keep is missing.
    *transport.run.lock().unwrap() = Some((
        Vec::new(),
        br#"{"error":{"code":"not_found","message":"task not found"}}"#.to_vec(),
    ));
    key(&mut app, NamedKey::F9);
    key(&mut app, NamedKey::F9);
    app.tick();
    assert!(sheet(&mut app).text().contains("RUN task-1  [missing]"));

    // A run on another host is neither read nor controlled here.
    *transport.thread.lock().unwrap() = Some(linked("studio-mac"));
    let reads = transport.run_reads.lock().unwrap().len();
    key(&mut app, NamedKey::F9);
    key(&mut app, NamedKey::F4);
    key(&mut app, NamedKey::F4);
    app.tick();
    key(&mut app, NamedKey::F9);
    app.tick();
    let text = sheet(&mut app).text();
    assert!(text.contains("RUN task-1 on host studio-mac"));
    assert!(text.contains("CONTROLS none here"));
    assert_eq!(transport.run_reads.lock().unwrap().len(), reads);
}

#[test]
fn a_run_read_answers_only_for_the_run_asked() {
    use crate::run::{Sent, Unread, decode, decode_receipt};
    let evidence = serde_json::json!({"state": "sealed", "total_steps": 0, "steps": []});
    let view = run_view("task-1", "opencode", "finished", evidence);
    assert!(decode(&view, b"", "task-1").is_ok());
    assert!(
        matches!(decode(&view, b"", "task-2"), Err(Unread::Unavailable(why)) if why.contains("another run"))
    );
    assert!(matches!(
        decode(b"", b"", "task-1"),
        Err(Unread::Unavailable(_))
    ));
    assert_eq!(
        decode(
            b"",
            br#"{"error":{"code":"store_busy","message":"busy"}}"#,
            "task-1"
        ),
        Err(Unread::Unavailable("busy (store_busy)".into()))
    );
    assert_eq!(
        decode_receipt(
            b"",
            br#"{"error":{"code":"revision_mismatch","message":"stale"}}"#,
            "c"
        ),
        Sent::Refused("stale (revision_mismatch)".into())
    );
    assert_eq!(
        decode_receipt(br#"{"command_id":"other","status":"cancelled"}"#, b"", "c"),
        Sent::Unknown("the task owner's answer was not readable".into())
    );
}

#[test]
fn the_files_page_shows_a_runs_retained_bytes_only_when_their_digest_holds() {
    use crate::files::digest;
    use crate::input::{KeyCode, NamedKey};
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
    let sheet = |app: &mut Application| app.paper_sheet(120, 40, "12:00:00", "0.50");
    let key = |app: &mut Application, named: NamedKey| press(app, KeyCode::Unidentified, named);
    let arrow = |app: &mut Application, code: KeyCode| press(app, code, NamedKey::Unidentified);

    app.paper_ask("greet the studio");
    app.tick();
    press(&mut app, KeyCode::Escape, NamedKey::Escape);
    let thread = transport.requests.lock().unwrap()[0].thread.clone();
    *transport.thread.lock().unwrap() = Some(
        serde_json::json!({
            "thread": thread, "title": "Greeting", "busy": false, "turns": [],
            "coder": {"host": "local", "task": "task-1"},
        })
        .to_string()
        .into_bytes(),
    );
    let greeting = b"Hello, studio\n\tand welcome\n".to_vec();
    let logo = vec![0x89, 0x50, 0xff, 0xfe];
    let entry = |path: &str, state: &str, bytes: Option<&[u8]>| {
        serde_json::json!({
            "path": path, "state": state, "link_target": null,
            "digest": bytes.map(digest), "bytes": bytes.map(<[u8]>::len),
        })
    };
    let manifest = serde_json::json!({
        "schema": "openagents.coder.task-artifacts.v1",
        "source_snapshot": format!("sha256:{}", "a".repeat(64)),
        "candidate_snapshot": format!("sha256:{}", "b".repeat(64)),
        "complete": false,
        "omitted_changes": 0,
        "changes": [
            {"change": "created", "path": "greeting.txt"},
            {"change": "modified", "path": "README.md"},
            {"change": "renamed", "from": "old.txt", "to": "big.txt", "altered": true},
            {"change": "removed", "path": "gone.txt"},
            {"change": "created", "path": "logo.png"},
            {"change": "created", "path": "../escape"},
            {"change": "created", "path": "lost.txt"},
        ],
        "entries": [
            entry("greeting.txt", "retained", Some(&greeting)),
            entry("README.md", "retained", Some(b"recorded")),
            entry("big.txt", "unavailable_or_over_limit", None),
            entry("gone.txt", "removed", None),
            entry("logo.png", "retained", Some(&logo)),
            entry("../escape", "retained", Some(b"x")),
            entry("lost.txt", "retained", Some(b"lost")),
        ],
    });
    let mut view: serde_json::Value = serde_json::from_slice(&run_view(
        "task-1",
        "opencode",
        "finished",
        serde_json::json!({"state": "sealed", "total_steps": 0, "steps": []}),
    ))
    .unwrap();
    view["artifacts"] = manifest;
    *transport.run.lock().unwrap() = Some((view.to_string().into_bytes(), Vec::new()));
    let answer = |path: &str, bytes: &[u8]| {
        (
            serde_json::json!({"path": path, "digest": digest(bytes), "bytes": bytes})
                .to_string()
                .into_bytes(),
            Vec::new(),
        )
    };
    let refusal = |code: &str, message: &str| {
        (
            Vec::new(),
            serde_json::json!({"error": {"code": code, "message": message}})
                .to_string()
                .into_bytes(),
        )
    };
    {
        let mut artifacts = transport.artifacts.lock().unwrap();
        artifacts.insert("greeting.txt".into(), answer("greeting.txt", &greeting));
        // The store's bytes moved on from what the run recorded.
        artifacts.insert("README.md".into(), answer("README.md", b"edited later"));
        artifacts.insert("logo.png".into(), answer("logo.png", &logo));
        artifacts.insert("../escape".into(), refusal("unsafe_path", "unsafe path"));
        artifacts.insert("lost.txt".into(), refusal("not_found", "not found"));
    }

    key(&mut app, NamedKey::F9);
    app.tick();
    key(&mut app, NamedKey::F9);
    app.tick();
    assert!(
        sheet(&mut app)
            .text()
            .contains("ARTIFACTS 7 retained, 7 changes")
    );

    // F2 lists the run's changes from its own manifest.
    key(&mut app, NamedKey::F2);
    assert!(app.paper.files.open);
    let list = sheet(&mut app);
    let text = list.text();
    assert!(text.contains("FILES of run task-1  7 changes  [incomplete]"));
    assert!(text.contains("SOURCE sha256:aaaaaaaaaaaa  RESULT sha256:bbbbbbbbbbbb"));
    assert!(text.contains("> created  greeting.txt  retained 27 bytes"));
    assert!(text.contains("  renamed  big.txt (from old.txt)  unavailable or over limit"));
    assert!(
        list.row_text(37)
            .starts_with("| FILES > UP DOWN pick a file")
    );
    assert!(list.row_text(39).starts_with(crate::paper::FILE_KEYS));
    for row in 0..40 {
        let row_text = list.row_text(row);
        assert_eq!(row_text.len(), 120, "row {row}: {row_text:?}");
        assert!(
            row_text.chars().all(|c| (' '..='~').contains(&c)),
            "row {row}"
        );
    }

    // ENTER shows the picked file, line-numbered and literal, once its
    // digest holds.
    press(&mut app, KeyCode::Enter, NamedKey::Enter);
    app.tick();
    let text = sheet(&mut app).text();
    assert!(text.contains("FILE greeting.txt  [digest checked]"));
    assert!(text.contains("CHANGE created  sha256:"));
    assert!(text.contains("1 | Hello, studio"));
    assert!(text.contains("2 |     and welcome"));
    press(&mut app, KeyCode::Escape, NamedKey::Escape);
    assert!(app.paper.files.open && app.paper.files.viewing.is_none());

    // Bytes that moved on from the recorded digest are not shown.
    arrow(&mut app, KeyCode::ArrowDown);
    press(&mut app, KeyCode::Enter, NamedKey::Enter);
    app.tick();
    let text = sheet(&mut app).text();
    assert!(text.contains("FILE README.md  [changed]"));
    assert!(!text.contains("edited later"));
    press(&mut app, KeyCode::Escape, NamedKey::Escape);

    // An over-limit or removed file is described without a read.
    let reads = transport.artifact_reads.lock().unwrap().len();
    arrow(&mut app, KeyCode::ArrowDown);
    press(&mut app, KeyCode::Enter, NamedKey::Enter);
    app.tick();
    let text = sheet(&mut app).text();
    assert!(text.contains("FILE big.txt  [unavailable or over limit]"));
    assert!(text.contains("CHANGE renamed from old.txt"));
    assert!(text.contains("Over the retained bound"));
    press(&mut app, KeyCode::Escape, NamedKey::Escape);
    arrow(&mut app, KeyCode::ArrowDown);
    press(&mut app, KeyCode::Enter, NamedKey::Enter);
    assert!(sheet(&mut app).text().contains("The run removed this file"));
    assert_eq!(transport.artifact_reads.lock().unwrap().len(), reads);
    press(&mut app, KeyCode::Escape, NamedKey::Escape);

    // Binary bytes are described, a forbidden path is refused, and a file
    // the store no longer keeps is missing.
    for (expected, absent) in [
        (
            "Binary content, 4 bytes; this page shows text only.",
            "\u{fffd}",
        ),
        ("The task owner refused the read: unsafe path.", "1 | x"),
        ("The task store no longer keeps these bytes.", "1 | lost"),
    ] {
        arrow(&mut app, KeyCode::ArrowDown);
        press(&mut app, KeyCode::Enter, NamedKey::Enter);
        app.tick();
        let text = sheet(&mut app).text();
        assert!(text.contains(expected), "{expected}");
        assert!(!text.contains(absent), "{absent}");
        press(&mut app, KeyCode::Escape, NamedKey::Escape);
    }

    // ESC steps back to the run; reading files sent nothing.
    press(&mut app, KeyCode::Escape, NamedKey::Escape);
    assert!(!app.paper.files.open && app.paper.run.open);
    assert!(transport.commands.lock().unwrap().is_empty());
    assert_eq!(transport.requests.lock().unwrap().len(), 1);
}

#[test]
fn the_rules_page_shows_host_rules_and_pauses_them_only_after_confirm() {
    use crate::input::{KeyCode, NamedKey};
    let transport = Arc::new(Fake {
        bridge: true,
        ..Fake::default()
    });
    let mut app = Application::new(Sessions(transport.clone()));
    app.paper.on = true;
    app.toggle();
    app.ensure_started();
    let sheet = |app: &mut Application| app.paper_sheet(120, 40, "12:00:00", "0.50");
    let key = |app: &mut Application, named: NamedKey| press(app, KeyCode::Unidentified, named);
    let arrow = |app: &mut Application, code: KeyCode| press(app, code, NamedKey::Unidentified);
    transport
        .output
        .lock()
        .unwrap()
        .push_back(b"\x1b]7;file:///test/work\x07\x1b]133;A\x07$ \x1b]133;B\x07".to_vec());
    app.tick();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // A host that can't answer shows the rules as unavailable, with no
    // control.
    key(&mut app, NamedKey::F11);
    app.tick();
    let text = sheet(&mut app).text();
    assert!(text.contains("RULES on this computer  [unavailable]"));
    assert!(text.contains("the host's rules store is locked"));
    press(&mut app, KeyCode::Enter, NamedKey::Enter);
    assert!(app.paper.rules.change.is_none());
    key(&mut app, NamedKey::F11);
    assert!(!app.paper.rules.open);

    *transport.rules.lock().unwrap() = Some(serde_json::json!({
        "host": "The host runs the rules (process 4242).",
        "rules": [
            {"id": "disk", "name": "Disk cleanup", "version": 3,
             "digest": format!("sha256:{}", "c".repeat(64)), "enabled": true,
             "state": {"last_check": now - 120, "next_check": now + 630,
                       "last_run": now - 7200, "last_run_id": "run-9",
                       "last_result": "Freed 2.1 GB.", "last_escalation": now - 86_400 * 3}},
            {"id": "locked", "name": "Nightly fetch", "version": 1, "digest": "sha256:d",
             "enabled": false, "plugin": "npub1x:git-fetch", "state": {}},
            {"id": "torn", "name": "torn", "version": 0, "digest": "", "enabled": false,
             "error": "the rule file is not JSON", "state": {}},
        ],
    }));
    key(&mut app, NamedKey::F11);
    app.tick();
    let page = sheet(&mut app);
    let text = page.text();
    for row in 0..40 {
        let row_text = page.row_text(row);
        assert_eq!(row_text.len(), 120, "row {row}: {row_text:?}");
        assert!(
            row_text.chars().all(|c| (' '..='~').contains(&c)),
            "row {row}"
        );
    }
    assert!(text.contains("RULES on this computer  3 rules, 1 on  [current]"));
    assert!(text.contains("HOST The host runs the rules (process 4242)."));
    assert!(text.contains("> disk (Disk cleanup)  on  version 3"));
    assert!(text.contains("last run 2 h ago: Freed 2.1 GB."));
    assert!(
        text.contains("checked 2 min ago, next check in 10 min, started a Coder run 3 days ago")
    );
    assert!(text.contains("edit: openagents background edit disk --message TEXT"));
    assert!(text.contains("  locked (Nightly fetch)  paused  version 1"));
    assert!(text.contains("from plugin npub1x:git-fetch"));
    assert!(text.contains("can't be read: the rule file is not JSON"));
    assert!(
        page.row_text(37)
            .starts_with("| RULES > UP DOWN pick a rule")
    );
    assert!(page.row_text(39).starts_with(crate::paper::RULE_KEYS));

    // ENTER arms the pause; REJECT drops it and sends nothing.
    press(&mut app, KeyCode::Enter, NamedKey::Enter);
    assert!(
        sheet(&mut app)
            .row_text(37)
            .starts_with("| RULES > pause rule disk   ENTER confirms")
    );
    press(&mut app, KeyCode::Escape, NamedKey::Escape);
    assert!(app.paper.rules.open && app.paper.rules.change.is_none());
    assert!(transport.rule_commands.lock().unwrap().is_empty());

    // CONFIRM pauses it through the host, and the page reads the rule back.
    press(&mut app, KeyCode::Enter, NamedKey::Enter);
    press(&mut app, KeyCode::Enter, NamedKey::Enter);
    app.tick();
    app.tick();
    let text = sheet(&mut app).text();
    assert!(text.contains("PAUSE disk: done"));
    assert!(text.contains("> disk (Disk cleanup)  paused  version 3"));
    assert_eq!(
        transport.rule_commands.lock().unwrap().as_slice(),
        [("pause".to_owned(), "disk".to_owned())]
    );

    // Closing and reopening the page leaves the rule as the host keeps it.
    key(&mut app, NamedKey::F11);
    key(&mut app, NamedKey::F11);
    app.tick();
    let text = sheet(&mut app).text();
    assert!(text.contains("disk (Disk cleanup)  paused"));
    assert!(text.contains("last run 2 h ago: Freed 2.1 GB."));
    assert_eq!(transport.rule_commands.lock().unwrap().len(), 1);

    // A refused change says why; a rule that can't be read can't be changed.
    arrow(&mut app, KeyCode::ArrowDown);
    press(&mut app, KeyCode::Enter, NamedKey::Enter);
    assert!(sheet(&mut app).row_text(37).contains("resume rule locked"));
    press(&mut app, KeyCode::Enter, NamedKey::Enter);
    app.tick();
    assert!(
        sheet(&mut app)
            .text()
            .contains("RESUME locked: refused: the rule is managed by its plugin")
    );
    arrow(&mut app, KeyCode::ArrowDown);
    press(&mut app, KeyCode::Enter, NamedKey::Enter);
    assert!(
        sheet(&mut app)
            .text()
            .contains("LAST A rule that can't be read can't be changed here.")
    );
    assert_eq!(transport.rule_commands.lock().unwrap().len(), 2);

    // ESC returns to the transcript.
    press(&mut app, KeyCode::Escape, NamedKey::Escape);
    assert!(!app.paper.rules.open);
}

fn study_fixture(dir: &str, agreement: &str, shown: &str, reported: &str) -> serde_json::Value {
    let recomputed = if agreement == "unverifiable" {
        serde_json::Value::Null
    } else {
        serde_json::json!("pass")
    };
    serde_json::json!({
        "v": "openagents.ext-eval-study.v1",
        "dir": dir,
        "report": format!("sha256:{}", "a".repeat(64)),
        "suite": "5be6:repo-map-tests/eval-suite",
        "suite_ref": {},
        "gate": "ext-eval-v2",
        "gate_digest": format!("sha256:{}", "b".repeat(64)),
        "subject": {
            "definition": {"id": "5be6:repo-map/repo-map",
                           "artifact": {"digest": format!("sha256:{}", "c".repeat(64))}},
            "lock": {"digest": format!("sha256:{}", "d".repeat(64))},
        },
        "baseline": {
            "definition": {"id": "5be6:coder/coder",
                           "artifact": {"digest": format!("sha256:{}", "e".repeat(64))}},
            "lock": {"digest": format!("sha256:{}", "f".repeat(64))},
        },
        "evaluator": "0b1c",
        "started_at": 1_790_000_000u64,
        "ended_at": 1_790_000_900u64,
        "defaults": null,
        "development": 2,
        "held_out": 0,
        "reported": reported,
        "recomputed": recomputed,
        "agreement": agreement,
        "shown": shown,
        "coverage": {
            "subject": {"planned": 2, "attempted": 4, "completed": 3, "refused": 0,
                        "failed": 0, "cancelled": 0, "unknown": 1, "excluded": 0},
            "baseline": {"planned": 2, "attempted": 4, "completed": 4, "refused": 0,
                         "failed": 0, "cancelled": 0, "unknown": 0, "excluded": 0},
        },
        "totals": {
            "subject": {"cost_usd": null, "cost_unknown": 1, "seconds": 41.5,
                        "seconds_unknown": 0, "cases_scored": 2, "cases_passed": 2,
                        "cases_unknown": 0},
            "baseline": {"cost_usd": 0.0123, "cost_unknown": 0, "seconds": 30.0,
                         "seconds_unknown": 0, "cases_scored": 2, "cases_passed": 1,
                         "cases_unknown": 0},
        },
        "cases": [
            {"id": "callers", "kind": "should-fire", "compared": true, "subject_only": false,
             "subject": {"planned": 2, "scored": 1, "runs_passed": 1, "score": 1.0, "passed": true},
             "baseline": {"planned": 2, "scored": 2, "runs_passed": 0, "score": 0.0, "passed": false},
             "change": 1.0},
        ],
        "attempts": [
            {"case": "callers", "arm": "subject", "attempt": 1, "outcome": "completed",
             "reason": null, "score": 1.0, "passed": true, "cost_usd": 0.004, "seconds": 20.5,
             "grades": "retained", "trajectory": "retained"},
            {"case": "callers", "arm": "subject", "attempt": 2, "outcome": "unknown",
             "reason": null, "score": null, "passed": null, "cost_usd": null, "seconds": 21.0,
             "grades": "retained", "trajectory": "missing"},
        ],
        "partial": "1 run did not finish",
        "limitations": ["Trajectories stay private; the report carries their digests."],
        "published": null,
        "relay": null,
        "problems": if agreement == "disputes" {
            serde_json::json!(["the retained attempts give fail, and the report says pass"])
        } else {
            serde_json::json!([])
        },
        "missing": if agreement == "unverifiable" {
            serde_json::json!(["the ext-eval-v2 gate is not on this computer"])
        } else {
            serde_json::json!([])
        },
    })
}

#[test]
fn the_gym_page_recomputes_a_retained_study_and_runs_nothing() {
    use crate::input::{KeyCode, NamedKey};
    let transport = Arc::new(Fake {
        bridge: true,
        ..Fake::default()
    });
    let mut app = Application::new(Sessions(transport.clone()));
    app.paper.on = true;
    app.toggle();
    app.ensure_started();
    let sheet = |app: &mut Application| app.paper_sheet(120, 40, "12:00:00", "0.50");
    let key = |app: &mut Application, named: NamedKey| press(app, KeyCode::Unidentified, named);
    let arrow = |app: &mut Application, code: KeyCode| press(app, code, NamedKey::Unidentified);
    transport
        .output
        .lock()
        .unwrap()
        .push_back(b"\x1b]7;file:///test/work\x07\x1b]133;A\x07$ \x1b]133;B\x07".to_vec());
    app.tick();

    // Without the helper the listing is unavailable and nothing is read.
    key(&mut app, NamedKey::F12);
    app.tick();
    let text = sheet(&mut app).text();
    assert!(
        text.contains("GYM studies under /test/work  [unavailable]"),
        "{text}"
    );
    assert!(text.contains("openagents is not installed"));
    press(&mut app, KeyCode::Enter, NamedKey::Enter);
    assert!(transport.study_reads.lock().unwrap().is_empty());
    key(&mut app, NamedKey::F12);
    assert!(!app.paper.gym.open);

    let agreed = "/test/work/evals/results/2026-10-05T12-00-00Z";
    let disputed = "/test/work/results/2026-10-04T09-00-00Z";
    *transport.studies.lock().unwrap() = Some(serde_json::json!({
        "v": "openagents.ext-eval-studies.v1",
        "root": "/test/work",
        "studies": [
            {"dir": agreed, "subject": "5be6:repo-map/repo-map", "reported": "pass",
             "ended_at": 1_790_000_900u64},
            {"dir": disputed, "subject": "5be6:repo-map/repo-map", "reported": "pass",
             "ended_at": 1_789_000_000u64},
            {"dir": "/test/work/results/gone", "subject": null, "reported": "inconclusive",
             "ended_at": null},
        ],
    }));
    transport.study.lock().unwrap().insert(
        agreed.into(),
        study_fixture(agreed, "agrees", "Better", "pass"),
    );
    transport.study.lock().unwrap().insert(
        disputed.into(),
        study_fixture(disputed, "disputes", "Disputed", "pass"),
    );
    key(&mut app, NamedKey::F12);
    app.tick();
    let page = sheet(&mut app);
    let text = page.text();
    for row in 0..40 {
        let row_text = page.row_text(row);
        assert_eq!(row_text.len(), 120, "row {row}: {row_text:?}");
        assert!(
            row_text.chars().all(|c| (' '..='~').contains(&c)),
            "row {row}"
        );
    }
    assert_eq!(
        transport.study_lists.lock().unwrap().as_slice(),
        ["/test/work".to_owned(), "/test/work".to_owned()]
    );
    assert!(text.contains("GYM studies under /test/work  3 results  [current]"));
    assert!(text.contains("The verdicts listed are the reports' own, unchecked."));
    assert!(text.contains("> 5be6:repo-map/repo-map  reported pass  ended "));
    assert!(
        page.row_text(37)
            .starts_with("| GYM > UP DOWN pick a result")
    );
    assert!(page.row_text(39).starts_with(crate::paper::GYM_KEYS));

    // ENTER recomputes the picked study: both arms, the exact release,
    // unknown costs kept unknown, and every attempt.
    press(&mut app, KeyCode::Enter, NamedKey::Enter);
    app.tick();
    let text = sheet(&mut app).text();
    assert!(
        text.contains(
            "STUDY Better  reported pass, recomputed pass  [the retained attempts agree]"
        ),
        "{text}"
    );
    assert!(text.contains("SUBJECT 5be6:repo-map/repo-map  package cccccccccccc"));
    assert!(text.contains("run lock dddddddddddd"));
    assert!(text.contains("BASELINE 5be6:coder/coder"));
    assert!(text.contains("GATE ext-eval-v2 bbbbbbbbbbbb"));
    assert!(text.contains("not published"));
    assert!(text.contains("PARTIAL 1 run did not finish"));
    assert!(text.contains("SUBJECT 4 attempted of 2 cases: 3 completed"));
    assert!(text.contains("1 unknown"));
    assert!(text.contains("cost unknown (1 of 4 attempts unknown)  time 41.5 s"));
    assert!(text.contains("cost $0.0123  time 30.0 s"));
    assert!(text.contains("callers subject #2  unknown  -  score -  cost unknown"));
    assert!(text.contains("transcript missing"));
    assert!(text.contains("Reading ran and published nothing."));

    // A disputed study never shows the report's Better.
    press(&mut app, KeyCode::Escape, NamedKey::Escape);
    assert!(app.paper.gym.open && app.paper.gym.viewing.is_none());
    arrow(&mut app, KeyCode::ArrowDown);
    press(&mut app, KeyCode::Enter, NamedKey::Enter);
    app.tick();
    let text = sheet(&mut app).text();
    assert!(text.contains(
        "STUDY Disputed  reported pass, recomputed pass  [the retained attempts dispute"
    ));
    assert!(text.contains("DISPUTES the retained attempts give fail, and the report says pass"));
    assert!(!text.contains("STUDY Better"));

    // One that can't be read says so.
    press(&mut app, KeyCode::Escape, NamedKey::Escape);
    arrow(&mut app, KeyCode::ArrowDown);
    press(&mut app, KeyCode::Enter, NamedKey::Enter);
    app.tick();
    let text = sheet(&mut app).text();
    assert!(text.contains("[unavailable]"));
    assert!(text.contains("report.json: No such file or directory"));

    // Reading never asked the host or a door for anything else.
    assert_eq!(transport.study_reads.lock().unwrap().len(), 3);
    assert!(transport.commands.lock().unwrap().is_empty());
    assert!(transport.rule_commands.lock().unwrap().is_empty());
    assert!(transport.requests.lock().unwrap().is_empty());

    press(&mut app, KeyCode::Escape, NamedKey::Escape);
    press(&mut app, KeyCode::Escape, NamedKey::Escape);
    assert!(!app.paper.gym.open);
}

#[test]
fn the_gym_page_lists_plugins_by_exact_release_and_opens_their_evidence() {
    use crate::input::{KeyCode, NamedKey};
    let transport = Arc::new(Fake {
        bridge: true,
        ..Fake::default()
    });
    let mut app = Application::new(Sessions(transport.clone()));
    app.paper.on = true;
    app.toggle();
    app.ensure_started();
    let sheet = |app: &mut Application| app.paper_sheet(120, 40, "12:00:00", "0.50");
    let key = |app: &mut Application, named: NamedKey| press(app, KeyCode::Unidentified, named);
    let arrow = |app: &mut Application, code: KeyCode| press(app, code, NamedKey::Unidentified);
    transport
        .output
        .lock()
        .unwrap()
        .push_back(b"\x1b]7;file:///test/work\x07\x1b]133;A\x07$ \x1b]133;B\x07".to_vec());
    app.tick();
    let exact = "/test/work/results/2026-10-05T12-00-00Z";
    *transport.studies.lock().unwrap() = Some(serde_json::json!({
        "root": "/test/work", "studies": [],
    }));
    *transport.components.lock().unwrap() = Some(serde_json::json!({
        "v": "openagents.plugin-inspect.v1",
        "plugins": [
            {"id": "0000:notes", "name": "Meeting notes", "version": "0.2.0",
             "digest": format!("sha256:{}", "a".repeat(64)), "enabled": true,
             "workflow": true, "background": [], "revocation": "not_checked",
             "evidence": [
                {"dir": exact, "reported": "pass", "ended_at": 1_790_000_900u64, "exact": true},
                {"dir": "/test/work/results/old", "reported": "fail",
                 "ended_at": 1_780_000_000u64, "exact": false},
             ],
             "commands": {"test": "openagents plugin test run /x/notes/0.2.0",
                          "turn": "openagents plugin disable 0000:notes",
                          "use": "openagents plugin use 0000:notes --version 0.2.0 --digest sha256:aaaa --request TEXT"}},
            {"id": "0000:cleanup", "name": "Disk cleanup", "version": "1.0.0",
             "digest": format!("sha256:{}", "b".repeat(64)), "enabled": false,
             "workflow": false, "background": ["disk"], "revocation": "not_checked",
             "evidence": [], "commands": {}},
        ],
    }));
    transport.study.lock().unwrap().insert(
        exact.into(),
        study_fixture(exact, "agrees", "Better", "pass"),
    );

    key(&mut app, NamedKey::F12);
    app.tick();
    key(&mut app, NamedKey::F2);
    app.tick();
    let page = sheet(&mut app);
    let text = page.text();
    for row in 0..40 {
        let row_text = page.row_text(row);
        assert_eq!(row_text.len(), 120, "row {row}: {row_text:?}");
    }
    assert!(
        text.contains("PLUGINS on this computer  2 installed  [current]"),
        "{text}"
    );
    assert!(text.contains("> 0000:notes (Meeting notes)  0.2.0  on  package aaaaaaaaaaaa"));
    assert!(text.contains("tested: pass "));
    assert!(
        text.contains("1 results for other releases; revocation not checked here; runs a workflow")
    );
    assert!(text.contains("openagents plugin use 0000:notes --version 0.2.0"));
    assert!(text.contains("  0000:cleanup (Disk cleanup)  1.0.0  off"));
    assert!(text.contains("not tested in this release; 0 results for other releases"));
    assert!(page.row_text(39).starts_with(crate::paper::GYM_KEYS));

    // ENTER opens the newest result for exactly this release in the Gym.
    press(&mut app, KeyCode::Enter, NamedKey::Enter);
    app.tick();
    let text = sheet(&mut app).text();
    assert!(
        text.contains("STUDY Better  reported pass, recomputed pass"),
        "{text}"
    );
    assert_eq!(
        transport.study_reads.lock().unwrap().as_slice(),
        [exact.to_owned()]
    );

    // ESC returns to the plugins; a plugin with no exact result opens
    // nothing and says so.
    press(&mut app, KeyCode::Escape, NamedKey::Escape);
    assert!(app.paper.gym.components && app.paper.gym.viewing.is_none());
    arrow(&mut app, KeyCode::ArrowDown);
    press(&mut app, KeyCode::Enter, NamedKey::Enter);
    assert!(app.paper.gym.viewing.is_none());
    assert!(
        sheet(&mut app)
            .text()
            .contains("No result tested exactly this release")
    );
    assert_eq!(transport.study_reads.lock().unwrap().len(), 1);
    // Nothing ran, turned on, or was sent.
    assert!(transport.commands.lock().unwrap().is_empty());
    assert!(transport.rule_commands.lock().unwrap().is_empty());
    assert!(transport.requests.lock().unwrap().is_empty());

    press(&mut app, KeyCode::Escape, NamedKey::Escape);
    assert!(!app.paper.gym.components && app.paper.gym.open);
    press(&mut app, KeyCode::Escape, NamedKey::Escape);
    assert!(!app.paper.gym.open);
}

#[test]
fn a_noncoding_plugin_turns_notes_into_one_checked_artifact_from_the_page() {
    use crate::input::{KeyCode, NamedKey};
    let transport = Arc::new(Fake {
        bridge: true,
        ..Fake::default()
    });
    let mut app = Application::new(Sessions(transport.clone()));
    app.paper.on = true;
    app.toggle();
    app.ensure_started();
    let sheet = |app: &mut Application| app.paper_sheet(120, 40, "12:00:00", "0.50");
    let key = |app: &mut Application, named: NamedKey| press(app, KeyCode::Unidentified, named);
    transport
        .output
        .lock()
        .unwrap()
        .push_back(b"\x1b]7;file:///test/work\x07\x1b]133;A\x07$ \x1b]133;B\x07".to_vec());
    app.tick();
    *transport.studies.lock().unwrap() =
        Some(serde_json::json!({"root": "/test/work", "studies": []}));
    *transport.components.lock().unwrap() = Some(serde_json::json!({
        "plugins": [{"id": "a7cf:action-items", "name": "Action items", "version": "0.1.0",
                     "digest": format!("sha256:{}", "d".repeat(64)), "enabled": true,
                     "workflow": true, "background": [], "revocation": "not_checked",
                     "evidence": [], "runs": [], "commands": {}}],
    }));
    key(&mut app, NamedKey::F12);
    app.tick();
    key(&mut app, NamedKey::F2);
    app.tick();
    assert!(sheet(&mut app).row_text(37).starts_with("| USE > "));

    // The typed request arms one use; ESC would reject it, ENTER confirms.
    typing(&mut app, "List the action items in notes/standup.md");
    press(&mut app, KeyCode::Enter, NamedKey::Enter);
    assert!(transport.uses.lock().unwrap().is_empty());
    let page = sheet(&mut app);
    assert!(
        page.row_text(37)
            .starts_with("| CONFIRM? use a7cf:action-items 0.1.0 once on this directory"),
        "{}",
        page.row_text(37)
    );
    press(&mut app, KeyCode::Enter, NamedKey::Enter);
    app.tick();
    app.tick();
    assert_eq!(
        transport.uses.lock().unwrap().as_slice(),
        [[
            "a7cf:action-items".to_owned(),
            "0.1.0".to_owned(),
            format!("sha256:{}", "d".repeat(64)),
            "List the action items in notes/standup.md".to_owned(),
            "/test/work".to_owned(),
        ]]
    );
    let page = sheet(&mut app);
    let text = page.text();
    for row in 0..40 {
        let row_text = page.row_text(row);
        assert_eq!(row_text.len(), 120, "row {row}: {row_text:?}");
        assert!(
            row_text.chars().all(|c| (' '..='~').contains(&c)),
            "row {row}"
        );
    }
    assert!(
        text.contains("USE \"List the action items in notes/standup.md\": ran"),
        "{text}"
    );
    assert!(text.contains("Action items (5)"));
    // The plugin, its exact release, the run, its check, and the kept
    // output, in one place.
    assert!(text.contains("RUN use-8d1b6c07a651cae1  completed  release 0.1.0 (this release)"));
    assert!(text.contains("check verified, output cccccccccccc retained, cost $0.0000"));

    // Closing and reopening the page, or asking again, never runs twice.
    key(&mut app, NamedKey::F12);
    key(&mut app, NamedKey::F12);
    app.tick();
    key(&mut app, NamedKey::F2);
    app.tick();
    assert_eq!(transport.uses.lock().unwrap().len(), 1);
    assert!(sheet(&mut app).text().contains("check verified"));
    typing(&mut app, "List the action items in notes/standup.md");
    press(&mut app, KeyCode::Enter, NamedKey::Enter);
    press(&mut app, KeyCode::Enter, NamedKey::Enter);
    app.tick();
    assert!(
        sheet(&mut app)
            .text()
            .contains("USE \"List the action items in notes/standup.md\": followed")
    );

    // A lost output shows as missing.
    transport.components.lock().unwrap().as_mut().unwrap()["plugins"][0]["runs"][0]["outputs"][0]
        ["state"] = serde_json::json!("missing");
    key(&mut app, NamedKey::F2);
    key(&mut app, NamedKey::F2);
    key(&mut app, NamedKey::F2);
    app.tick();
    assert!(
        sheet(&mut app)
            .text()
            .contains("output cccccccccccc missing")
    );
    assert!(transport.requests.lock().unwrap().is_empty());
}

fn knowledge_entry(id: &str, version: u32, status: &str, body: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id, "version": version, "kind": "procedure", "title": format!("About {id}"),
        "summary": "s", "tags": [], "applies_when": "", "status": status,
        "author": "c".repeat(64), "written_from": ["run-7"], "cites": ["docs/budget.md"],
        "evidence": ["reviewed 2026-10-01"], "answer": null, "body": body,
        "digest": format!("sha256:{}{version}", "e".repeat(63)),
    })
}

#[test]
fn knowledge_and_plans_are_cited_at_exact_versions_and_sent_as_previewed() {
    use crate::input::{KeyCode, NamedKey};
    let transport = Arc::new(Fake {
        bridge: true,
        ..Fake::default()
    });
    let mut app = Application::new(Sessions(transport.clone()));
    app.paper.on = true;
    app.toggle();
    app.ensure_started();
    let sheet = |app: &mut Application| app.paper_sheet(120, 40, "12:00:00", "0.50");
    let key = |app: &mut Application, named: NamedKey| press(app, KeyCode::Unidentified, named);
    let arrow = |app: &mut Application, code: KeyCode| press(app, code, NamedKey::Unidentified);
    transport
        .output
        .lock()
        .unwrap()
        .push_back(b"\x1b]7;file:///test/work\x07\x1b]133;A\x07$ \x1b]133;B\x07".to_vec());
    app.tick();
    *transport.studies.lock().unwrap() =
        Some(serde_json::json!({"root": "/test/work", "studies": []}));
    *transport.components.lock().unwrap() = Some(serde_json::json!({"plugins": []}));
    *transport.goals.lock().unwrap() = Some(serde_json::json!({"goals": [
        {"goal_id": "g1", "status": "working", "text": "Ship the offsite budget",
         "entries": [{"id": "e1", "seat": "ana", "progress": "running", "title": "Draft the budget"}]},
    ], "spend": {}}));
    let admitted = knowledge_entry("budget-rules", 2, "admitted", "Keep travel under $500.");
    let candidate = knowledge_entry("venue-tips", 1, "candidate", "Book early.");
    let moved = knowledge_entry("room-codes", 4, "admitted", "Room codes rotate weekly.");
    *transport.hits.lock().unwrap() = vec![
        admitted.clone(),
        candidate.clone(),
        knowledge_entry("room-codes", 3, "admitted", "old"),
    ];
    for entry in [&admitted, &candidate, &moved] {
        transport
            .entries
            .lock()
            .unwrap()
            .insert(entry["id"].as_str().unwrap().into(), entry.clone());
    }

    key(&mut app, NamedKey::F12);
    app.tick();
    key(&mut app, NamedKey::F2);
    key(&mut app, NamedKey::F2);
    app.tick();
    let page = sheet(&mut app);
    assert!(
        page.row_text(37).starts_with("| FIND > "),
        "{}",
        page.row_text(37)
    );
    let text = page.text();
    assert!(
        text.contains("KNOWLEDGE AND PLANS  0 cited with the next question"),
        "{text}"
    );
    assert!(text.contains("STUDIO PLANS (studio memory, not published knowledge)"));
    assert!(text.contains("> plan g1  working  1 steps  Ship the offsite budget"));

    // Search, then the picks: plan, admitted, candidate, moved.
    typing(&mut app, "budget");
    press(&mut app, KeyCode::Enter, NamedKey::Enter);
    app.tick();
    let text = sheet(&mut app).text();
    assert!(text.contains("budget-rules v2  admitted  by cccccccc...  About budget-rules"));
    assert!(text.contains("venue-tips v1  candidate"));

    // The plan opens and is cited as studio memory.
    press(&mut app, KeyCode::Enter, NamedKey::Enter);
    let text = sheet(&mut app).text();
    assert!(text.contains("PLAN g1  working  digest "), "{text}");
    assert!(text.contains("[ana] Draft the budget  running"));
    press(&mut app, KeyCode::Enter, NamedKey::Enter);
    press(&mut app, KeyCode::Escape, NamedKey::Escape);

    // The admitted entry opens with its provenance and is cited.
    arrow(&mut app, KeyCode::ArrowDown);
    press(&mut app, KeyCode::Enter, NamedKey::Enter);
    app.tick();
    let text = sheet(&mut app).text();
    assert!(
        text.contains("ENTRY budget-rules version 2  admitted  [ENTER cites it]"),
        "{text}"
    );
    assert!(text.contains("WRITTEN FROM run-7"));
    assert!(text.contains("EVIDENCE reviewed 2026-10-01"));
    press(&mut app, KeyCode::Enter, NamedKey::Enter);
    press(&mut app, KeyCode::Escape, NamedKey::Escape);

    // A candidate and a version that moved since the search are refused.
    arrow(&mut app, KeyCode::ArrowDown);
    press(&mut app, KeyCode::Enter, NamedKey::Enter);
    app.tick();
    press(&mut app, KeyCode::Enter, NamedKey::Enter);
    assert!(
        sheet(&mut app)
            .text()
            .contains("NOT CITED venue-tips version 1 is a candidate, not admitted context")
    );
    press(&mut app, KeyCode::Escape, NamedKey::Escape);
    arrow(&mut app, KeyCode::ArrowDown);
    press(&mut app, KeyCode::Enter, NamedKey::Enter);
    app.tick();
    press(&mut app, KeyCode::Enter, NamedKey::Enter);
    let text = sheet(&mut app).text();
    assert!(
        text.contains("CHANGED since the search found version 3"),
        "{text}"
    );
    assert!(text.contains("NOT CITED room-codes changed since the search (now version 4)"));
    press(&mut app, KeyCode::Escape, NamedKey::Escape);
    assert_eq!(app.paper.gym.knowledge.cited.len(), 2);

    // The page previews exactly what the next question sends.
    let text = sheet(&mut app).text();
    assert!(text.contains("KNOWLEDGE AND PLANS  2 cited with the next question"));
    assert!(text.contains("| Knowledge budget-rules version 2 (admitted), by "));
    assert!(text.contains("| Keep travel under $500."));
    let preview = crate::context::Context {
        cited: app.paper.gym.knowledge.cited.clone(),
        ..crate::context::Context::default()
    }
    .preview();

    // Asking sends the citations once, with that preview.
    press(&mut app, KeyCode::Escape, NamedKey::Escape);
    key(&mut app, NamedKey::F12);
    app.paper_ask("How much can we spend on travel?");
    let requests = transport.requests.lock().unwrap();
    let request = requests.last().expect("the question was sent");
    assert_eq!(request.context.cited.len(), 2);
    assert_eq!(request.context.cited[0].kind, "plan");
    assert_eq!(request.context.cited[1].id, "budget-rules");
    assert_eq!(request.context.cited[1].version, "2");
    let message = request.message().unwrap();
    assert!(message.contains(preview.trim_end()), "{message}");
    drop(requests);
    assert!(app.paper.gym.knowledge.cited.is_empty());
}

#[test]
fn a_block_is_shared_only_as_the_consented_static_excerpt() {
    let transport = Arc::new(Fake {
        bridge: true,
        ..Fake::default()
    });
    let mut app = Application::new(Sessions(transport.clone()));
    app.toggle();
    app.ensure_started();
    let request = |text: &str| -> crate::control::Request { serde_json::from_str(text).unwrap() };
    // Two finished blocks, one with a secret in its command, then a
    // full-screen program that is still running.
    transport.output.lock().unwrap().push_back(
        b"\x1b]7;file:///home/ana/private\x07\x1b]133;A\x07$ \x1b]133;B\x07\x1b]777;openagents;command;6563686f206f6b\x07\x1b]133;C\x07ok\r\n\x1b]133;D;0\x07\x1b]133;A\x07$ \x1b]133;B\x07\x1b]777;openagents;command;6661696c\x07\x1b]133;C\x07boom\r\n\x1b]133;D;1\x07\x1b]133;A\x07$ \x1b]133;B\x07".to_vec(),
    );
    app.tick();
    transport
        .output
        .lock()
        .unwrap()
        .push_back(b"\x1b]777;openagents;command;746f70\x07\x1b]133;C\x07\x1b[?1049h".to_vec());
    app.tick();
    let preview = app
        .apply(&request(r#"{"op": "excerpt", "block": 2}"#))
        .unwrap();
    assert_eq!(preview["state"], "preview");
    let excerpt: crate::excerpt::Excerpt =
        serde_json::from_value(preview["excerpt"].clone()).unwrap();
    crate::excerpt::verify(&excerpt).unwrap();
    assert_eq!(excerpt.command, "fail");
    assert_eq!(excerpt.head, vec!["boom".to_owned()]);
    assert_eq!(excerpt.status, Some(1));
    let json = preview.to_string();
    assert!(
        !json.contains("/home/ana") && !json.contains("echo ok"),
        "{json}"
    );
    // Nothing is recorded until the preview is consented to.
    assert!(app.status()["exports"].as_array().unwrap().is_empty());
    assert!(
        app.apply(&request(
            r#"{"op": "excerpt", "block": 2, "consent": "sha256:00"}"#
        ))
        .unwrap_err()
        .contains("changed since that preview")
    );
    let consent = preview["consent"].as_str().unwrap();
    let exported = app
        .apply(&request(&format!(
            r#"{{"op": "excerpt", "block": 2, "consent": "{consent}"}}"#
        )))
        .unwrap();
    assert_eq!(exported["state"], "exported");
    assert_eq!(exported["excerpt"], preview["excerpt"]);
    assert_eq!(exported["text"], preview["text"]);
    // The second fixture: the first block, by default the newest finished.
    let first = app
        .apply(&request(r#"{"op": "excerpt", "block": 1}"#))
        .unwrap();
    assert_eq!(first["excerpt"]["command"], "echo ok");
    let newest = app.apply(&request(r#"{"op": "excerpt"}"#)).unwrap();
    assert_eq!(newest["excerpt"]["source"]["block"], 2);
    let exports = app.status()["exports"].clone();
    assert_eq!(exports.as_array().unwrap().len(), 1);
    assert_eq!(exports[0]["digest"], consent);
    // The running full-screen block and an evicted one refuse.
    assert!(
        app.apply(&request(r#"{"op": "excerpt", "block": 3}"#))
            .unwrap_err()
            .contains("still running")
    );
    assert!(
        app.apply(&request(r#"{"op": "excerpt", "block": 9}"#))
            .unwrap_err()
            .contains("no longer holds block 9")
    );
    // Sharing sent nothing to the shell or to a thread.
    assert!(transport.input.lock().unwrap().is_empty());
    assert!(transport.requests.lock().unwrap().is_empty());
}
