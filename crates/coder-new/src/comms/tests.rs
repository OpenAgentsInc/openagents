use super::*;

fn channel() -> (tempfile::TempDir, Channel) {
    let dir = tempfile::tempdir().unwrap();
    let channel = Channel::open(dir.path(), "session-a").unwrap();
    (dir, channel)
}

/// The channel's own folder under `root`, `comms/<session>`.
fn folder_of(root: &std::path::Path) -> PathBuf {
    root.join(ROOT).join("session-a")
}

#[test]
fn an_overlap_is_a_shared_file_folder_crate_or_resource() {
    let a = Intent::new(
        "alpha",
        vec!["crates/coder-new/src/comms/mod.rs".into()],
        vec![],
    );
    let b = Intent::new(
        "beta",
        vec!["crates/coder-new/src/comms/mod.rs".into()],
        vec![],
    );
    // Same file: both the file and the crate it lives in are shared.
    assert_eq!(
        overlap(&a, &b),
        vec!["crate coder-new", "file crates/coder-new/src/comms/mod.rs"]
    );
    assert!(a.crates.contains(&"coder-new".to_string()));

    // Different files in one crate: the crate is shared.
    let c = Intent::new("gamma", vec!["crates/coder-new/src/lib.rs".into()], vec![]);
    assert_eq!(overlap(&a, &c), vec!["crate coder-new"]);

    // A file inside a named folder.
    let folder = Intent::new("folder", vec!["crates/coder-new/src/comms".into()], vec![]);
    assert!(
        overlap(&a, &folder)
            .iter()
            .any(|shared| shared.starts_with("folder")),
        "{:?}",
        overlap(&a, &folder)
    );

    // A shared resource.
    let mut res_a = Intent::new("alpha", vec!["x.txt".into()], vec!["port:8080".into()]);
    let res_b = Intent::new("beta", vec!["y.txt".into()], vec!["port:8080".into()]);
    // x.txt and y.txt share no folder or crate.
    res_a.files = vec!["x.txt".into()];
    assert_eq!(overlap(&res_a, &res_b), vec!["resource port:8080"]);

    // Nothing shared.
    let d = Intent::new("delta", vec!["docs/a.md".into()], vec![]);
    let e = Intent::new("epsilon", vec!["docs/b.md".into()], vec![]);
    assert!(overlap(&d, &e).is_empty());
}

#[test]
fn posting_an_intent_records_one_overlap_notice_naming_both_members() {
    let (_dir, channel) = channel();
    channel.join("alpha").unwrap();
    channel.join("beta").unwrap();
    channel
        .intent(Intent::new(
            "alpha",
            vec!["crates/coder/src/lib.rs".into()],
            vec![],
        ))
        .unwrap();
    let overlaps = channel
        .intent(Intent::new(
            "beta",
            vec!["crates/coder/src/router.rs".into()],
            vec![],
        ))
        .unwrap();
    assert_eq!(overlaps.len(), 1, "one notice, once");
    assert_eq!(
        overlaps[0].members,
        ["alpha".to_string(), "beta".to_string()]
    );
    assert_eq!(overlaps[0].shared, vec!["crate coder"]);
    let notices = channel.overlaps().unwrap();
    assert_eq!(notices.len(), 1);
    assert_eq!(
        notices[0].body["shared"],
        json!(["crate coder"]),
        "both members are told and what they share is named"
    );
}

#[test]
fn a_lease_is_acquired_renewed_expired_and_released() {
    let (_dir, channel) = channel();
    let want = [("port".to_string(), "8080".to_string())];
    let taken = channel.acquire("alpha", &want).unwrap();
    assert_eq!(taken.len(), 1);
    assert_eq!(channel.leases().unwrap().len(), 1);

    // Another member finds it held and is told who holds it and until when.
    let refused = channel.acquire("beta", &want).unwrap_err();
    assert!(refused.contains("held by alpha"), "{refused}");
    assert!(refused.contains("until"), "{refused}");

    // Renewal keeps it while it is worked on.
    let renewed = channel.renew("alpha", "port", "8080").unwrap();
    assert!(renewed.expires_at > taken[0].expires_at);

    // A failed renewal by another member does not extend it.
    assert!(channel.renew("beta", "port", "8080").is_err());

    // Release frees it for the next member.
    channel.release("alpha", "port", "8080").unwrap();
    assert!(channel.leases().unwrap().is_empty());
    assert!(channel.acquire("beta", &want).is_ok());

    // Expiry is by time: hand-expire and sweep.
    channel
        .with_state(|state| {
            for lease in state.leases.values_mut() {
                lease.expires_at = 0;
            }
            Ok(())
        })
        .unwrap();
    let expired = channel.sweep().unwrap();
    assert_eq!(expired.len(), 1);
    assert!(
        channel.leases().unwrap().is_empty(),
        "a crashed holder's lease expires"
    );
}

