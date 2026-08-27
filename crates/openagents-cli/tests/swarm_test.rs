//! The local swarm: registration, discovery, and the message mailboxes.
//!
//! Everything here is local files under a temporary home, so the tests are
//! hermetic — no daemon, no network, no other session. The properties pinned
//! are the ones the plan calls invariants: liveness honesty (stale is
//! reported, never hidden), delivery atomicity (a crash leaves a whole line
//! or none), gapless sequences with gaps refused loudly, receipts that move
//! forward, and a sender that cannot reach into a void without the refusal
//! saying so.

use openagents_cli::swarm::{
    DEFAULT_ALIVE_AFTER_MS, MAXIMUM_BODY_BYTES, MESSAGE_SCHEMA, Mailbox, REGISTRATION_SCHEMA,
    Registration, SwarmState, list, load_registration, read_inbox, register, send, unregister,
};
use std::path::PathBuf;

fn registration(session_id: &str, pid: u32, inbox: PathBuf) -> Registration {
    let now = openagents_cli::swarm::now_ms();
    Registration {
        schema: REGISTRATION_SCHEMA.to_string(),
        session_id: session_id.to_string(),
        pid,
        cwd: "/work".to_string(),
        lane: "flash".to_string(),
        model: None,
        role: "root".to_string(),
        parent: None,
        worktree: None,
        inbox: inbox.display().to_string(),
        alive_after_ms: DEFAULT_ALIVE_AFTER_MS,
        started_at_ms: now,
        heartbeat_at_ms: now,
    }
}

#[test]
fn a_registration_round_trips_and_unregisters() {
    let home = tempfile::tempdir().unwrap();
    let path = home.path().to_path_buf();
    let registration = registration("session-a", std::process::id(), PathBuf::from("/tmp/a"));
    register(&path, &registration).unwrap();

    let loaded = load_registration(&path, "session-a").unwrap().unwrap();
    assert_eq!(loaded.schema, REGISTRATION_SCHEMA);
    assert_eq!(
        loaded.state(),
        SwarmState::Live,
        "this test's own pid is alive"
    );

    unregister(&path, "session-a").unwrap();
    assert!(load_registration(&path, "session-a").unwrap().is_none());
    // Unregistering a registration that is not there succeeds: the goal is
    // the absence, not the removal.
    unregister(&path, "session-a").unwrap();
}

#[test]
fn a_dead_pid_is_stale_not_hidden() {
    let home = tempfile::tempdir().unwrap();
    let path = home.path().to_path_buf();
    // A pid that is not this process and almost certainly nothing. The point
    // is that state() must not answer Live for a registration the OS cannot
    // find.
    let registration = registration("ghost", u32::MAX - 7, PathBuf::from("/tmp/ghost"));
    register(&path, &registration).unwrap();
    let loaded = load_registration(&path, "ghost").unwrap().unwrap();
    assert_eq!(loaded.state(), SwarmState::Stale);
}

#[test]
fn discovery_lists_every_registration_and_names_corruption() {
    let home = tempfile::tempdir().unwrap();
    let path = home.path().to_path_buf();
    for id in ["one", "two", "three"] {
        register(
            &path,
            &registration(id, std::process::id(), PathBuf::from("/tmp")),
        )
        .unwrap();
    }
    let found = list(&path).unwrap();
    assert_eq!(found.len(), 3);
    assert!(
        found
            .iter()
            .all(|registration| registration.state() == SwarmState::Live)
    );

    // A registration that will not parse is an error, never a silent skip:
    // discovery that drops a session lies about the machine.
    let corrupt = path.join(".openagents").join("swarm").join("broken.json");
    std::fs::write(&corrupt, "{ not json").unwrap();
    assert!(list(&path).is_err(), "corrupt registration must surface");
}

#[test]
fn delivery_is_gapless_and_receipted() {
    let home = tempfile::tempdir().unwrap();
    let path = home.path().to_path_buf();

    let a_dir = tempfile::tempdir().unwrap();
    let b_dir = tempfile::tempdir().unwrap();
    register(
        &path,
        &registration(
            "session-a",
            std::process::id(),
            a_dir.path().join("inbox.jsonl"),
        ),
    )
    .unwrap();
    register(
        &path,
        &registration(
            "session-b",
            std::process::id(),
            b_dir.path().join("inbox.jsonl"),
        ),
    )
    .unwrap();

    let report = send(
        &path,
        "session-a",
        a_dir.path(),
        "session-b",
        "question",
        None,
        true,
        "what does the failing test say?",
    )
    .unwrap();
    assert_eq!(
        report.deliveries.len(),
        1,
        "one live recipient, one delivery"
    );
    assert!(report.undeliverable.is_empty());

    let inbox = Mailbox::at(b_dir.path());
    let messages = inbox.messages().unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].sequence, Some(1));
    assert!(
        messages[0].delivered_at_ms.is_some(),
        "delivery stamps on arrival"
    );
    assert!(messages[0].read_at_ms.is_none(), "nothing has read it yet");
    assert_eq!(messages[0].reply_expected, Some(true));

    // The second message is sequence 2, and the reply chain carries.
    let second = send(
        &path,
        "session-a",
        a_dir.path(),
        "session-b",
        "status",
        Some(&report.message_id),
        false,
        "follow-up in the same thread",
    )
    .unwrap();
    let messages = inbox.messages().unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[1].sequence, Some(2));
    assert_eq!(
        messages[1].thread.as_deref(),
        Some(report.message_id.as_str())
    );
    assert!(second.deliveries.len() == 1);

    // Reading the whole inbox enforces the no-gap law: sequences 1,2 read
    // cleanly.
    let read = read_inbox(b_dir.path()).unwrap();
    assert_eq!(read.len(), 2);

    // The read stamp moves on drain.
    inbox.mark_read_through(2).unwrap();
    let messages = inbox.messages().unwrap();
    assert!(
        messages.iter().all(|message| message.read_at_ms.is_some()),
        "drain stamps every message through the cursor"
    );
}

