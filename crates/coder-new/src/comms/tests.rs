use std::path::{Path, PathBuf};
use std::sync::{Arc, atomic::AtomicBool};

use serde_json::{Value, json};

use super::*;
use crate::bundled_runtime::RuntimeEvent;
use crate::live::Entry;

fn channel() -> (tempfile::TempDir, Channel) {
    let dir = tempfile::tempdir().unwrap();
    let channel = Channel::open(dir.path().join("session")).unwrap();
    (dir, channel)
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_owned()).collect()
}

#[test]
fn overlaps_are_found_on_files_folders_crates_and_resources() {
    let intent = |member: &str, files: &[&str], crates: &[&str], resources: &[&str]| {
        Intent::new(
            member,
            &strings(files),
            &strings(crates),
            &strings(resources),
            "",
        )
    };
    // The same file, one written with a solver's worktree prefix.
    let a = intent("A", &["/r/runs/1/worktree/docs/a.md"], &[], &[]);
    let b = intent("B", &["./docs/a.md", "docs/b.md"], &[], &[]);
    let found = overlap::between(&a, &b).expect("same file");
    assert_eq!(found.files, ["docs/a.md"]);
    assert!(found.sentence().contains("A and B overlap on docs/a.md"));
    // A file inside a folder the other names.
    let c = intent("C", &["crates/x/src/"], &[], &[]);
    let d = intent("D", &["crates/x/src/lib.rs"], &[], &[]);
    let found = overlap::between(&c, &d).unwrap();
    assert_eq!(found.files, ["crates/x/src/lib.rs"]);
    // A crate named outright, or by a file in it.
    let e = intent("E", &[], &["coder-new"], &[]);
    let f = intent("F", &["crates/coder-new/src/ui.rs"], &[], &[]);
    let found = overlap::between(&e, &f).unwrap();
    assert!(found.files.is_empty());
    assert_eq!(found.crates, ["coder-new"]);
    assert_eq!(found.shared(), ["crate coder-new"]);
    // A resource.
    let g = intent("G", &[], &[], &["migration"]);
    let h = intent("H", &["docs/z.md"], &[], &["migration"]);
    assert_eq!(overlap::between(&g, &h).unwrap().resources, ["migration"]);
    // Nothing shared; a sibling folder is not inside.
    let i = intent("I", &["docs/ab.md", "crates/xy/a.rs"], &[], &[]);
    let j = intent("J", &["docs/a", "crates/x/a.rs"], &[], &[]);
    assert!(overlap::between(&i, &j).is_none());
}

#[test]
fn an_overlap_is_posted_once_to_both_sides_and_again_only_when_it_changes() {
    let (_dir, channel) = channel();
    let a = channel.join("codex", "Codex").unwrap();
    let b = channel.join("opencode", "OpenCode").unwrap();
    assert!(
        a.intent(&strings(&["docs/a.md"]), &[], &[], "")
            .unwrap()
            .is_empty()
    );
    let overlaps = b
        .intent(&strings(&["docs/a.md", "docs/b.md"]), &[], &[], "")
        .unwrap();
    assert_eq!(overlaps.len(), 1);
    assert_eq!(
        (overlaps[0].a.as_str(), overlaps[0].b.as_str()),
        ("OpenCode", "Codex")
    );
    // Restating the same intent tells no one again.
    b.intent(&strings(&["docs/b.md", "docs/a.md"]), &[], &[], "")
        .unwrap();
    let notices = |channel: &Channel| -> Vec<Record> {
        channel
            .records(0)
            .0
            .into_iter()
            .filter(|r| r.kind == Kind::Overlap)
            .collect()
    };
    assert_eq!(notices(&channel).len(), 1);
    let notice = &notices(&channel)[0];
    assert!(notice.concerns("Codex") && notice.concerns("OpenCode"));
    let mut cursor = 0;
    assert!(a.inbox(&mut cursor).iter().any(|r| r.kind == Kind::Overlap));
    // A wider overlap is news.
    a.intent(&strings(&["docs/a.md", "docs/b.md"]), &[], &[], "")
        .unwrap();
    assert_eq!(notices(&channel).len(), 2);
    // A member whose change won't land overlaps no one.
    a.finish("failed").unwrap();
    assert!(
        b.intent(&strings(&["docs/a.md"]), &[], &[], "")
            .unwrap()
            .is_empty()
    );
}

