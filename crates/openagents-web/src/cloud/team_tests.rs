//! WEB-12 acceptance: team membership, invitations, roles, removal epochs,
//! recovery, narrowing policy, concurrent holds, and bounded reports, all
//! through the browser adapter against a synthetic native team book.

use super::*;
use axum::routing::{delete, patch};
use receipts::team_policy::{
    Capability, Effect, Placement, PlacementKind, Reference, Revision, Rule, SCHEMA, Source, Terms,
};

const TEAM: &str = "alice-team";

fn hash(seed: char) -> String {
    format!("sha256:{}", seed.to_string().repeat(64))
}

#[derive(Clone)]
pub(super) struct Invitation {
    role: String,
    status: String,
    token: String,
    expires: u64,
    accepted_by: Option<String>,
}

/// One synthetic native team: alice owns alice-team; bob may join.
#[derive(Clone)]
pub(super) struct Book {
    members: BTreeMap<String, (String, String)>,
    invitations: BTreeMap<String, Invitation>,
    epoch: u64,
    policy: Option<Revision>,
    budget: Value,
    budget_version_digest: String,
    reserved: u64,
    unknown: u64,
    recovery: BTreeMap<String, String>,
    access: Vec<Value>,
    next: u32,
}

impl Book {
    fn new() -> Self {
        let mut book = Self {
            members: [("alice".into(), ("owner".into(), "active".into()))].into(),
            invitations: BTreeMap::new(),
            epoch: 3,
            policy: Some(revision(1, now() + 86_400, 2)),
            budget: json!({"schema":"openagents.money.budgets.v1","version":1,"currency":"USD","scale":1_000_000,
                "route":"gateway-monetary-v1","effective_from":0,
                "workspace":{"cap":1000,"alert_at":500},
                "teams":{"core":{"cap":800,"alert_at":400}},
                "people":{"alice":{"team":"core","limit":{"cap":600,"alert_at":300}}}}),
            budget_version_digest: String::new(),
            reserved: 300,
            unknown: 100,
            recovery: BTreeMap::new(),
            access: Vec::new(),
            next: 0,
        };
        book.budget_version_digest = digest(&book.budget);
        book
    }
    fn active(&self, account: &str) -> Option<&str> {
        self.members
            .get(account)
            .filter(|(_, status)| status == "active")
            .map(|(role, _)| role.as_str())
    }
    fn admin(&self, account: &str) -> bool {
        matches!(self.active(account), Some("owner" | "admin"))
    }
    fn log(&mut self, actor: &str, action: &str, detail: Option<&str>) {
        self.access.push(json!({"at":now(),"actor":actor,"action":action,"workspace":TEAM,"session":null,"detail":detail}));
    }
}

fn digest(value: &Value) -> String {
    use sha2::Digest;
    format!(
        "sha256:{}",
        sha2::Sha256::digest(value.to_string().as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    )
}

fn rule(seed: char, model: &str) -> Rule {
    Rule {
        effect: Effect {
            capability: Capability::SystemOne,
            release: hash(seed),
            model: Some(model.into()),
            plugin: None,
            recipients: vec![hash('e')],
            source: Source {
                request: hash('c'),
                material: hash('d'),
            },
            placement: Placement {
                kind: PlacementKind::LocalGateway,
                identity: hash('f'),
            },
        },
        data_classes: vec!["synthetic-notes".into()],
    }
}

fn revision(version: u64, expires: u64, rules: usize) -> Revision {
    let mut revision = Revision {
        schema: SCHEMA.into(),
        workspace: TEAM.into(),
        terms: Terms {
            version,
            expires_unix: expires,
            rules: [rule('a', "kev-0.6b"), rule('b', "kev-1b")]
                .into_iter()
                .take(rules)
                .collect(),
        },
        supersedes: None,
        reviewer: "alice".into(),
        reviewer_epoch: 1,
        owner: "alice".into(),
        owner_epoch: 1,
        reviewed_at: now(),
        digest: String::new(),
    };
    revision.digest = revision.compute_digest();
    revision
}

pub(super) fn details(book: &Book, account: &str, removed: bool) -> Response {
    let mut workspaces = Vec::new();
    if !removed {
        workspaces.push(json!({"id":format!("{account}-personal"),"name":format!("{account} personal"),"role":"owner"}));
        if let Some(role) = book.active(account) {
            workspaces.push(json!({"id":TEAM,"name":"Alice team","role":role}));
        }
    }
    Json(json!({"account":{"id":account,"label":format!("{account} <account>"),"principals":[CANARY]},"workspaces":workspaces})).into_response()
}

pub(super) fn workspace(book: &Book, account: &str, id: &str, headers: &HeaderMap) -> Response {
    if let Some(expected) = headers.get("x-openagents-team-account")
        && expected.to_str().ok() != Some(account)
    {
        return native_refusal(StatusCode::CONFLICT);
    }
    if id == format!("{account}-personal") {
        return Json(json!({"v":"openagents.accounts.v1","workspace":{"id":id,"name":"personal","kind":"personal","tenant":"synthetic","seats":null,"members_epoch":1},"role":"owner","members":[{"account":account,"role":"owner","status":"active"}],"invitations":[]})).into_response();
    }
    let Some(role) = book.active(account).filter(|_| id == TEAM) else {
        return native_refusal(StatusCode::FORBIDDEN);
    };
    let members: Vec<Value> = book
        .members
        .iter()
        .map(|(a, (r, s))| json!({"account":a,"role":r,"status":s}))
        .collect();
    let invitations: Vec<Value> = if book.admin(account) {
        book.invitations.iter().map(|(id, i)| json!({"id":id,"role":i.role,"status":i.status,"invited_by":"alice","expires_unix":i.expires,"accepted_by":i.accepted_by})).collect()
    } else {
        Vec::new()
    };
    Json(json!({"v":"openagents.accounts.v1","workspace":{"id":TEAM,"name":"Alice team","kind":"organization","tenant":"synthetic","seats":4,"members_epoch":book.epoch},"role":role,"members":members,"invitations":invitations})).into_response()
}

type Shared = State<Arc<Mutex<Native>>>;

/// The acting account, with the pinned team-account header when present.
fn actor(headers: &HeaderMap, state: &Native) -> Result<&'static str, Response> {
    let account = acting(headers, state).ok_or_else(|| native_refusal(StatusCode::UNAUTHORIZED))?;
    if let Some(expected) = headers.get("x-openagents-team-account")
        && expected.to_str().ok() != Some(account)
    {
        return Err(native_refusal(StatusCode::CONFLICT));
    }
    Ok(account)
}

