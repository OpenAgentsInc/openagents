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
    opened: AtomicUsize,
    input: Arc<Mutex<Vec<Vec<u8>>>>,
    closed: Arc<AtomicUsize>,
}
struct Pane {
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
        None
    }
    fn target(&self) -> Option<crate::proposals::Binding> {
        None
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
    fn request(&self, _: &crate::bridge::Request) -> Result<crate::bridge::Connection, String> {
        panic!("A preview must not send a request")
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