#[test]
fn messages_reach_everyone_or_one_member_and_an_appended_line_reads_as_a_message() {
    let (_dir, channel) = channel();
    let a = channel.join("codex", "Codex").unwrap();
    let b = channel.join("codex", "Codex").unwrap();
    assert_eq!(b.name(), "Codex 2", "names are made unique");
    let c = channel.join("devin", "Devin").unwrap();
    a.post(None, "I'm starting on the docs.").unwrap();
    a.post(Some("Codex 2"), "Leave ui.rs to me.").unwrap();
    assert!(
        a.post(Some("Nobody"), "hi")
            .unwrap_err()
            .contains("isn't in this session")
    );
    // An engine with no binary at hand appends a line itself.
    let mut log = std::fs::OpenOptions::new()
        .append(true)
        .open(channel.log_path())
        .unwrap();
    std::io::Write::write_all(
        &mut log,
        b"{\"from\":\"Devin\",\"text\":\"Done with the tests.\"}\n",
    )
    .unwrap();
    let (mut b_cursor, mut c_cursor, mut a_cursor) = (0, 0, 0);
    let b_sees: Vec<String> = b.inbox(&mut b_cursor).iter().map(view::plain).collect();
    let c_sees: Vec<String> = c.inbox(&mut c_cursor).iter().map(view::plain).collect();
    let a_sees: Vec<String> = a.inbox(&mut a_cursor).iter().map(view::plain).collect();
    assert!(b_sees.contains(&"Codex to Codex 2: Leave ui.rs to me.".to_owned()));
    assert!(
        !c_sees.iter().any(|line| line.contains("Leave ui.rs")),
        "a direct message is private"
    );
    assert!(c_sees.contains(&"Codex to everyone: I'm starting on the docs.".to_owned()));
    assert!(a_sees.contains(&"Devin to everyone: Done with the tests.".to_owned()));
    assert!(
        !a_sees
            .iter()
            .any(|line| line.contains("starting on the docs")),
        "no echo of its own words"
    );
}

#[test]
fn a_lease_is_taken_renewed_expires_and_is_released() {
    let (_dir, channel) = channel();
    let a = channel.join("solver", "A").unwrap();
    let b = channel.join("solver", "B").unwrap();
    let slot = [LeaseRequest::new(LeaseKind::DeploySlot, None)];
    let t0 = 1_000_000;
    let taken = a.acquire_at(&slot, 60_000, t0).unwrap();
    assert_eq!(taken[0].resource, "deploy-slot:staging");
    assert_eq!(taken[0].expires_ms, t0 + 60_000);
    // Held: B is told who holds it and until when, and gets nothing.
    match b.acquire_at(&slot, 60_000, t0 + 1000) {
        Err(lease::Refusal::Held {
            holder,
            resource,
            expires_ms,
        }) => {
            assert_eq!(
                (holder.as_str(), resource.as_str(), expires_ms),
                ("A", "deploy-slot:staging", t0 + 60_000)
            );
        }
        other => panic!("expected held, got {other:?}"),
    }
    // Renewed while held: it outlives its first expiry.
    let renewed = a
        .renew_at("deploy-slot:staging", 60_000, t0 + 50_000)
        .unwrap();
    assert_eq!((renewed.expires_ms, renewed.renewals), (t0 + 110_000, 1));
    assert!(b.acquire_at(&slot, 60_000, t0 + 70_000).is_err());
    // Not renewed again: it expires and B takes it.
    let took = b.acquire_at(&slot, 60_000, t0 + 110_001).unwrap();
    assert_eq!(took[0].holder, "B");
    assert!(
        a.renew_at("staging", 1000, t0 + 110_002)
            .unwrap_err()
            .contains("may have expired")
    );
    let records = channel.records(0).0;
    assert!(records.iter().any(
        |r| r.kind == Kind::Lease && r.text.contains("A's lease on deploy-slot:staging ended")
    ));
    // Released: the next member takes it at once.
    assert!(b.release("staging").unwrap(), "found by its name alone");
    assert!(!b.release("staging").unwrap());
    assert!(a.acquire_at(&slot, 60_000, t0 + 110_003).is_ok());
    // Finishing releases everything it holds.
    a.finish("passed").unwrap();
    assert!(channel.read_state().unwrap().leases.is_empty());
}

