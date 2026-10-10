//! Who each gateway route is for, declared once (#11157,
//! docs/api/design.md sections 1 and 2.3).
//!
//! Every path the gateway mounts is exactly one of:
//!
//! - **PUBLIC**: outside developers, partner apps, other agents. Described
//!   in `GET /v1/openapi.json`.
//! - **FIRST-PARTY**: our own web, mobile, desktop, and terminal clients.
//!   Described in `docs/api/openapi.first-party.json`, committed and not
//!   served.
//! - **INTERNAL**: service to service, operators, provider webhooks, the
//!   decision service. Described nowhere.
//!
//! [`ROUTES`] is that declaration. The served document, the committed
//! first-party document, and their tests all read it; a test here fails
//! when a route module mounts a path [`ROUTES`] does not name.
//!
//! A PUBLIC route with no operations here is described in full by
//! [`crate::inference_openapi::document`] (inference, catalog, the calling
//! key, key issue and revoke, limits, own provider keys).

use serde_json::{Map, Value, json};

/// Who a route is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Audience {
    Public,
    FirstParty,
    Internal,
}

use Audience::{FirstParty, Internal, Public};

/// One documented operation: method (lowercase), operation id, summary.
pub struct Op {
    pub method: &'static str,
    pub id: &'static str,
    pub summary: &'static str,
}

const fn op(method: &'static str, id: &'static str, summary: &'static str) -> Op {
    Op {
        method,
        id,
        summary,
    }
}

/// One mounted path, who it is for, and (for PUBLIC and FIRST-PARTY
/// routes) what each of its methods does.
pub struct Route {
    pub path: &'static str,
    pub audience: Audience,
    pub tag: &'static str,
    pub ops: &'static [Op],
}

const fn route(
    path: &'static str,
    audience: Audience,
    tag: &'static str,
    ops: &'static [Op],
) -> Route {
    Route {
        path,
        audience,
        tag,
        ops,
    }
}

/// A path no document describes on its own: the root copy of
/// `/v1/openapi.json` the website reads.
pub const ALIASES: &[&str] = &[crate::inference_openapi::ROOT];

