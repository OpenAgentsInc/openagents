//! WEB-16 acceptance: partner, referral, earnings, and payout views read only
//! original retained records under the viewer's own session, workspace, and
//! delegation; an invitation, an attribution, or an identity grants nothing.

use super::*;
use axum::extract::Query;
use coder::task::sales::partners::{Action, Proposal, Terms};
use coder::task::sales::remote::{self, Service};
use coder::task::sales::{COMMAND_SCHEMA, Command, NextAction, Operation, Role, Store};
use pay_ledger::PayoutState;
use pay_ledger::earnings::{Earning, Figures, Obligation, Payment, Statement};
use receipts::service_sale::Reference;

const PAGE: &str = "/cloud/app/partners";
const REFERRER: &str = "ref_0123456789abcdef0123456789abcdef";

fn d(c: char) -> String {
    format!("sha256:{}", c.to_string().repeat(64))
}

fn hex(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn private_dir(path: &std::path::Path) {
    std::fs::create_dir_all(path).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
}

fn private_file(path: &std::path::Path, bytes: &[u8]) {
    std::fs::write(path, bytes).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
}

// ---- The account service's referral records ----------------------------------------

fn identity() -> Value {
    json!({"id":REFERRER,"version":2,"kind":"partner","source_only":false})
}

fn binding() -> Value {
    json!({"id":"binding-1","customer":"alice","referrer":identity(),"policy_digest":d('7'),"accepted_decision":d('2')})
}

fn decision(sequence: u64, status: &str, review: Value, digest: String) -> Value {
    json!({"schema":"openagents.referral.attribution-decision.v1","customer":"alice","sequence":sequence,
        "prior":null,"request":format!("request-{sequence}"),"policy_digest":d('7'),"introduction":"captured_source",
        "status":status,"review":review,"referrer":identity(),"referrer_owner":"referrer-owner","source":null,
        "evidence":[],"reason":"synthetic","actor":"alice","confirmed":null,"at":50+sequence,"digest":digest})
}

fn records() -> BTreeMap<&'static str, Value> {
    let acceptance = |party: &str, actor: &str, digest: char| {
        json!({"party":party,"actor":actor,"request":format!("accept-{party}"),"account_revision":d('4'),
            "accepted_at":100,"manager_version":null,"manager_successor":null,"digest":d(digest)})
    };
    BTreeMap::from([
        (
            "acquisition",
            json!({"schema":"openagents.referral-source.v1","account":"alice","request":"join-1",
                "outcome":"captured","referrer":identity(),"consent_version":"consent-3","captured_at":40}),
        ),
        (
            "attribution",
            json!({"schema":"openagents.referral.attribution.v1","customer":"alice","status":"accepted",
                "binding":binding(),"commission_eligibility":true,"decisions":[
                    decision(1, "review", json!("self_referral"), d('8')),
                    decision(2, "accepted", Value::Null, d('2'))]}),
        ),
        (
            "workspace",
            json!({"schema":"openagents.referral.workspace-attribution.v1","workspace":"alice-personal",
                "status":"accepted","binding":binding(),"commission_eligibility":true}),
        ),
        (
            "agreement",
            json!({"agreement":{"schema":"openagents.referral.commission-agreement.v1","id":d('1'),
                "customer":"alice","binding":binding(),"attribution_decision":d('2'),"terms_digest":d('3'),
                "acceptances":{"customer":acceptance("customer","alice",'5'),"referrer":acceptance("referrer","referrer-owner",'6')}},
                "terms":{"terms":{"schema":"openagents.referral.commission-terms.v1","digest":d('3'),"version":"terms-v7"},
                    "published_at":90,"account_revision":d('4')},
                "state":"accepted-terms","terms_qualified":true,"active_for_new_transactions":true,
                "accrual_enabled":false,"payout_qualified":false,"payout_enabled":false}),
        ),
    ])
}

/// Serve alice's records to alice only, pinned to her own account.
fn referral(state: &Native, headers: &HeaderMap, name: &str) -> Response {
    if state.offline {
        return native_refusal(StatusCode::SERVICE_UNAVAILABLE);
    }
    let Some(account) = acting(headers, state) else {
        return native_refusal(StatusCode::UNAUTHORIZED);
    };
    if headers
        .get("x-openagents-referral-account")
        .and_then(|v| v.to_str().ok())
        != Some(account)
    {
        return native_refusal(StatusCode::CONFLICT);
    }
    let value = if account == "alice" {
        state.referral.get(name).cloned().unwrap_or(Value::Null)
    } else {
        Value::Null
    };
    Json(json!({"v":"openagents.accounts.v1","referral":value})).into_response()
}