#[test]
fn fresh_migration_numbers_and_ports_are_never_handed_out_twice() {
    let (_dir, channel) = channel();
    let a = channel.join("solver", "A").unwrap();
    let b = channel.join("solver", "B").unwrap();
    let migration = [LeaseRequest::new(LeaseKind::Migration, None)];
    let first = a.acquire(&migration, 60_000).unwrap();
    let second = b.acquire(&migration, 60_000).unwrap();
    assert_eq!(first[0].value, "0001");
    assert_eq!(second[0].value, "0002");
    a.release(&first[0].resource).unwrap();
    // A released number is not reused.
    assert_eq!(a.acquire(&migration, 60_000).unwrap()[0].value, "0003");
    // A number asked for by name moves the counter past it.
    b.acquire(
        &[LeaseRequest::new(LeaseKind::Migration, Some("0010"))],
        60_000,
    )
    .unwrap();
    assert_eq!(b.acquire(&migration, 60_000).unwrap()[0].value, "0011");
    let ports = [LeaseRequest::new(LeaseKind::Port, None)];
    let p1 = a.acquire(&ports, 60_000).unwrap()[0].value.clone();
    let p2 = b.acquire(&ports, 60_000).unwrap()[0].value.clone();
    assert_ne!(p1, p2);
    assert!(LeaseRequest::parse("port:notaport").is_ok());
    assert!(
        a.acquire(&[LeaseRequest::parse("port:notaport").unwrap()], 60_000)
            .is_err()
    );
    assert!(LeaseRequest::parse("gpu").is_err());
}

#[test]
fn leases_are_taken_in_order_all_or_none_so_no_two_members_deadlock() {
    let (_dir, channel) = channel();
    let a = channel.join("solver", "A").unwrap();
    let b = channel.join("solver", "B").unwrap();
    let folder = LeaseRequest::new(LeaseKind::BuildFolder, Some("/t"));
    let host = LeaseRequest::new(LeaseKind::BuildHost, Some("coderos"));
    let slot = LeaseRequest::new(LeaseKind::DeploySlot, None);
    // Holding the deploy slot, A can't then take the build folder, which
    // sorts first: that is how a cycle would start.
    a.acquire(std::slice::from_ref(&slot), 60_000).unwrap();
    match a.acquire(std::slice::from_ref(&folder), 60_000) {
        Err(lease::Refusal::Order { resource, held }) => {
            assert_eq!(
                (resource.as_str(), held.as_str()),
                ("build-folder:/t", "deploy-slot:staging")
            );
        }
        other => panic!("expected an order refusal, got {other:?}"),
    }
    // Asking for both in one call is fine, in any order; renewing the slot.
    let both = a.acquire(&[slot.clone(), folder.clone()], 60_000).unwrap();
    assert_eq!(
        both[0].resource, "build-folder:/t",
        "taken in the fixed order"
    );
    // All or none: B gets neither while A holds one of them.
    a.release("deploy-slot:staging").unwrap();
    assert!(matches!(
        b.acquire(&[host.clone(), slot.clone(), folder.clone()], 60_000),
        Err(lease::Refusal::Held { .. })
    ));
    assert!(b.leases().is_empty(), "nothing taken on a refusal");
    a.finish("done").unwrap();

    // Two members that each want all three, asking in opposite orders,
    // over and over on two threads: both always finish.
    let forward = vec![folder.clone(), host.clone(), slot.clone()];
    let backward = vec![slot, host, folder];
    let run = |member: Member, wants: Vec<LeaseRequest>| {
        std::thread::spawn(move || {
            for _ in 0..25 {
                loop {
                    match member.acquire(&wants, 60_000) {
                        Ok(_) => break,
                        Err(lease::Refusal::Held { .. }) => std::thread::yield_now(),
                        Err(other) => panic!("{other}"),
                    }
                }
                for lease in member.leases() {
                    member.release(&lease.resource).unwrap();
                }
            }
        })
    };
    let c = channel.join("solver", "C").unwrap();
    let d = channel.join("solver", "D").unwrap();
    let (one, two) = (run(c, forward), run(d, backward));
    one.join().unwrap();
    two.join().unwrap();
    assert!(channel.read_state().unwrap().leases.is_empty());
}