fn denied(status: StatusCode) -> Response {
    (
        status,
        Json(json!({"error":{"code":"team_denied","message":CANARY}})),
    )
        .into_response()
}

macro_rules! book {
    ($state:ident, $headers:ident, $id:ident) => {{
        let actor = match actor(&$headers, &$state) {
            Ok(value) => value,
            Err(response) => return response,
        };
        if $id != TEAM
            || $state
                .team
                .as_ref()
                .is_none_or(|b| b.active(actor).is_none())
        {
            return denied(StatusCode::FORBIDDEN);
        }
        (actor, $state.team.as_mut().unwrap())
    }};
}

async fn native_invite(
    State(state): Shared,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let mut state = state.lock().unwrap();
    let (actor, book) = book!(state, headers, id);
    if !book.admin(actor) {
        return denied(StatusCode::FORBIDDEN);
    }
    book.next += 1;
    let invitation = format!("inv{}", book.next);
    let token = format!("inv_{invitation}.synthetic-invitation-{}", book.next);
    let role = body["role"].as_str().unwrap().to_string();
    let expires = now() + body["ttl_secs"].as_u64().unwrap();
    book.invitations.insert(
        invitation.clone(),
        Invitation {
            role: role.clone(),
            status: "pending".into(),
            token: token.clone(),
            expires,
            accepted_by: None,
        },
    );
    book.log(actor, "invite", Some(&invitation));
    Json(json!({"v":"openagents.accounts.v1","invitation":{"id":invitation,"workspace":TEAM,"role":role,"expires_unix":expires},"token":token})).into_response()
}

