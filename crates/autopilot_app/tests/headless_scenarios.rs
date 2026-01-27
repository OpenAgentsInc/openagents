use std::time::Duration;

use autopilot_app::{App, AppEvent, UserAction};
use futures::StreamExt;

#[tokio::test]
async fn scenario_open_workspace_and_send_message() {
    let app = App::default();
    let workspace = app.open_workspace("/tmp/autopilot-scenario");
    let mut events = workspace.events();

    let session = workspace.start_session(Some("Scenario".to_string()));
    workspace.dispatch(UserAction::Message {
        session_id: session.session_id(),
        text: "ping".to_string(),
    });

    let mut collected = Vec::new();
    for _ in 0..3 {
        let next = tokio::time::timeout(Duration::from_secs(1), events.next())
            .await
            .expect("event timeout")
            .expect("event missing");
        collected.push(next);
    }

    assert!(matches!(collected[0], AppEvent::WorkspaceOpened { .. }));
    assert!(collected.iter().any(|event| matches!(event, AppEvent::SessionStarted { .. })));
    assert!(collected
        .iter()
        .any(|event| matches!(event, AppEvent::UserActionDispatched { .. })));
}
