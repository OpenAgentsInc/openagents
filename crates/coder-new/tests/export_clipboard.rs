use coder_new::App;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

fn export(app: &mut App, name: &str) {
    assert!(app.handle(Event::Paste(format!("/export {name}"))));
    assert!(app.handle(Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    ))));
}

#[test]
fn export_copies_the_saved_absolute_path_once_and_does_not_copy_failed_exports() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = App::default();
    app.cwd = Some(directory.path().to_owned());
    export(&mut app, "conversation with spaces.atif.json");
    let path = directory.path().join("conversation with spaces.atif.json");
    let document = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert!(atif::validate(&document).is_empty());
    let mut copied = Vec::new();
    app.copy_export_path(|text| {
        copied.push(text.to_owned());
        Ok(())
    });
    assert_eq!(copied, [path.canonicalize().unwrap().to_string_lossy()]);
    assert!(
        app.notice
            .as_deref()
            .unwrap()
            .contains("Path copied to clipboard.")
    );
    app.copy_export_path(|_| panic!("The clipboard must only be written once."));

    let saved = std::fs::read(&path).unwrap();
    export(&mut app, "conversation with spaces.atif.json");
    app.copy_export_path(|_| panic!("A failed export must not change the clipboard."));
    assert_eq!(std::fs::read(&path).unwrap(), saved);
    assert!(!app.notice.as_deref().unwrap().contains("Path copied"));
}

#[test]
fn clipboard_failure_keeps_the_export_and_shows_its_path() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = App::default();
    app.cwd = Some(directory.path().to_owned());
    export(&mut app, "conversation.atif.json");
    let path = directory.path().join("conversation.atif.json");
    app.copy_export_path(|_| Err(std::io::Error::other("Clipboard unavailable")));
    assert!(path.is_file());
    let notice = app.notice.as_deref().unwrap();
    assert!(notice.contains(path.canonicalize().unwrap().to_str().unwrap()));
    assert!(notice.contains("Cannot copy path to clipboard: Clipboard unavailable."));
    app.copy_export_path(|_| panic!("A clipboard failure must not trigger repeated writes."));
}