/// Every path the gateway can mount, by audience. Keep one route per line
/// (`scripts/api/first_party_openapi.py` reads this table too).
#[rustfmt::skip]
pub const ROUTES: &[Route] = &[
    // ---- PUBLIC: inference and catalog (described by inference_openapi).
    route("/v1/responses", Public, "Inference", &[]),
    route("/v1/chat/completions", Public, "Inference", &[]),
    route("/v1/responses/compact", Public, "Inference", &[]),
    route("/v1/responses/{id}", Public, "Inference", &[]),
    route("/v1/models", Public, "Catalog", &[]),
    route("/v1/rates", Public, "Catalog", &[]),
    route("/v1/usage/tokens-served", Public, "Catalog", &[]),
    route("/v1/openapi.json", Public, "Catalog", &[]),
    route("/openapi.json", Public, "Catalog", &[]),
    route("/v1/key", Public, "Account", &[]),
    route("/v1/usage/{request_id}", Public, "Account", &[]),
    route("/v1/workspaces/{workspace}/keys", Public, "Account", &[]),
    route("/v1/workspaces/{workspace}/keys/{key}", Public, "Account", &[]),
    route("/v1/workspaces/{workspace}/keys/{key}/limits", Public, "Account", &[]),
    route("/v1/workspaces/{workspace}/provider-keys", Public, "Account", &[]),
    route("/v1/workspaces/{workspace}/provider-keys/{provider}", Public, "Account", &[]),
    // ---- PUBLIC: account, keys, workspaces.
    route("/v1/account", Public, "Account", &[op("get", "getAccount", "Your account.")]),
    route("/v1/account/access", Public, "Account", &[op("get", "getAccountAccess", "What you may do, per workspace.")]),
    route("/v1/workspaces/{workspace}/keys/{key}/copy", Public, "Account", &[op("post", "copyKey", "Make a new key with the same scopes and limits as this one. The secret is shown once.")]),
    route("/v1/workspaces/{workspace}/keys/{key}/pause", Public, "Account", &[op("post", "pauseKey", "Pause a key; its calls are refused until it is resumed.")]),
    route("/v1/workspaces/{workspace}/keys/{key}/resume", Public, "Account", &[op("post", "resumeKey", "Resume a paused key.")]),
    route("/v1/workspaces/{workspace}/keys/{key}/rotate", Public, "Account", &[op("post", "rotateKey", "Replace a key's secret. The new secret is shown once.")]),
    route("/v1/workspaces", Public, "Workspaces", &[op("post", "createWorkspace", "Make a workspace.")]),
    route("/v1/workspaces/{workspace}", Public, "Workspaces", &[op("get", "getWorkspace", "A workspace and its members."), op("patch", "updateWorkspace", "Rename a workspace.")]),
    route("/v1/workspaces/{workspace}/invitations", Public, "Workspaces", &[op("post", "inviteMember", "Invite someone to the workspace.")]),
    route("/v1/workspaces/{workspace}/invitations/{invitation}", Public, "Workspaces", &[op("delete", "revokeInvitation", "Take an invitation back.")]),
    route("/v1/invitations/accept", Public, "Workspaces", &[op("post", "acceptInvitation", "Join a workspace with an invitation.")]),
    route("/v1/invitations/accept-reviewed", Public, "Workspaces", &[op("post", "acceptReviewedInvitation", "Join a workspace with an invitation, after reviewing its terms.")]),
    route("/v1/workspaces/{workspace}/members/{account}", Public, "Workspaces", &[op("patch", "setMemberRole", "Change a member's role."), op("delete", "removeMember", "Remove a member.")]),
    route("/v1/workspaces/{workspace}/transfer", Public, "Workspaces", &[op("post", "transferWorkspace", "Give the workspace to another member.")]),
    route("/v1/workspaces/{workspace}/access", Public, "Workspaces", &[op("get", "getWorkspaceAccess", "Who may do what in the workspace.")]),
    route("/v1/workspaces/{workspace}/team-policy", Public, "Workspaces", &[op("get", "getTeamPolicy", "The workspace's team rules."), op("put", "setTeamPolicy", "Set the workspace's team rules.")]),
    route("/v1/workspaces/{workspace}/sso", Public, "Workspaces", &[op("get", "getSso", "The workspace's single sign-on provider."), op("put", "setSso", "Set the workspace's single sign-on provider.")]),
    route("/v1/workspaces/{workspace}/sso/links", Public, "Workspaces", &[op("post", "linkSso", "Link a member to a single sign-on identity.")]),
    route("/v1/workspaces/{workspace}/sso/unlink", Public, "Workspaces", &[op("post", "unlinkSso", "Unlink a member's single sign-on identity.")]),
    route("/v1/workspaces/{workspace}/sso/sign-in", Public, "Workspaces", &[op("post", "signInSso", "Sign in through the workspace's single sign-on provider.")]),
    route("/v1/workspaces/{workspace}/sso/audit", Public, "Workspaces", &[op("get", "getSsoAudit", "Single sign-on events.")]),
    // ---- PUBLIC: usage and money.
    route("/v1/workspaces/{workspace}/usage", Public, "Usage", &[op("get", "getWorkspaceUsage", "The workspace's usage, totaled.")]),
    route("/v1/workspaces/{workspace}/usage/activity", Public, "Usage", &[op("get", "listWorkspaceActivity", "The workspace's calls, newest first (`data`, `next`, `?after=`).")]),
    route("/v1/workspaces/{workspace}/usage/timeseries", Public, "Usage", &[op("get", "getWorkspaceTimeseries", "The workspace's usage over time.")]),
    route("/v1/workspaces/{workspace}/usage/receipts/{digest}", Public, "Usage", &[op("get", "getReceipt", "One execution receipt.")]),
    route("/v1/workspaces/{workspace}/usage/export", Public, "Usage", &[op("get", "exportWorkspaceUsage", "The workspace's usage as a file.")]),
    route("/v1/balance", Public, "Usage", &[op("get", "getBalance", "The workspace's money position (the workspace from `X-Workspace-Id`). Same as `GET /v1/workspaces/{workspace}/balance`.")]),
    route("/v1/workspaces/{workspace}/balance", Public, "Usage", &[op("get", "getWorkspaceBalance", "The workspace's money position: credited, reserved, settled, refunded, available, and the price versions charged.")]),
    route("/v1/workspaces/{workspace}/topups/{door}", Public, "Usage", &[op("post", "topUp", "Top up with Lightning: quote, issue the invoice, read, or reconcile (`op`).")]),
    route("/v1/workspaces/{workspace}/decision-funding/{door}", Public, "Usage", &[op("post", "fundDecisions", "Older path of `POST /v1/workspaces/{workspace}/topups/{door}` (answers with `Deprecation` and `Link`).")]),
    route("/v1/workspaces/{workspace}/budgets", Public, "Usage", &[op("get", "getBudgets", "The workspace's team budgets."), op("put", "setBudgets", "Set the workspace's team budgets.")]),
    // ---- FIRST-PARTY: sessions and device sign-in.
    route("/v1/sessions", FirstParty, "Sessions", &[op("post", "createSession", "Trade an API key for a session.")]),
    route("/v1/session", FirstParty, "Sessions", &[op("get", "getSession", "The current session."), op("delete", "signOut", "Sign out.")]),
    route("/v1/sessions/device", FirstParty, "Sessions", &[op("post", "startDeviceSignIn", "Start device sign-in (RFC 8628).")]),
    route("/v1/sessions/device/poll", FirstParty, "Sessions", &[op("post", "pollDeviceSignIn", "Poll device sign-in; the app session once approved.")]),
    route("/v1/account/identities/github", FirstParty, "Sessions", &[op("post", "linkGithub", "Link GitHub to the signed-in account.")]),
    route("/v1/account/sessions", FirstParty, "Sessions", &[op("get", "listAppSessions", "Signed-in apps and computers.")]),
    route("/v1/account/sessions/{session}", FirstParty, "Sessions", &[op("delete", "signOutApp", "Sign one app or computer out.")]),
    route("/v1/workspaces/{workspace}/recovery", FirstParty, "Sessions", &[op("post", "issueRecovery", "An owner recovery token.")]),
    route("/v1/recovery/redeem", FirstParty, "Sessions", &[op("post", "redeemRecovery", "Redeem an owner recovery token.")]),
    // ---- FIRST-PARTY: GitHub access and projects.
    route("/v1/account/github", FirstParty, "GitHub", &[op("get", "getGithubAccess", "Connected GitHub access and projects.")]),
    route("/v1/account/github/grant", FirstParty, "GitHub", &[op("post", "grantGithub", "Connect GitHub repositories."), op("delete", "disconnectGithub", "Disconnect GitHub repositories.")]),
    route("/v1/account/github/repositories", FirstParty, "GitHub", &[op("get", "listGithubRepositories", "Repositories you can connect (`data`, `next`, `?after=`).")]),
    route("/v1/account/github/app/grant", FirstParty, "GitHub", &[op("post", "grantGithubApp", "Connect repositories through the GitHub App.")]),
    route("/v1/account/github/app/refresh", FirstParty, "GitHub", &[op("post", "refreshGithubApp", "Read the GitHub App's installations again.")]),
    route("/v1/account/github/broker", FirstParty, "GitHub", &[op("post", "issueGitTicket", "A ticket for a machine to fetch or push one project's repository.")]),
    route("/v1/github/git-credential", FirstParty, "GitHub", &[op("post", "redeemGitTicket", "Git's credential helper redeems a ticket.")]),
    route("/v1/projects", FirstParty, "Projects", &[op("post", "addProject", "Add a project (one repository).")]),
    route("/v1/projects/{id}", FirstParty, "Projects", &[op("delete", "removeProject", "Remove a project.")]),
    route("/v1/account/projects", FirstParty, "Projects", &[op("post", "addAccountProject", "Older path of `POST /v1/projects` (answers with `Deprecation` and `Link`).")]),
    route("/v1/account/projects/{id}", FirstParty, "Projects", &[op("delete", "removeAccountProject", "Older path of `DELETE /v1/projects/{id}` (answers with `Deprecation` and `Link`).")]),
    // ---- FIRST-PARTY: purchase authority, billing, earnings.
    route("/v1/workspaces/{workspace}/commercial/{product}", FirstParty, "Billing", &[op("get", "getCommercialAuthority", "Purchase authority for one product.")]),
    route("/v1/workspaces/{workspace}/plugin-reader", FirstParty, "Billing", &[op("get", "getPluginReader", "Purchase authority for the plugin host.")]),
    route("/v1/workspaces/{workspace}/purchase-context/{door}", FirstParty, "Billing", &[op("get", "getPurchaseContext", "What a purchase through one door would cost.")]),
    route("/v1/plans", FirstParty, "Billing", &[op("get", "listPlans", "The plans.")]),
    route("/v1/billing/sessions/{checkout}", FirstParty, "Billing", &[op("get", "getCheckout", "One card checkout's state.")]),
    route("/v1/workspaces/{workspace}/billing", FirstParty, "Billing", &[op("get", "getBilling", "The workspace's plan.")]),
    route("/v1/workspaces/{workspace}/billing/subscribe", FirstParty, "Billing", &[op("post", "subscribe", "Subscribe to a plan.")]),
    route("/v1/workspaces/{workspace}/billing/checkout", FirstParty, "Billing", &[op("post", "startCheckout", "Start a card checkout.")]),
    route("/v1/workspaces/{workspace}/billing/portal", FirstParty, "Billing", &[op("post", "openBillingPortal", "Open the card processor's billing page.")]),
    route("/v1/workspaces/{workspace}/billing/plan", FirstParty, "Billing", &[op("post", "changePlan", "Change plan.")]),
    route("/v1/workspaces/{workspace}/billing/cancel", FirstParty, "Billing", &[op("post", "cancelPlan", "Cancel the plan.")]),
    route("/v1/workspaces/{workspace}/card-funding/{door}", FirstParty, "Billing", &[op("post", "fundByCard", "Top up by card.")]),
    route("/v1/earnings", FirstParty, "Earnings", &[op("get", "listEarnings", "What you earned.")]),
    route("/v1/earnings/{party}", FirstParty, "Earnings", &[op("get", "getEarnings", "One party's statement.")]),
    route("/v1/earnings/{party}/export", FirstParty, "Earnings", &[op("get", "exportEarnings", "One party's statement as a file.")]),
    route("/v1/earnings/{party}/destination", FirstParty, "Earnings", &[op("get", "getPayoutDestination", "Where payouts go."), op("put", "setPayoutDestination", "Change where payouts go.")]),
    route("/v1/earnings/{party}/payouts/{payout}", FirstParty, "Earnings", &[op("get", "getPayout", "One payout.")]),
    // ---- FIRST-PARTY: referrals.
    route("/join", FirstParty, "Referrals", &[op("get", "join", "A referral link's landing.")]),
    route("/v1/account/acquisition", FirstParty, "Referrals", &[op("get", "getAcquisition", "How the account arrived."), op("post", "recordAcquisition", "Record how the account arrived.")]),
    route("/v1/account/referrers", FirstParty, "Referrals", &[op("post", "createReferrer", "Make a referral identity.")]),
    route("/v1/account/attribution/policy", FirstParty, "Referrals", &[op("get", "getAttributionPolicy", "The attribution rules.")]),
    route("/v1/account/attribution", FirstParty, "Referrals", &[op("get", "getAttribution", "Who referred the account."), op("post", "proposeAttribution", "Propose who referred the account.")]),
    route("/v1/account/attribution/confirm", FirstParty, "Referrals", &[op("post", "confirmAttribution", "Confirm a proposed attribution.")]),
    route("/v1/account/referral-terms", FirstParty, "Referrals", &[op("get", "getReferralTerms", "The referral commission terms.")]),
    route("/v1/account/referral-agreement", FirstParty, "Referrals", &[op("get", "getReferralAgreement", "The referral agreement."), op("post", "acceptReferralAgreement", "Accept the referral agreement.")]),
    route("/v1/workspaces/{workspace}/attribution", FirstParty, "Referrals", &[op("get", "getWorkspaceAttribution", "Who referred the workspace."), op("post", "adoptAttribution", "Adopt an attribution for the workspace.")]),
    route("/v1/account/referrers/{referrer}/lineage", FirstParty, "Referrals", &[op("get", "getReferrerLineage", "A referral identity's history.")]),
    route("/v1/account/referrers/{referrer}", FirstParty, "Referrals", &[op("get", "getReferrer", "One referral identity.")]),
    route("/v1/account/referrers/{referrer}/link", FirstParty, "Referrals", &[op("post", "enableReferralLink", "Turn a referral link on."), op("delete", "disableReferralLink", "Turn a referral link off.")]),
    route("/v1/account/referrers/{referrer}/migration", FirstParty, "Referrals", &[op("post", "migrateReferrer", "Move a referral identity.")]),
    route("/v1/account/referrers/{referrer}/migration/accept", FirstParty, "Referrals", &[op("post", "acceptReferrerMigration", "Accept a referral identity's move.")]),
    // ---- FIRST-PARTY: the older skills directory and team reports.
    route("/v1/skills", FirstParty, "Skills", &[op("get", "listSkills", "Browse skills (`data`, `next`, `?after=`)."), op("post", "submitSkill", "Submit a skill.")]),
    route("/v1/skills/{name}", FirstParty, "Skills", &[op("get", "getSkill", "One skill.")]),
    route("/v1/skills/{name}/versions/{version}", FirstParty, "Skills", &[op("get", "getSkillVersion", "One version of a skill.")]),
    route("/v1/skills/{name}/versions/{version}/SKILL.md", FirstParty, "Skills", &[op("get", "getSkillMarkdown", "One version's SKILL.md.")]),
    route("/v1/skills/{name}/versions/{version}/review", FirstParty, "Skills", &[op("get", "getSkillReview", "One version's review.")]),
    route("/v1/skills/{name}/versions/{version}/withdraw", FirstParty, "Skills", &[op("post", "withdrawSkill", "Withdraw one version.")]),
    route("/v1/submissions", FirstParty, "Skills", &[op("get", "listSubmissions", "Your submissions.")]),
    route("/v1/submissions/{submission}/appeal", FirstParty, "Skills", &[op("post", "appealSubmission", "Appeal a refused submission.")]),
    route("/v1/workspaces/{workspace}/reports", FirstParty, "Reports", &[op("get", "getTeamReport", "The workspace's team report.")]),
    route("/v1/workspaces/{workspace}/reports/export", FirstParty, "Reports", &[op("get", "exportTeamReport", "The team report as a file.")]),
    route("/v1/workspaces/{workspace}/reports/evidence", FirstParty, "Reports", &[op("post", "attachReportEvidence", "Attach evidence to the team report.")]),
    // ---- INTERNAL: decision service, operators, webhooks, service-to-service.
    route("/v1/systemone", Internal, "", &[]),
    route("/v1/classify", Internal, "", &[]),
    route("/v1/jobs", Internal, "", &[]),
    route("/v1/jobs/{id}", Internal, "", &[]),
    route("/v1/jobs/{id}/cancel", Internal, "", &[]),
    route("/v1/jobs/{id}/results", Internal, "", &[]),
    route("/v1/jobs/{id}/notify/rotate", Internal, "", &[]),
    route("/v1/feedback", Internal, "", &[]),
    route("/v1/feedback/{id}", Internal, "", &[]),
    route("/v1/updates", Internal, "", &[]),
    route("/healthz", Internal, "", &[]),
    route("/v1/accounts", Internal, "", &[]),
    route("/v1/sessions/github", Internal, "", &[]),
    route("/v1/sessions/device/lookup", Internal, "", &[]),
    route("/v1/sessions/device/decide", Internal, "", &[]),
    route("/v1/sessions/device/paired", Internal, "", &[]),
    route("/v1/account/github/token", Internal, "", &[]),
    route("/v1/billing/webhook", Internal, "", &[]),
    route("/v1/billing/prepaid/webhook", Internal, "", &[]),
    route("/v1/workspaces/{workspace}/billing/reconcile", Internal, "", &[]),
    route("/v1/admin/inference/status", Internal, "", &[]),
    route("/v1/admin/inference/outcomes", Internal, "", &[]),
    route("/admin/inference", Internal, "", &[]),
    route("/admin/inference/session", Internal, "", &[]),
    route("/dashboard", Internal, "", &[]),
    route("/dashboard/session", Internal, "", &[]),
    route("/dashboard/sign-out", Internal, "", &[]),
    route("/dashboard/w/{workspace}", Internal, "", &[]),
    route("/dashboard/w/{workspace}/usage", Internal, "", &[]),
    route("/dashboard/w/{workspace}/activity", Internal, "", &[]),
    route("/dashboard/w/{workspace}/receipts/{digest}", Internal, "", &[]),
    route("/dashboard/w/{workspace}/members", Internal, "", &[]),
    route("/dashboard/w/{workspace}/keys", Internal, "", &[]),
    route("/dashboard/w/{workspace}/billing", Internal, "", &[]),
    route("/dashboard/w/{workspace}/funding", Internal, "", &[]),
    route("/dashboard/funding/{id}", Internal, "", &[]),
    route("/dashboard/w/{workspace}/reports", Internal, "", &[]),
    route("/dashboard/w/{workspace}/reports/export", Internal, "", &[]),
    route("/dashboard/earnings", Internal, "", &[]),
    route("/dashboard/earnings/{party}", Internal, "", &[]),
    route("/dashboard/earnings/{party}/export", Internal, "", &[]),
    route("/dashboard/earnings/{party}/destination", Internal, "", &[]),
    route("/dashboard/earnings/{party}/payouts/{payout}", Internal, "", &[]),
    route("/playground", Internal, "", &[]),
    route("/playground/session", Internal, "", &[]),
    route("/playground/run", Internal, "", &[]),
    route("/playground/native", Internal, "", &[]),
    route("/playground/upload", Internal, "", &[]),
    route("/playground/chat", Internal, "", &[]),
];

