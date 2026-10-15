use super::*;
use host::{Host, request};
use nostr::{contracts, workspace};
use secp256k1::SecretKey;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Command,
};
fn secret(n: u8) -> SecretKey {
    SecretKey::from_byte_array([n; 32]).unwrap()
}
fn key(n: u8) -> String {
    nostr::domain::RelaySigner::from_secret_hex(&secret(n).display_secret().to_string())
        .unwrap()
        .pubkey()
        .into()
}
fn git(root: &Path, args: &[&str], input: Option<&[u8]>) -> String {
    use std::io::Write;
    let mut child = Command::new("git")
        .args(args)
        .current_dir(root)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "Fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
        .env("GIT_COMMITTER_NAME", "Fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(input) = input {
        child.stdin.take().unwrap().write_all(input).unwrap();
    }
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().into()
}
fn repository(root: &Path) -> PathBuf {
    let path = root.join("repo.git");
    std::fs::create_dir(&path).unwrap();
    git(&path, &["init", "--bare"], None);
    git(&path, &["config", "core.logAllRefUpdates", "true"], None);
    let blob = git(&path, &["hash-object", "-w", "--stdin"], Some(b"hello\n"));
    let tree = git(
        &path,
        &["mktree"],
        Some(format!("100644 blob {blob}\tdoc.txt\n").as_bytes()),
    );
    let commit = git(&path, &["commit-tree", &tree], Some(b"fixture\n"));
    git(&path, &["update-ref", "refs/heads/docs", &commit], None);
    path
}
fn setup() -> (tempfile::TempDir, Host, workspace::ResourceRef, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let repo = repository(temp.path());
    let host = Host::create(temp.path().join("host"), secret(1), 100).unwrap();
    let resource = host
        .register_git(
            git::GitDocument::admit(&repo, "refs/heads/docs", "doc.txt").unwrap(),
            "Document".into(),
            100,
        )
        .unwrap();
    (temp, host, resource, repo)
}
fn read(host: &Host, r: &workspace::ResourceRef, n: u8, now: u64) -> Value {
    let event = request(
        &secret(n),
        &key(1),
        json!({"v":"openagents.ws-resource-read.v1","requires":[],"resource":r}),
        now,
    )
    .unwrap();
    host.read_document(&event, now).unwrap()
}
fn change(
    host: &Host,
    r: &workspace::ResourceRef,
    snapshot: Value,
    content: &str,
    n: u8,
    command: Option<String>,
    now: u64,
) -> (nostr::domain::Event, BTreeMap<String, Vec<u8>>) {
    let bytes = content.as_bytes();
    let content = artifact::reference(bytes, "text/plain", None);
    let replacement = json!({"v":"openagents.document-replacement.v1","requires":[],"snapshot":snapshot["snapshot"],"content":content,"encoding":"openagents.git-blob-utf8.v1"});
    let proposal = json!({"v":"openagents.resource-change.v1","requires":[],"command":command.unwrap_or_else(artifact::token),"actor":key(n),"resource":r,"binding":snapshot["body"]["binding"],"expected":snapshot["body"]["observation"],"operation":snapshot["operation"],"input":artifact::json_ref(&replacement).unwrap(),"authorization":snapshot["authorization"],"issued_at":now,"expires_at":now+100});
    let admission = json!({"v":"openagents.workspace-action-admission.v1","requires":[],"proposal":artifact::json_ref(&proposal).unwrap(),"actor":key(n),"approval":null});
    let event = request(&secret(n), &key(1), admission, now).unwrap();
    let mut files = BTreeMap::from([(content["digest"].as_str().unwrap().into(), bytes.to_vec())]);
    for value in [proposal, replacement] {
        let bytes = contracts::jcs(&value).unwrap();
        files.insert(contracts::digest_bytes(&bytes), bytes);
    }
    let _ = host;
    (event, files)
}
#[test]
fn source_conditional_change_exact_retry_and_independent_writer() {
    let (_temp, host, r, repo) = setup();
    let before = read(&host, &r, 1, 100);
    let (event, files) = change(&host, &r, before.clone(), "new\n", 1, None, 101);
    let result = host.change(&event, &files, 101, None).unwrap();
    assert_eq!(result["outcome"], "completed");
    assert_eq!(host.change(&event, &files, 102, None).unwrap(), result);
    assert_eq!(read(&host, &r, 1, 102)["content_utf8"], "new\n");
    let (stale, files) = change(&host, &r, before, "lost\n", 1, None, 103);
    assert_eq!(
        host.change(&stale, &files, 103, None).unwrap()["outcome"],
        "refused"
    );
    assert_eq!(read(&host, &r, 1, 103)["content_utf8"], "new\n");
    assert_eq!(
        git(&repo, &["rev-parse", "--is-bare-repository"], None),
        "true"
    );
    let envelope = nostr::private_artifact::open(&event, &secret(1)).unwrap();
    let admission = contracts::parse_strict(envelope.inline_bytes().unwrap()).unwrap();
    let proposal = host.artifacts.json(&admission["proposal"]).unwrap();
    let original_command = proposal["command"].as_str().unwrap().to_string();
    let changed_before = read(&host, &r, 1, 104);
    let (conflict, files) = change(
        &host,
        &r,
        changed_before,
        "conflict\n",
        1,
        Some(original_command),
        104,
    );
    assert!(
        host.change(&conflict, &files, 104, None)
            .unwrap_err()
            .contains("idempotency_conflict")
    );
}
#[test]
fn revoked_access_and_approvals_refuse() {
    let (_temp, host, r, _) = setup();
    host.grant(&key(2), [r.resource.clone()].into(), true, true, 200)
        .unwrap();
    let before = read(&host, &r, 2, 100);
    let (event, files) = change(&host, &r, before, "private\n", 2, None, 101);
    assert!(
        host.change(&event, &files, 101, None)
            .unwrap_err()
            .contains("approval")
    );
    host.revoke(&key(2)).unwrap();
    assert!(host.change(&event, &files, 102, None).is_err());
}
fn projection_read(
    host: &Host,
    p: &Value,
    mode: &str,
    after: Value,
    n: u8,
    now: u64,
    max_items: u64,
) -> (Value, Value) {
    let request_value = json!({"v":"openagents.projection-read.v1","requires":[],"request":artifact::token(),"projection":p,"mode":mode,"after":after,"max_items":max_items,"max_bytes":65536});
    let event = request(&secret(n), &key(1), request_value.clone(), now).unwrap();
    let result = host.projection_read(&event, now).unwrap();
    (request_value, result)
}
fn fetch(host: &Host, p: &Value, reference: &Value, n: u8, now: u64) -> Value {
    let event=request(&secret(n),&key(1),json!({"v":"openagents.ws-projection-fetch.v1","requires":[],"projection":p,"artifact":reference,"max_bytes":1048576}),now).unwrap();
    contracts::parse_strict(&host.projection_fetch(&event, now).unwrap()).unwrap()
}
#[test]
fn clients_snapshot_and_delta_converge_without_skipping_updates() {
    let (_temp, host, r, _) = setup();
    let p = host
        .define_projection(&key(1), [r.resource.clone()].into(), 500, 100)
        .unwrap();
    let (initial_read, result) = projection_read(&host, &p, "snapshot", Value::Null, 1, 100, 1);
    assert_eq!(result["status"], "completed");
    let page = fetch(&host, &p, &result["page"], 1, 100);
    let cut = fetch(&host, &p, &page["cut"], 1, 100);
    let mut a = projection::Client::default();
    let mut b = projection::Client::default();
    a.snapshot_page(&initial_read, &page, &cut).unwrap();
    b.snapshot_page(&initial_read, &page, &cut).unwrap();
    let before = read(&host, &r, 1, 100);
    let (event, files) = change(&host, &r, before, "next\n", 1, None, 101);
    host.change(&event, &files, 101, None).unwrap();
    let (_, result) = projection_read(&host, &p, "changes", page["resume"].clone(), 1, 102, 1);
    let page = fetch(&host, &p, &result["page"], 1, 102);
    assert_eq!(page["deltas"].as_array().unwrap().len(), 1);
    let delta = fetch(&host, &p, &page["deltas"][0], 1, 102);
    let to = fetch(&host, &p, &delta["to"], 1, 102);
    a.delta(&delta, &cut, &to).unwrap();
    a.delta(&delta, &cut, &to).unwrap();
    b.delta(&delta, &cut, &to).unwrap();
    assert_eq!(a.rows, b.rows);
    assert_eq!(a.cut, b.cut);
    assert!(a.snapshot_page(&initial_read,&json!({"v":"openagents.projection-page.v1","requires":[],"request":result["request"],"cut":artifact::json_ref(&cut).unwrap(),"rows":[],"next":null,"coverage":"complete","resume":"old"}),&cut).is_err());
}
#[test]
fn checkpoint_restore_uses_separate_plan_and_safety_capture() {
    let (_temp, host, r, _) = setup();
    let event=request(&secret(1),&key(1),json!({"v":"openagents.ws-checkpoint-create.v1","requires":[],"resources":[r],"retain_until":500}),100).unwrap();
    let checkpoint = host.checkpoint(&event, 100).unwrap();
    let before = read(&host, &r, 1, 100);
    let (change, files) = change(&host, &r, before, "after\n", 1, None, 101);
    host.change(&change, &files, 101, None).unwrap();
    let event=request(&secret(1),&key(1),json!({"v":"openagents.ws-restore-plan.v1","requires":[],"checkpoint":checkpoint["reference"]}),102).unwrap();
    let plan = host.plan_restore(&event, 102).unwrap();
    assert_ne!(plan["body"]["safety_checkpoint"], checkpoint["reference"]);
    assert_eq!(read(&host, &r, 1, 102)["content_utf8"], "after\n");
    let event=request(&secret(1),&key(1),json!({"v":"openagents.workspace-action-admission.v1","requires":[],"proposal":plan["reference"],"actor":key(1),"approval":null}),103).unwrap();
    let result = host.restore(&event, 103, None).unwrap();
    assert_eq!(result["status"], "completed");
    assert_eq!(read(&host, &r, 1, 104)["content_utf8"], "hello\n");
    assert_eq!(host.restore(&event, 104, None).unwrap(), result);
}
#[test]
fn command_visibility_uses_applied_frontier_and_rejects_another_reader() {
    let (_temp, host, r, _) = setup();
    let p = host
        .define_projection(&key(1), [r.resource.clone()].into(), 500, 100)
        .unwrap();
    let before = read(&host, &r, 1, 100);
    let (event, files) = change(&host, &r, before, "visible\n", 1, None, 101);
    let result = host.change(&event, &files, 101, None).unwrap();
    let query = json!({"v":"openagents.ws-command-visibility-read.v1","requires":[],"projection":p,"command":result["receipt"]});
    let event = request(&secret(1), &key(1), query.clone(), 102).unwrap();
    assert_eq!(
        host.command_visibility(&event, 102).unwrap()["body"]["state"],
        "pending"
    );
    projection_read(&host, &p, "snapshot", Value::Null, 1, 103, 1);
    assert_eq!(
        host.command_visibility(&event, 103).unwrap()["body"]["state"],
        "visible"
    );
    host.grant(&key(2), [r.resource.clone()].into(), false, false, 500)
        .unwrap();
    let event = request(&secret(2), &key(1), query, 104).unwrap();
    assert!(host.command_visibility(&event, 104).is_err());
}
#[test]
fn source_cas_survives_an_external_writer_and_source_rebinding_refuses() {
    let temp = tempfile::tempdir().unwrap();
    let repo = repository(temp.path());
    let source = git::GitDocument::admit(&repo, "refs/heads/docs", "doc.txt").unwrap();
    let before = source.capture().unwrap();
    let mine = source.prepare(&before.position, b"mine\n", "mine").unwrap();
    let other = source
        .prepare(&before.position, b"other\n", "other")
        .unwrap();
    git(
        &repo,
        &["update-ref", "refs/heads/docs", &other, &before.revision],
        None,
    );
    assert!(source.dispatch(&before.position, &mine).is_err());
    assert_eq!(source.capture().unwrap().bytes, b"other\n");
    let after = source.capture().unwrap();
    assert!(
        source
            .feed_contiguous(&before.position, &after.position)
            .unwrap()
    );
    git(
        &repo,
        &["update-ref", "refs/heads/docs", &before.revision, &other],
        None,
    );
    let reverted = source.capture().unwrap();
    assert_ne!(before.position, reverted.position);
    assert!(source.dispatch(&before.position, &mine).is_err());
    let fresh = source.capture().unwrap();
    let candidate = source
        .prepare(&fresh.position, b"valid after abort\n", "fresh")
        .unwrap();
    source.dispatch(&fresh.position, &candidate).unwrap();
    assert!(
        source
            .feed_contiguous(&before.position, &reverted.position)
            .unwrap()
    );
    std::fs::write(repo.join("logs/refs/heads/docs"), b"changed\n").unwrap();
    assert!(source.capture().is_err());
}
#[test]
fn restart_reconciles_a_lost_response_without_replaying_dispatch() {
    let (temp, mut host, r, repo) = setup();
    let before = read(&host, &r, 1, 100);
    let (event, files) = change(&host, &r, before, "once\n", 1, None, 101);
    host.fail_after_dispatch = true;
    assert!(host.change(&event, &files, 101, None).is_err());
    let log = std::fs::read(repo.join("logs/refs/heads/docs")).unwrap();
    drop(host);
    let host = Host::open(temp.path().join("host"), secret(1)).unwrap();
    let result = host.change(&event, &files, 102, None).unwrap();
    assert_eq!(result["outcome"], "completed");
    assert_eq!(
        std::fs::read(repo.join("logs/refs/heads/docs")).unwrap(),
        log
    );
}
#[test]
fn outbox_retains_exact_bytes_and_quarantines_corruption() {
    let (temp, host, r, _) = setup();
    let before = read(&host, &r, 1, 100);
    let (event, attachments) = change(&host, &r, before, "queued\n", 1, None, 101);
    let bundle = outbox::Bundle { event, attachments };
    let root = temp.path().join("outbox");
    let outbox = outbox::Outbox::open(root.clone(), secret(1)).unwrap();
    let id = outbox.enqueue(&bundle).unwrap();
    let retry = outbox.retry(&id).unwrap();
    assert_eq!(retry.event, bundle.event);
    assert_eq!(retry.attachments, bundle.attachments);
    drop(outbox);
    let restarted = outbox::Outbox::open(root.clone(), secret(1)).unwrap();
    assert_eq!(restarted.retry(&id).unwrap().event, bundle.event);
    std::fs::write(root.join(format!("{id}.json")), b"broken").unwrap();
    assert!(restarted.retry(&id).is_err());
    assert!(
        restarted
            .enqueue(&bundle)
            .unwrap_err()
            .contains("quarantined")
    );
    assert!(
        std::fs::read_dir(root)
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| e.file_name().to_string_lossy().starts_with("quarantined-"))
    );
}
#[test]
fn partial_restore_keeps_confirmed_steps_and_stops_at_a_changed_source() {
    let (temp, mut host, first, repo) = setup();
    git(&repo, &["branch", "second", "refs/heads/docs"], None);
    let second = host
        .register_git(
            git::GitDocument::admit(&repo, "refs/heads/second", "doc.txt").unwrap(),
            "Second".into(),
            100,
        )
        .unwrap();
    let resources = vec![first.clone(), second.clone()];
    let checkpoint=host.checkpoint(&request(&secret(1),&key(1),json!({"v":"openagents.ws-checkpoint-create.v1","requires":[],"resources":resources,"retain_until":500}),100).unwrap(),100).unwrap();
    for r in [&first, &second] {
        let before = read(&host, r, 1, 101);
        let (event, files) = change(&host, r, before, "modified\n", 1, None, 101);
        host.change(&event, &files, 101, None).unwrap();
    }
    let plan=host.plan_restore(&request(&secret(1),&key(1),json!({"v":"openagents.ws-restore-plan.v1","requires":[],"checkpoint":checkpoint["reference"]}),102).unwrap(),102).unwrap();
    let second_step = host
        .artifacts
        .json(&plan["body"]["steps"][1]["input"])
        .unwrap();
    let target_snapshot = host.artifacts.json(&second_step["snapshot"]).unwrap();
    let target_resource = workspace::resource(&target_snapshot["resource"]).unwrap();
    let state = host.state().unwrap();
    let source = state.resources[&target_resource.resource].source.clone();
    let capture = source.capture().unwrap();
    let candidate = source
        .prepare(&capture.position, b"external\n", "external")
        .unwrap();
    host.restore_hook = Some(Box::new(move |index| {
        if index == 0 {
            source.dispatch(&capture.position, &candidate).unwrap();
        }
    }));
    let event=request(&secret(1),&key(1),json!({"v":"openagents.workspace-action-admission.v1","requires":[],"proposal":plan["reference"],"actor":key(1),"approval":null}),103).unwrap();
    let result = host.restore(&event, 103, None).unwrap();
    assert_eq!(result["status"], "partial");
    assert_eq!(result["steps"][0]["result"]["outcome"], "completed");
    assert_eq!(result["steps"][1]["result"]["outcome"], "refused");
    drop(host);
    let host = Host::open(temp.path().join("host"), secret(1)).unwrap();
    assert_eq!(host.restore(&event, 104, None).unwrap(), result);
    assert_eq!(
        read(&host, &target_resource, 1, 104)["content_utf8"],
        "external\n"
    );
}
#[test]
fn snapshot_paging_keeps_a_fixed_cut_and_revocation_stops_followup_reads() {
    let (_temp, host, first, repo) = setup();
    git(&repo, &["branch", "second", "refs/heads/docs"], None);
    let second = host
        .register_git(
            git::GitDocument::admit(&repo, "refs/heads/second", "doc.txt").unwrap(),
            "Second".into(),
            100,
        )
        .unwrap();
    host.grant(
        &key(2),
        [first.resource.clone(), second.resource.clone()].into(),
        false,
        false,
        500,
    )
    .unwrap();
    let p = host
        .define_projection(
            &key(2),
            [first.resource.clone(), second.resource.clone()].into(),
            500,
            100,
        )
        .unwrap();
    let (initial, result) = projection_read(&host, &p, "snapshot", Value::Null, 2, 100, 1);
    let first_page = fetch(&host, &p, &result["page"], 2, 100);
    assert!(!first_page["next"].is_null());
    assert!(first_page["resume"].is_null());
    let cut = fetch(&host, &p, &first_page["cut"], 2, 100);
    let mut client = projection::Client::default();
    client.snapshot_page(&initial, &first_page, &cut).unwrap();
    let before = read(&host, &first, 1, 100);
    let (event, files) = change(&host, &first, before, "during paging\n", 1, None, 101);
    host.change(&event, &files, 101, None).unwrap();
    let (next, result) =
        projection_read(&host, &p, "snapshot", first_page["next"].clone(), 2, 102, 1);
    let last = fetch(&host, &p, &result["page"], 2, 102);
    assert_eq!(last["cut"], first_page["cut"]);
    client.snapshot_page(&next, &last, &cut).unwrap();
    let (_, changes) = projection_read(&host, &p, "changes", last["resume"].clone(), 2, 103, 1);
    let page = fetch(&host, &p, &changes["page"], 2, 103);
    assert_eq!(page["deltas"].as_array().unwrap().len(), 1);
    host.revoke(&key(2)).unwrap();
    let (_, refused) = projection_read(&host, &p, "changes", page["resume"].clone(), 2, 104, 1);
    assert_eq!(refused["reason"], "not_admitted");
    let event=request(&secret(2),&key(1),json!({"v":"openagents.ws-projection-fetch.v1","requires":[],"projection":p,"artifact":result["page"],"max_bytes":1048576}),104).unwrap();
    assert!(host.projection_fetch(&event, 104).is_err());
}