#[tokio::test]
async fn a_member_waiting_on_a_lease_asks_the_holder_once_and_takes_it_when_free() {
    let (_dir, channel) = channel();
    let a = channel.join("solver", "A").unwrap();
    let b = channel.join("solver", "B").unwrap();
    let host = [LeaseRequest::new(LeaseKind::BuildHost, Some("coderos"))];
    a.acquire(&host, 60_000).unwrap();
    let releaser = {
        let a = a.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(800)).await;
            a.release("build-host:coderos").unwrap();
        })
    };
    let taken = b
        .acquire_waiting(&host, 60_000, &AtomicBool::new(false))
        .await
        .unwrap();
    releaser.await.unwrap();
    assert_eq!(taken[0].holder, "B");
    let asks: Vec<Record> = channel
        .records(0)
        .0
        .into_iter()
        .filter(|r| r.kind == Kind::Message && r.to == "A")
        .collect();
    assert_eq!(asks.len(), 1);
    assert!(asks[0].text.contains("When will you be done"));
}

#[test]
fn negotiation_outcomes_are_recorded_with_who_decided() {
    let (_dir, channel) = channel();
    let a = channel.join("codex", "Codex").unwrap();
    let b = channel.join("cursor", "Cursor").unwrap();
    a.intent(&strings(&["crates/x/src/a.rs"]), &[], &[], "")
        .unwrap();
    b.intent(&strings(&["crates/x/src/a.rs"]), &[], &[], "")
        .unwrap();
    let agreed = b
        .agree(
            "Codex",
            Outcome::First,
            Some("Codex"),
            &[],
            "Mine is smaller.",
        )
        .unwrap();
    assert_eq!(agreed.order, ["Codex", "Cursor"]);
    assert_eq!(
        agreed.on,
        ["crates/x/src/a.rs"],
        "what they overlap on, by default"
    );
    assert_eq!(agreed.decided_by, "Cursor");
    assert_eq!(
        agreed.sentence(),
        "Codex goes first on crates/x/src/a.rs; Cursor rebases on its branch after it lands. Mine is smaller."
    );
    // A later agreement for the same pair replaces it.
    let split = a
        .agree(
            "Cursor",
            Outcome::Split,
            None,
            &strings(&["crates/x/src/a.rs"]),
            "I take parse(), you take render().",
        )
        .unwrap();
    let state = channel.read_state().unwrap();
    assert_eq!(state.agreements, vec![split]);
    let records: Vec<Record> = channel
        .records(0)
        .0
        .into_iter()
        .filter(|r| r.kind == Kind::Agreement)
        .collect();
    assert_eq!(records.len(), 2, "both are in the log");
    assert!(records[1].text.contains("split the change"));
    // The merge outcome, and the swarm's rule for an unsettled pair.
    assert!(
        a.agree("Codex", Outcome::Merge, None, &[], "").is_err(),
        "with itself"
    );
    let c = channel.join("solver", "Solver #1").unwrap();
    let d = channel.join("solver", "Solver #2").unwrap();
    c.intent(&strings(&["docs/a.md"]), &[], &[], "").unwrap();
    let overlap = d
        .intent(&strings(&["docs/a.md"]), &[], &[], "")
        .unwrap()
        .remove(0);
    let ruled = channel
        .settle_by_order(&overlap, "Solver swarm")
        .unwrap()
        .unwrap();
    assert_eq!(
        ruled.order,
        ["Solver #1", "Solver #2"],
        "the earlier intent goes first"
    );
    assert_eq!(ruled.decided_by, "Solver swarm");
    assert!(
        channel
            .settle_by_order(&overlap, "Solver swarm")
            .unwrap()
            .is_none(),
        "once"
    );
    assert!(
        d.notes()
            .contains("Solver #1 goes first on docs/a.md; Solver #2 rebases")
    );
}