/// The declared audience of a mounted path. Discovery documents
/// (`crate::discovery`) are not API and are not declared here.
#[must_use]
pub fn audience(path: &str) -> Option<Audience> {
    ROUTES
        .iter()
        .find(|route| route.path == path)
        .map(|route| route.audience)
}

/// The declared routes of one audience.
pub fn of(audience: Audience) -> impl Iterator<Item = &'static Route> {
    ROUTES
        .iter()
        .filter(move |route| route.audience == audience)
}

fn error_response(description: &str) -> Value {
    json!({"description": description,
           "content": {"application/json": {"schema": {"$ref": "#/components/schemas/ErrorBody"}}}})
}

/// The OpenAPI path item for one declared route: each operation with its
/// path parameters, a JSON answer, and the shared error object.
#[must_use]
pub fn path_item(route: &Route) -> Value {
    let parameters: Vec<Value> = route
        .path
        .split('/')
        .filter_map(|segment| segment.strip_prefix('{')?.strip_suffix('}'))
        .map(|name| {
            json!({"name": name, "in": "path", "required": true,
                   "description": format!("The {name} id."),
                   "schema": {"type": "string"}})
        })
        .collect();
    let mut item = Map::new();
    for op in route.ops {
        let mut operation = json!({
            "operationId": op.id,
            "summary": op.summary,
            "tags": [route.tag],
            "security": [{"bearer": []}],
            "responses": {
                "200": {"description": "Done.",
                        "content": {"application/json": {"schema": {"type": "object"}}}},
                "400": error_response("The request isn't valid (`invalid_request`)."),
                "401": error_response("No key or session, or it was rejected (`authentication`)."),
                "403": error_response("Not allowed here (`permission`)."),
                "404": error_response("Nothing with that id here (`not_found`).")
            }
        });
        if !parameters.is_empty() {
            operation["parameters"] = json!(parameters);
        }
        if matches!(op.method, "post" | "put" | "patch") {
            operation["requestBody"] = json!({"required": false,
                "content": {"application/json": {"schema": {"type": "object"}}}});
        }
        item.insert(op.method.to_owned(), operation);
    }
    Value::Object(item)
}

