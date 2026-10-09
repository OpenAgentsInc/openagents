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