async fn acquisition(State(state): State<Arc<Mutex<Native>>>, headers: HeaderMap) -> Response {
    referral(&state.lock().unwrap(), &headers, "acquisition")
}

async fn attribution(State(state): State<Arc<Mutex<Native>>>, headers: HeaderMap) -> Response {
    referral(&state.lock().unwrap(), &headers, "attribution")
}

async fn workspace_attribution(
    State(state): State<Arc<Mutex<Native>>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let state = state.lock().unwrap();
    if id != "alice-personal" {
        return referral(&state, &headers, "none");
    }
    referral(&state, &headers, "workspace")
}

async fn agreement(
    State(state): State<Arc<Mutex<Native>>>,
    Query(query): Query<BTreeMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    let state = state.lock().unwrap();
    if query.get("customer").map(String::as_str) != Some("alice") {
        return referral(&state, &headers, "none");
    }
    referral(&state, &headers, "agreement")
}

pub(super) fn native_routes() -> Router<Arc<Mutex<Native>>> {
    Router::new()
        .route("/v1/account/acquisition", get(acquisition))
        .route("/v1/account/attribution", get(attribution))
        .route(
            "/v1/workspaces/{id}/attribution",
            get(workspace_attribution),
        )
        .route("/v1/account/referral-agreement", get(agreement))
}

// ---- The payee owner's statement --------------------------------------------------

fn payee_statement() -> Statement {
    Statement {
        figures: Figures {
            earned_msat: 1000,
            accrued_msat: 300,
            reserved_msat: 0,
            consumed_msat: 700,
            sent_msat: 690,
            rounding_msat: 10,
            unverified_sent_msat: 0,
        },
        earnings: vec![Earning {
            sequence: 7,
            resource: "/v1/plugins/meeting-action-items".into(),
            plugin_id: Some("meeting-action-items".into()),
            release_id: Some("release-3184".into()),
            settled_at: 1_700_000_000,
            rule_version: 2,
            obligations: vec![
                Obligation {
                    role: "author".into(),
                    amount_msat: 300,
                    state: "accrued".into(),
                    payout: None,
                },
                Obligation {
                    role: "commission".into(),
                    amount_msat: 700,
                    state: "sent".into(),
                    payout: Some("payout-1".into()),
                },
            ],
        }],
        payouts: vec![
            Payment {
                cursor: 1,
                id: "payout-1".into(),
                amount_msat: 700,
                destination: "synthetic@spark.invalid".into(),
                rail: "spark".into(),
                state: PayoutState::Sent,
                wallet_reference: Some("transfer-1".into()),
                attempts: 1,
                created_at: 1,
                updated_at: 2,
                sent_msat: Some(690),
                fee_msat: Some(1),
                reason: None,
                reason_digest: None,
                lookup_required: false,
            },
            Payment {
                cursor: 2,
                id: "payout-2".into(),
                amount_msat: 300,
                destination: "synthetic@spark.invalid".into(),
                rail: "spark".into(),
                state: PayoutState::Unknown,
                wallet_reference: Some("transfer-2".into()),
                attempts: 2,
                created_at: 3,
                updated_at: 4,
                sent_msat: None,
                fee_msat: None,
                reason: Some("outcome_unresolved"),
                reason_digest: None,
                lookup_required: true,
            },
        ],
        next_earning: Some(7),
        next_payout: None,
    }
}

fn joined(payee: Value) -> Value {
    json!({"native_workspace":"alice-personal","statement":{"schema":"openagents.joined-statement.v1",
        "origin":"canonical-origin","customer":"customer-a","workspace":"canonical-a",
        "unit":{"kind":"millisatoshis"},"unit_scale":100_000_000_000u64,"balance":null,
        "snapshot":"b".repeat(64),"rows":[],"next":null,"scanned":0,"disclosure":[]},
        "payee":{"party":"referrer:referrer-owner","unit":{"kind":"millisatoshis"},"statement":payee},
        "payee_disclosure":"Payee earnings remain separate.","source_attribution":[],
        "attribution_disclosure":"x","native_projection":[],"native_projection_disclosure":"x"})
}

// ---- The sales owner host with one partner assignment -----------------------------

