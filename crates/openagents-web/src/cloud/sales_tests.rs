//! Sales delegation acceptance against the actual resident owner adapter
//! over loopback HTTP, with a synthetic private pipeline and credentials.

use super::*;
use coder::task::sales::remote::{self, Service};
use coder::task::sales::{Role, Store};
use std::sync::atomic::{AtomicBool, Ordering};

const PAGE: &str = "/cloud/app/sales";
const CONTACT: &str = "prospect@synthetic.invalid";
const SITE_BEARER: &str = "synthetic-sales-site-bearer-alice";

fn private_dir(path: &std::path::Path) {
    std::fs::create_dir_all(path).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
}

fn hex_digest(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The sales-owner host: its private pipeline, the resident adapter, and a
/// loopback listener that can lose one apply reply after processing it.
struct Owner {
    _temp: tempfile::TempDir,
    root: PathBuf,
    lead: String,
    url: String,
    lose_apply: Arc<AtomicBool>,
    server: tokio::task::JoinHandle<()>,
}

impl Drop for Owner {
    fn drop(&mut self) {
        self.server.abort();
    }
}

async fn lose_reply(
    State(flag): State<Arc<AtomicBool>>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let (parts, body) = request.into_parts();
    let bytes = to_bytes(body, 64 * 1024).await.unwrap();
    let apply = serde_json::from_slice::<Value>(&bytes)
        .map(|v| v["op"]["kind"] == "apply")
        .unwrap_or(false);
    let response = next
        .run(Request::from_parts(parts, Body::from(bytes)))
        .await;
    if apply && flag.swap(false, Ordering::SeqCst) {
        StatusCode::BAD_GATEWAY.into_response()
    } else {
        response
    }
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
    store
        .issue(&admin, "writer-a", Role::Writer, &root.join("writer-a"))
        .unwrap();
    let writer = store
        .authenticate(&Store::read_credential(&root.join("writer-a")).unwrap())
        .unwrap();
    let at = now();
    let create = json!({"schema":coder::task::sales::COMMAND_SCHEMA,"id":"seed-lead","lead":null,"expected_revision":0,"operation":{"kind":"create","ownership_acceptance":"writer accepted responsibility","input":{
        "contact":format!("email:{CONTACT}"),"source":"synthetic private introduction","source_at":at-20,
        "details":{"account":"synthetic-account","jurisdiction":"synthetic jurisdiction record",
        "permission":{"state":"granted","reference":"synthetic-consent-v1","recorded_at":at-10,"expires_at":at+86_400,"channels":["email"]},
        "workflow":"one synthetic repository maintenance task","baseline_reference":"private-baseline-reference",
        "data":{"recipients":["human:operator","human:writer-a"],"permitted_use":"one agreed pilot; no marketing reuse","retain_until":at+2*86_400},
        "stage":"qualified","next":{"description":"review pilot scope","due_at":at+3600},"customer_decision":null,"readers":[]}}}});
    let receipt = store
        .apply(&writer, &serde_json::to_vec(&create).unwrap())
        .unwrap();
    drop(store);
    let config = root.join("remote.json");
    let document = json!({"schema":remote::CONFIG_SCHEMA,"root":host,"journal":root.join("journal"),
        "bindings":[{"id":"alice-sales","account":"alice","workspace":"alice-personal","members_epoch":3,
        "principal":"writer-a","credential":root.join("writer-a"),
        "client_digest":hex_digest(SITE_BEARER.as_bytes()),"effects":["update"]}]});
    private_file(&config, &serde_json::to_vec(&document).unwrap());
    let service = Arc::new(Service::open(&config).unwrap());
    let lose_apply = Arc::new(AtomicBool::new(false));
    let router = crate::sales_remote::router(service).layer(axum::middleware::from_fn_with_state(
        lose_apply.clone(),
        lose_reply,
    ));
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
        lead: receipt.lead,
        url,
        lose_apply,
        server,
    }
}

impl Owner {
    /// The site's delegation configuration, attached as a fresh process.
    fn attach(&self, fixture: &mut Fixture) {
        let site = self.root.join("site");
        private_dir(&site);
        let bearer = site.join("bearer");
        private_file(&bearer, SITE_BEARER.as_bytes());
        let path = site.join("sales.json");
        private_file(
            &path,
            &serde_json::to_vec(
                &json!({"schema":super::super::sales::SCHEMA,"directory":site,
                "delegations":[{"id":"alice-sales","account":"alice","workspace":"alice-personal",
                "members_epoch":3,"endpoint":self.url,"binding":"alice-sales","bearer_file":bearer,
                "development_loopback":true}]}),
            )
            .unwrap(),
        );
        fixture.config.cloud_sales = Some(Arc::new(
            super::super::sales::Delegations::load(&path).unwrap(),
        ));
        fixture.site = crate::router(fixture.config.clone());
    }

