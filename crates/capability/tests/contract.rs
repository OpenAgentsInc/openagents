//! The A17 contract as acceptance tests: discovery is inert, an
//! executable probe runs only under a host-owned approval that names
//! exact bytes, and a probe that cannot answer cleanly is `unknown`,
//! never `present`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use capability::{Decision, Presence, Proof, Registry, Source, SourceDir, Trust};
use serde_json::{Value, json};

/// A manifest body for `slug`, with the fields a test overrides merged
/// in. `binary` and `version` describe `detect`.
fn manifest(slug: &str, binary: &str, version: Value, extra: Value) -> Value {
    let mut body = json!({
        "v": 1,
        "slug": slug,
        "name": slug,
        "summary": "a contract-test capability",
        "transport": "subprocess",
        "detect": { "binary": binary, "version": version },
        "enforces": ["max_turns"],
        "cannot_enforce": [],
        "sees_repository": true,
        "cost": "local",
        "invoke": [binary, "--"],
    });
    merge(&mut body, &extra);
    body
}

/// `extra`'s keys land on `body`, shallowly.
fn merge(body: &mut Value, extra: &Value) {
    if let (Some(body), Some(extra)) = (body.as_object_mut(), extra.as_object()) {
        for (key, value) in extra {
            body.insert(key.clone(), value.clone());
        }
    }
}

/// A repository-shaped directory holding `capabilities/<slug>.json`,
/// with the manifest's file bytes on disk.
fn repository(slug: &str, body: &Value) -> tempfile::TempDir {
    let repository = tempfile::tempdir().unwrap();
    let capabilities = repository.path().join("capabilities");
    std::fs::create_dir_all(&capabilities).unwrap();
    std::fs::write(
        capabilities.join(format!("{slug}.json")),
        serde_json::to_vec_pretty(body).unwrap(),
    )
    .unwrap();
    repository
}

/// The single entry `repository` declares, read without running it.
fn entry(repository: &Path, slug: &str) -> capability::Entry {
    Registry::open(&[SourceDir::repository(repository.join("capabilities"))])
        .entry(slug)
        .unwrap_or_else(|| panic!("{slug} loaded"))
        .clone()
}

/// A trust over a store in `outside` — a directory that is not the
/// repository and not the workspace.
fn trust_in(outside: &Path) -> Trust {
    Trust::load(&outside.join("capability-trust.json")).unwrap()
}

/// An executable file whose bytes the test controls — an adapter a
/// manifest can name by absolute path.
fn adapter(dir: &Path, body: &str) -> PathBuf {
    let path = dir.join("contract-adapter");
    std::fs::write(&path, body).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    path
}

/// The checked-in manifest is the schema's proof: it parses under v1
/// rules, fields and all.
#[test]
fn the_checked_in_manifest_parses() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../capabilities/devin-local.json");
    let manifest = capability::Manifest::load(&path).expect("devin-local parses");

    assert_eq!(manifest.slug, "devin-local");
    assert_eq!(manifest.transport, "subprocess");
    assert_eq!(manifest.detect.binary, "devin");
    assert!(!manifest.summary.is_empty());
    assert_eq!(manifest.cost, "operator_account");
    assert_eq!(manifest.concurrent_max, Some(6));
    let probe = manifest.workspace_probe.expect("a workspace probe");
    assert!(
        probe
            .accepts
            .iter()
            .any(|word| word == "openagents-capability-probe"),
        "a probe that exits non-zero by design declares its acceptance word"
    );
    assert_eq!(
        manifest.refuses[0].matches, "Refusing to run in an untrusted workspace",
        "the v1 refusal text is one string"
    );
}

/// Reading a registry never runs a manifest's argv — the hostile
/// manifest below writes a marker if it ever executes.
#[test]
fn reading_a_registry_runs_nothing() {
    let outside = tempfile::tempdir().unwrap();
    let marker = outside.path().join("it-ran");
    let body = manifest(
        "contract-side-effect",
        "sh",
        json!([
            "sh",
            "-c",
            format!("touch {}; echo side 1.0.0", marker.display())
        ]),
        json!({}),
    );
    let repository = repository("contract-side-effect", &body);

    let registry = Registry::open(&[SourceDir::repository(
        repository.path().join("capabilities"),
    )]);
    assert!(registry.entry("contract-side-effect").is_some());
    assert!(!marker.exists(), "opening a registry is inert: nothing ran");
}

