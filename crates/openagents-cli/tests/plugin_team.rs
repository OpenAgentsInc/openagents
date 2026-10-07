//! Actual installed colleague reuse over distinct native credentials and bytes.
#![cfg(unix)]
#[path = "../../discovery/tests/support/curated.rs"]
mod fixture;
use fixture::Fixture;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use tenancy::accounts::team_capabilities as team;
use tenancy::accounts::{Accounts, Role, WorkspaceKind};

fn write(path: &Path, bytes: impl AsRef<[u8]>) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
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
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
fn reviewed_source(root: &Path) -> Fixture {
    let mut f = Fixture::new(now());
    let mut body: Value = serde_json::from_str(&f.release.content).unwrap();
    body["fee_msat"] = json!(0);
    f.release = fixture::sign(
        &f.publisher,
        f.now - 10,
        nostr::ext::RELEASE_KIND,
        "release",
        None,
        body,
    );
    f.source
        .events
        .insert(f.release.id.clone(), f.release.clone());
    let mut body: Value = serde_json::from_str(&f.listing.content).unwrap();
    body["release"] = fixture::pointer(&f.release);
    f.listing = fixture::sign(
        &f.publisher,
        f.now - 9,
        nostr::ext::LISTING_KIND,
        "listing",
        Some("demo"),
        body,
    );
    f.set_head(f.listing.clone());
    f.catalog.items[0].event = f.release.id.clone();
    let review = f.catalog.items[0].review.as_mut().unwrap();
    review.event = f.release.id.clone();
    review.publisher_fee_msat = Some(0);
    review.data_requirements = vec!["explicit-request-text".into()];
    review.recipients = vec!["local-wasm".into()];
    // Preserve the real evaluation owner's package-lock contract. Protected
    // grader artifacts stay in the signed source, never in the installed guest.
    let manifest = nostr::ext::parse_manifest(&f.manifest).unwrap();
    for file in &manifest.files {
        write(&root.join(&file.path), &f.source.artifacts[&file.digest]);
    }
    let record = fs::read(root.join("package.json")).unwrap();
    let package = coder::package::Package::load(&root.join("package.json")).unwrap();
    let lock = coder::package::Package::resolve(root, &package).unwrap();
    let subject = ext_eval::arms::Subject {
        slug: package.slug.clone(),
        definition: ext_eval::arms::definition(
            &package.publisher,
            &package.slug,
            "explain-error",
            &record,
        ),
        package_lock: serde_json::to_value(lock).unwrap(),
        programs: vec![ext_eval::arms::Program {
            slug: "explain-error".into(),
            bytes: serde_json::to_vec(&f.program).unwrap(),
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
    f.source
        .artifacts
        .insert(nostr::contracts::digest_bytes(&lock), lock.clone());
    let body: Value = serde_json::from_str(&f.evaluation.content).unwrap();
    let mut report: Value =
        serde_json::from_str(body["meta"]["ext_eval_report"].as_str().unwrap()).unwrap();
    report["subject"]["definition"]["event"] = fixture::pointer(&f.release);
    report["subject"]["lock"] = fixture::art(&lock, Some(ext_eval::arms::LOCK_SCHEMA));
    let parts = nostr::eval_ext::publication(&report.to_string(), None).unwrap();
    f.evaluation = fixture::signer("22").sign(f.now - 5, parts.kind, parts.tags, parts.content);
    f.source
        .events
        .insert(f.evaluation.id.clone(), f.evaluation.clone());
    f.catalog.items[0].evaluations = vec![f.evaluation.id.clone()];
    f.catalog.items[0].review.as_mut().unwrap().evaluations = vec![f.evaluation.id.clone()];
    f
}
struct Case {
    temp: tempfile::TempDir,
    accounts: Accounts,
    workspace: String,
    owner: String,
    member: String,
    member_key: String,
    source: Fixture,
}
impl Case {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        for name in ["registry", "home"] {
            fs::create_dir(root.join(name)).unwrap();
            fs::set_permissions(root.join(name), fs::Permissions::from_mode(0o700)).unwrap();
        }
        let registry = tenancy::Registry::install(
            &root.join("registry"),
            tenancy::Manifest {
                v: tenancy::SCHEMA.into(),
                sequence: 0,
                supersedes: None,
                shared: BTreeMap::new(),
                tenants: BTreeMap::from([(
                    "team".into(),
                    tenancy::Tenant {
                        credential: "key-ref:fixture".into(),
                        principals: vec![],
                        doors: BTreeMap::new(),
                        quota: None,
                    },
                )]),
                digest: String::new(),
            },
        )
        .unwrap();
        let accounts = Accounts::install(&root.join("registry")).unwrap();
        let issue = |actions: &[&str]| {
            tenancy::keys::issue_scoped(
                &root.join("registry"),
                registry.manifest(),
                "team",
                None,
                Some(tenancy::keys::Scopes {
                    models: None,
                    actions: Some(
                        actions
                            .iter()
                            .map(|s| s.to_string())
                            .collect::<BTreeSet<_>>(),
                    ),
                }),
            )
            .unwrap()
        };
        let owner_key = issue(&[team::READ, team::REVIEW]);
        let member_key = issue(&[team::READ, team::ENABLE, team::USE]);
        let reader_key = issue(&[team::READ]);
        let owner = accounts
            .create_account("Creator", &[format!("key:{}", owner_key.key.id)])
            .unwrap();
        let member = accounts
            .create_account(
                "Colleague",
                &[
                    format!("key:{}", member_key.key.id),
                    format!("key:{}", reader_key.key.id),
                ],
            )
            .unwrap();
        let workspace = accounts
            .create_workspace(&owner.id, "Team", WorkspaceKind::Organization, "team", None)
            .unwrap();
        let invite = accounts
            .invite(&owner.id, &workspace.id, Role::Member, 3600)
            .unwrap();
        accounts.accept(&member.id, &invite.token).unwrap();
        write(&root.join("owner.key"), owner_key.token.as_bytes());
        write(&root.join("member.key"), member_key.token.as_bytes());
        write(&root.join("reader.key"), reader_key.token.as_bytes());
        let source = reviewed_source(&root.join("eval-owner"));
        mirror(&source, &root.join("mirror"));
        write(&root.join("catalog.json"), source.bytes());
        write(
            &root.join("input.txt"),
            b"src/colleague.rs:7:2: error: SECOND_MEMBER_EXACT_INPUT\n",
        );
        write(
            &root.join("home/creator-private.txt"),
            b"CREATOR_PRIVATE_EXAMPLE_NEVER_DISCLOSED",
        );
        write(
            &root.join("home/protected-labels"),
            b"PROTECTED_LABELS_NEVER_DISCLOSED",
        );
        Self {
            temp,
            accounts,
            workspace: workspace.id,
            owner: owner.id,
            member: member.id,
            member_key: member_key.token,
            source,
        }
    }
    fn command(&self, action: &str, key: &str, extra: &[&str]) -> std::process::Output {
        let root = self.temp.path();
        let selected = format!("{}/explain-error", self.source.catalog.items[0].id);
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_openagents"));
        cmd.env_clear()
            .env("HOME", root.join("home"))
            .env("OPENAGENTS_SETTINGS", root.join("home/settings.json"))
            .args(["--json", "plugin", "team", action, "--registry"])
            .arg(root.join("registry"))
            .args(["--credential"])
            .arg(root.join(key))
            .args(["--workspace", &self.workspace]);
        if action != "revoke" {
            cmd.arg("--catalog")
                .arg(root.join("catalog.json"))
                .arg("--mirror")
                .arg(root.join("mirror"))
                .args(["--select", &selected]);
        }
        cmd.args(extra).output().unwrap()
    }
    fn run(&self, action: &str, key: &str, extra: &[&str]) -> Value {
        let out = self.command(action, key, extra);
        assert!(
            out.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).unwrap()
    }
    fn grant(&self) -> Value {
        let input = self.temp.path().join("input.txt");
        let expiry = (now() + 90).to_string();
        let prepared = self.run(
            "prepare",
            "owner.key",
            &[
                "--id",
                "colleague-v1",
                "--member",
                &self.member,
                "--input",
                input.to_str().unwrap(),
                "--purpose",
                "Colleague-approved error explanation",
                "--expires-at",
                &expiry,
            ],
        );
        let request = self.temp.path().join("request.json");
        write(&request, serde_json::to_vec(&prepared["request"]).unwrap());
        self.run(
            "grant",
            "owner.key",
            &[
                "--request",
                request.to_str().unwrap(),
                "--approve",
                prepared["approval"].as_str().unwrap(),
            ],
        )
    }
    fn install_enable(&self) {
        self.run(
            "install",
            "member.key",
            &["--grant", "colleague-v1", "--operation-id", "install-1"],
        );
        self.run(
            "enable",
            "member.key",
            &["--grant", "colleague-v1", "--operation-id", "enable-1"],
        );
    }
    fn refuse_use(&self, id: &str, reason: &str) {
        let mut words = self.use_args();
        words[3] = id.into();
        let refs = words.iter().map(String::as_str).collect::<Vec<_>>();
        let result = self.command("use", "member.key", &refs);
        let report = format!(
            "{} {}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(!result.status.success(), "Unexpected use success: {report}");
        assert!(
            report.to_lowercase().contains(&reason.to_lowercase()),
            "Expected {reason}: {report}"
        );
    }
    fn use_args(&self) -> Vec<String> {
        vec![
            "--grant".into(),
            "colleague-v1".into(),
            "--operation-id".into(),
            "use-1".into(),
            "--input".into(),
            self.temp.path().join("input.txt").display().to_string(),
            "--approve-input".into(),
            team::bytes_digest(&fs::read(self.temp.path().join("input.txt")).unwrap()),
        ]
    }
}
#[test]
fn installed_colleague_reuses_exact_release_with_own_bytes_and_replays_receipt_without_dispatch() {
    let c = Case::new();
    let discovery = c.run("inspect", "member.key", &[]);
    assert_eq!(discovery["selected"]["state"], "qualified_source");
    assert_eq!(discovery["selected"]["execution_authorized"], false);
    let reader = c.run("inspect", "reader.key", &[]);
    assert_eq!(reader["authorization"]["state"], "insufficient_rights");
    assert_eq!(reader["authorization"]["execution_authorized"], false);
    let grant = c.grant();
    assert_eq!(grant["enabled"], false);
    assert!(
        c.accounts
            .team_list(&c.workspace, &c.member_key)
            .unwrap()
            .len()
            == 1
    );
    let installed = c.run(
        "install",
        "member.key",
        &["--grant", "colleague-v1", "--operation-id", "install-1"],
    );
    assert_eq!(installed["effect"]["receipt"]["enabled"], false);
    c.run(
        "enable",
        "member.key",
        &["--grant", "colleague-v1", "--operation-id", "enable-1"],
    );
    let words = c.use_args();
    let refs = words.iter().map(String::as_str).collect::<Vec<_>>();
    let result = c.run("use", "member.key", &refs);
    assert_eq!(result["effect"]["state"], "complete");
    assert_eq!(result["output"]["status"], "ok");
    assert!(
        result["output"]
            .to_string()
            .contains("SECOND_MEMBER_EXACT_INPUT")
    );
    assert!(!result.to_string().contains("CREATOR_PRIVATE_EXAMPLE"));
    assert!(!result.to_string().contains("PROTECTED_LABELS"));
    assert_eq!(result["model_cost"], "none_no_model_invoked");
    let retry = c.run("use", "member.key", &refs);
    assert_eq!(retry["replayed"], true);
    assert_eq!(retry["output"], Value::Null);
    assert_eq!(retry["effect"], result["effect"]);
    let store = fs::read_to_string(c.temp.path().join("registry/accounts.json")).unwrap();
    assert!(!store.contains("SECOND_MEMBER_EXACT_INPUT"));
    assert!(!store.contains(&c.member_key));
}
#[test]
fn evaluation_never_grants_execution_and_current_member_scope_input_and_installation_are_required()
{
    let c = Case::new();
    let words = c.use_args();
    let refs = words.iter().map(String::as_str).collect::<Vec<_>>();
    assert!(!c.command("use", "member.key", &refs).status.success());
    c.grant();
    assert!(
        !c.command(
            "install",
            "reader.key",
            &["--grant", "colleague-v1", "--operation-id", "read-only"]
        )
        .status
        .success()
    );
    assert!(!c.command("use", "reader.key", &refs).status.success());
    c.install_enable();
    write(
        &c.temp.path().join("input.txt"),
        b"CHANGED_DATA_WIDER_THAN_APPROVAL",
    );
    assert!(!c.command("use", "member.key", &refs).status.success());
    write(
        &c.temp.path().join("input.txt"),
        b"src/colleague.rs:7:2: error: SECOND_MEMBER_EXACT_INPUT\n",
    );
    c.accounts
        .remove_member(&c.owner, &c.workspace, &c.member)
        .unwrap();
    assert!(!c.command("use", "member.key", &refs).status.success());
    assert_eq!(
        fs::read(c.temp.path().join("home/protected-labels")).unwrap(),
        b"PROTECTED_LABELS_NEVER_DISCLOSED"
    );
}
#[test]
fn changed_installed_bytes_source_scope_and_withdrawn_signed_head_refuse_new_use() {
    let mut c = Case::new();
    let grant = c.grant();
    c.install_enable();
    let path = c
        .temp
        .path()
        .join("home/.openagents/extensions")
        .join(c.source.publisher.pubkey())
        .join("demo/1.0.0/programs/explain-error.json");
    let original = fs::read(&path).unwrap();
    write(&path, b"{}");
    c.refuse_use("changed-install", "reviewed signed release");
    write(&path, original);
    c.refuse_use("changed-install", "outcome is unknown");
    let review = c.source.catalog.items[0].review.as_mut().unwrap();
    review.recipients.push("unapproved-remote-recipient".into());
    write(&c.temp.path().join("catalog.json"), c.source.bytes());
    c.refuse_use("widened-recipient", "recipient requirements");
    c.source.catalog.items[0]
        .review
        .as_mut()
        .unwrap()
        .recipients = vec!["local-wasm".into()];
    write(&c.temp.path().join("catalog.json"), c.source.bytes());
    let artifact = nostr::contracts::digest_bytes(&serde_json::to_vec(&c.source.program).unwrap());
    let artifact_path = c
        .temp
        .path()
        .join("mirror/artifacts/sha256")
        .join(artifact.strip_prefix("sha256:").unwrap());
    let original = fs::read(&artifact_path).unwrap();
    write(&artifact_path, b"{}");
    c.refuse_use("substituted-artifact", "artifact");
    write(&artifact_path, original);
    let catalog = fs::read(c.temp.path().join("catalog.json")).unwrap();
    c.source.catalog.items[0]
        .review
        .as_mut()
        .unwrap()
        .valid_until = now() - 1;
    write(&c.temp.path().join("catalog.json"), c.source.bytes());
    c.refuse_use("expired-review", "unqualified");
    write(&c.temp.path().join("catalog.json"), catalog);
    c.source.set_head(c.source.hidden());
    mirror(&c.source, &c.temp.path().join("mirror"));
    c.refuse_use("observed-withdrawal", "withdrew");
    assert_eq!(
        c.run("inspect", "member.key", &[])["selected"]["state"],
        "withdrawn"
    );
    // A refused observed withdrawal remains known when the explicit mirror
    // later withholds that newer signed head.
    c.source.set_head(c.source.listing.clone());
    mirror(&c.source, &c.temp.path().join("mirror"));
    c.refuse_use("withheld-newer-head", "rolls back");
    let state = c.run("inspect", "member.key", &[]);
    assert_eq!(state["selected"]["state"], "unavailable");
    c.run(
        "revoke",
        "owner.key",
        &[
            "--grant",
            "colleague-v1",
            "--approve",
            grant["revision"]["digest"].as_str().unwrap(),
        ],
    );
    c.refuse_use("revoked-grant", "withdrawn");
}

#[test]
fn active_native_policy_denies_installed_team_effects_but_keeps_original_receipt_and_inspection() {
    let c = Case::new();
    c.grant();
    c.install_enable();
    let words = c.use_args();
    let refs = words.iter().map(String::as_str).collect::<Vec<_>>();
    let original = c.run("use", "member.key", &refs);
    c.accounts
        .review_team_policy(
            &c.workspace,
            receipts::team_policy::Change {
                expected_digest: None,
                terms: receipts::team_policy::Terms {
                    version: 1,
                    expires_unix: now() + 60,
                    rules: vec![],
                },
            },
            |_| {
                c.accounts
                    .authorize(&c.workspace, &c.owner)
                    .map_err(|e| e.to_string())
            },
        )
        .unwrap();
    let sequence = c.accounts.store().unwrap().sequence;
    c.refuse_use("new-policy-use", "team policy");
    for action in ["install", "enable"] {
        let out = c.command(
            action,
            "member.key",
            &[
                "--grant",
                "colleague-v1",
                "--operation-id",
                "new-policy-effect",
            ],
        );
        assert!(!out.status.success());
        assert!(String::from_utf8_lossy(&out.stdout).contains("team policy"));
    }
    assert_eq!(c.accounts.store().unwrap().sequence, sequence);
    let inspected = c.run("inspect", "reader.key", &[]);
    assert_eq!(inspected["authorization"]["execution_authorized"], false);
    assert_eq!(inspected["authorization"]["state"], "policy_blocked");
    assert_eq!(
        inspected["authorization"]["permissions"]["policy_blocks_new_effects"],
        true
    );
    assert_eq!(inspected["selected"]["state"], "policy_blocked");
    assert_eq!(inspected["selected"]["source_state"], "qualified_source");
    assert_eq!(inspected["grants"][0]["state"], "policy_blocked");
    assert_eq!(inspected["grants"][0]["id"], "colleague-v1");
    let replay = c.run("use", "member.key", &refs);
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["effect"], original["effect"]);
    assert_eq!(replay["output"], Value::Null);
    assert_eq!(c.accounts.store().unwrap().sequence, sequence);
}