    fn revoke(&self) {
        let mut store = Store::open(&self.root.join("host")).unwrap();
        let admin = store
            .authenticate(&Store::read_credential(&self.root.join("operator")).unwrap())
            .unwrap();
        store.revoke(&admin, "writer-a").unwrap();
    }

    fn revision(&self) -> u64 {
        let mut store = Store::open(&self.root.join("host")).unwrap();
        let admin = store
            .authenticate(&Store::read_credential(&self.root.join("operator")).unwrap())
            .unwrap();
        store.show(&admin, &self.lead).unwrap().revision
    }
}

fn private_file(path: &std::path::Path, bytes: &[u8]) {
    std::fs::write(path, bytes).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
}

async fn get(fixture: &Fixture, cookies: &Cookies, path: &str) -> Answer {
    request(&fixture.site, Method::GET, path, cookies, None, None).await
}

/// The hidden fields of the first form posting to `action`.
fn hidden(html: &str, action: &str) -> Vec<(String, String)> {
    let form = html
        .split("<form ")
        .skip(1)
        .find(|form| form.contains(&format!("action=\"{action}\"")))
        .unwrap_or_else(|| panic!("no form for {action}"))
        .split("</form>")
        .next()
        .unwrap();
    form.split("<input type=\"hidden\" name=\"")
        .skip(1)
        .map(|part| {
            let (name, rest) = part.split_once("\" value=\"").unwrap();
            (
                name.into(),
                rest.split('"').next().unwrap().replace("&amp;", "&"),
            )
        })
        .collect()
}

async fn post(
    fixture: &Fixture,
    cookies: &Cookies,
    path: &str,
    fields: &[(String, String)],
) -> Answer {
    let fields: Vec<(&str, &str)> = fields
        .iter()
        .map(|(n, v)| (n.as_str(), v.as_str()))
        .collect();
    request(
        &fixture.site,
        Method::POST,
        path,
        cookies,
        Some(&form(&fields)),
        Some(ORIGIN),
    )
    .await
}

fn with(mut fields: Vec<(String, String)>, name: &str, value: &str) -> Vec<(String, String)> {
    fields.retain(|(n, _)| n != name);
    fields.push((name.into(), value.into()));
    fields
}