/// An approval is the difference between a declared capability and a
/// probed one: the same manifest is `unprobed` under an empty store.
#[test]
fn an_unapproved_manifest_stays_inert() {
    let outside = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let marker = outside.path().join("it-ran");
    let body = manifest(
        "contract-side-effect",
        "sh",
        json!([
            "sh",
            "-c",
            format!("touch {}; echo side 1.0.0", marker.display())
        ]),
        json!({}),
    );
    let repository = repository("contract-side-effect", &body);
    let entry = entry(repository.path(), "contract-side-effect");
    let trust = trust_in(outside.path());

    let found = entry.probe(workspace.path(), &trust);
    assert!(
        matches!(found.presence, Presence::Unprobed { .. }),
        "unapproved is unprobed: {}",
        found.output()
    );
    assert!(matches!(found.proof, Proof::None));
    assert!(!marker.exists(), "no argv ran for an unprobed manifest");

    match trust.decide(&entry, workspace.path()) {
        Decision::Unapproved(reason) => {
            assert!(reason.contains("capability-trust approve"), "{reason}")
        }
        other => panic!("an unrecorded manifest is unapproved: {other:?}"),
    }
}

/// The operator's own path: `approve` resolves the manifest, pins the
/// manifest digest and the adapter, writes the record, and the probe
/// then runs.
#[test]
fn an_approved_manifest_runs_its_probe() {
    let outside = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let marker = outside.path().join("it-ran");
    let body = manifest(
        "contract-side-effect",
        "sh",
        json!([
            "sh",
            "-c",
            format!("touch {}; echo side 1.0.0", marker.display())
        ]),
        json!({}),
    );
    let repository = repository("contract-side-effect", &body);

    let mut trust = trust_in(outside.path());
    let approval = trust
        .approve(Some(repository.path()), "contract-side-effect", &[])
        .expect("the approval is recorded");
    assert_eq!(approval.source, Source::Repository);

    let entry = entry(repository.path(), "contract-side-effect");
    let found = entry.probe(workspace.path(), &trust);
    assert!(
        matches!(found.presence, Presence::Present { .. }),
        "approved and probed: {}",
        found.output()
    );
    assert!(marker.exists(), "the approved argv ran");
    match &found.proof {
        Proof::Approved { digest, .. } => assert_eq!(digest, &entry.digest),
        other => panic!("an approved probe names its proof: {other:?}"),
    }
    let call = found.call();
    assert_eq!(call.extra["trusted_by"], "approved");
    assert_eq!(call.extra["manifest_source"], "repository");
}

/// The approval names the manifest's bytes, not its path: rewrite the
/// file and the record stops matching.
#[test]
fn a_changed_manifest_loses_its_approval() {
    let outside = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let marker = outside.path().join("it-ran");
    let body = manifest(
        "contract-side-effect",
        "sh",
        json!([
            "sh",
            "-c",
            format!("touch {}; echo side 1.0.0", marker.display())
        ]),
        json!({}),
    );
    let repository = repository("contract-side-effect", &body);
    let mut trust = trust_in(outside.path());
    trust
        .approve(Some(repository.path()), "contract-side-effect", &[])
        .unwrap();

    let mut changed = body.clone();
    merge(
        &mut changed,
        &json!({"summary": "rewritten under the approval's feet"}),
    );
    let path = repository
        .path()
        .join("capabilities")
        .join("contract-side-effect.json");
    std::fs::write(&path, serde_json::to_vec_pretty(&changed).unwrap()).unwrap();

    let entry = entry(repository.path(), "contract-side-effect");
    let found = entry.probe(workspace.path(), &trust);
    assert!(
        matches!(found.presence, Presence::Unprobed { .. }),
        "a changed manifest approves nothing: {}",
        found.output()
    );
    assert!(!marker.exists());
}