const BEARERS: [(&str, &str); 3] = [
    ("alice-sales", "synthetic-partner-bearer-alice"),
    ("alice-owner", "synthetic-partner-bearer-owner"),
    ("bob-sales", "synthetic-partner-bearer-bob"),
];

struct Owner {
    _temp: tempfile::TempDir,
    root: PathBuf,
    sources: PathBuf,
    lead: String,
    url: String,
    server: tokio::task::JoinHandle<()>,
}

impl Drop for Owner {
    fn drop(&mut self) {
        self.server.abort();
    }
}

fn retain(root: &std::path::Path, name: &str, bytes: &[u8]) -> Reference {
    std::fs::write(root.join(name), bytes).unwrap();
    Reference {
        path: name.into(),
        sha256: hex(bytes),
    }
}

fn command(id: &str, lead: &str, revision: u64, operation: Operation) -> Vec<u8> {
    serde_json::to_vec(&Command {
        schema: COMMAND_SCHEMA.into(),
        id: id.into(),
        lead: Some(lead.into()),
        expected_revision: revision,
        operation,
    })
    .unwrap()
}

async fn owner() -> Owner {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap().join("owner");
    private_dir(&root);
    let host = root.join("host");
    let mut store = Store::open(&host).unwrap();
    store
        .initialize("operator", &root.join("operator"))
        .unwrap();
    let admin = store
        .authenticate(&Store::read_credential(&root.join("operator")).unwrap())
        .unwrap();
    for human in ["writer-a", "writer-b"] {
        store
            .issue(&admin, human, Role::Writer, &root.join(human))
            .unwrap();
    }
    let writer = store
        .authenticate(&Store::read_credential(&root.join("writer-a")).unwrap())
        .unwrap();
    let at = now();
    let create = json!({"schema":COMMAND_SCHEMA,"id":"seed-lead","lead":null,"expected_revision":0,"operation":{"kind":"create","ownership_acceptance":"writer accepted responsibility","input":{
        "contact":"email:partner-prospect@synthetic.invalid","source":"synthetic private introduction","source_at":at-20,
        "details":{"account":"synthetic-account","jurisdiction":"synthetic jurisdiction record",
        "permission":{"state":"granted","reference":"synthetic-consent-v1","recorded_at":at-10,"expires_at":at+86_400,"channels":["email"]},
        "workflow":"one synthetic repository maintenance task","baseline_reference":"private-baseline-reference",
        "data":{"recipients":["human:operator","human:writer-a"],"permitted_use":"one agreed pilot; no marketing reuse","retain_until":at+2*86_400},
        "stage":"qualified","next":{"description":"review pilot scope","due_at":at+3600},"customer_decision":null,"readers":[]}}}});
    let lead = store
        .apply(&writer, &serde_json::to_vec(&create).unwrap())
        .unwrap()
        .lead;
    // The owner proposes one discovery assignment to writer-a.
    let sources = root.join("sources");
    private_dir(&sources);
    let mut proposal = Proposal {
        id: "introduction".into(),
        recipient_human: "writer-a".into(),
        expires_at: at + 3600,
        next: NextAction {
            description: "prepare one consented introduction".into(),
            due_at: at + 1800,
        },
        terms: Terms::Discovery {
            brief: retain(&sources, "brief", b"synthetic partner brief"),
            permitted_use: "private partner preparation".into(),
        },
        consent: retain(&sources, "consent", b"synthetic consent"),
        provenance: retain(&sources, "provenance", b"synthetic provenance"),
        approval: Reference {
            path: "placeholder.json".into(),
            sha256: "0".repeat(64),
        },
        commission: None,
    };
    let digest = store.partner_digest(&admin, &lead, &proposal).unwrap()["proposal_sha256"]
        .as_str()
        .unwrap()
        .to_owned();
    proposal.approval = retain(
        &sources,
        "approval.json",
        &serde_json::to_vec(&json!({"schema":"openagents.sales.partner-approval.v1",
            "pipeline_lead":lead,"assignment":"introduction","proposal_sha256":digest,
            "approved_by":"operator","approved_at":at,"allow_private_assignment":true}))
        .unwrap(),
    );
    let revision = store.show(&admin, &lead).unwrap().revision;
    store
        .apply_with_evidence_root(
            &admin,
            &command(
                "propose",
                &lead,
                revision,
                Operation::ProposePartner { proposal },
            ),
            Some(&sources),
        )
        .unwrap();
    drop(store);
    let config = root.join("remote.json");
    let principals = ["writer-a", "operator", "writer-b"];
    let workspaces = [
        ("alice", "alice-personal"),
        ("alice", "alice-team"),
        ("bob", "bob-personal"),
    ];
    let bindings: Vec<Value> = BEARERS
        .iter()
        .zip(principals.iter().zip(workspaces))
        .map(|((id, bearer), (principal, (account, workspace)))| {
            json!({"id":id,"account":account,"workspace":workspace,"members_epoch":3,
                "principal":principal,"credential":root.join(principal),
                "client_digest":hex(bearer.as_bytes()),"effects":[]})
        })
        .collect();
    private_file(
        &config,
        &serde_json::to_vec(&json!({"schema":remote::CONFIG_SCHEMA,"root":host,
            "journal":root.join("journal"),"bindings":bindings}))
        .unwrap(),
    );
    let service = Arc::new(Service::open(&config).unwrap());
    let router = crate::sales_remote::router(service);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!(
        "http://{}{}",
        listener.local_addr().unwrap(),
        crate::sales_remote::PATH
    );
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    Owner {
        _temp: temp,
        root,
        sources,
        lead,
        url,
        server,
    }
}