/// The OpenAPI path a route is listed under in the public document: the
/// part after `/v1`, since the document's server is `<origin>/v1`.
#[must_use]
pub fn public_key(path: &str) -> &str {
    path.strip_prefix("/v1").unwrap_or(path)
}

/// Add the declared PUBLIC routes that `mounted` names and the inference
/// document does not already describe.
pub fn extend_public(document: &mut Value, mounted: &[&str]) {
    let mut tags: Vec<&str> = Vec::new();
    for route in of(Public) {
        if route.ops.is_empty() || !mounted.contains(&route.path) {
            continue;
        }
        let key = public_key(route.path);
        if document["paths"].get(key).is_some() {
            continue;
        }
        document["paths"][key] = path_item(route);
        if !tags.contains(&route.tag) {
            tags.push(route.tag);
        }
    }
    if let Some(list) = document["tags"].as_array_mut() {
        for tag in tags {
            if !list.iter().any(|known| known["name"] == tag) {
                list.push(json!({"name": tag, "description": tag_description(tag)}));
            }
        }
    }
}

fn tag_description(tag: &str) -> &'static str {
    match tag {
        "Workspaces" => "Workspaces, members, invitations, and single sign-on.",
        "Usage" => "What the workspace spent, its balance, and top-ups.",
        "Sessions" => "Signing in: sessions, device sign-in, recovery.",
        "GitHub" => "Connected GitHub repositories.",
        "Projects" => "Projects: one repository each.",
        "Billing" => "Plans, card checkout, and purchase authority.",
        "Earnings" => "What you earned and where it goes.",
        "Referrals" => "Referral links and attribution.",
        "Skills" => "The older skills directory.",
        "Reports" => "Team reports.",
        "Computers" => "Your computers and whether their chats sync.",
        "Threads" => "Chats synced from the terminal.",
        "Traces" => "Agent traces (ATIF).",
        _ => "",
    }
}

