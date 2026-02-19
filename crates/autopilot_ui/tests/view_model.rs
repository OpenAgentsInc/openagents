use std::path::PathBuf;

use autopilot_app::{AppEvent, SessionId, UserAction, WorkspaceId};
use autopilot_ui::AppViewModel;

#[test]
fn view_model_tracks_workspace_and_session() {
    let mut view_model = AppViewModel::default();
    let workspace_id = WorkspaceId::new();
    let path = PathBuf::from("/tmp/autopilot-ui-test");

    view_model.apply_event(&AppEvent::WorkspaceOpened {
        workspace_id,
        path: path.clone(),
    });

    assert_eq!(view_model.workspace_path(), Some(&path));
    assert_eq!(view_model.event_count(), 1);

    let session_id = SessionId::new();
    view_model.apply_event(&AppEvent::SessionStarted {
        workspace_id,
        session_id,
        label: Some("Bootstrap".to_string()),
    });

    assert_eq!(view_model.session_id(), Some(session_id));
    assert_eq!(view_model.event_count(), 2);

    view_model.apply_event(&AppEvent::UserActionDispatched {
        workspace_id,
        action: UserAction::Command {
            session_id,
            name: "status".to_string(),
            args: vec![],
        },
    });

    assert_eq!(view_model.event_count(), 3);
}
