//! Actual-process curated discovery uses explicit public fixtures and no grants.
#![cfg(unix)]

#[path = "../../discovery/tests/support/curated.rs"]
mod fixture;

use fixture::Fixture;
use serde_json::{Value, json};
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn write(path: &Path, bytes: impl AsRef<[u8]>) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}
fn mirror(f: &Fixture, root: &Path) {
    for (id, event) in &f.source.events {
        write(
            &root.join("events").join(format!("{id}.json")),
            serde_json::to_vec(event).unwrap(),
        );
    }
    for ((kind, key, slug), events) in &f.source.heads {
        write(
            &root
                .join("heads")
                .join(kind.to_string())
                .join(key)
                .join(format!("{slug}.json")),
            serde_json::to_vec(events).unwrap(),
        );
    }
    for (id, bytes) in &f.source.artifacts {
        write(
            &root
                .join("artifacts/sha256")
                .join(id.strip_prefix("sha256:").unwrap()),
            bytes,
        );
    }
}
fn run(home: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_openagents"))
        .args(args)
        .env_clear()
        .env("HOME", home)
        .env("OPENAGENTS_SETTINGS", home.join("settings.json"))
        .output()
        .unwrap()
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn owned_evaluation_lock(f: &mut Fixture, root: &Path) {
    // Resolve real native package pins and use the existing evaluation owner's
    // builder. No agent or guest runs while this acceptance source is created.
    let manifest = nostr::ext::parse_manifest(&f.manifest).unwrap();
    for file in &manifest.files {
        write(&root.join(&file.path), &f.source.artifacts[&file.digest]);
    }
    let record_bytes = std::fs::read(root.join("package.json")).unwrap();
    let package = coder::package::Package::load(&root.join("package.json")).unwrap();
    let package_lock = coder::package::Package::resolve(root, &package).unwrap();
    let program_bytes = serde_json::to_vec(&f.program).unwrap();
    let subject = ext_eval::arms::Subject {
        slug: package.slug.clone(),
        definition: ext_eval::arms::definition(
            &package.publisher,
            &package.slug,
            "explain-error",
            &record_bytes,
        ),
        package_lock: serde_json::to_value(package_lock).unwrap(),
        programs: vec![ext_eval::arms::Program {
            slug: "explain-error".into(),
            bytes: program_bytes,
        }],
        skills: vec![],
    };
    let agent = ext_eval::arms::AgentPin {
        path: root.join("unused-agent"),
        digest: nostr::contracts::digest_bytes(b"fixture agent"),
        size: 13,
        questions: vec![],
    };
    let lock = subject.lock_document(&agent);
    let digest = nostr::contracts::digest_bytes(&lock);
    f.source.artifacts.insert(digest, lock.clone());
    let body: Value = serde_json::from_str(&f.evaluation.content).unwrap();
    let mut report: Value =
        serde_json::from_str(body["meta"]["ext_eval_report"].as_str().unwrap()).unwrap();
    report["subject"]["lock"] = fixture::art(&lock, Some(ext_eval::arms::LOCK_SCHEMA));
    let parts = nostr::eval_ext::publication(&report.to_string(), None).unwrap();
    let event = fixture::signer("22").sign(f.now - 5, parts.kind, parts.tags, parts.content);
    f.catalog.items[0].evaluations = vec![event.id.clone()];
    f.catalog.items[0].review.as_mut().unwrap().evaluations = vec![event.id.clone()];
    f.source.events.insert(event.id.clone(), event);
}
fn inventory(root: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = vec![];
    fn collect(path: &Path, root: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        for entry in std::fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                collect(&path, root, out);
            } else {
                out.push((
                    path.strip_prefix(root).unwrap().display().to_string(),
                    std::fs::read(&path).unwrap(),
                ));
            }
        }
    }
    collect(root, root, &mut out);
    out.sort();
    out
}