/// The FIRST-PARTY document for the gateway's routes: every declared
/// FIRST-PARTY route, whether or not a deployment mounts it. Committed at
/// `docs/api/openapi.first-party.json` and never served.
#[must_use]
pub fn first_party_document() -> Value {
    let mut paths = Map::new();
    let mut tags: Vec<&str> = Vec::new();
    for route in of(FirstParty) {
        paths.insert(route.path.to_owned(), path_item(route));
        if !tags.contains(&route.tag) {
            tags.push(route.tag);
        }
    }
    let tags: Vec<Value> = tags
        .into_iter()
        .map(|tag| json!({"name": tag, "description": tag_description(tag)}))
        .collect();
    json!({
        "openapi": "3.1.0",
        "info": {
            "title": "OpenAgents first-party API",
            "version": "1.0.0-beta",
            "description": "Routes our own web, mobile, desktop, and terminal clients call. Not a public contract: they may change with a client release and keep working for the two newest releases (docs/api/design.md, section 2.4). The public API is https://openagents.com/api/v1/openapi.json.",
            "license": {"name": "CC0-1.0", "identifier": "CC0-1.0"}
        },
        "servers": [{"url": "https://api.openagents.com"}],
        "tags": tags,
        "paths": paths,
        "components": {
            "securitySchemes": {
                "bearer": {"type": "http", "scheme": "bearer",
                           "description": "An account session (`sess_...`) from sign-in, or an API key (`oak_...`)."}
            },
            "schemas": {
                "Error": {
                    "type": "object",
                    "required": ["type", "code", "message"],
                    "properties": {
                        "type": {"type": "string"},
                        "code": {"type": ["string", "null"]},
                        "message": {"type": "string"},
                        "param": {"type": ["string", "null"]},
                        "request_id": {"type": "string"}
                    }
                },
                "ErrorBody": {"type": "object", "required": ["error"],
                              "properties": {"error": {"$ref": "#/components/schemas/Error"}}}
            }
        }
    })
}