#[test]
fn the_cli_speaks_for_an_agent_that_runs_as_its_own_process() {
    let (_dir, channel) = channel();
    let dir = channel.dir().display().to_string();
    let cli = |words: &[&str]| {
        let mut args = vec!["--channel".to_owned(), dir.clone()];
        args.extend(words.iter().map(|word| (*word).to_owned()));
        cli::run(&args)
    };
    assert!(
        cli(&["--as", "Codex", "intent", "--file", "docs/a.md"])
            .unwrap()
            .contains("No one else")
    );
    let said = cli(&[
        "--as",
        "OpenCode",
        "intent",
        "docs/a.md",
        "--crate",
        "coder-new",
    ])
    .unwrap();
    assert!(
        said.contains("OpenCode and Codex overlap on docs/a.md"),
        "{said}"
    );
    assert!(
        cli(&[
            "--as",
            "OpenCode",
            "agree",
            "--with",
            "Codex",
            "--outcome",
            "first",
            "--first",
            "Codex"
        ])
        .unwrap()
        .contains("Codex goes first")
    );
    assert!(
        cli(&[
            "--as", "Codex", "post", "--to", "OpenCode", "Pushed;", "rebase", "now."
        ])
        .unwrap()
        .contains("Posted to OpenCode")
    );
    assert!(
        cli(&["--as", "OpenCode", "read"])
            .unwrap()
            .contains("Codex to OpenCode: Pushed; rebase now.")
    );
    let took = cli(&[
        "--as",
        "Codex",
        "lease",
        "take",
        "port",
        "migration",
        "--ttl",
        "120",
    ])
    .unwrap();
    assert!(
        took.contains("Holding migration:0001 for 120 seconds"),
        "{took}"
    );
    assert!(
        cli(&["--as", "OpenCode", "lease", "take", "migration", "0001"])
            .unwrap_err()
            .contains("Codex holds migration:0001")
    );
    assert!(
        cli(&["--as", "Codex", "lease", "list"])
            .unwrap()
            .contains("held by Codex")
    );
    assert!(
        cli(&["--as", "Codex", "lease", "release", "migration:0001"])
            .unwrap()
            .contains("Released")
    );
    assert!(cli(&["members"]).unwrap().contains("OpenCode (agent)"));
    assert!(cli(&["--help"]).unwrap().starts_with("usage: coder comms"));
}

#[test]
fn an_engine_is_told_who_it_is_and_how_to_reach_the_channel() {
    let (_dir, channel) = channel();
    let member = channel.join("codex", "Codex").unwrap();
    let preamble = member.preamble();
    assert!(preamble.starts_with("You are Codex, one of several agents"));
    for command in [
        "intent --file",
        " read`",
        "post [--to NAME]",
        "agree --with",
        "lease take build-folder",
    ] {
        assert!(preamble.contains(command), "{command} in {preamble}");
    }
    assert!(preamble.contains(&channel.dir().display().to_string()));
    assert!(preamble.ends_with("Your task:\n"));
}

