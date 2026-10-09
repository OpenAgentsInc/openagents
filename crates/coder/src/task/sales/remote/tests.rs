use super::*;
use crate::task::sales::tests::create;
use std::os::unix::fs::PermissionsExt;
use tempfile::TempDir;

fn clock() -> u64 {
    1000
}

const SITE: &str = "synthetic-site-bearer-for-writer-binding";
const CONTACT: &str = "prospect@synthetic.invalid";

struct Owner {
    _dir: TempDir,
    dir: PathBuf,
    host: PathBuf,
    owner: PathBuf,
    config: PathBuf,
}

fn private_file(path: &Path, bytes: &[u8]) {
    std::fs::write(path, bytes).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
}

fn owner(effects: &[Effect]) -> Owner {
    let temp = TempDir::new().unwrap();
    let dir = temp.path().canonicalize().unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    let host = dir.join("host");
    let owner = dir.join("owner-credential");
    let mut store = Store::open_with_clock(&host, clock).unwrap();
    store.initialize("operator", &owner).unwrap();
    let admin = store
        .authenticate(&Store::read_credential(&owner).unwrap())
        .unwrap();
    store
        .issue(&admin, "writer-a", Role::Writer, &dir.join("writer-a"))
        .unwrap();
    drop(store);
    let config = dir.join("remote.json");
    let fixture = Owner {
        _dir: temp,
        dir,
        host,
        owner,
        config,
    };
    fixture.bind(effects);
    fixture
}

impl Owner {
    fn bind(&self, effects: &[Effect]) {
        private_file(
            &self.config,
            &serde_json::to_vec(&json!({
                "schema":CONFIG_SCHEMA,"root":self.host,"journal":self.dir.join("journal"),
                "bindings":[{"id":"alice-sales","account":"alice","workspace":"alice-personal",
                "members_epoch":3,"principal":"writer-a","credential":self.dir.join("writer-a"),
                "client_digest":digest(SITE.as_bytes()),"effects":effects}]
            }))
            .unwrap(),
        );
    }
    fn revoke(&self) {
        let mut store = Store::open_with_clock(&self.host, clock).unwrap();
        let admin = store
            .authenticate(&Store::read_credential(&self.owner).unwrap())
            .unwrap();
        store.revoke(&admin, "writer-a").unwrap();
    }
}

fn actor() -> Value {
    json!({"account":"alice","workspace":"alice-personal","members_epoch":3})
}

fn call(service: &Service, bearer: &str, actor: Value, op: Value) -> Reply {
    let body = json!({"schema":REQUEST_SCHEMA,"actor":actor,"op":op});
    let reply = service.call("alice-sales", bearer, body.to_string().as_bytes());
    if reply.status != 200 || op_kind(&body) != "show" {
        assert!(
            !reply.body.to_string().contains(CONTACT),
            "contact left the owner outside an authorized record read"
        );
    }
    reply
}

fn op_kind(body: &Value) -> &str {
    body["op"]["kind"].as_str().unwrap()
}

fn error(reply: &Reply) -> &str {
    reply.body["error"].as_str().unwrap()
}

fn apply(service: &Service, request: &str, command: &[u8]) -> Reply {
    call(
        service,
        SITE,
        actor(),
        json!({"kind":"apply","request":request,"command":String::from_utf8(command.to_vec()).unwrap()}),
    )
}