#[cfg(unix)]
#[test]
fn a_changed_root_mapping_requires_explicit_registration() {
    let temp = tempfile::tempdir().unwrap();
    let repo = repository(temp.path());
    let source = git::GitDocument::admit(&repo, "refs/heads/docs", "doc.txt").unwrap();
    let moved = temp.path().join("moved.git");
    std::fs::rename(&repo, &moved).unwrap();
    std::os::unix::fs::symlink(&moved, &repo).unwrap();
    assert!(source.capture().unwrap_err().contains("identity changed"));
}
#[test]
fn a_digest_does_not_authorize_copying_another_resources_retained_bytes() {
    let (_temp, host, first, repo) = setup();
    git(&repo, &["branch", "private", "refs/heads/docs"], None);
    let private = host
        .register_git(
            git::GitDocument::admit(&repo, "refs/heads/private", "doc.txt").unwrap(),
            "Private".into(),
            100,
        )
        .unwrap();
    let before = read(&host, &private, 1, 100);
    let (event, files) = change(
        &host,
        &private,
        before,
        "private retained content\n",
        1,
        None,
        101,
    );
    host.change(&event, &files, 101, None).unwrap();
    let private_snapshot = read(&host, &private, 1, 102);
    let private_content = private_snapshot["body"]["content"].clone();
    host.grant(&key(2), [first.resource.clone()].into(), true, false, 500)
        .unwrap();
    let before = read(&host, &first, 2, 102);
    let (event, mut files) = change(&host, &first, before.clone(), "placeholder\n", 2, None, 103);
    let opened = nostr::private_artifact::open(&event, &secret(2)).unwrap();
    let mut admission = contracts::parse_strict(opened.inline_bytes().unwrap()).unwrap();
    let mut proposal =
        contracts::parse_strict(&files[admission["proposal"]["digest"].as_str().unwrap()]).unwrap();
    let mut replacement =
        contracts::parse_strict(&files[proposal["input"]["digest"].as_str().unwrap()]).unwrap();
    replacement["content"] = private_content;
    let bytes = contracts::jcs(&replacement).unwrap();
    let reference = artifact::json_ref(&replacement).unwrap();
    files.insert(reference["digest"].as_str().unwrap().into(), bytes);
    proposal["input"] = reference;
    let bytes = contracts::jcs(&proposal).unwrap();
    let reference = artifact::json_ref(&proposal).unwrap();
    files.insert(reference["digest"].as_str().unwrap().into(), bytes);
    admission["proposal"] = reference;
    assert!(
        !files
            .values()
            .any(|bytes| bytes == b"private retained content\n")
    );
    let event = request(&secret(2), &key(1), admission, 103).unwrap();
    let result = host.change(&event, &files, 103, None);
    assert!(
        result.is_err(),
        "retained private bytes were copied: {result:?}"
    );
    assert_eq!(
        read(&host, &first, 2, 104)["content_utf8"],
        before["content_utf8"]
    );
}