/// Off: no channel tool, no channel at all, and a delegation joins
/// nothing; the choice is saved. On: the tool is offered and a delegation
/// joins and finishes even when it fails at once.
#[tokio::test]
async fn the_plugin_off_means_no_channel_and_isolated_delegations() {
    let state = tempfile::tempdir().unwrap();
    let store = crate::plugin_store::Store::under(state.path());
    let mut bundled = crate::bundled_settings::BundledSettings::default();
    bundled.load(store.clone()).unwrap();
    bundled.set_live(true);
    assert!(bundled.enabled(PLUGIN), "on by default");
    assert!(bundled.enabled(swarm::PLUGIN), "on by default");
    assert!(bundled.toggle(PLUGIN));
    let mut restarted = crate::bundled_settings::BundledSettings::default();
    restarted.load(store).unwrap();
    restarted.set_live(true);
    assert!(!restarted.enabled(PLUGIN), "the choice is saved");

    let mut plugins = crate::plugins::Plugins::default();
    plugins.bundled.comms = false;
    let mut execution = plugins.execution_settings(std::env::temp_dir());
    execution.memory = None;
    assert!(execution.comms.is_none());
    let names: Vec<Value> = execution
        .defs()
        .iter()
        .map(|tool| tool["function"]["name"].clone())
        .collect();
    assert!(!names.contains(&json!(tools::TOOL)));
    assert!(!execution.instructions().contains("share a channel"));
    let refused = execution
        .execute(
            tools::TOOL,
            json!({"action":"read"}),
            None,
            &Arc::new(AtomicBool::new(false)),
            &mut |_| {},
        )
        .await;
    assert_eq!(
        refused.unwrap_err(),
        "This tool is unknown or its plugin is turned off."
    );

    // On, with a channel of its own: offered, and a delegation joins it.
    let (_dir, channel) = channel();
    execution.comms = Some(channel.clone());
    execution.acp = true;
    execution.agents.clear();
    let names: Vec<Value> = execution
        .defs()
        .iter()
        .map(|tool| tool["function"]["name"].clone())
        .collect();
    assert!(names.contains(&json!(tools::TOOL)));
    assert!(execution.instructions().contains("share a channel"));
    let failed = execution
        .execute(
            "acp_subagent",
            json!({"agent":"codex","task":"Fix it."}),
            None,
            &Arc::new(AtomicBool::new(false)),
            &mut |_| {},
        )
        .await;
    assert!(failed.is_err(), "no such agent here");
    let state = channel.read_state().unwrap();
    assert_eq!(state.members["codex"].finished.as_deref(), Some("failed"));
    let read = execution
        .execute(
            tools::TOOL,
            json!({"action":"post","text":"Status?"}),
            None,
            &Arc::new(AtomicBool::new(false)),
            &mut |_| {},
        )
        .await
        .unwrap();
    assert_eq!(read["to"], "all");

    // Off again: the same delegation leaves the channel untouched.
    execution.comms = None;
    let before = channel.records(0).1;
    let _ = execution
        .execute(
            "acp_subagent",
            json!({"agent":"codex","task":"Fix it."}),
            None,
            &Arc::new(AtomicBool::new(false)),
            &mut |_| {},
        )
        .await;
    assert_eq!(channel.records(0).1, before);
}

/// A recorded solver run, rewritten as another issue that changes `file`.
fn recording(dir: &Path, issue: u64, file: &str) -> PathBuf {
    let source =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/solver-run-11338.jsonl");
    let text = std::fs::read_to_string(source)
        .unwrap()
        .replace("11338", &issue.to_string())
        .replace("docs/verse/zones.md", file);
    let path = dir.join(format!("run-{issue}.jsonl"));
    std::fs::write(&path, text).unwrap();
    path
}

fn plan(dir: &Path, issue: u64, file: &str, speed: f64) -> swarm::Plan {
    let arguments = json!({"issue": issue, "no_pr": true});
    let mut options = crate::solver_plugin::options(&arguments, Path::new("/")).unwrap();
    options.inner.replay = Some(recording(dir, issue, file));
    options.inner.speed = speed;
    swarm::Plan {
        issue,
        options,
        arguments,
    }
}