#[test]
fn binding_reads_summaries_and_refuses_other_actors_bearers_and_unlisted_effects() {
    let owner = owner(&[Effect::Create, Effect::Update]);
    let service = Service::open_with_clock(&owner.config, clock).unwrap();
    let standing = call(&service, SITE, actor(), json!({"kind":"standing"}));
    assert_eq!(standing.status, 200);
    assert_eq!(standing.body["result"]["principal"], "writer-a");
    assert_eq!(standing.body["result"]["role"], "writer");

    let created = apply(&service, "create-one", &create("create-one"));
    assert_eq!(created.status, 200, "{}", created.body);
    let lead = created.body["result"]["lead"].as_str().unwrap().to_owned();
    let list = call(
        &service,
        SITE,
        actor(),
        json!({"kind":"list","after":null,"limit":10}),
    );
    assert_eq!(list.status, 200);
    assert_eq!(list.body["result"][0]["id"], json!(lead));
    assert_eq!(list.body["result"][0]["stage"], "qualified");
    let shown = call(&service, SITE, actor(), json!({"kind":"show","lead":lead}));
    assert_eq!(shown.status, 200);
    assert!(shown.body.to_string().contains(CONTACT));

    // Another bearer, actor, workspace, or epoch reads nothing.
    for (bearer, actor) in [
        ("another-site-bearer", actor()),
        (
            SITE,
            json!({"account":"bob","workspace":"alice-personal","members_epoch":3}),
        ),
        (
            SITE,
            json!({"account":"alice","workspace":"alice-team","members_epoch":3}),
        ),
        (
            SITE,
            json!({"account":"alice","workspace":"alice-personal","members_epoch":4}),
        ),
    ] {
        let refused = call(&service, bearer, actor, json!({"kind":"show","lead":lead}));
        assert_eq!(refused.status, 403);
        assert_eq!(error(&refused), "access_denied");
    }
    // Suppression is outside this binding's narrowing allowlist.
    let suppress = serde_json::to_vec(&json!({"schema":crate::task::sales::COMMAND_SCHEMA,"id":"suppress-one","lead":lead,"expected_revision":1,"operation":{"kind":"suppress","reference":"synthetic request"}})).unwrap();
    assert_eq!(
        error(&apply(&service, "suppress-one", &suppress)),
        "access_denied"
    );
    // The command identity must be the retry identity.
    assert_eq!(
        error(&apply(&service, "other-id", &create("create-two"))),
        "invalid_request"
    );

    // Observation-only bindings offer and admit no effect.
    owner.bind(&[]);
    let standing = call(&service, SITE, actor(), json!({"kind":"standing"}));
    assert_eq!(standing.body["result"]["effects"], json!([]));
    assert_eq!(
        error(&apply(&service, "create-three", &create("create-three"))),
        "access_denied"
    );
}

#[test]
fn partner_reads_are_scoped_and_desks_and_earnings_stay_with_the_owner() {
    let owner = owner(&[Effect::Create]);
    let service = Service::open_with_clock(&owner.config, clock).unwrap();
    let created = apply(&service, "create-one", &create("create-one"));
    assert_eq!(created.status, 200, "{}", created.body);
    let lead = created.body["result"]["lead"].as_str().unwrap().to_owned();
    // No assignment names this principal, so its page is empty.
    let page = call(
        &service,
        SITE,
        actor(),
        json!({"kind":"partners","after":null,"limit":8}),
    );
    assert_eq!(page.status, 200, "{}", page.body);
    assert_eq!(page.body["result"], json!([]));
    // Absent and refused assignments answer alike; bad ids are refused.
    let one = call(
        &service,
        SITE,
        actor(),
        json!({"kind":"partner","lead":lead,"assignment":"introduction"}),
    );
    assert_eq!(error(&one), "access_denied");
    let bad = call(
        &service,
        SITE,
        actor(),
        json!({"kind":"partners","after":{"lead":"../x","assignment":"a"},"limit":8}),
    );
    assert_eq!(error(&bad), "invalid_request");
    let unbounded = call(
        &service,
        SITE,
        actor(),
        json!({"kind":"partners","after":null,"limit":500}),
    );
    assert_eq!(error(&unbounded), "invalid_request");
    // Arthur's and Vanna's projections and the earned ledger are owner-only.
    for op in [
        json!({"kind":"desk","desk":"arthur"}),
        json!({"kind":"desk","desk":"vanna"}),
        json!({"kind":"earned"}),
    ] {
        assert_eq!(error(&call(&service, SITE, actor(), op)), "access_denied");
    }
    // Revocation refuses partner reads too.
    owner.revoke();
    let revoked = call(
        &service,
        SITE,
        actor(),
        json!({"kind":"partners","after":null,"limit":8}),
    );
    assert_eq!(error(&revoked), "access_denied");
}