/// The approval pins the adapter by path and content: a binary that
/// changes under an unchanged manifest invalidates the record.
#[test]
fn a_changed_adapter_loses_its_approval() {
    let outside = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let binary = adapter(outside.path(), "#!/bin/sh\necho adapter 1.0.0\n");
    let body = manifest(
        "contract-adapter",
        &binary.display().to_string(),
        json!(["contract-adapter", "--version"]),
        json!({}),
    );
    let repository = repository("contract-adapter", &body);
    let mut trust = trust_in(outside.path());
    trust
        .approve(Some(repository.path()), "contract-adapter", &[])
        .unwrap();
    let entry = entry(repository.path(), "contract-adapter");
    assert!(matches!(
        entry.probe(workspace.path(), &trust).presence,
        Presence::Present { .. }
    ));

    adapter(outside.path(), "#!/bin/sh\necho adapter 9.9.9\n");

    let found = entry.probe(workspace.path(), &trust);
    match &found.presence {
        Presence::Unprobed { reason } => {
            assert!(reason.contains("changed on disk"), "{reason}")
        }
        _ => panic!("a changed adapter approves nothing: {}", found.output()),
    }
}

/// A script the manifest's argv interprets is part of the adapter: an
/// approval pins it too, and editing it invalidates the record while
/// the manifest and binary stay the same.
#[test]
fn a_changed_argv_script_loses_its_approval() {
    let outside = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let repository_dir = tempfile::tempdir().unwrap();
    let script = repository_dir.path().join("probe.sh");
    std::fs::write(&script, "echo scripted 1.0.0\n").unwrap();
    let body = manifest(
        "contract-scripted",
        "sh",
        json!(["sh", script.display().to_string()]),
        json!({}),
    );
    let repository = repository("contract-scripted", &body);
    let mut trust = trust_in(outside.path());
    let approval = trust
        .approve(Some(repository.path()), "contract-scripted", &[])
        .unwrap();
    assert!(
        approval
            .pinned
            .iter()
            .any(|pinned| { pinned.path == script.canonicalize().unwrap() }),
        "the argv's script is pinned: {:?}",
        approval.pinned
    );
    let entry = entry(repository.path(), "contract-scripted");
    assert!(matches!(
        entry.probe(workspace.path(), &trust).presence,
        Presence::Present { .. }
    ));

    std::fs::write(&script, "echo scripted 9.9.9\n").unwrap();

    let found = entry.probe(workspace.path(), &trust);
    assert!(
        matches!(found.presence, Presence::Unprobed { .. }),
        "a changed argv script approves nothing: {}",
        found.output()
    );
}

/// A store inside the workspace it would approve is the workspace's own
/// word for itself: `approve` refuses to write it, and a record copied
/// there decides nothing.
#[test]
fn a_store_inside_the_workspace_approves_nothing() {
    let outside = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let body = manifest(
        "contract-side-effect",
        "sh",
        json!(["sh", "-c", "echo side 1.0.0"]),
        json!({}),
    );
    let repository = repository("contract-side-effect", &body);

    let mut inside = trust_in(repository.path());
    assert!(
        inside
            .approve(Some(repository.path()), "contract-side-effect", &[])
            .is_err(),
        "approve refuses a store inside the repository"
    );

    // A record the repository copies into itself is still not the
    // operator's word.
    let mut outside_trust = trust_in(outside.path());
    outside_trust
        .approve(Some(repository.path()), "contract-side-effect", &[])
        .unwrap();
    let copied = workspace.path().join("capability-trust.json");
    std::fs::copy(outside.path().join("capability-trust.json"), &copied).unwrap();
    let copied_trust = Trust::load(&copied).unwrap();
    let entry = entry(repository.path(), "contract-side-effect");
    let found = entry.probe(workspace.path(), &copied_trust);
    assert!(
        matches!(found.presence, Presence::Unprobed { .. }),
        "a record inside the workspace approves nothing: {}",
        found.output()
    );
}

/// A relative `writable` is refused before any normalization, and a
/// missing path is refused rather than trusted unresolved.
#[test]
fn an_approval_names_real_paths() {
    let outside = tempfile::tempdir().unwrap();
    let body = manifest(
        "contract-side-effect",
        "sh",
        json!(["sh", "-c", "echo side 1.0.0"]),
        json!({}),
    );
    let repository = repository("contract-side-effect", &body);
    let mut trust = trust_in(outside.path());

    let relative = trust.approve(
        Some(repository.path()),
        "contract-side-effect",
        &[PathBuf::from("relative/dir")],
    );
    assert!(relative.unwrap_err().contains("not absolute"));

    let missing = trust.approve(
        Some(repository.path()),
        "contract-side-effect",
        &[outside.path().join("not-there")],
    );
    assert!(missing.is_err(), "a missing path is not trusted unresolved");

    assert!(
        trust
            .approve(Some(repository.path()), "no-such-slug", &[])
            .is_err()
    );
    assert!(
        trust
            .approve(Some(repository.path()), "Not a Slug!", &[])
            .is_err()
    );
}

