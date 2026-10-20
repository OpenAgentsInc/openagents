use super::*;

#[test]
fn pending_team_effect_survives_restart_without_replay_or_public_token_material() {
    let dir = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let mut store = Store::open(dir.path()).unwrap();
    let command = TeamCommand {
        id: "invite-one".into(),
        origin: "https://fixture.invalid".into(),
        account: "buyer".into(),
        credential_alias: "buyer".into(),
        action: TeamAction::Invite {
            workspace: "team".into(),
            role: jev::TeamRole::Member,
            ttl_secs: 60,
        },
    };
    let mut next = store.book.clone();
    next.team.insert(
        command.id.clone(),
        Operation {
            command,
            status: TeamStatus::Pending,
            authority: format!("sha256:{}", "a".repeat(64)),
            input: format!("sha256:{}", "b".repeat(64)),
            result: None,
            refusal_status: None,
        },
    );
    store.persist(next).unwrap();
    drop(store);
    let store = Store::open(dir.path()).unwrap();
    let op = &store.book.team["invite-one"];
    assert_eq!(op.status, TeamStatus::Unknown);
    let view = serde_json::to_value(store.team_view(op)).unwrap();
    assert!(view["invitation_file"].is_null());
    assert!(!view.to_string().contains("sha256:"));
    assert!(
        view["limitation"]
            .as_str()
            .unwrap()
            .contains("Do not repeat")
    );
}