async fn native_withdraw(
    State(state): Shared,
    Path((id, invitation)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    let mut state = state.lock().unwrap();
    let (actor, book) = book!(state, headers, id);
    if !book.admin(actor) {
        return denied(StatusCode::FORBIDDEN);
    }
    match book.invitations.get_mut(&invitation) {
        Some(i) if i.status == "pending" => i.status = "revoked".into(),
        _ => return denied(StatusCode::CONFLICT),
    }
    Json(json!({"v":"openagents.accounts.v1","invitation":{"id":invitation,"status":"revoked"}}))
        .into_response()
}

async fn native_accept(
    State(state): Shared,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let mut state = state.lock().unwrap();
    let actor = match actor(&headers, &state) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let Some(book) = state.team.as_mut() else {
        return denied(StatusCode::FORBIDDEN);
    };
    let token = body["token"].as_str().unwrap_or("");
    let Some((id, invitation)) = book.invitations.iter_mut().find(|(_, i)| i.token == token) else {
        return denied(StatusCode::BAD_REQUEST);
    };
    if body["workspace"] != TEAM
        || body["role"].as_str() != Some(invitation.role.as_str())
        || invitation.status != "pending"
        || invitation.expires <= now()
    {
        return denied(StatusCode::BAD_REQUEST);
    }
    invitation.status = "accepted".into();
    invitation.accepted_by = Some(actor.into());
    let (id, role) = (id.clone(), invitation.role.clone());
    book.members
        .insert(actor.into(), (role.clone(), "active".into()));
    book.epoch += 1;
    book.log(actor, "invite-accept", Some(&id));
    Json(json!({"v":"openagents.accounts.v1","membership":{"workspace":TEAM,"account":actor,"role":role}})).into_response()
}

async fn native_member(
    State(state): Shared,
    Path((id, account)): Path<(String, String)>,
    method: Method,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let mut state = state.lock().unwrap();
    let (actor, book) = book!(state, headers, id);
    if !book.admin(actor) || account == actor || book.active(&account).is_none_or(|r| r == "owner")
    {
        return denied(StatusCode::FORBIDDEN);
    }
    let entry = book.members.get_mut(&account).unwrap();
    if method == Method::DELETE {
        entry.1 = "revoked".into();
    } else {
        let body: Value = serde_json::from_slice(&body).unwrap();
        entry.0 = body["role"].as_str().unwrap().into();
    }
    let member = json!({"account":account,"role":entry.0,"status":entry.1});
    book.epoch += 1;
    book.log(actor, "member-change", Some(&account));
    Json(json!({"v":"openagents.accounts.v1","membership":member})).into_response()
}

async fn native_recovery(
    State(state): Shared,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let mut state = state.lock().unwrap();
    let (actor, book) = book!(state, headers, id);
    let target = body["account"].as_str().unwrap_or("").to_string();
    if !book.admin(actor) || book.active(&target).is_none() {
        return denied(StatusCode::FORBIDDEN);
    }
    book.next += 1;
    let token = format!("rcv_synthetic{}", book.next);
    book.recovery.insert(token.clone(), target.clone());
    book.log(actor, "recovery-issue", Some(&target));
    Json(json!({"v":"openagents.accounts.v1","recovery":{"user":target,"issued_at":now(),"expires_at":now()+3600},"token":token})).into_response()
}

async fn native_redeem(
    State(state): Shared,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    assert!(headers.get(header::AUTHORIZATION).is_none());
    let mut state = state.lock().unwrap();
    let Some(book) = state.team.as_mut() else {
        return denied(StatusCode::FORBIDDEN);
    };
    let Some(account) = book.recovery.remove(body["token"].as_str().unwrap_or("")) else {
        return denied(StatusCode::BAD_REQUEST);
    };
    Json(json!({"account":account,"key":{"id":"key-recovered","tenant":"synthetic"},"key_token":"oak_recovered.synthetic-replacement"})).into_response()
}

async fn native_access(
    State(state): Shared,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let mut state = state.lock().unwrap();
    let (actor, book) = book!(state, headers, id);
    let admin = book.admin(actor);
    let access: Vec<&Value> = book
        .access
        .iter()
        .filter(|e| admin || e["actor"] == actor)
        .collect();
    Json(json!({"v":"openagents.accounts.v1","access":access})).into_response()
}

fn policy_view(book: &Book, actor: &str) -> Response {
    let Some(revision) = &book.policy else {
        return denied(StatusCode::FORBIDDEN);
    };
    let reference: Reference = revision.reference();
    Json(json!({"schema":SCHEMA,"reference":reference,"reviewed":book.admin(actor).then(|| revision.clone()),
        "enabled":["systemone-local"],"unsupported":["plugin","cloud","customer-host"]}))
    .into_response()
}

async fn native_policy(
    State(state): Shared,
    Path(id): Path<String>,
    method: Method,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let mut state = state.lock().unwrap();
    let (actor, book) = book!(state, headers, id);
    if headers.get("x-workspace-id").and_then(|v| v.to_str().ok()) != Some(TEAM) {
        return denied(StatusCode::CONFLICT);
    }
    if method == Method::PUT {
        let change: receipts::team_policy::Change = serde_json::from_slice(&body).unwrap();
        let current = book.policy.clone().unwrap();
        if !book.admin(actor)
            || change.expected_digest.as_deref() != Some(current.digest.as_str())
            || !change.terms.narrows(&current.terms)
        {
            return denied(StatusCode::FORBIDDEN);
        }
        let mut next = current.clone();
        next.terms = change.terms;
        next.supersedes = Some(current.digest.clone());
        next.reviewer = actor.into();
        next.digest = next.compute_digest();
        book.policy = Some(next);
    }
    policy_view(book, actor)
}

fn budget_answer(book: &Book, actor: &str) -> Response {
    let position = |limit: &Value| {
        let cap = limit["cap"].as_u64().unwrap();
        let used = book.reserved + book.unknown;
        json!({"level":"workspace","cap":cap,"alert_at":limit["alert_at"],"reserved":book.reserved,"unknown":book.unknown,
            "settled_net":0,"used":used,"remaining":cap.saturating_sub(used),"alert":"clear"})
    };
    let doc = &book.budget;
    let admin = book.admin(actor);
    let teams: serde_json::Map<String, Value> = doc["teams"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| (k.clone(), position(v)))
        .collect();
    let people: serde_json::Map<String, Value> = doc["people"]
        .as_object()
        .unwrap()
        .iter()
        .filter(|(k, _)| admin || *k == actor)
        .map(|(k, v)| (k.clone(), position(&v["limit"])))
        .collect();
    Json(json!({"v":"openagents.money.budgets.v1","workspace":TEAM,"account":actor,"role":book.active(actor),
        "account_revision":"rev","ledger_head":"head","as_of":now(),
        "budget":{"schema":"openagents.money.budgets.v1","policy":book.budget_version_digest,"version":doc["version"],
            "effective_from":0,"activated_at":0,"currency":"USD","scale":1_000_000,"route":"gateway-monetary-v1",
            "scope":if admin {"workspace"} else {"member"},"unattributed_used":0,"workspace":position(&doc["workspace"]),
            "teams":teams,"people":people,"requested":null,"blocked":null},
        "policy_document":admin.then(|| doc.clone()),"enabled_route":"gateway-monetary-v1","cross_product_budgets":false,
        "limitations":["Caps are cumulative in the native monetary currency."]}))
    .into_response()
}

async fn native_budgets(
    State(state): Shared,
    Path(id): Path<String>,
    method: Method,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let mut state = state.lock().unwrap();
    let (actor, book) = book!(state, headers, id);
    if method == Method::PUT {
        let input: Value = serde_json::from_slice(&body).unwrap();
        if book.active(actor) != Some("owner")
            || input["expected_policy"].as_str() != Some(book.budget_version_digest.as_str())
        {
            return denied(StatusCode::CONFLICT);
        }
        book.budget = input["policy"].clone();
        book.budget_version_digest = digest(&book.budget);
    }
    budget_answer(book, actor)
}

fn report_row(task: &str, member: &str) -> Value {
    json!({"task":task,"payer_workspace":TEAM,"original_member":{"account":member},"receipt":hash('1'),
        "team_policy_reference":hash('2'),"budget_policy_reference":hash('3'),"state":"settled",
        "service_outcome":null,"requested_artifact":null,"served_artifact":null,"model_reference":null,
        "plugin_release":null,"placement":"local-gateway","wait_ms":12,"total_service_ms":40,"resolved_at":null,
        "hold_phase":"settled","hold_reference":format!("{task}#1"),"price_reference":hash('4'),
        "statement_reference":hash('5'),"reserved":300,"charged":120,"refunded":0,"provider_cost":null,
        "hosting_cost":null,"evidence":{"status":"accepted","accepted":true}})
}

async fn native_reports(
    State(state): Shared,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let mut state = state.lock().unwrap();
    let (actor, book) = book!(state, headers, id);
    let admin = book.admin(actor);
    let rows: Vec<Value> = [
        report_row("task-alice", "alice"),
        report_row("task-bob", "bob"),
    ]
    .into_iter()
    .filter(|r| admin || r["original_member"]["account"] == actor)
    .collect();
    Json(json!({"schema":"openagents.team-report.v1","workspace":TEAM,"scope":if admin {"workspace"} else {"member"},
        "account_revision":"rev-1","statement_reference":hash('6'),"receipt_log_reference":hash('7'),
        "unit":{"currency":"USD","scale":1_000_000},"rows":rows,
        "totals":{"tasks":rows.len(),"accepted":rows.len(),"failed":0,"delivered":rows.len(),"unknown_charges":0,"known_charges":rows.len(),"refunded":0},
        "more":false,"maximum_rows":200,"wallet_liquidity":null,"production_qualification":false}))
    .into_response()
}

pub(super) fn native_routes() -> Router<Arc<Mutex<Native>>> {
    Router::new()
        .route("/v1/workspaces/{id}/invitations", post(native_invite))
        .route(
            "/v1/workspaces/{id}/invitations/{invitation}",
            delete(native_withdraw),
        )
        .route("/v1/invitations/accept-reviewed", post(native_accept))
        .route(
            "/v1/workspaces/{id}/members/{account}",
            patch(native_member).delete(native_member),
        )
        .route("/v1/workspaces/{id}/recovery", post(native_recovery))
        .route("/v1/recovery/redeem", post(native_redeem))
        .route("/v1/workspaces/{id}/access", get(native_access))
        .route(
            "/v1/workspaces/{id}/team-policy",
            get(native_policy).put(native_policy),
        )
        .route(
            "/v1/workspaces/{id}/budgets",
            get(native_budgets).put(native_budgets),
        )
        .route("/v1/workspaces/{id}/reports", get(native_reports))
}

// ---- Browser acceptance ---------------------------------------------------------

fn qualification(fixture: &Fixture, surface: &str, origin: &str, lanes: &[&str]) -> PathBuf {
    let directory = fixture._root.path().canonicalize().unwrap().join("private");
    let path = directory.join(format!("team-{surface}-{}.json", lanes.len()));
    std::fs::write(
        &path,
        serde_json::to_vec(&json!({"schema":crate::cloud::team::SCHEMA,"surface":surface,"origin":origin,"lanes":lanes,"evidence":"synthetic browser qualification"})).unwrap(),
    )
    .unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    path
}

/// A fixture with a native team book and the full browser lane qualified.
async fn team_fixture() -> Fixture {
    let mut fixture = fixture().await;
    fixture.state.lock().unwrap().team = Some(Book::new());
    let path = qualification(
        &fixture,
        "browser",
        ORIGIN,
        &["membership", "recovery", "policy", "budgets", "reports"],
    );
    fixture.config.cloud_team = Some(Arc::new(
        crate::cloud::team::Qualification::load(&path).unwrap(),
    ));
    fixture.site = crate::router(fixture.config.clone());
    fixture
}

async fn read(fixture: &Fixture, cookies: &Cookies, path: &str) -> Answer {
    request(&fixture.site, Method::GET, path, cookies, None, None).await
}

async fn post_form(fixture: &Fixture, cookies: &Cookies, path: &str, input: &str) -> Answer {
    request(
        &fixture.site,
        Method::POST,
        path,
        cookies,
        Some(input),
        Some(ORIGIN),
    )
    .await
}

async fn choose(fixture: &Fixture, cookies: &mut Cookies, workspace: &str) {
    let page = read(fixture, cookies, "/cloud/app").await;
    let csrf = action_token(&page.body, "/cloud/select-workspace", Some(workspace));
    let answer = post_form(
        fixture,
        cookies,
        "/cloud/select-workspace",
        &form(&[("workspace", workspace), ("csrf", &csrf)]),
    )
    .await;
    assert_eq!(answer.status, StatusCode::SEE_OTHER, "{}", answer.body);
    cookies.apply(&answer);
}

/// The CSRF ticket in the form posting to `action` that carries `marker`.
fn form_token(html: &str, action: &str, marker: &str) -> String {
    html.split("<form ")
        .skip(1)
        .find_map(|form| {
            let form = form.split("</form>").next().unwrap();
            (form.contains(&format!("action=\"{action}\"")) && form.contains(marker))
                .then(|| field(form, "csrf"))
        })
        .unwrap_or_else(|| panic!("no {action} form with {marker}"))
}

/// A valid ticket minted directly, to prove the server refuses on role.
async fn minted(fixture: &Fixture, cookies: &Cookies, scope: &str, target: &str) -> String {
    let service = fixture.config.cloud.as_ref().unwrap();
    let mut headers = HeaderMap::new();
    headers.insert(header::HOST, HOST.parse().unwrap());
    headers.insert(header::COOKIE, cookies.header().parse().unwrap());
    let viewer = service.authenticate(&headers).await.unwrap();
    service.csrf(&headers, &viewer, scope, target).unwrap()
}

fn team_book(fixture: &Fixture) -> Book {
    fixture.state.lock().unwrap().team.clone().unwrap()
}

#[tokio::test]
async fn only_an_explicit_browser_qualification_enables_the_team_lane() {
    let mut fixture = fixture().await;
    fixture.state.lock().unwrap().team = Some(Book::new());
    let mut alice = login(&fixture, "alice").await;
    choose(&fixture, &mut alice, TEAM).await;
    let shell = read(&fixture, &alice, "/cloud/app").await;
    assert!(shell.body.contains("Team · Unavailable"));
    let page = read(&fixture, &alice, "/cloud/app/team").await;
    assert_eq!(page.status, StatusCode::SERVICE_UNAVAILABLE);
    private(&page);
    for surface in ["native", "mobile", "desktop"] {
        let path = qualification(&fixture, surface, ORIGIN, &["membership"]);
        assert!(crate::cloud::team::Qualification::load(&path).is_err());
    }
    // A browser qualification for another origin does not enable this one.
    let path = qualification(
        &fixture,
        "browser",
        "https://other.example",
        &["membership"],
    );
    fixture.config.cloud_team = Some(Arc::new(
        crate::cloud::team::Qualification::load(&path).unwrap(),
    ));
    fixture.site = crate::router(fixture.config.clone());
    let page = read(&fixture, &alice, "/cloud/app/team").await;
    assert_eq!(page.status, StatusCode::SERVICE_UNAVAILABLE);
    // Membership alone leaves the other lanes unavailable.
    let path = qualification(&fixture, "browser", ORIGIN, &["membership"]);
    fixture.config.cloud_team = Some(Arc::new(
        crate::cloud::team::Qualification::load(&path).unwrap(),
    ));
    fixture.site = crate::router(fixture.config.clone());
    let page = read(&fixture, &alice, "/cloud/app/team").await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    assert!(page.body.contains("href=\"/cloud/app/team\""));
    assert!(!page.body.contains("<h3>Policy</h3>") && !page.body.contains("<h3>Limits</h3>"));
    assert_eq!(
        read(&fixture, &alice, "/cloud/app/team/reports")
            .await
            .status,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        read(&fixture, &alice, "/cloud/recover").await.status,
        StatusCode::SERVICE_UNAVAILABLE
    );
}

#[tokio::test]
async fn invitations_are_single_use_and_bind_workspace_role_and_epoch() {
    let fixture = team_fixture().await;
    let mut alice = login(&fixture, "alice").await;
    choose(&fixture, &mut alice, TEAM).await;
    let page = read(&fixture, &alice, "/cloud/app/team").await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    private(&page);
    assert!(page.body.contains("membership epoch <code>3</code>"));
    assert!(page.body.contains("stored Claude credential"));
    let csrf = form_token(&page.body, "/cloud/app/team/invite", "name=\"days\"");
    let created = post_form(
        &fixture,
        &alice,
        "/cloud/app/team/invite",
        &form(&[("role", "member"), ("days", "7"), ("csrf", &csrf)]),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    private(&created);
    let token = team_book(&fixture).invitations["inv1"].token.clone();
    assert!(created.body.contains(&token));
    // The token is shown once: the team page never repeats it.
    let page = read(&fixture, &alice, "/cloud/app/team").await;
    assert!(!page.body.contains(&token) && page.body.contains("inv1"));
    // Out-of-range lifetimes refuse before the native owner.
    let csrf = form_token(&page.body, "/cloud/app/team/invite", "name=\"days\"");
    let refused = post_form(
        &fixture,
        &alice,
        "/cloud/app/team/invite",
        &form(&[("role", "member"), ("days", "31"), ("csrf", &csrf)]),
    )
    .await;
    assert_eq!(refused.status, StatusCode::BAD_REQUEST);

    let mut bob = login(&fixture, "bob").await;
    let accept = read(&fixture, &bob, "/cloud/app/team/accept").await;
    assert_eq!(accept.status, StatusCode::OK, "{}", accept.body);
    let csrf = form_token(&accept.body, "/cloud/app/team/accept", "name=\"token\"");
    for (workspace, role) in [(TEAM, "admin"), ("bob-personal", "member")] {
        let wrong = post_form(
            &fixture,
            &bob,
            "/cloud/app/team/accept",
            &form(&[
                ("workspace", workspace),
                ("role", role),
                ("token", &token),
                ("csrf", &csrf),
            ]),
        )
        .await;
        assert_eq!(wrong.status, StatusCode::CONFLICT, "{workspace} {role}");
        private(&wrong);
        assert!(!wrong.body.contains(&token));
    }
    assert_eq!(team_book(&fixture).invitations["inv1"].status, "pending");
    let joined = post_form(
        &fixture,
        &bob,
        "/cloud/app/team/accept",
        &form(&[
            ("workspace", TEAM),
            ("role", "member"),
            ("token", &token),
            ("csrf", &csrf),
        ]),
    )
    .await;
    assert_eq!(joined.status, StatusCode::OK, "{}", joined.body);
    assert!(
        joined.body.contains("Invitation accepted")
            && joined.body.contains("stored Claude credential")
    );
    assert_eq!(team_book(&fixture).epoch, 4);
    // Single use: the same token refuses afterwards.
    let again = post_form(
        &fixture,
        &bob,
        "/cloud/app/team/accept",
        &form(&[
            ("workspace", TEAM),
            ("role", "member"),
            ("token", &token),
            ("csrf", &csrf),
        ]),
    )
    .await;
    assert_ne!(again.status, StatusCode::OK);
    choose(&fixture, &mut bob, TEAM).await;
    assert!(
        read(&fixture, &bob, "/cloud/app/team")
            .await
            .body
            .contains("membership epoch <code>4</code>")
    );
    // Alice's form was reviewed at epoch 3 and refuses at epoch 4.
    let stale = post_form(
        &fixture,
        &alice,
        "/cloud/app/team/invite",
        &form(&[
            ("role", "member"),
            ("days", "7"),
            ("csrf", &csrf_from(&page)),
        ]),
    )
    .await;
    assert_eq!(stale.status, StatusCode::FORBIDDEN);
    assert_eq!(team_book(&fixture).invitations.len(), 1);
    // A pending invitation can be withdrawn and then never accepted.
    let page = read(&fixture, &alice, "/cloud/app/team").await;
    let csrf = form_token(&page.body, "/cloud/app/team/invite", "name=\"days\"");
    post_form(
        &fixture,
        &alice,
        "/cloud/app/team/invite",
        &form(&[("role", "admin"), ("days", "1"), ("csrf", &csrf)]),
    )
    .await;
    let page = read(&fixture, &alice, "/cloud/app/team").await;
    let csrf = form_token(&page.body, "/cloud/app/team/withdraw", "value=\"inv2\"");
    let withdrawn = post_form(
        &fixture,
        &alice,
        "/cloud/app/team/withdraw",
        &form(&[("invitation", "inv2"), ("csrf", &csrf)]),
    )
    .await;
    assert_eq!(withdrawn.status, StatusCode::OK, "{}", withdrawn.body);
    assert_eq!(team_book(&fixture).invitations["inv2"].status, "revoked");
}

fn csrf_from(page: &Answer) -> String {
    form_token(&page.body, "/cloud/app/team/invite", "name=\"days\"")
}

fn with_bob(fixture: &Fixture) {
    let mut state = fixture.state.lock().unwrap();
    let book = state.team.as_mut().unwrap();
    book.members
        .insert("bob".into(), ("member".into(), "active".into()));
    book.access.push(json!({"at":1,"actor":"bob","action":"sign-in","workspace":TEAM,"session":null,"detail":null}));
    book.access.push(json!({"at":2,"actor":"alice","action":"invite","workspace":TEAM,"session":null,"detail":"inv0"}));
}

#[tokio::test]
async fn read_only_members_and_other_workspaces_cannot_change_the_team() {
    let fixture = team_fixture().await;
    with_bob(&fixture);
    let mut bob = login(&fixture, "bob").await;
    choose(&fixture, &mut bob, TEAM).await;
    let page = read(&fixture, &bob, "/cloud/app/team").await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    assert!(page.body.contains("read-only"));
    for action in [
        "invite", "role", "remove", "recovery", "policy", "budgets", "withdraw",
    ] {
        assert!(
            !page
                .body
                .contains(&format!("action=\"/cloud/app/team/{action}\"")),
            "{action}"
        );
    }
    // Policy rule details and other people's limits stay with admins.
    assert!(page.body.contains("Rule details are visible to admins"));
    assert!(!page.body.contains("Person <code>alice</code>"));
    // Even with a valid ticket the browser refuses before the native owner.
    let before = team_book(&fixture);
    for (path, scope, target, fields) in [
        (
            "/cloud/app/team/invite",
            "team-invite",
            "",
            vec![("role", "admin"), ("days", "1")],
        ),
        (
            "/cloud/app/team/remove",
            "team-remove",
            "alice",
            vec![("account", "alice")],
        ),
        (
            "/cloud/app/team/role",
            "team-role",
            "alice:member",
            vec![("account", "alice"), ("role", "member")],
        ),
        (
            "/cloud/app/team/recovery",
            "team-recovery",
            "alice",
            vec![("account", "alice")],
        ),
    ] {
        let csrf = minted(&fixture, &bob, scope, target).await;
        let mut fields = fields;
        fields.push(("csrf", &csrf));
        let refused = post_form(&fixture, &bob, path, &form(&fields)).await;
        assert_eq!(refused.status, StatusCode::FORBIDDEN, "{path}");
        assert!(refused.body.contains("Read-only membership"), "{path}");
        private(&refused);
    }
    let after = team_book(&fixture);
    assert_eq!(after.epoch, before.epoch);
    assert_eq!(after.invitations.len(), before.invitations.len());
    assert_eq!(after.members["alice"].0, "owner");

    // Alice's reviewed removal binds the selected team; after she selects
    // another workspace the same ticket refuses and bob stays a member.
    let mut alice = login(&fixture, "alice").await;
    choose(&fixture, &mut alice, TEAM).await;
    let page = read(&fixture, &alice, "/cloud/app/team").await;
    let csrf = form_token(&page.body, "/cloud/app/team/remove", "value=\"bob\"");
    choose(&fixture, &mut alice, "alice-personal").await;
    let crossed = post_form(
        &fixture,
        &alice,
        "/cloud/app/team/remove",
        &form(&[("account", "bob"), ("csrf", &csrf)]),
    )
    .await;
    assert_eq!(crossed.status, StatusCode::FORBIDDEN);
    assert_eq!(team_book(&fixture).active("bob"), Some("member"));
    // The personal workspace shows only its own roster.
    let personal = read(&fixture, &alice, "/cloud/app/team").await;
    assert!(
        personal.body.contains("alice-personal") && !personal.body.contains("<code>bob</code>")
    );
    // Bob never reads alice's personal workspace.
    let bob_page = read(&fixture, &bob, "/cloud/app/team").await;
    assert!(!bob_page.body.contains("alice-personal"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn role_and_removal_epochs_retire_connected_observers() {
    let fixture = team_fixture().await;
    with_bob(&fixture);
    let mut alice = login(&fixture, "alice").await;
    choose(&fixture, &mut alice, TEAM).await;
    let mut bob = login(&fixture, "bob").await;
    choose(&fixture, &mut bob, TEAM).await;

    // Bob's observer is connected before alice changes his role.
    let site = fixture.site.clone();
    let observer = Cookies(bob.0.clone());
    let watching = tokio::spawn(async move {
        request(
            &site,
            Method::GET,
            "/cloud/app/team/watch",
            &observer,
            None,
            None,
        )
        .await
    });
    tokio::time::sleep(std::time::Duration::from_millis(60)).await;
    let page = read(&fixture, &alice, "/cloud/app/team").await;
    let csrf = form_token(&page.body, "/cloud/app/team/role", "value=\"bob\"");
    let promoted = post_form(
        &fixture,
        &alice,
        "/cloud/app/team/role",
        &form(&[("account", "bob"), ("role", "admin"), ("csrf", &csrf)]),
    )
    .await;
    assert_eq!(promoted.status, StatusCode::OK, "{}", promoted.body);
    assert!(promoted.body.contains("epoch <code>4</code>"));
    assert!(
        promoted
            .body
            .contains("stays hidden after the change until that member adds it again")
    );
    let retired = watching.await.unwrap();
    assert_eq!(retired.status, StatusCode::OK);
    assert!(retired.body.contains("event: retire"), "{}", retired.body);
    // Bob's earlier page is stale; a fresh read shows his new role.
    assert!(
        read(&fixture, &bob, "/cloud/app/team")
            .await
            .body
            .contains("your role <strong>admin</strong>")
    );

    let site = fixture.site.clone();
    let observer = Cookies(bob.0.clone());
    let watching = tokio::spawn(async move {
        request(
            &site,
            Method::GET,
            "/cloud/app/team/watch",
            &observer,
            None,
            None,
        )
        .await
    });
    tokio::time::sleep(std::time::Duration::from_millis(60)).await;
    let page = read(&fixture, &alice, "/cloud/app/team").await;
    let csrf = form_token(&page.body, "/cloud/app/team/remove", "value=\"bob\"");
    let removed = post_form(
        &fixture,
        &alice,
        "/cloud/app/team/remove",
        &form(&[("account", "bob"), ("csrf", &csrf)]),
    )
    .await;
    assert_eq!(removed.status, StatusCode::OK, "{}", removed.body);
    assert!(
        removed.body.contains("no longer belongs") && removed.body.contains("epoch <code>5</code>")
    );
    let retired = watching.await.unwrap();
    assert!(retired.body.contains("event: retire"));
    assert!(!retired.body.contains("<code>alice</code>"));
    // Removed: every team read refuses for bob.
    for path in [
        "/cloud/app/team",
        "/cloud/app/team/reports",
        "/cloud/app/team/export",
    ] {
        let lost = read(&fixture, &bob, path).await;
        assert_eq!(lost.status, StatusCode::FORBIDDEN, "{path}");
        private(&lost);
    }
}

#[tokio::test]
async fn recovery_redeems_once_without_restoring_membership() {
    let fixture = team_fixture().await;
    with_bob(&fixture);
    let mut alice = login(&fixture, "alice").await;
    choose(&fixture, &mut alice, TEAM).await;
    let page = read(&fixture, &alice, "/cloud/app/team").await;
    let csrf = form_token(&page.body, "/cloud/app/team/recovery", "value=\"bob\"");
    let issued = post_form(
        &fixture,
        &alice,
        "/cloud/app/team/recovery",
        &form(&[("account", "bob"), ("csrf", &csrf)]),
    )
    .await;
    assert_eq!(issued.status, StatusCode::OK, "{}", issued.body);
    private(&issued);
    let token = team_book(&fixture).recovery.keys().next().unwrap().clone();
    assert!(
        issued.body.contains(&token) && issued.body.contains("never restores a removed membership")
    );
    assert_eq!(team_book(&fixture).epoch, 3);

    // Signed out, the recovery page redeems once and shows the new key once.
    let mut visitor = Cookies::default();
    let sign_in = read(&fixture, &visitor, "/cloud/sign-in").await;
    assert!(sign_in.body.contains("href=\"/cloud/recover\""));
    let page = read(&fixture, &visitor, "/cloud/recover").await;
    assert_eq!(page.status, StatusCode::OK);
    visitor.apply(&page);
    let csrf = field(&page.body, "csrf");
    let redeemed = post_form(
        &fixture,
        &visitor,
        "/cloud/recover",
        &form(&[("token", &token), ("csrf", &csrf)]),
    )
    .await;
    assert_eq!(redeemed.status, StatusCode::OK, "{}", redeemed.body);
    private(&redeemed);
    assert!(
        redeemed
            .body
            .contains("oak_recovered.synthetic-replacement")
    );
    let again = post_form(
        &fixture,
        &visitor,
        "/cloud/recover",
        &form(&[("token", &token), ("csrf", &csrf)]),
    )
    .await;
    assert_eq!(again.status, StatusCode::FORBIDDEN);
    assert!(!again.body.contains("oak_recovered"));
    // A forged form without its login nonce refuses.
    let forged = post_form(
        &fixture,
        &Cookies::default(),
        "/cloud/recover",
        &form(&[("token", "rcv_other"), ("csrf", &csrf)]),
    )
    .await;
    assert_eq!(forged.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn policy_and_limits_only_narrow_and_show_concurrent_holds() {
    let fixture = team_fixture().await;
    with_bob(&fixture);
    let mut alice = login(&fixture, "alice").await;
    choose(&fixture, &mut alice, TEAM).await;
    let page = read(&fixture, &alice, "/cloud/app/team").await;
    assert!(page.body.contains("kev-0.6b") && page.body.contains("kev-1b"));
    // Concurrent holds: reserved and unknown both count against the caps.
    assert!(page.body.contains("Reserved (holds)"));
    assert!(
        page.body
            .contains("<td>Workspace</td><td>1000</td><td>500</td><td>300</td><td>100</td>")
    );
    assert!(page.body.contains("Person <code>alice</code>"));

    let current = team_book(&fixture).policy.unwrap();
    let csrf = form_token(&page.body, "/cloud/app/team/policy", "name=\"keep\"");
    let expires = current.terms.expires_unix.to_string();
    let later = (current.terms.expires_unix + 1).to_string();
    // Extending the expiry or naming an unknown rule refuses locally.
    for fields in [
        vec![
            ("expected", current.digest.as_str()),
            ("expires", later.as_str()),
            ("keep", "0"),
            ("csrf", &csrf),
        ],
        vec![
            ("expected", current.digest.as_str()),
            ("expires", expires.as_str()),
            ("keep", "7"),
            ("csrf", &csrf),
        ],
    ] {
        let refused = post_form(&fixture, &alice, "/cloud/app/team/policy", &form(&fields)).await;
        assert_eq!(refused.status, StatusCode::BAD_REQUEST, "{}", refused.body);
    }
    assert_eq!(team_book(&fixture).policy.unwrap().digest, current.digest);
    let narrowed = post_form(
        &fixture,
        &alice,
        "/cloud/app/team/policy",
        &form(&[
            ("expected", &current.digest),
            ("expires", &expires),
            ("keep", "0"),
            ("csrf", &csrf),
        ]),
    )
    .await;
    assert_eq!(narrowed.status, StatusCode::OK, "{}", narrowed.body);
    let next = team_book(&fixture).policy.unwrap();
    assert_eq!(next.terms.version, 2);
    assert_eq!(next.terms.rules.len(), 1);
    assert!(next.terms.narrows(&current.terms));
    // The reviewed ticket names the old digest; replaying it refuses.
    let replay = post_form(
        &fixture,
        &alice,
        "/cloud/app/team/policy",
        &form(&[
            ("expected", &current.digest),
            ("expires", &expires),
            ("csrf", &csrf),
        ]),
    )
    .await;
    assert_eq!(replay.status, StatusCode::CONFLICT);

    // Limits: lowering works, raising refuses, and nothing resets.
    let page = read(&fixture, &alice, "/cloud/app/team").await;
    let expected = team_book(&fixture).budget_version_digest;
    let csrf = form_token(&page.body, "/cloud/app/team/budgets", "workspace.cap");
    let raised = post_form(
        &fixture,
        &alice,
        "/cloud/app/team/budgets",
        &form(&[
            ("expected", &expected),
            ("workspace.cap", "2000"),
            ("csrf", &csrf),
        ]),
    )
    .await;
    assert_eq!(raised.status, StatusCode::BAD_REQUEST);
    let unknown = post_form(
        &fixture,
        &alice,
        "/cloud/app/team/budgets",
        &form(&[
            ("expected", &expected),
            ("person.mallory.cap", "1"),
            ("csrf", &csrf),
        ]),
    )
    .await;
    assert_eq!(unknown.status, StatusCode::BAD_REQUEST);
    let lowered = post_form(
        &fixture,
        &alice,
        "/cloud/app/team/budgets",
        &form(&[
            ("expected", &expected),
            ("workspace.cap", "900"),
            ("team.core.alert", "350"),
            ("csrf", &csrf),
        ]),
    )
    .await;
    assert_eq!(lowered.status, StatusCode::OK, "{}", lowered.body);
    let book = team_book(&fixture);
    assert_eq!(book.budget["workspace"]["cap"], 900);
    assert_eq!(book.budget["teams"]["core"]["alert_at"], 350);
    assert_eq!(book.budget["version"], 2);
    assert_eq!(book.reserved, 300);

    // An admin who is not the owner reads limits but cannot change them.
    fixture
        .state
        .lock()
        .unwrap()
        .team
        .as_mut()
        .unwrap()
        .members
        .insert("bob".into(), ("admin".into(), "active".into()));
    let mut bob = login(&fixture, "bob").await;
    choose(&fixture, &mut bob, TEAM).await;
    let page = read(&fixture, &bob, "/cloud/app/team").await;
    assert!(!page.body.contains("action=\"/cloud/app/team/budgets\""));
    assert!(page.body.contains("action=\"/cloud/app/team/policy\""));
}

#[tokio::test]
async fn reports_and_export_are_bounded_to_current_read_rights() {
    let fixture = team_fixture().await;
    with_bob(&fixture);
    let mut alice = login(&fixture, "alice").await;
    choose(&fixture, &mut alice, TEAM).await;
    let report = read(&fixture, &alice, "/cloud/app/team/reports").await;
    assert_eq!(report.status, StatusCode::OK, "{}", report.body);
    private(&report);
    assert!(report.body.contains("task-alice") && report.body.contains("task-bob"));
    assert!(report.body.contains("not remote attestation"));
    let export = read(&fixture, &alice, "/cloud/app/team/export").await;
    assert_eq!(export.status, StatusCode::OK);
    private(&export);
    assert!(
        export.headers[header::CONTENT_DISPOSITION]
            .to_str()
            .unwrap()
            .contains("attachment")
    );
    let document: Value = serde_json::from_str(&export.body).unwrap();
    assert_eq!(document["workspace"], TEAM);
    assert_eq!(document["members_epoch"], 3);
    assert_eq!(document["report"]["rows"].as_array().unwrap().len(), 2);
    assert_eq!(document["access"].as_array().unwrap().len(), 2);
    assert_eq!(
        read(
            &fixture,
            &alice,
            "/cloud/app/team/export?workspace=bob-personal"
        )
        .await
        .status,
        StatusCode::BAD_REQUEST
    );

    let mut bob = login(&fixture, "bob").await;
    choose(&fixture, &mut bob, TEAM).await;
    let own = read(&fixture, &bob, "/cloud/app/team/reports").await;
    assert!(own.body.contains("task-bob") && !own.body.contains("task-alice"));
    let document: Value =
        serde_json::from_str(&read(&fixture, &bob, "/cloud/app/team/export").await.body).unwrap();
    assert_eq!(document["report"]["rows"].as_array().unwrap().len(), 1);
    let access = document["access"].as_array().unwrap();
    assert!(access.iter().all(|e| e["actor"] == "bob"));
    // Without a selected workspace there is no report to read.
    let mut carol = login(&fixture, "bob").await;
    carol.0.remove("oa_cloud_workspace");
    assert_eq!(
        read(&fixture, &carol, "/cloud/app/team/export")
            .await
            .status,
        StatusCode::FORBIDDEN
    );
}
