//! Frozen v1 consumer shapes demonstrate why the extended contract needs v2.
use aethyrforge_host::*;
use serde::Deserialize;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct V1Status {
    project: Project,
    generation: String,
    frame: u64,
    save: SaveState,
    checkpoint: Option<Checkpoint>,
}
#[test]
fn extended_status_and_authorization_never_cross_the_frozen_v1_boundary() {
    let old = r#"{"project":{"id":"house","branch":"main","revision":0},"generation":"old-session","frame":1,"save":"unsaved","checkpoint":null}"#;
    let old_status: V1Status = serde_json::from_str(old).unwrap();
    assert_eq!(old_status.project.id, "house");
    assert_eq!(old_status.generation, "old-session");
    assert_eq!(old_status.frame, 1);
    assert_eq!(old_status.save, SaveState::Unsaved);
    assert!(old_status.checkpoint.is_none());
    let mut status: Status = serde_json::from_str(old).unwrap();
    status.selection = Some(SelectedObject {
        id: "werewolf-1".into(),
        asset: "custom:haunt:werewolf".into(),
        position: [0., 0., 0.],
    });
    assert!(serde_json::from_str::<V1Status>(&serde_json::to_string(&status).unwrap()).is_err());
    let old_hello = r#"{"protocol":"openagents.aethyrforge.host","version":1,"session":"v1-session","sequence":1,"message":{"type":"hello","versions":[1]}}"#;
    assert_eq!(Envelope::decode(old_hello), Err(Error::Version));
    assert!(matches!(
        Engine::accept(old_hello, "release-v2".into(), "abi".into(), 1, vec![]),
        Err(Error::Version)
    ));
    let mut host = Host::new(
        "https://engine.example",
        "v2-session".into(),
        "release-v2".into(),
        "abi".into(),
        1,
        Project {
            id: "house".into(),
            branch: "main".into(),
            revision: 0,
        },
    )
    .unwrap();
    let hello = host.hello().unwrap();
    assert_eq!(hello.version, 2);
    assert_eq!(hello.message, Message::Hello { versions: vec![2] });
    let (_, ready) = Engine::accept(
        &hello.encode().unwrap(),
        "release-v2".into(),
        "abi".into(),
        1,
        vec![],
    )
    .unwrap();
    assert!(host.receive(&ready.encode().unwrap()).is_ok());
    // Keep the original schema as an immutable compatibility artifact.
    let v1: serde_json::Value =
        serde_json::from_str(include_str!("../schema/host-v1.json")).unwrap();
    assert!(
        v1["$defs"]["Status"]["properties"]
            .get("selection")
            .is_none()
    );
    assert!(!include_str!("../schema/host-v1.json").contains("action_sha256"));
    assert!(include_str!("../schema/host-v2.json").contains("action_sha256"));
}