#[test]
fn card_selection_is_a_read_only_native_inspection_with_exact_price_and_support() {
    let work = tempfile::tempdir().unwrap();
    let root = work.path().join("mirror");
    let home = work.path().join("home");
    std::fs::create_dir(&home).unwrap();
    let mut f = Fixture::new(now());
    owned_evaluation_lock(&mut f, &work.path().join("evaluation-source"));
    mirror(&f, &root);
    let catalog = work.path().join("catalog.json");
    write(&catalog, f.bytes());
    let before = inventory(work.path());
    let selected = format!("{}/explain-error", f.catalog.items[0].id);
    let output = run(
        &home,
        &[
            "--json",
            "plugin",
            "discover",
            "--catalog",
            catalog.to_str().unwrap(),
            "--mirror",
            root.to_str().unwrap(),
            "--select",
            &selected,
        ],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    let card = &report["selected"];
    assert_eq!(card["state"], "verified_discovery");
    assert_eq!(card["price"]["publisher_fee_msat"], 2000);
    assert_eq!(
        card["price"]["total_price"],
        "requires_separate_current_quote"
    );
    assert_eq!(
        card["operation"]["host_support"]["state"],
        "supported_packet_shape"
    );
    assert_eq!(card["operation"]["host_support"]["operation"], "explain");
    assert_eq!(card["operation"]["host_support"]["workspace_reads"], false);
    assert_eq!(card["review"]["state"], "current_scoped_review");
    assert_eq!(card["evaluation"][0]["state"], "signed_measurement");
    assert_eq!(card["purchase_authorized"], false);
    let plain = run(
        &home,
        &[
            "plugin",
            "discover",
            "--catalog",
            catalog.to_str().unwrap(),
            "--mirror",
            root.to_str().unwrap(),
            "--select",
            &selected,
        ],
    );
    assert!(plain.status.success());
    let text = String::from_utf8(plain.stdout).unwrap();
    assert!(text.contains("2000 msat publisher fee"));
    assert!(text.contains("Operation: explain"));
    assert!(text.contains(&f.release.id));
    assert_eq!(inventory(work.path()), before);
    assert!(!home.join("wallet").exists());
    assert!(!home.join("background").exists());
    let bad = run(
        &home,
        &[
            "--json",
            "plugin",
            "discover",
            "--catalog",
            catalog.to_str().unwrap(),
            "--mirror",
            root.to_str().unwrap(),
            "--select",
            "Demo error explanation",
        ],
    );
    assert!(!bad.status.success());
}

#[test]
fn retained_process_snapshot_prevents_withdrawal_replay_and_works_without_reputation() {
    let work = tempfile::tempdir().unwrap();
    let root = work.path().join("mirror");
    let home = work.path().join("home");
    std::fs::create_dir(&home).unwrap();
    let mut f = Fixture::new(now());
    f.catalog.items[0].reputation = Some(json!({"state":"unavailable","score":null}));
    mirror(&f, &root);
    let catalog = work.path().join("catalog.json");
    write(&catalog, f.bytes());
    let args = [
        "--json",
        "plugin",
        "discover",
        "--catalog",
        catalog.to_str().unwrap(),
        "--mirror",
        root.to_str().unwrap(),
    ];
    let first = run(&home, &args);
    assert!(first.status.success());
    let old = work.path().join("old.json");
    write(&old, &first.stdout);
    f.set_head(f.hidden());
    mirror(&f, &root);
    let mut next = args.to_vec();
    next.extend(["--previous", old.to_str().unwrap()]);
    let withdrawn = run(&home, &next);
    assert!(
        withdrawn.status.success(),
        "{}",
        String::from_utf8_lossy(&withdrawn.stderr)
    );
    let report: Value = serde_json::from_slice(&withdrawn.stdout).unwrap();
    assert_eq!(report["cards"][0]["state"], "withdrawn");
    assert_eq!(report["cards"][0]["purchase_authorized"], false);
    let retained = work.path().join("withdrawn.json");
    write(&retained, &withdrawn.stdout);
    f.set_head(f.listing.clone());
    mirror(&f, &root);
    next.pop();
    next.push(retained.to_str().unwrap());
    let replay = run(&home, &next);
    assert!(replay.status.success());
    let report: Value = serde_json::from_slice(&replay.stdout).unwrap();
    assert_eq!(report["cards"][0]["state"], "unavailable");
    assert!(
        report["cards"][0]["error"]
            .as_str()
            .unwrap()
            .contains("rolls back")
    );
}

#[test]
fn mirror_escape_and_nonregular_sources_refuse_without_disclosure() {
    let work = tempfile::tempdir().unwrap();
    let root = work.path().join("mirror");
    let home = work.path().join("home");
    std::fs::create_dir(&home).unwrap();
    let f = Fixture::new(now());
    mirror(&f, &root);
    let catalog = work.path().join("catalog.json");
    write(&catalog, f.bytes());
    let copied = work.path().join("outside");
    std::fs::rename(root.join("heads"), &copied).unwrap();
    std::os::unix::fs::symlink(&copied, root.join("heads")).unwrap();
    let args = [
        "--json",
        "plugin",
        "discover",
        "--catalog",
        catalog.to_str().unwrap(),
        "--mirror",
        root.to_str().unwrap(),
    ];
    let output = run(&home, &args);
    assert!(output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["cards"][0]["state"], "unavailable");
    let linked = work.path().join("linked.json");
    std::os::unix::fs::symlink(&catalog, &linked).unwrap();
    let output = run(
        &home,
        &[
            "--json",
            "plugin",
            "discover",
            "--catalog",
            linked.to_str().unwrap(),
            "--mirror",
            root.to_str().unwrap(),
        ],
    );
    assert!(!output.status.success());
    let fifo = work.path().join("fifo");
    let path = std::ffi::CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
    // SAFETY: the path is a fixture-owned terminated string, and mkfifo only
    // creates a private test object. The read command must not block on it.
    assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
    let output = run(
        &home,
        &[
            "--json",
            "plugin",
            "discover",
            "--catalog",
            fifo.to_str().unwrap(),
            "--mirror",
            root.to_str().unwrap(),
        ],
    );
    assert!(!output.status.success());
    assert!(inventory(&home).is_empty());
}