/// A probe that hangs answers `unknown`, at the wall — never `present`.
#[test]
fn a_hung_probe_is_unknown() {
    let outside = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let body = manifest(
        "contract-hung",
        "sh",
        json!(["sh", "-c", "sleep 30"]),
        json!({}),
    );
    let repository = repository("contract-hung", &body);
    let mut trust = trust_in(outside.path());
    trust
        .approve(Some(repository.path()), "contract-hung", &[])
        .unwrap();

    let entry = entry(repository.path(), "contract-hung");
    let found = entry.probe_within(workspace.path(), &trust, Duration::from_millis(300));
    match &found.presence {
        Presence::Unknown { reason, .. } => assert!(reason.contains("no answer"), "{reason}"),
        other => panic!("a hung probe is unknown, not {other:?}"),
    }
    assert!(!found.available());
}

/// A version argv that exits wrong answers `unknown`: the executor is
/// there and did not prove itself.
#[test]
fn a_failed_probe_is_unknown() {
    let outside = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let body = manifest(
        "contract-failed",
        "sh",
        json!(["sh", "-c", "exit 7"]),
        json!({}),
    );
    let repository = repository("contract-failed", &body);
    let mut trust = trust_in(outside.path());
    trust
        .approve(Some(repository.path()), "contract-failed", &[])
        .unwrap();

    let entry = entry(repository.path(), "contract-failed");
    let found = entry.probe(workspace.path(), &trust);
    assert!(
        matches!(found.presence, Presence::Unknown { .. }),
        "a failed probe is unknown: {}",
        found.output()
    );
    assert!(!found.available());
}

/// An answer past the output bound is `unknown` — evidence the probe
/// could not read is not read as half an answer.
#[test]
fn a_noisy_probe_is_unknown() {
    let outside = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let body = manifest(
        "contract-noisy",
        "sh",
        json!(["sh", "-c", "head -c 200000 /dev/zero | tr '\\0' 'x'"]),
        json!({}),
    );
    let repository = repository("contract-noisy", &body);
    let mut trust = trust_in(outside.path());
    trust
        .approve(Some(repository.path()), "contract-noisy", &[])
        .unwrap();

    let entry = entry(repository.path(), "contract-noisy");
    let found = entry.probe(workspace.path(), &trust);
    match &found.presence {
        Presence::Unknown { reason, .. } => {
            assert!(reason.contains("output bound"), "{reason}")
        }
        other => panic!("a truncated answer is unknown, not {other:?}"),
    }
}

/// The workspace probe's declared refusal is `unavailable`: present,
/// and this workspace is one it will not work.
#[test]
fn a_declared_refusal_is_unavailable() {
    let outside = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let body = manifest(
        "contract-refusing",
        "sh",
        json!(["sh", "-c", "echo refusing 1.0.0"]),
        json!({
            "workspace_probe": {
                "argv": ["sh", "-c", "echo 'Refusing to run in an untrusted workspace' >&2; exit 1"],
            },
            "refuses": [{
                "name": "untrusted_workspace",
                "match": "Refusing to run in an untrusted workspace",
                "explanation": "the executor checks trust first",
            }],
        }),
    );
    let repository = repository("contract-refusing", &body);
    let mut trust = trust_in(outside.path());
    trust
        .approve(Some(repository.path()), "contract-refusing", &[])
        .unwrap();

    let entry = entry(repository.path(), "contract-refusing");
    let found = entry.probe(workspace.path(), &trust);
    match &found.presence {
        Presence::Unavailable { refusal, .. } => assert_eq!(refusal, "untrusted_workspace"),
        other => panic!("a declared refusal is unavailable, not {other:?}"),
    }
    assert!(!found.available());
}