#[test]
fn several_leases_are_taken_in_one_fixed_order_without_deadlock() {
    let (_dir, channel) = channel();
    // Alpha takes port first, then build-folder would rank before it and be
    // refused: taking it first could let two members wait on each other.
    channel
        .acquire("alpha", &[("port".into(), "9000".into())])
        .unwrap();
    let refused = channel
        .acquire("alpha", &[("build-folder".into(), "/t".into())])
        .unwrap_err();
    assert!(refused.contains("fixed order"), "{refused}");

    // In the sorted order the same two go through atomically.
    let built = channel
        .acquire(
            "beta",
            &[
                ("port".into(), "9000".into()),
                ("build-folder".into(), "/t".into()),
            ],
        )
        .unwrap_err();
    assert!(
        built.contains("held by alpha"),
        "the lowest rank is held: {built}"
    );

    // Beta takes, in one call, its own sorted set with nothing held.
    let leases = channel
        .acquire(
            "beta",
            &[
                ("port".into(), "9001".into()),
                ("build-folder".into(), "/t".into()),
            ],
        )
        .unwrap();
    let kinds: Vec<&str> = leases.iter().map(|lease| lease.kind.as_str()).collect();
    assert_eq!(
        kinds,
        vec!["build-folder", "port"],
        "acquired in the fixed order"
    );
}

#[test]
fn a_negotiation_records_its_outcome_decider_and_reason() {
    let (_dir, channel) = channel();
    channel.join("alpha").unwrap();
    channel.join("beta").unwrap();
    channel
        .intent(Intent::new("alpha", vec!["a.rs".into()], vec![]))
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(2));
    channel
        .intent(Intent::new("beta", vec!["a.rs".into()], vec![]))
        .unwrap();
    let agreement = channel.settle_first("alpha", "beta").unwrap();
    assert_eq!(agreement.outcome, Outcome::First);
    assert_eq!(agreement.detail, "alpha", "the first intent goes first");
    assert_eq!(agreement.decided_by, "swarm");
    assert!(agreement.why.contains("rebases"));

    let recorded = channel.agreements().unwrap();
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].outcome, Outcome::First);

    // A made agreement is recorded with who decided and why.
    channel
        .agree(Agreement {
            members: ["alpha".into(), "beta".into()],
            outcome: Outcome::Split,
            detail: "alpha: the type, beta: the caller".into(),
            decided_by: "alpha".into(),
            why: "one file, two regions".into(),
            at: 0,
        })
        .unwrap();
    assert_eq!(channel.agreements().unwrap().len(), 2);
}

#[test]
fn finishing_releases_every_lease_the_member_held() {
    let (_dir, channel) = channel();
    channel
        .acquire(
            "alpha",
            &[
                ("port".into(), "9100".into()),
                ("migration".into(), "42".into()),
            ],
        )
        .unwrap();
    channel.post("alpha", "all", "starting now").unwrap();
    let released = channel.finish("alpha").unwrap();
    assert_eq!(released.len(), 2);
    assert!(channel.leases().unwrap().is_empty());

    let members = channel.members().unwrap();
    assert!(
        members
            .iter()
            .any(|m| m.name == "alpha" && m.finished_at.is_some())
    );
}