impl Owner {
    fn attach(&self, fixture: &mut Fixture) {
        let site = self.root.join("site");
        private_dir(&site);
        let workspaces = [
            ("alice", "alice-personal"),
            ("alice", "alice-team"),
            ("bob", "bob-personal"),
        ];
        let delegations: Vec<Value> = BEARERS
            .iter()
            .zip(workspaces)
            .map(|((id, bearer), (account, workspace))| {
                let file = site.join(format!("{id}.bearer"));
                private_file(&file, bearer.as_bytes());
                json!({"id":id,"account":account,"workspace":workspace,"members_epoch":3,
                    "endpoint":self.url,"binding":id,"bearer_file":file,"development_loopback":true})
            })
            .collect();
        let path = site.join("sales.json");
        private_file(
            &path,
            &serde_json::to_vec(&json!({"schema":super::super::sales::SCHEMA,
                "directory":site,"delegations":delegations}))
            .unwrap(),
        );
        fixture.config.cloud_sales = Some(Arc::new(
            super::super::sales::Delegations::load(&path).unwrap(),
        ));
        fixture.site = crate::router(fixture.config.clone());
    }

    /// The named recipient accepts the exact proposal with its owner.
    fn accept(&self) {
        let mut store = Store::open(&self.root.join("host")).unwrap();
        let writer = store
            .authenticate(&Store::read_credential(&self.root.join("writer-a")).unwrap())
            .unwrap();
        let invitation = store
            .partner_show(&writer, &self.lead, "introduction")
            .unwrap();
        let digest = invitation["invitation"]["proposal_sha256"]
            .as_str()
            .unwrap()
            .to_owned();
        let revision = invitation["invitation"]["pipeline_revision"]
            .as_u64()
            .unwrap();
        let evidence = retain(&self.sources, "accepted", b"synthetic recipient acceptance");
        store
            .apply_with_evidence_root(
                &writer,
                &command(
                    "accept",
                    &self.lead,
                    revision,
                    Operation::AdvancePartner {
                        assignment: "introduction".into(),
                        action: Action::Accept {
                            proposal_sha256: digest,
                            evidence,
                        },
                    },
                ),
                Some(&self.sources),
            )
            .unwrap();
    }
}

async fn fetch(fixture: &Fixture, cookies: &Cookies, path: &str) -> Answer {
    request(&fixture.site, Method::GET, path, cookies, None, None).await
}

async fn select(fixture: &Fixture, cookies: &mut Cookies, workspace: &str) {
    let page = fetch(fixture, cookies, "/cloud/app").await;
    let csrf = action_token(&page.body, "/cloud/select-workspace", Some(workspace));
    let answer = request(
        &fixture.site,
        Method::POST,
        "/cloud/select-workspace",
        cookies,
        Some(&form(&[("workspace", workspace), ("csrf", &csrf)])),
        Some(ORIGIN),
    )
    .await;
    cookies.apply(&answer);
}

fn no_money_controls(body: &str) {
    // Only the shell's own workspace switch, sign-out, and theme toggle
    // (its no-JavaScript fallback) post anywhere.
    for form in body.split("method=\"post\" action=\"").skip(1) {
        let action = form.split('"').next().unwrap();
        assert!(
            matches!(
                action,
                "/cloud/select-workspace" | "/cloud/sign-out" | "/theme"
            ),
            "a page offered a control: {action}"
        );
    }
}