/// Three replayed solver runs, two of which change the same file: each
/// posts its intent from its run, the overlap is found and both are told,
/// an order is agreed and recorded, and the summary gives the merge order.
#[tokio::test]
async fn a_swarm_of_three_replayed_solvers_settles_the_overlap_between_two() {
    let (dir, channel) = channel();
    // #11401 plays a little faster, so its intent comes first.
    let plans = vec![
        plan(dir.path(), 11401, "docs/verse/zones.md", 400.0),
        plan(dir.path(), 11402, "docs/verse/zones.md", 200.0),
        plan(dir.path(), 11403, "docs/coder/other.md", 300.0),
    ];
    let limits = swarm::Limits {
        concurrency: 3,
        budget_usd: None,
    };
    let mut events = Vec::new();
    let result = swarm::run(
        plans,
        limits,
        Some(&channel),
        &AtomicBool::new(false),
        &mut |event| events.push(event),
    )
    .await
    .unwrap();

    // Every solver passed and each posted its intent from its plan.
    let rows = result["members"].as_array().unwrap();
    assert_eq!(rows.len(), 3);
    assert!(
        rows.iter().all(|row| row["outcome"] == "passed"),
        "{rows:?}"
    );
    assert!(rows.iter().all(|row| row["stage"] == "Done"));
    let state = channel.read_state().unwrap();
    assert_eq!(
        state.intents["Solver #11401"].files,
        ["docs/verse/zones.md"]
    );
    assert_eq!(
        state.intents["Solver #11402"].files,
        ["docs/verse/zones.md"]
    );
    assert_eq!(
        state.intents["Solver #11403"].files,
        ["docs/coder/other.md"]
    );
    // One overlap, between the two, told to both; the third overlaps no one.
    let records = channel.records(0).0;
    let overlaps: Vec<&Record> = records.iter().filter(|r| r.kind == Kind::Overlap).collect();
    assert_eq!(overlaps.len(), 1, "{overlaps:?}");
    assert!(overlaps[0].text.contains("overlap on docs/verse/zones.md"));
    assert!(overlaps[0].concerns("Solver #11401") && overlaps[0].concerns("Solver #11402"));
    assert!(!overlaps[0].concerns("Solver #11403"));
    // The order agreed and recorded: the earlier intent first, the other
    // rebases on its branch.
    assert_eq!(state.agreements.len(), 1);
    let agreement = &state.agreements[0];
    assert_eq!(agreement.order, ["Solver #11401", "Solver #11402"]);
    assert_eq!(agreement.outcome, Outcome::First);
    assert_eq!(agreement.on, ["docs/verse/zones.md"]);
    assert_eq!(agreement.decided_by, swarm::NAME);
    assert!(records.iter().any(|r| r.kind == Kind::Agreement
        && r.text.starts_with(
            "Solver #11401 goes first on docs/verse/zones.md; Solver #11402 rebases on its branch"
        )));
    assert_eq!(result["merge_order"][0], "Solver #11401");
    assert!(
        result["merge_order"]
            .as_array()
            .unwrap()
            .iter()
            .position(|n| n == "Solver #11401")
            < result["merge_order"]
                .as_array()
                .unwrap()
                .iter()
                .position(|n| n == "Solver #11402")
    );
    let reply = result["reply"].as_str().unwrap();
    assert!(reply.starts_with("3 of 3 solvers passed."), "{reply}");
    assert!(reply.contains("1 overlap settled"));
    assert!(reply.contains("Merge in this order: Solver #11401"));
    assert!(
        reply.contains("Total cost: $0.32"),
        "three runs at $0.107: {reply}"
    );
    // Every member finished, so no lease or overlap outlives the swarm.
    assert!(
        state
            .members
            .values()
            .all(|member| member.finished.as_deref() == Some("passed"))
    );

    // The chats: each solver is a child chat that shows the channel; the
    // swarm's own chat holds the swarm view.
    let mut app = crate::App::default();
    let task = swarm::task(&json!({"issues":[11401,11402,11403],"no_pr":true})).unwrap();
    for event in events {
        app.apply_delegation("call-1".into(), swarm::NAME.into(), task.clone(), event);
    }
    let child = |name: &str| {
        app.delegations
            .iter()
            .find(|child| child.name == name)
            .unwrap_or_else(|| panic!("{name}"))
    };
    for name in ["Solver #11401", "Solver #11402"] {
        let shown = child(name).chat.entries.iter().any(|entry| matches!(entry,
            Entry::Tool { name, output, .. } if name == view::CHANNEL && output["text"].as_str().unwrap_or_default().contains("overlap on docs/verse/zones.md")));
        assert!(shown, "{name}'s chat shows the overlap");
        assert!(!child(name).running, "{name} ended");
    }
    let third = child("Solver #11403").chat.entries.iter().any(|entry| {
        matches!(entry,
        Entry::Tool { name, input, .. } if name == view::CHANNEL && input["kind"] == "overlap")
    });
    assert!(!third, "the third is not told about an overlap it isn't in");
    let swarm_view = child(swarm::NAME)
        .chat
        .entries
        .iter()
        .find_map(|entry| match entry {
            Entry::Tool {
                name,
                output,
                running,
                ..
            } if name == view::SWARM => Some((output.clone(), *running)),
            _ => None,
        });
    let (output, running) = swarm_view.expect("the swarm view");
    assert!(!running);
    let rows: Vec<String> = view::swarm_lines(&output, false, 120, 0)
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect()
        })
        .collect();
    assert!(
        rows[0].contains("Solver swarm") && rows[0].contains("3 solvers"),
        "{rows:?}"
    );
    assert!(
        rows.iter().any(|row| row.contains("Solver #11402")
            && row.contains("Done")
            && row.contains("$0.11")
            && row.contains("passed")),
        "{rows:?}"
    );
    let shown = rows
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        shown.contains("Merge in this order: Solver #11401, then Solver #11402"),
        "{shown}"
    );
}