/// Where the committed first-party document lives.
pub const FIRST_PARTY_FILE: &str = "docs/api/openapi.first-party.json";

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// Every path any gateway route module can mount, whatever the
    /// deployment's configuration.
    fn every_mountable_path() -> BTreeSet<&'static str> {
        let mut paths: BTreeSet<&'static str> = [
            "/v1/systemone",
            "/v1/classify",
            "/v1/jobs",
            "/v1/jobs/{id}",
            "/v1/jobs/{id}/cancel",
            "/v1/jobs/{id}/results",
            "/v1/jobs/{id}/notify/rotate",
            "/v1/feedback",
            "/v1/feedback/{id}",
            "/v1/updates",
            "/v1/models",
            "/healthz",
            "/v1/balance",
            "/v1/workspaces/{workspace}/balance",
            crate::inference_public::KEY,
            crate::inference_public::USAGE,
            crate::inference_public::LIMITS,
        ]
        .into_iter()
        .collect();
        let modules = [
            crate::budgets::routes(),
            crate::accounts::routes(),
            crate::sso::routes(),
            crate::github_repos::routes(),
            crate::usage::routes(),
            crate::dashboard::routes(),
            crate::playground::routes(),
            crate::team_reports::routes(),
            crate::card_funding::controller::routes(),
            crate::billing::routes(),
            crate::skills::routes(),
            crate::funding::routes(),
            crate::earnings::routes(),
            crate::inference_status::routes(),
            crate::inference_routes::routes(),
            crate::inference_rates::routes(),
            crate::inference_byok::routes(),
            crate::inference_openapi::routes(),
        ];
        for module in modules {
            paths.extend(module.into_iter().map(|(path, _)| path));
        }
        paths
    }

    #[test]
    fn every_mountable_route_declares_its_audience_once() {
        let mountable = every_mountable_path();
        for path in &mountable {
            assert!(
                audience(path).is_some(),
                "{path} is mounted but has no audience in audience::ROUTES"
            );
        }
        let mut seen = BTreeSet::new();
        for route in ROUTES {
            assert!(seen.insert(route.path), "{} is declared twice", route.path);
            assert!(
                mountable.contains(route.path),
                "{} is declared but no module mounts it",
                route.path
            );
        }
    }

    #[test]
    fn public_and_first_party_routes_say_what_they_do() {
        let described = crate::inference_openapi::document("https://api.openagents.com", &[]);
        for route in ROUTES.iter().filter(|route| route.audience != Internal) {
            if route.ops.is_empty() {
                assert_eq!(route.audience, Public, "{}", route.path);
                assert!(
                    ALIASES.contains(&route.path)
                        || described["paths"].get(public_key(route.path)).is_some(),
                    "{} has no operations and the inference document does not describe it",
                    route.path
                );
            }
            assert!(!route.tag.is_empty(), "{} has no tag", route.path);
            for op in route.ops {
                assert!(
                    matches!(op.method, "get" | "post" | "put" | "patch" | "delete"),
                    "{} {}",
                    op.method,
                    route.path
                );
            }
        }
        for route in ROUTES.iter().filter(|route| route.audience == Internal) {
            assert!(route.ops.is_empty(), "{} is internal", route.path);
        }
    }

    #[test]
    fn operation_ids_are_unique() {
        let mut ids = BTreeSet::new();
        for route in ROUTES {
            for op in route.ops {
                assert!(ids.insert(op.id), "{} twice", op.id);
            }
        }
    }

    /// The committed first-party document is the one [`ROUTES`] makes.
    /// After changing a FIRST-PARTY route, write it again with
    /// `OPENAGENTS_WRITE_OPENAPI=1 cargo test -p gateway audience`.
    #[test]
    fn the_committed_first_party_document_is_current() {
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(FIRST_PARTY_FILE);
        let generated = first_party_document();
        let committed: Value = std::fs::read_to_string(&file)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or(Value::Null);
        // The website's first-party routes share the file, marked
        // `x-served-by: web`; this test holds only the gateway's part.
        let mut gateway_part = committed.clone();
        if let Some(paths) = gateway_part["paths"].as_object_mut() {
            paths.retain(|_, item| item.get("x-served-by").is_none());
        }
        if let Some(tags) = gateway_part["tags"].as_array_mut() {
            let used: BTreeSet<String> = generated["tags"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|tag| tag["name"].as_str().map(str::to_owned))
                .collect();
            tags.retain(|tag| tag["name"].as_str().is_some_and(|name| used.contains(name)));
        }
        if gateway_part == generated {
            return;
        }
        if std::env::var_os("OPENAGENTS_WRITE_OPENAPI").is_some() {
            let mut merged = generated.clone();
            if let (Some(paths), Some(old)) = (
                merged["paths"].as_object_mut(),
                committed["paths"].as_object(),
            ) {
                for (path, item) in old {
                    if item.get("x-served-by").is_some() {
                        paths.insert(path.clone(), item.clone());
                    }
                }
            }
            if let (Some(tags), Some(old)) =
                (merged["tags"].as_array_mut(), committed["tags"].as_array())
            {
                for tag in old {
                    if !tags.iter().any(|known| known["name"] == tag["name"]) {
                        tags.push(tag.clone());
                    }
                }
            }
            let text = serde_json::to_string_pretty(&merged).unwrap();
            std::fs::write(&file, format!("{text}\n")).unwrap();
            return;
        }
        panic!(
            "{FIRST_PARTY_FILE} is out of date; write it with OPENAGENTS_WRITE_OPENAPI=1 cargo test -p gateway audience"
        );
    }
}
