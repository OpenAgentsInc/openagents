use aethyrforge_host::*;
fn checkpoint() -> Checkpoint {
    Checkpoint {
        eye: [0., 1.62, 3.],
        target: [0., 1.62, 2.],
        mode: CameraMode::Walk,
    }
}
#[test]
fn sign_in_checkpoint_keeps_the_requested_change_and_rejects_untrusted_returns() {
    let pending = PendingChange {
        world: "haunted-house".into(),
        checkpoint: checkpoint(),
        selection: Some("werewolf-1".into()),
        panels: vec!["edit".into(), "kiln".into()],
        draft: "Move the werewolf".into(),
        action: Action::Propose {
            operations: vec![Edit::Move {
                target: "werewolf-1".into(),
                position: [-2., 0., -14.],
            }],
        },
    };
    pending.validate().unwrap();
    let encoded = serde_json::to_string(&pending).unwrap();
    let restored: PendingChange = serde_json::from_str(&encoded).unwrap();
    assert_eq!(restored, pending);
    for world in ["https://outside.example", "../another", "another-person"] {
        let mut bad = pending.clone();
        bad.world = world.into();
        assert!(bad.validate().is_err());
    }
    let mut bad = pending.clone();
    bad.action = Action::Inspect;
    assert!(bad.validate().is_err());
    let mut bad = pending;
    bad.panels.push("<script>".into());
    assert!(bad.validate().is_err());
}
#[test]
fn grant_is_narrow_and_cannot_name_the_shared_tutorial_as_its_draft() {
    let project = Project {
        id: "haunted-house".into(),
        branch: "main".into(),
        revision: 0,
    };
    let mut grant = ScopedGrant {
        token: "a".repeat(64),
        action_sha256: "b".repeat(64),
        session: "tab-session".into(),
        release: "house-v1".into(),
        project: project.clone(),
        draft: Project {
            id: "personal-draft".into(),
            branch: "house".into(),
            revision: 0,
        },
        capability: Capability::Propose,
        expires_at: 1234,
    };
    grant.validate().unwrap();
    grant.draft = project;
    assert!(grant.validate().is_err());
    grant.draft.branch = "private".into();
    grant.capability = Capability::Inspect;
    assert!(grant.validate().is_err());
}

#[test]
fn expired_login_keeps_its_request_but_another_account_cannot_inherit_it() {
    assert_eq!(
        resume_policy(None, None, Some("alice")),
        ResumePolicy::Reconfirm
    );
    assert_eq!(
        resume_policy(Some("alice"), Some("alice"), None),
        ResumePolicy::Reconfirm
    );
    assert_eq!(
        resume_policy(Some("alice"), None, Some("alice")),
        ResumePolicy::Reconfirm
    );
    assert_eq!(
        resume_policy(Some("alice"), None, Some("bob")),
        ResumePolicy::Discard
    );
    assert_eq!(
        resume_policy(None, Some("alice"), Some("bob")),
        ResumePolicy::Discard
    );
    assert_eq!(
        resume_policy(Some("alice"), Some("alice"), Some("alice")),
        ResumePolicy::Keep
    );
}