/// The person's limits hold: one at a time, and once the spend limit is
/// reached no more solvers start.
#[tokio::test]
async fn a_swarm_keeps_to_the_concurrency_and_spend_limits_set() {
    let dir = tempfile::tempdir().unwrap();
    let plans = vec![
        plan(dir.path(), 11501, "docs/a.md", 400.0),
        plan(dir.path(), 11502, "docs/b.md", 400.0),
        plan(dir.path(), 11503, "docs/c.md", 400.0),
    ];
    let limits = swarm::Limits {
        concurrency: 1,
        budget_usd: Some(0.15),
    };
    let mut most = 0;
    let mut running = std::collections::BTreeSet::new();
    let result = swarm::run(plans, limits, None, &AtomicBool::new(false), &mut |event| {
        if let RuntimeEvent::Delegation { id, event, .. } = event {
            if let RuntimeEvent::Tool {
                name,
                running: busy,
                ..
            } = *event
            {
                if name == crate::solver_plugin::TOOL {
                    if busy {
                        running.insert(id);
                    } else {
                        running.remove(&id);
                    }
                    most = most.max(running.len());
                }
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(most, 1, "one at a time");
    let outcomes: Vec<&str> = result["members"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["outcome"].as_str().unwrap())
        .collect();
    assert_eq!(
        outcomes,
        ["passed", "passed", "not_started"],
        "two runs spend $0.21, over the $0.15 limit"
    );
    assert!(
        result["reply"]
            .as_str()
            .unwrap()
            .contains("Solver #11503 not started: The spend limit was reached.")
    );
    assert_eq!(swarm::default_concurrency(Path::new("/"), 1), 1);
    assert!(swarm::arguments(&json!({"issues":[]})).is_err());
    assert!(
        swarm::arguments(&json!({"issues":["#1"],"steps":3})).is_err(),
        "no step limits"
    );
    let args = swarm::arguments(
        &json!({"issues":["#1", 2, "acme/app#3", "#1"], "concurrency": 2, "budget_usd": 5}),
    )
    .unwrap();
    assert_eq!(args.issues.len(), 3, "a repeat is dropped");
    assert_eq!((args.concurrency, args.budget_usd), (Some(2), Some(5.0)));
}

#[test]
fn a_solver_run_posts_its_plan_then_what_it_edited() {
    use crate::issue_run::Event;
    let mut watch = watch::SolverWatch::default();
    let set = |entry: Entry| Event::Set { index: 0, entry };
    let decision = |title: &str| Entry::Tool {
        name: crate::issue_run::DECISION.into(),
        input: json!({"kind":"agent","title":title}),
        output: Value::Null,
        running: true,
    };
    assert_eq!(
        watch.see(&set(decision("Briefer"))),
        [watch::Change::Stage("Planning")]
    );
    watch.see(&set(Entry::Assistant {
        text: "## Files to change\n\n- `crates/a/src/lib.rs`: add it.\n- `docs/a.md` too\n\n## Checks\n- `cargo test`".into(),
        model: None,
        elapsed_ms: None,
    }));
    let changes = watch.see(&set(decision("Coder")));
    assert_eq!(
        changes[0],
        watch::Change::Intent {
            files: strings(&["crates/a/src/lib.rs", "docs/a.md"]),
            note: "From its plan."
        }
    );
    watch.see(&set(Entry::Tool {
        name: "Edit".into(),
        input: json!({"path":"/x/worktree/crates/a/src/main.rs"}),
        output: json!({}),
        running: false,
    }));
    let changes = watch.see(&set(decision("Tester")));
    assert_eq!(
        changes[0],
        watch::Change::Intent {
            files: strings(&["crates/a/src/main.rs"]),
            note: "These are the files it changed so far."
        }
    );
    assert_eq!(watch.stage(), "Testing");
}