#[test]
fn unchanged_current_bytes_can_be_reused_without_an_attachment() {
    let (_temp, host, resource, _) = setup();
    let before = read(&host, &resource, 1, 100);
    let (event, mut files) = change(&host, &resource, before, "hello\n", 1, None, 101);
    files.remove(&contracts::digest_bytes(b"hello\n"));
    assert_eq!(
        host.change(&event, &files, 101, None).unwrap()["outcome"],
        "completed"
    );
}
#[test]
fn a_caller_snapshot_cannot_substitute_cached_bytes_for_the_native_source() {
    let (_temp, host, resource, _) = setup();
    let before = read(&host, &resource, 1, 100);
    let foreign = host
        .artifacts
        .put(b"unrelated cached bytes\n", "text/plain", None)
        .unwrap();
    let mut snapshot = before["body"].clone();
    let mut observation = host.artifacts.json(&snapshot["observation"]).unwrap();
    snapshot["content"] = foreign.clone();
    observation["content"] = foreign;
    snapshot["observation"] = host.artifacts.put_json(&observation).unwrap();
    let mut forged = before.clone();
    forged["snapshot"] = host.artifacts.put_json(&snapshot).unwrap();
    forged["body"] = snapshot;
    let (event, mut files) = change(
        &host,
        &resource,
        forged,
        "unrelated cached bytes\n",
        1,
        None,
        101,
    );
    files.remove(&contracts::digest_bytes(b"unrelated cached bytes\n"));
    assert!(host.change(&event, &files, 101, None).is_err());
    assert_eq!(
        read(&host, &resource, 1, 102)["content_utf8"],
        before["content_utf8"]
    );
}