fn no_secret(text: &str) {
    assert!(!text.contains(SITE_BEARER), "disclosed the binding bearer");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sales_delegation_reads_and_changes_only_through_the_bound_owner_credential() {
    let mut fixture = fixture().await;
    // Without a delegation, Sales stays unavailable.
    let mut cookies = login(&fixture, "alice").await;
    choose_personal(&fixture, &mut cookies).await;
    let unavailable = get(&fixture, &cookies, PAGE).await;
    assert!(unavailable.body.contains("Sales · Unavailable"));

    let owner = owner().await;
    owner.attach(&mut fixture);
    let index = get(&fixture, &cookies, PAGE).await;
    assert_eq!(index.status, StatusCode::OK, "{}", index.body);
    private(&index);
    no_secret(&index.body);
    assert!(index.body.contains("Principal writer-a"));
    assert!(index.body.contains("Admitted changes: update"));
    assert!(index.body.contains("Qualified"));
    // The aggregate pipeline view carries no contact.
    assert!(!index.body.contains(CONTACT));
    assert!(
        index
            .body
            .contains("<a aria-current=\"page\" href=\"/cloud/app/sales\">Sales</a>")
    );

    let path = format!("{PAGE}/alice-sales/leads/{}", owner.lead);
    let record = get(&fixture, &cookies, &path).await;
    assert_eq!(record.status, StatusCode::OK, "{}", record.body);
    private(&record);
    no_secret(&record.body);
    assert!(record.body.contains(CONTACT));
    assert!(!record.headers.contains_key(header::LOCATION));

    // Bob and Alice's other workspace hold no delegation.
    let bob = login(&fixture, "bob").await;
    let refused = get(&fixture, &bob, &path).await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN);
    assert!(!refused.body.contains(CONTACT));
    let bob_index = get(&fixture, &bob, PAGE).await;
    assert!(
        bob_index
            .body
            .contains("no sales delegation is provisioned")
    );
    assert!(!bob_index.body.contains("Qualified"));

    // A stage change journals, dispatches once, and an exact retry recovers.
    let action = format!("{path}/stage");
    let fields = with(hidden(&record.body, &action), "stage", "pilot");
    let changed = post(&fixture, &cookies, &action, &fields).await;
    assert_eq!(changed.status, StatusCode::SEE_OTHER, "{}", changed.body);
    assert_eq!(owner.revision(), 2);
    let again = post(&fixture, &cookies, &action, &fields).await;
    assert_eq!(again.status, StatusCode::SEE_OTHER, "{}", again.body);
    assert_eq!(owner.revision(), 2);
    // The same identity with changed parameters conflicts.
    let reused = post(
        &fixture,
        &cookies,
        &action,
        &with(fields.clone(), "stage", "active"),
    )
    .await;
    assert_eq!(reused.status, StatusCode::CONFLICT);
    assert_eq!(owner.revision(), 2);
    // A form rendered at an older revision is refused as changed.
    let stale = post(
        &fixture,
        &cookies,
        &action,
        &with(hidden(&record.body, &action), "stage", "active"),
    )
    .await;
    assert_eq!(stale.status, StatusCode::CONFLICT);
    assert_eq!(owner.revision(), 2);

    // A lost reply leaves Outcome unknown; the retry reconciles the original.
    let record = get(&fixture, &cookies, &path).await;
    assert!(record.body.contains("Recorded at revision 2"));
    owner.lose_apply.store(true, Ordering::SeqCst);
    let fields = with(hidden(&record.body, &action), "stage", "active");
    let lost = post(&fixture, &cookies, &action, &fields).await;
    assert_eq!(
        lost.status,
        StatusCode::SERVICE_UNAVAILABLE,
        "{}",
        lost.body
    );
    assert!(lost.body.contains("Outcome unknown"));
    assert_eq!(owner.revision(), 3);
    // A restarted site shows the unknown request and retries it exactly.
    owner.attach(&mut fixture);
    let record = get(&fixture, &cookies, &path).await;
    assert!(record.body.contains("Outcome unknown"));
    let retry = hidden(
        record.body.split("Outcome unknown").nth(1).unwrap(),
        &action,
    );
    let settled = post(&fixture, &cookies, &action, &retry).await;
    assert_eq!(settled.status, StatusCode::SEE_OTHER, "{}", settled.body);
    assert_eq!(owner.revision(), 3);
    let record = get(&fixture, &cookies, &path).await;
    assert!(record.body.contains("Recorded at revision 3"));
    assert!(!record.body.contains("Outcome unknown"));

    // The site's journal keeps receipts and digests, never the contact.
    for entry in std::fs::read_dir(owner.root.join("site/sales-requests")).unwrap() {
        let text = std::fs::read_to_string(entry.unwrap().path()).unwrap();
        assert!(!text.contains(CONTACT));
        assert!(!text.contains("prospect"));
        no_secret(&text);
    }

    // A changed membership epoch or a revoked sales credential refuses.
    fixture.state.lock().unwrap().epoch = 4;
    let moved = get(&fixture, &cookies, &path).await;
    assert_ne!(moved.status, StatusCode::OK);
    assert!(!moved.body.contains(CONTACT));
    fixture.state.lock().unwrap().epoch = 3;
    owner.revoke();
    let revoked = get(&fixture, &cookies, &path).await;
    assert_eq!(revoked.status, StatusCode::FORBIDDEN);
    assert!(!revoked.body.contains(CONTACT));
    let index = get(&fixture, &cookies, PAGE).await;
    assert!(index.body.contains("Access refused for this binding"));
    assert!(!index.body.contains(CONTACT));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_owner_adapter_refuses_browser_requests_and_unbound_callers() {
    let owner = owner().await;
    let client = reqwest::Client::new();
    let body = json!({"schema":remote::REQUEST_SCHEMA,
        "actor":{"account":"alice","workspace":"alice-personal","members_epoch":3},
        "op":{"kind":"list","after":null,"limit":10}});
    let browser = client
        .post(&owner.url)
        .bearer_auth(SITE_BEARER)
        .header("x-sales-binding", "alice-sales")
        .header("origin", ORIGIN)
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(browser.status(), 403);
    let wrong = client
        .post(&owner.url)
        .bearer_auth("not-the-site-bearer")
        .header("x-sales-binding", "alice-sales")
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(wrong.status(), 403);
    let ok = client
        .post(&owner.url)
        .bearer_auth(SITE_BEARER)
        .header("x-sales-binding", "alice-sales")
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(ok.status(), 200);
    assert_eq!(ok.headers()["cache-control"], "no-store");
    let text = ok.text().await.unwrap();
    assert!(text.contains(&owner.lead));
    assert!(!text.contains(CONTACT));
}

#[path = "sales_views_tests.rs"]
mod views;