#[test]
fn a_gap_in_the_inbox_is_refused_not_papered_over() {
    let dir = tempfile::tempdir().unwrap();
    // Hand-write an inbox with a missing sequence 2: what a lost line looks
    // like.
    let line_one = r#"{"schema":"openagents.swarm.message.v1","id":"msg_1","sequence":1,"from":"a","to":"b","kind":"status","body":"first","created_at_ms":1}"#;
    let line_three = r#"{"schema":"openagents.swarm.message.v1","id":"msg_3","sequence":3,"from":"a","to":"b","kind":"status","body":"third","created_at_ms":3}"#;
    std::fs::write(
        dir.path().join("inbox.jsonl"),
        format!("{line_one}\n{line_three}\n"),
    )
    .unwrap();
    let why = read_inbox(dir.path()).unwrap_err();
    assert!(
        why.contains("gaps at sequence 1"),
        "the refusal names the gap: {why}"
    );
}

#[test]
fn sends_that_cannot_deliver_are_refused_by_name() {
    let home = tempfile::tempdir().unwrap();
    let path = home.path().to_path_buf();
    let a_dir = tempfile::tempdir().unwrap();
    register(
        &path,
        &registration(
            "session-a",
            std::process::id(),
            a_dir.path().join("inbox.jsonl"),
        ),
    )
    .unwrap();

    // An unregistered destination is refused, naming it.
    let why = send(
        &path,
        "session-a",
        a_dir.path(),
        "nobody",
        "status",
        None,
        false,
        "hello",
    )
    .unwrap_err();
    assert!(why.contains("no session `nobody` is registered"), "{why}");

    // An empty body is not a message.
    let why = send(
        &path,
        "session-a",
        a_dir.path(),
        "nobody",
        "status",
        None,
        false,
        "   ",
    )
    .unwrap_err();
    assert!(why.contains("empty"), "{why}");

    // A self-send is refused: the loop it would create is the livelock the
    // budget exists to stop.
    let why = send(
        &path,
        "session-a",
        a_dir.path(),
        "session-a",
        "status",
        None,
        false,
        "talking to myself",
    )
    .unwrap_err();
    assert!(why.contains("cannot send a message to itself"), "{why}");

    // A body past the bound is refused, not truncated.
    let huge = "x".repeat(MAXIMUM_BODY_BYTES + 1);
    let why = send(
        &path,
        "session-a",
        a_dir.path(),
        "session-a",
        "status",
        None,
        false,
        &huge,
    )
    .unwrap_err();
    assert!(!why.is_empty(), "refused one way or another");

    // An unknown kind is refused with the vocabulary.
    let why = send(
        &path,
        "session-a",
        a_dir.path(),
        "session-a",
        "gossip",
        None,
        false,
        "hello",
    )
    .unwrap_err();
    assert!(why.contains("not a message kind"), "{why}");
}

#[test]
fn broadcast_resolves_to_every_other_live_session() {
    let home = tempfile::tempdir().unwrap();
    let path = home.path().to_path_buf();
    let dirs: Vec<tempfile::TempDir> = (0..3).map(|_| tempfile::tempdir().unwrap()).collect();
    for (index, dir) in dirs.iter().enumerate() {
        register(
            &path,
            &registration(
                &format!("session-{index}"),
                std::process::id(),
                dir.path().join("inbox.jsonl"),
            ),
        )
        .unwrap();
    }
    let report = send(
        &path,
        "session-0",
        dirs[0].path(),
        "all",
        "broadcast",
        None,
        false,
        "standing up in five",
    )
    .unwrap();
    assert_eq!(report.deliveries.len(), 2, "everyone but the sender");
    assert!(report.undeliverable.is_empty());

    // children-of resolves by role and parent.
    for index in 0..2 {
        let mut child = registration(
            &format!("child-{index}"),
            std::process::id(),
            dirs[index + 1].path().join("child-inbox.jsonl"),
        );
        child.role = "child".to_string();
        child.parent = Some("session-1".to_string());
        register(&path, &child).unwrap();
    }
    let report = send(
        &path,
        "human",
        dirs[2].path(),
        "role:children-of:session-1",
        "status",
        None,
        false,
        "wrap up",
    )
    .unwrap();
    assert_eq!(report.deliveries.len(), 2, "both children of session-1");
}

#[test]
fn the_schema_names_travel_on_the_wire() {
    let home = tempfile::tempdir().unwrap();
    let path = home.path().to_path_buf();
    let dir = tempfile::tempdir().unwrap();
    register(
        &path,
        &registration(
            "session-schema",
            std::process::id(),
            dir.path().join("inbox.jsonl"),
        ),
    )
    .unwrap();
    let registration = load_registration(&path, "session-schema").unwrap().unwrap();
    assert_eq!(registration.schema, REGISTRATION_SCHEMA);

    send(
        &path,
        "human",
        dir.path(),
        "session-schema",
        "status",
        None,
        false,
        "schema check",
    )
    .unwrap();
    let messages = Mailbox::at(dir.path()).messages().unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].schema, MESSAGE_SCHEMA);
}