#[test]
fn exact_retries_recover_changed_bytes_conflict_and_stale_revisions_refuse() {
    let owner = owner(&[Effect::Create, Effect::Update]);
    let service = Service::open_with_clock(&owner.config, clock).unwrap();
    let bytes = create("create-once");
    let first = apply(&service, "create-once", &bytes);
    assert_eq!(first.status, 200);
    // A restarted adapter recovers the same receipt.
    let service = Service::open_with_clock(&owner.config, clock).unwrap();
    let again = apply(&service, "create-once", &bytes);
    assert_eq!(again.body, first.body);
    let mut changed: Value = serde_json::from_slice(&bytes).unwrap();
    changed["operation"]["input"]["source"] = json!("changed source");
    let changed = serde_json::to_vec(&changed).unwrap();
    assert_eq!(error(&apply(&service, "create-once", &changed)), "conflict");

    let lead = first.body["result"]["lead"].as_str().unwrap().to_owned();
    let shown = call(&service, SITE, actor(), json!({"kind":"show","lead":lead}));
    let mut details = shown.body["result"]["details"].clone();
    details["stage"] = json!("pilot");
    let update = |id: &str, revision: u64| {
        serde_json::to_vec(&json!({"schema":crate::task::sales::COMMAND_SCHEMA,"id":id,"lead":lead,"expected_revision":revision,"operation":{"kind":"update","details":details}})).unwrap()
    };
    let stale = apply(&service, "update-stale", &update("update-stale", 7));
    assert_eq!(error(&stale), "stale");
    // A definitive refusal leaves no unsettled entry behind.
    let settled = call(
        &service,
        SITE,
        actor(),
        json!({"kind":"reconcile","request":"update-stale","digest":digest(&update("update-stale", 7))}),
    );
    assert_eq!(settled.body["result"]["state"], "absent");
    let updated = apply(&service, "update-ok", &update("update-ok", 1));
    assert_eq!(updated.status, 200, "{}", updated.body);
    assert_eq!(updated.body["result"]["revision"], 2);
    let settled = call(
        &service,
        SITE,
        actor(),
        json!({"kind":"reconcile","request":"update-ok","digest":digest(&update("update-ok", 1))}),
    );
    assert_eq!(settled.body["result"]["state"], "recorded");
    assert_eq!(settled.body["result"]["receipt"], updated.body["result"]);
    let mismatch = call(
        &service,
        SITE,
        actor(),
        json!({"kind":"reconcile","request":"update-ok","digest":digest(b"other")}),
    );
    assert_eq!(error(&mismatch), "conflict");
}

