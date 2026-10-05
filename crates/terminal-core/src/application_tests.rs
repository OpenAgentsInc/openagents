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
        if request.new {
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