#[test]
fn a_member_reads_only_what_is_new_and_addressed_to_everyone_or_it() {
    let (_dir, channel) = channel();
    channel.post("alpha", "all", "first").unwrap();
    channel.post("alpha", "beta", "for beta only").unwrap();
    channel.post("alpha", "gamma", "for gamma only").unwrap();
    let (seen, cursor) = channel.read_for("beta", 0).unwrap();
    let texts: Vec<&str> = seen
        .iter()
        .filter(|record| record.kind == "message")
        .map(|record| record.body["text"].as_str().unwrap())
        .collect();
    assert_eq!(texts, vec!["first", "for beta only"]);
    assert_eq!(cursor, 3);
    channel.post("alpha", "all", "later").unwrap();
    let (new, cursor) = channel.read_for("beta", cursor).unwrap();
    assert_eq!(new.len(), 1);
    assert_eq!(new[0].body["text"], "later");
    assert_eq!(cursor, 4);
}

#[test]
fn the_log_records_every_kind_of_event_as_one_line() {
    let (dir, channel) = channel();
    channel.join("alpha").unwrap();
    channel.post("alpha", "all", "hello").unwrap();
    channel
        .intent(Intent::new(
            "alpha",
            vec!["crates/coder/src/lib.rs".into()],
            vec![],
        ))
        .unwrap();
    channel
        .acquire("alpha", &[("port".into(), "9200".into())])
        .unwrap();
    channel.release("alpha", "port", "9200").unwrap();
    channel.finish("alpha").unwrap();
    let text = std::fs::read_to_string(folder_of(dir.path()).join(LOG)).unwrap();
    assert!(text.lines().count() >= 5);
    for line in text.lines() {
        let value: Value = serde_json::from_str(line).unwrap();
        assert!(value["kind"].is_string());
        assert!(value["at"].is_number());
    }
    // The channel snapshot names members, intents, leases and agreements.
    let snapshot = channel.snapshot().unwrap();
    assert_eq!(snapshot["members"][0]["name"], "alpha");
}

#[test]
fn the_preamble_names_the_folder_member_and_command() {
    let (dir, channel) = channel();
    let text = preamble(&channel, "solver-11346", "/opt/oa/coder comms");
    // `dir.path()` and the channel can differ on macOS (`/var` is a link to
    // `/private/var`), so compare the folder the channel itself reports.
    let folder = channel.dir().to_path_buf();
    assert!(text.contains(&folder.display().to_string()));
    let _ = dir;
    assert!(text.contains("--as solver-11346"));
    assert!(text.contains("`/opt/oa/coder comms post"));
    // No side file: an agent that can't run the command says so (#11468).
    assert!(!text.contains(&folder.join(LOG).display().to_string()));
    assert!(text.contains("Never write to the channel folder yourself."));
}

#[test]
fn the_command_is_this_program_by_its_full_path() {
    assert_eq!(
        invocation_for(Path::new("/Users/a/.openagents/bin/coder")),
        "/Users/a/.openagents/bin/coder comms"
    );
    assert_eq!(
        invocation_for(Path::new("/usr/local/bin/openagents")),
        "/usr/local/bin/openagents coder comms"
    );
    assert_eq!(
        invocation_for(Path::new("/Applications/Open Agents/coder")),
        "'/Applications/Open Agents/coder' comms"
    );
    assert_eq!(
        invocation_for(Path::new("/tmp/deps/coder_new-1234")),
        "coder comms"
    );
}

#[test]
fn a_message_is_bounded_and_never_empty() {
    let (_dir, channel) = channel();
    assert!(channel.post("alpha", "all", "   ").is_err());
    assert!(
        channel
            .post("alpha", "all", &"x".repeat(MESSAGE_LIMIT + 1))
            .is_err()
    );
    assert!(channel.post("alpha", "all", "short").is_ok());
}

#[test]
fn a_session_channel_name_is_safe_as_one_path_segment() {
    assert_eq!(session_id(Some("chat-1")), "chat-1");
    assert_eq!(session_id(Some("a/../b")), "a-..-b");
    assert!(!session_id(None).is_empty());
    assert_eq!(Channel::member_name("solver", "11346"), "solver-11346");
}

/// A delegated agent joins as its id and a number, never its task (#11467).
#[test]
fn a_delegated_agent_joins_under_a_short_numbered_name() {
    let (_dir, channel) = channel();
    assert_eq!(channel.join_numbered("grok-build").unwrap(), "grok-build-1");
    assert_eq!(channel.join_numbered("grok-build").unwrap(), "grok-build-2");
    assert_eq!(
        channel.join_numbered("claude-code").unwrap(),
        "claude-code-1"
    );
    assert_eq!(channel.members().unwrap().len(), 3);
}