/// The nonzero-by-design contract: a workspace probe that exits 1 on
/// every path still answers `present` when it names its declared
/// acceptance word — this is how the Devin manifest's invalid-model
/// probe proves acceptance without a session.
#[test]
fn a_nonzero_probe_naming_its_acceptance_is_present() {
    let outside = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let body = manifest(
        "contract-accepting",
        "sh",
        json!(["sh", "-c", "echo accepting 1.0.0"]),
        json!({
            "workspace_probe": {
                "argv": ["sh", "-c", "echo 'unknown model: openagents-capability-probe' >&2; exit 1"],
                "accepts": ["openagents-capability-probe"],
            },
        }),
    );
    let repository = repository("contract-accepting", &body);
    let mut trust = trust_in(outside.path());
    trust
        .approve(Some(repository.path()), "contract-accepting", &[])
        .unwrap();

    let entry = entry(repository.path(), "contract-accepting");
    let found = entry.probe(workspace.path(), &trust);
    assert!(
        matches!(found.presence, Presence::Present { .. }),
        "a declared acceptance word on a nonzero exit is present: {}",
        found.output()
    );
}

/// The same nonzero exit without a declared word is `unknown` — a
/// workspace probe's failure cannot manufacture availability.
#[test]
fn a_nonzero_probe_naming_nothing_is_unknown() {
    let outside = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let body = manifest(
        "contract-mute",
        "sh",
        json!(["sh", "-c", "echo mute 1.0.0"]),
        json!({
            "workspace_probe": {
                "argv": ["sh", "-c", "exit 2"],
                "accepts": ["openagents-capability-probe"],
            },
            "refuses": [{
                "name": "untrusted_workspace",
                "match": "Refusing to run in an untrusted workspace",
            }],
        }),
    );
    let repository = repository("contract-mute", &body);
    let mut trust = trust_in(outside.path());
    trust
        .approve(Some(repository.path()), "contract-mute", &[])
        .unwrap();

    let entry = entry(repository.path(), "contract-mute");
    let found = entry.probe(workspace.path(), &trust);
    assert!(
        matches!(found.presence, Presence::Unknown { .. }),
        "an undeclared exit is unknown, not a route: {}",
        found.output()
    );
}

/// A manifest for a transport this host does not run stays `unprobed`
/// even with an approval — the approval does not make an argv out of a
/// transport that is not one.
#[test]
fn a_transport_this_host_does_not_run_stays_unprobed() {
    let outside = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let body = manifest(
        "contract-relayed",
        "sh",
        json!(["sh", "-c", "echo relayed 1.0.0"]),
        json!({"transport": "http"}),
    );
    let repository = repository("contract-relayed", &body);
    let mut trust = trust_in(outside.path());
    trust
        .approve(Some(repository.path()), "contract-relayed", &[])
        .unwrap();

    let entry = entry(repository.path(), "contract-relayed");
    let found = entry.probe(workspace.path(), &trust);
    match &found.presence {
        Presence::Unprobed { reason } => assert!(reason.contains("transport"), "{reason}"),
        other => panic!("a non-argv transport is unprobed, not {other:?}"),
    }
}

/// A malformed manifest is refused at load, in the record — and an argv
/// whose head is not the manifest's binary is malformed.
#[test]
fn a_mismatched_argv_is_refused_at_load() {
    let repository = tempfile::tempdir().unwrap();
    let capabilities = repository.path().join("capabilities");
    std::fs::create_dir_all(&capabilities).unwrap();
    std::fs::write(
        capabilities.join("contract-mismatched.json"),
        serde_json::to_vec_pretty(&json!({
            "v": 1,
            "slug": "contract-mismatched",
            "transport": "subprocess",
            "detect": { "binary": "sh", "version": ["curl", "https://example.invalid"] },
        }))
        .unwrap(),
    )
    .unwrap();
    std::fs::write(capabilities.join("not-json.json"), b"{ not json").unwrap();

    let registry = Registry::open(&[SourceDir::repository(
        repository.path().join("capabilities"),
    )]);
    assert!(registry.entry("contract-mismatched").is_none());
    let refused = registry.refused();
    assert_eq!(refused.len(), 2);
    assert!(
        refused.iter().any(|(_, why)| why.contains("curl")),
        "{refused:?}"
    );
}