#[test]
fn a_journaled_unsettled_effect_reconciles_once_and_revocation_refuses_everything() {
    let owner = owner(&[Effect::Create]);
    let service = Service::open_with_clock(&owner.config, clock).unwrap();
    // Journaled before dispatch, then the reply was lost before the store ran.
    let bytes = create("lost-reply");
    let config = load(&owner.config).unwrap();
    let binding = &config.bindings[0];
    let mut journal = Journal::load(&config.journal, binding).unwrap();
    journal.entries.insert(
        "lost-reply".into(),
        Entry {
            ledger: Ledger::Pipeline,
            digest: digest(&bytes),
            command: String::from_utf8(bytes.clone()).unwrap(),
            at: 1000,
            receipt: None,
        },
    );
    journal.save(&config.journal, binding).unwrap();
    let settled = call(
        &service,
        SITE,
        actor(),
        json!({"kind":"reconcile","request":"lost-reply","digest":digest(&bytes)}),
    );
    assert_eq!(settled.status, 200, "{}", settled.body);
    let receipt = settled.body["result"]["receipt"].clone();
    assert_eq!(receipt["outcome"], "created");
    // The exact apply retry returns the same single record.
    let again = apply(&service, "lost-reply", &bytes);
    assert_eq!(again.body["result"], receipt);
    let list = call(
        &service,
        SITE,
        actor(),
        json!({"kind":"list","after":null,"limit":10}),
    );
    assert_eq!(list.body["result"].as_array().unwrap().len(), 1);

    owner.revoke();
    for op in [
        json!({"kind":"standing"}),
        json!({"kind":"list","after":null,"limit":10}),
        json!({"kind":"show","lead":receipt["lead"]}),
        json!({"kind":"reconcile","request":"lost-reply","digest":digest(&bytes)}),
    ] {
        let refused = call(&service, SITE, actor(), op);
        assert_eq!(error(&refused), "access_denied");
    }
    assert_eq!(
        error(&apply(&service, "lost-reply", &bytes)),
        "access_denied"
    );
    // The journal holds no credential and names no contact in its file name.
    for entry in std::fs::read_dir(owner.dir.join("journal")).unwrap() {
        let entry = entry.unwrap();
        assert!(!entry.file_name().to_string_lossy().contains("prospect"));
        let text = std::fs::read_to_string(entry.path()).unwrap();
        assert!(!text.contains(SITE));
        assert!(!text.contains(CONTACT));
    }
}

impl Owner {
    /// Bind `principal` with an optional service evidence root.
    fn bind_as(&self, principal: &str, credential: &Path, evidence: Option<&Path>) {
        private_file(
            &self.config,
            &serde_json::to_vec(&json!({
                "schema":CONFIG_SCHEMA,"root":self.host,"journal":self.dir.join("journal"),
                "evidence":evidence,
                "bindings":[{"id":"alice-sales","account":"alice","workspace":"alice-personal",
                "members_epoch":3,"principal":principal,"credential":credential,
                "client_digest":digest(SITE.as_bytes()),"effects":["create"]}]
            }))
            .unwrap(),
        );
    }
}