#[tokio::test]
async fn referrals_and_earnings_are_original_scoped_records_without_money_controls() {
    let fixture = fixture().await;
    {
        let mut state = fixture.state.lock().unwrap();
        state.referral = records();
        state.statement = Some(joined(serde_json::to_value(payee_statement()).unwrap()));
    }
    let mut cookies = login(&fixture, "alice").await;
    // Without a selected workspace the section names why it is unavailable.
    let overview = fetch(&fixture, &cookies, "/cloud/app").await;
    assert!(overview.body.contains("Partners · Unavailable"));
    assert_eq!(
        fetch(&fixture, &cookies, PAGE).await.status,
        StatusCode::FORBIDDEN
    );
    select(&fixture, &mut cookies, "alice-personal").await;
    let overview = fetch(&fixture, &cookies, "/cloud/app").await;
    assert!(overview.body.contains("href=\"/cloud/app/partners\""));

    let page = fetch(&fixture, &cookies, PAGE).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    private(&page);
    no_money_controls(&page.body);
    let body = &page.body;
    // The original source and permanent binding, with no commission implied.
    assert!(body.contains(&format!("{REFERRER} version 2 · partner")));
    assert!(body.contains("creates no commission"));
    assert!(body.contains("Permanent binding"));
    assert!(body.contains("attribution alone accrues nothing"));
    // A self-referral decision stays under review and pays nothing.
    assert!(body.contains("under review: self referral"));
    assert!(body.contains("Workspace alice-personal adopts binding binding-1"));
    // The accepted agreement version; accrual and payout are not enabled.
    assert!(body.contains("version terms-v7"));
    assert!(body.contains("<dt>Accrual</dt><dd>not enabled</dd>"));
    assert!(body.contains("<dt>Payout</dt><dd>not enabled</dd>"));
    // Payee figures are the owner's own; roles stay separate lines.
    assert!(body.contains("<dt>Earned</dt><dd>1000 msat</dd>"));
    assert!(body.contains("commission share 700 msat · sent · payout payout-1"));
    assert!(body.contains("author share 300 msat · accrued"));
    assert!(body.contains("rail amount 690 msat"));
    // An unresolved payout stays unresolved; nothing here can move money.
    assert!(body.contains("Payout payout-2 · 300 msat"));
    assert!(body.contains("Outcome unresolved"));
    assert!(body.contains("cannot start, retry, or redirect a payout"));
    assert!(body.contains("after_earning=7"));
    // No sales delegation: partner assignments are unavailable.
    assert!(body.contains("Partner assignments · Unavailable"));

    // The scoped export carries the same original records.
    let export = fetch(&fixture, &cookies, &format!("{PAGE}/export")).await;
    assert_eq!(export.status, StatusCode::OK);
    assert_eq!(export.headers[header::CONTENT_TYPE], "application/x-ndjson");
    assert_eq!(export.headers[header::CACHE_CONTROL], "no-store, private");
    let lines: Vec<Value> = export
        .body
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines[0]["value"]["account"], "alice");
    let payee = lines.iter().find(|l| l["record"] == "payee").unwrap();
    assert_eq!(
        payee["value"]["statement"],
        serde_json::to_value(payee_statement()).unwrap()
    );
    assert!(lines.iter().any(|l| l["record"] == "commission_agreement"));

    // Bounded selections only.
    for bad in ["?customer=../x", "?after_earning=-1", "?after_lead=lead"] {
        assert_eq!(
            fetch(&fixture, &cookies, &format!("{PAGE}{bad}"))
                .await
                .status,
            StatusCode::BAD_REQUEST,
            "{bad}"
        );
    }

    // A payee record this page cannot type exactly is refused, not partly shown.
    let mut changed = serde_json::to_value(payee_statement()).unwrap();
    changed["figures"]["estimated_msat"] = json!(5);
    fixture.state.lock().unwrap().statement = Some(joined(changed));
    let refused = fetch(&fixture, &cookies, PAGE).await;
    assert_eq!(refused.status, StatusCode::CONFLICT);
    assert!(refused.body.contains("Earnings record unreadable"));
    assert!(!refused.body.contains("1000 msat"));
    fixture.state.lock().unwrap().statement =
        Some(joined(serde_json::to_value(payee_statement()).unwrap()));

    // Another account sees none of alice's referral, payee, or agreement records.
    let mut bob = login(&fixture, "bob").await;
    select(&fixture, &mut bob, "bob-personal").await;
    let theirs = fetch(&fixture, &bob, PAGE).await;
    assert_eq!(theirs.status, StatusCode::OK, "{}", theirs.body);
    private(&theirs);
    for leaked in [REFERRER, "terms-v7", "payout-1", "binding-1"] {
        assert!(!theirs.body.contains(leaked), "{leaked}");
    }
    assert!(theirs.body.contains("No payee read is approved"));
    // Naming alice as a customer reads no agreement bob is not party to.
    let named = fetch(&fixture, &bob, &format!("{PAGE}?customer=alice")).await;
    assert!(!named.body.contains("terms-v7"));

    // Alice's other workspace does not inherit the personal workspace's adoption.
    select(&fixture, &mut cookies, "alice-team").await;
    let team = fetch(&fixture, &cookies, PAGE).await;
    assert!(
        team.body
            .contains("This workspace has adopted no attribution.")
    );
    assert!(!team.body.contains("payout-1"));

    // A revoked session reads nothing.
    fixture.state.lock().unwrap().revoked.insert("alice".into());
    let revoked = fetch(&fixture, &cookies, PAGE).await;
    assert_ne!(revoked.status, StatusCode::OK);
    assert!(!revoked.body.contains(REFERRER));
}