/// Probing from inside an asynchronous host: the bounded run's own
/// runtime lives on the probe's thread, so no runtime nests.
#[tokio::test(flavor = "multi_thread")]
async fn a_probe_runs_inside_an_async_host() {
    let outside = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let body = manifest(
        "contract-async",
        "sh",
        json!(["sh", "-c", "echo async 1.0.0"]),
        json!({}),
    );
    let repository = repository("contract-async", &body);
    let mut trust = trust_in(outside.path());
    trust
        .approve(Some(repository.path()), "contract-async", &[])
        .unwrap();

    let entry = entry(repository.path(), "contract-async");
    let found = tokio::task::spawn_blocking(move || entry.probe(workspace.path(), &trust))
        .await
        .unwrap();
    assert!(matches!(found.presence, Presence::Present { .. }));
}

/// The store's boundary is the manifest's own checkout too: a record a
/// repository copies into itself approves nothing, even when the probe
/// would run in a different directory — an isolated worktree does not
/// launder a repository's word for itself.
#[test]
fn a_store_inside_the_manifests_repository_approves_nothing() {
    let outside = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let body = manifest(
        "contract-side-effect",
        "sh",
        json!(["sh", "-c", "echo side 1.0.0"]),
        json!({}),
    );
    let repository = repository("contract-side-effect", &body);

    let mut outside_trust = trust_in(outside.path());
    outside_trust
        .approve(Some(repository.path()), "contract-side-effect", &[])
        .unwrap();
    let copied = repository.path().join("capability-trust.json");
    std::fs::copy(outside.path().join("capability-trust.json"), &copied).unwrap();

    let copied_trust = Trust::load(&copied).unwrap();
    let entry = entry(repository.path(), "contract-side-effect");
    let found = entry.probe(workspace.path(), &copied_trust);
    assert!(
        matches!(found.presence, Presence::Unprobed { .. }),
        "a record inside the manifest's repository approves nothing: {}",
        found.output()
    );
}

/// A relative argv word that names a file under the repository is
/// refused at approval: the word would resolve wherever the argv runs,
/// and pinning it here would bless a different file there.
#[test]
fn a_relative_script_is_refused_at_approval() {
    let outside = tempfile::tempdir().unwrap();
    let repository = repository(
        "contract-relative",
        &manifest(
            "contract-relative",
            "sh",
            json!(["sh", "probe.sh"]),
            json!({}),
        ),
    );
    std::fs::write(repository.path().join("probe.sh"), "echo rel 1.0.0\n").unwrap();

    let mut trust = trust_in(outside.path());
    let refused = trust.approve(Some(repository.path()), "contract-relative", &[]);
    assert!(
        refused.unwrap_err().contains("relative"),
        "a relative script is refused, not silently pinned"
    );
}

/// The same relative word left unpinned at approval cannot be picked up
/// later: a file that appears in the directory the argv runs in is not
/// the file nobody approved — it is nothing, and nothing is what runs.
#[test]
fn a_relative_script_appearing_in_the_workspace_approves_nothing() {
    let outside = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let body = manifest(
        "contract-relative",
        "sh",
        json!(["sh", "probe.sh"]),
        json!({}),
    );
    let repository = repository("contract-relative", &body);
    let mut trust = trust_in(outside.path());
    trust
        .approve(Some(repository.path()), "contract-relative", &[])
        .unwrap();

    let entry = entry(repository.path(), "contract-relative");
    assert!(
        matches!(
            entry.probe(workspace.path(), &trust).presence,
            Presence::Present { .. } | Presence::Unknown { .. } | Presence::Absent { .. }
        ),
        "no file yet — the word resolves to nothing and the argv fails on its own"
    );

    std::fs::write(workspace.path().join("probe.sh"), "echo planted 1.0.0\n").unwrap();
    let found = entry.probe(workspace.path(), &trust);
    assert!(
        matches!(found.presence, Presence::Unprobed { .. }),
        "a file dropped into the run directory approves nothing: {}",
        found.output()
    );
}

/// An absolute argv word that resolves to nothing is refused at
/// approval — a probe reaching for a file nobody verified is not
/// silently skipped.
#[test]
fn an_unresolvable_argv_word_is_refused_at_approval() {
    let outside = tempfile::tempdir().unwrap();
    let body = manifest(
        "contract-missing",
        "sh",
        json!(["sh", "/no/such/script-anywhere.sh"]),
        json!({}),
    );
    let repository = repository("contract-missing", &body);
    let mut trust = trust_in(outside.path());
    assert!(
        trust
            .approve(Some(repository.path()), "contract-missing", &[])
            .is_err(),
        "an argv file that does not resolve cannot be approved"
    );
}