#[test]
fn records_delivery_audit_and_claims_are_fenced_and_scoped_to_one_record() {
    use crate::task::sales::tests::service_fixture::{self as fixture, Comparison, retain};
    let owner = owner(&[Effect::Create]);
    let service = Service::open_with_clock(&owner.config, clock).unwrap();
    let lead = apply(&service, "lead-one", &create("lead-one")).body["result"]["lead"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut second: Value = serde_json::from_slice(&create("lead-two")).unwrap();
    second["operation"]["input"]["contact"] = json!("email:unrelated@synthetic.invalid");
    let other = apply(&service, "lead-two", &serde_json::to_vec(&second).unwrap());
    let other = other.body["result"]["lead"].as_str().unwrap().to_owned();

    // The pipeline owner records one accepted service sale on the first record.
    let evidence = owner.dir.join("evidence");
    std::fs::create_dir(&evidence).unwrap();
    std::fs::set_permissions(&evidence, std::fs::Permissions::from_mode(0o700)).unwrap();
    let comparison = Comparison {
        manifest: retain(&evidence, "comparison.json", b"synthetic manifest"),
        report: retain(&evidence, "comparison-report.json", b"synthetic report"),
        candidate: retain(&evidence, "candidate.patch", b"synthetic candidate"),
        check: retain(&evidence, "independent-check", b"synthetic check"),
        decision: retain(&evidence, "buyer-decision", b"synthetic decision"),
        frozen_checks: vec![retain(&evidence, "frozen-command", b"synthetic frozen")],
    };
    let admission = fixture::admission(
        &evidence,
        1000,
        &lead,
        "synthetic-account",
        "offer-v1",
        comparison,
    );
    let mut store = Store::open_with_clock(&owner.host, clock).unwrap();
    let admin = store
        .authenticate(&Store::read_credential(&owner.owner).unwrap())
        .unwrap();
    let admit = json!({"schema":crate::task::sales::COMMAND_SCHEMA,"id":"admit-sale","lead":lead,
        "expected_revision":1,"operation":{"kind":"record_service_sale","admission":admission}});
    store
        .apply_with_evidence_root(
            &admin,
            &serde_json::to_vec(&admit).unwrap(),
            Some(&evidence),
        )
        .unwrap();
    drop(store);

    // Records carry the retained sale and no contact.
    let records = call(
        &service,
        SITE,
        actor(),
        json!({"kind":"records","after":null,"limit":10}),
    );
    assert_eq!(records.status, 200, "{}", records.body);
    let first = records.body["result"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["summary"]["id"] == json!(lead))
        .unwrap();
    assert_eq!(
        first["services"][0]["admission"]["invoice"]["id"],
        "synthetic-invoice"
    );
    assert_eq!(first["services"][0]["facts"]["support_human"], "operator");

    // The delivery handoff is reread only from a configured root, by digest.
    let delivery = json!({"kind":"delivery","lead":lead,"sale":"synthetic-sale"});
    let unconfigured = call(&service, SITE, actor(), delivery.clone());
    assert_eq!(unconfigured.body["result"]["unavailable"], "not_configured");
    owner.bind_as("writer-a", &owner.dir.join("writer-a"), Some(&evidence));
    let read = call(&service, SITE, actor(), delivery.clone());
    assert_eq!(read.status, 200, "{}", read.body);
    let handoff = &read.body["result"]["handoff"];
    assert_eq!(handoff["support"]["responsible_human"], "operator");
    assert_eq!(
        handoff["support"]["included_work"],
        "one synthetic correction"
    );
    assert!(!read.body.to_string().contains("synthetic support contact"));
    std::fs::write(evidence.join("service-handoff.json"), b"{}").unwrap();
    let changed = call(&service, SITE, actor(), delivery);
    assert_eq!(changed.body["result"]["unavailable"], "unreadable");
    assert!(changed.body["result"]["handoff"].is_null());

    // A writer reads no owner audit, claim register, or weekly review.
    for op in [
        json!({"kind":"audit","lead":lead}),
        json!({"kind":"claims"}),
        json!({"kind":"weekly"}),
    ] {
        assert_eq!(error(&call(&service, SITE, actor(), op)), "access_denied");
    }

    // The owner's audit is scoped to the one record asked for.
    owner.bind_as("operator", &owner.owner, None);
    let audit = call(&service, SITE, actor(), json!({"kind":"audit","lead":lead}));
    assert_eq!(audit.status, 200, "{}", audit.body);
    let entries = audit.body["result"].as_array().unwrap();
    assert!(entries.len() >= 2);
    assert!(entries.iter().all(|e| e["lead"] == json!(lead)));
    assert!(!audit.body.to_string().contains(&other));
    let claims = call(&service, SITE, actor(), json!({"kind":"claims"}));
    assert_eq!(claims.body["result"]["register"], json!([]));
    let weekly = call(&service, SITE, actor(), json!({"kind":"weekly"}));
    assert_eq!(weekly.status, 200);
    assert!(weekly.body["result"].is_null());
}

#[test]
fn delivery_and_records_show_verified_offboarding_read_only_without_paths() {
    use crate::task::sales::offboarding::tests::{kept, pending, removed};
    use crate::task::sales::offboarding::{Report, View};
    use crate::task::sales::tests::service_fixture::{self as fixture, Comparison, retain};
    let owner = owner(&[Effect::Create]);
    let service = Service::open_with_clock(&owner.config, clock).unwrap();
    let lead = apply(&service, "lead-one", &create("lead-one")).body["result"]["lead"]
        .as_str()
        .unwrap()
        .to_owned();
    let evidence = owner.dir.join("evidence");
    std::fs::create_dir(&evidence).unwrap();
    std::fs::set_permissions(&evidence, std::fs::Permissions::from_mode(0o700)).unwrap();
    let comparison = Comparison {
        manifest: retain(&evidence, "comparison.json", b"synthetic manifest"),
        report: retain(&evidence, "comparison-report.json", b"synthetic report"),
        candidate: retain(&evidence, "candidate.patch", b"synthetic candidate"),
        check: retain(&evidence, "independent-check", b"synthetic check"),
        decision: retain(&evidence, "buyer-decision", b"synthetic decision"),
        frozen_checks: vec![retain(&evidence, "frozen-command", b"synthetic frozen")],
    };
    let admission = fixture::admission(
        &evidence,
        1000,
        &lead,
        "synthetic-account",
        "offer-v1",
        comparison,
    );
    let handoff = admission.sources.handoff.sha256.clone();
    let mut store = Store::open_with_clock(&owner.host, clock).unwrap();
    let admin = store
        .authenticate(&Store::read_credential(&owner.owner).unwrap())
        .unwrap();
    let admit = json!({"schema":crate::task::sales::COMMAND_SCHEMA,"id":"admit-sale","lead":lead,
        "expected_revision":1,"operation":{"kind":"record_service_sale","admission":admission}});
    store
        .apply_with_evidence_root(
            &admin,
            &serde_json::to_vec(&admit).unwrap(),
            Some(&evidence),
        )
        .unwrap();
    drop(store);

    // Before any cleanup report, nothing reads as done.
    owner.bind_as("operator", &owner.owner, None);
    let delivery = json!({"kind":"delivery","lead":lead,"sale":"synthetic-sale"});
    let before = call(&service, SITE, actor(), delivery.clone());
    assert_eq!(before.status, 200, "{}", before.body);
    assert!(before.body["result"]["offboarding"].is_null());

    let report = Report {
        handoff_sha256: handoff,
        items: vec![
            removed(
                &evidence,
                "temporary-credentials",
                "synthetic-temporary-key",
                "synthetic-revoke-key",
            ),
            kept(&evidence, "test-data", 1800),
            pending("local-copies"),
        ],
    };
    let command = json!({"schema":crate::task::sales::COMMAND_SCHEMA,"id":"offboard","lead":lead,
        "expected_revision":2,"operation":{"kind":"record_offboarding","sale":"synthetic-sale","report":report}});
    let bytes = serde_json::to_vec(&command).unwrap();
    // The adapter only reads offboarding; it never records it.
    owner.bind_as("operator", &owner.owner, Some(&evidence));
    assert_ne!(apply(&service, "offboard", &bytes).status, 200);
    let mut store = Store::open_with_clock(&owner.host, clock).unwrap();
    let admin = store
        .authenticate(&Store::read_credential(&owner.owner).unwrap())
        .unwrap();
    store
        .apply_with_evidence_root(&admin, &bytes, Some(&evidence))
        .unwrap();
    drop(store);

    let read = call(&service, SITE, actor(), delivery);
    assert_eq!(read.status, 200, "{}", read.body);
    let view: View = serde_json::from_value(read.body["result"]["offboarding"].clone()).unwrap();
    let plain: Vec<String> = view.rows.iter().map(|r| r.state.plain()).collect();
    assert_eq!(
        plain,
        [
            "Removed 1970-01-01",
            "Kept until 1970-01-01 (required)",
            "Not yet removed (due 1970-01-01)",
        ]
    );
    let text = read.body.to_string();
    assert!(!text.contains("-operation.json") && !text.contains("-reread.json"));
    assert!(!text.contains("-requirement"));

    let records = call(
        &service,
        SITE,
        actor(),
        json!({"kind":"records","after":null,"limit":10}),
    );
    let views: Vec<RecordView> = serde_json::from_value(records.body["result"].clone()).unwrap();
    assert_eq!(views[0].offboarding, vec![view]);
}