#[tokio::test]
async fn partner_assignments_follow_acceptance_and_owner_projections_stay_with_the_owner() {
    let mut fixture = fixture().await;
    let owner = owner().await;
    owner.attach(&mut fixture);
    let mut alice = login(&fixture, "alice").await;
    select(&fixture, &mut alice, "alice-personal").await;

    // The named recipient sees only the pending invitation: no terms, no obligation.
    let page = fetch(&fixture, &alice, PAGE).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    private(&page);
    no_money_controls(&page.body);
    assert!(page.body.contains("introduction · Pending invitation"));
    assert!(page.body.contains("creates no obligation"));
    assert!(!page.body.contains("private partner preparation"));
    assert!(!page.body.contains("partner-prospect@synthetic.invalid"));

    // After the exact recipient accepts, the accepted scope is shown.
    owner.accept();
    let page = fetch(&fixture, &alice, PAGE).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    assert!(page.body.contains("introduction · Accepted"));
    assert!(
        page.body
            .contains("discovery · private partner preparation")
    );
    assert!(
        page.body
            .contains("<dt>Responsible human</dt><dd>writer-a</dd>")
    );
    assert!(
        page.body
            .contains("<dt>Referral agreement</dt><dd>none</dd>")
    );
    assert!(page.body.contains("accepted · writer-a"));
    // A writer gets no owner desk or earned ledger.
    assert!(!page.body.contains("Earned sales"));
    assert!(!page.body.contains("Arthur"));
    let export = fetch(&fixture, &alice, &format!("{PAGE}/export")).await;
    let assignment = export
        .body
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .find(|l| l["record"] == "partner_assignment")
        .unwrap();
    assert_eq!(assignment["value"]["delegation"], "alice-sales");
    assert_eq!(assignment["value"]["view"]["scope"], "accepted");
    assert_eq!(assignment["value"]["view"]["authority_granted"], false);
    assert!(!export.body.contains("partner-prospect@synthetic.invalid"));

    // Another principal not named on the assignment sees nothing of it.
    let mut bob = login(&fixture, "bob").await;
    select(&fixture, &mut bob, "bob-personal").await;
    let theirs = fetch(&fixture, &bob, PAGE).await;
    assert_eq!(theirs.status, StatusCode::OK, "{}", theirs.body);
    assert!(
        theirs
            .body
            .contains("No partner assignment or invitation names you.")
    );
    assert!(!theirs.body.contains("introduction"));

    // The owner binding (another workspace) projects Arthur, Vanna, and the
    // earned ledger from current records; unbound desks stay unavailable.
    select(&fixture, &mut alice, "alice-team").await;
    let owned = fetch(&fixture, &alice, PAGE).await;
    assert_eq!(owned.status, StatusCode::OK, "{}", owned.body);
    no_money_controls(&owned.body);
    assert!(owned.body.contains("sales owner"));
    assert!(owned.body.contains("introduction · Accepted"));
    assert!(owned.body.contains("Arthur · partner desk"));
    assert!(owned.body.contains("Vanna · affiliate desk"));
    assert!(owned.body.contains("no current approved projection"));
    assert!(owned.body.contains("Reading it rings no bell"));
    assert!(owned.body.contains("0 earned sales"));
    assert!(!owned.body.contains("alice-sales"));
}