#[test]
fn deleting_the_approval_store_revokes_cached_approval() {
    let outside = tempfile::tempdir().unwrap();
    let repository = repository(
        "cached",
        &manifest("cached", "sh", json!(["sh", "--version"]), json!({})),
    );
    let mut trust = trust_in(outside.path());
    trust
        .approve(Some(repository.path()), "cached", &[])
        .unwrap();
    let entry = entry(repository.path(), "cached");
    std::fs::remove_file(outside.path().join("capability-trust.json")).unwrap();
    assert!(matches!(
        trust.decide(&entry, repository.path()),
        Decision::Unapproved(_)
    ));
}

#[cfg(unix)]
#[test]
fn retargeting_an_argv_symlink_invalidates_approval() {
    let outside = tempfile::tempdir().unwrap();
    let first = outside.path().join("first.sh");
    let second = outside.path().join("second.sh");
    let alias = outside.path().join("probe.sh");
    std::fs::write(&first, "echo first").unwrap();
    std::fs::write(&second, "echo second").unwrap();
    std::os::unix::fs::symlink(&first, &alias).unwrap();
    let repository = repository(
        "aliased",
        &manifest("aliased", "sh", json!(["sh", alias]), json!({})),
    );
    let mut trust = trust_in(outside.path());
    trust
        .approve(Some(repository.path()), "aliased", &[])
        .unwrap();
    let entry = entry(repository.path(), "aliased");
    std::fs::remove_file(&alias).unwrap();
    std::os::unix::fs::symlink(&second, &alias).unwrap();
    assert!(matches!(
        trust.decide(&entry, repository.path()),
        Decision::Unapproved(_)
    ));
}

#[cfg(unix)]
#[test]
fn each_argv_alias_is_verified_even_when_targets_were_identical() {
    let outside = tempfile::tempdir().unwrap();
    let first = outside.path().join("first.sh");
    let changed = outside.path().join("changed.sh");
    let version = outside.path().join("version.sh");
    let invoke = outside.path().join("invoke.sh");
    std::fs::write(&first, "echo approved").unwrap();
    std::fs::write(&changed, "echo changed").unwrap();
    std::os::unix::fs::symlink(&first, &version).unwrap();
    std::os::unix::fs::symlink(&first, &invoke).unwrap();
    let repository = repository(
        "aliases",
        &manifest(
            "aliases",
            "sh",
            json!(["sh", version]),
            json!({"invoke": ["sh", invoke]}),
        ),
    );
    let mut trust = trust_in(outside.path());
    trust
        .approve(Some(repository.path()), "aliases", &[])
        .unwrap();
    let entry = entry(repository.path(), "aliases");
    std::fs::remove_file(&invoke).unwrap();
    std::os::unix::fs::symlink(&changed, &invoke).unwrap();
    assert!(matches!(
        trust.decide(&entry, repository.path()),
        Decision::Unapproved(_)
    ));
}

#[cfg(unix)]
#[test]
fn a_symlinked_manifest_cannot_move_its_repository_trust_boundary() {
    let outside = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let repository = repository(
        "linked",
        &manifest("linked", "sh", json!(["sh", "--version"]), json!({})),
    );
    let mut trust = trust_in(outside.path());
    trust
        .approve(Some(repository.path()), "linked", &[])
        .unwrap();
    let manifest_path = repository.path().join("capabilities/linked.json");
    std::fs::create_dir(outside.path().join("capabilities")).unwrap();
    let external = outside.path().join("capabilities/linked.json");
    std::fs::copy(&manifest_path, &external).unwrap();
    std::fs::remove_file(&manifest_path).unwrap();
    std::os::unix::fs::symlink(&external, &manifest_path).unwrap();
    let copied = repository.path().join("approval.json");
    std::fs::copy(outside.path().join("capability-trust.json"), &copied).unwrap();
    let trust = Trust::load(&copied).unwrap();
    let entry = entry(repository.path(), "linked");
    assert!(matches!(
        trust.decide(&entry, workspace.path()),
        Decision::Unapproved(_)
    ));
}
