//! WEB-10 purchase acceptance: progress, artifacts, cancellation, receipt,
//! and recovery against the actual retail HTTP service with isolated
//! synthetic owners and fake payments.

use super::*;

/// The journaled record for one browser request identity.
fn journaled(retail: &Retail, request: &str) -> Value {
    for (_, journal) in journals(retail) {
        if let Some(record) = journal["records"].get(request) {
            return record.clone();
        }
    }
    panic!("request {request} was not journaled");
}

fn journals(retail: &Retail) -> Vec<(PathBuf, Value)> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(retail.root.join("site").join("requests")).unwrap() {
        let path = entry.unwrap().path();
        let name = path.to_string_lossy().into_owned();
        if name.ends_with(".purchase.json") || name.ends_with(".next") {
            continue;
        }
        out.push((
            path.clone(),
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap(),
        ));
    }
    out
}

fn field_of(fields: &[(String, String)], name: &str) -> String {
    fields
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, v)| v.clone())
        .unwrap()
}

/// Request and pay an exact invoice through the delegation's page.
async fn funded(
    fixture: &Fixture,
    cookies: &Cookies,
    retail: &Retail,
    id: &str,
    sats: &str,
) -> Value {
    let page = get(fixture, cookies, PAGE).await;
    let (answer, fields) = submit(
        fixture,
        cookies,
        &page.body,
        &format!("{PAGE}/{id}/top-up"),
        &[("amount_sats", sats)],
    )
    .await;
    assert_eq!(answer.status, StatusCode::SEE_OTHER, "{}", answer.body);
    let invoice = journaled(retail, &field_of(&fields, "request"))["outcome"].clone();
    retail.fund(&invoice);
    invoice
}

async fn keyed(fixture: &Fixture, cookies: &Cookies, id: &str) {
    let page = get(fixture, cookies, PAGE).await;
    let (answer, _) = submit(
        fixture,
        cookies,
        &page.body,
        &format!("{PAGE}/{id}/key"),
        &[("key", KEY), ("consent", "custody")],
    )
    .await;
    assert_eq!(answer.status, StatusCode::SEE_OTHER, "{}", answer.body);
}

/// Review a quote; returns its review digest and request fields.
async fn quoted(
    fixture: &Fixture,
    cookies: &Cookies,
    retail: &Retail,
    id: &str,
) -> (String, Vec<(String, String)>) {
    let page = get(fixture, cookies, PAGE).await;
    let (answer, fields) = submit(
        fixture,
        cookies,
        &page.body,
        &format!("{PAGE}/{id}/quote"),
        &task_fields("Fix trailing commas."),
    )
    .await;
    assert_eq!(answer.status, StatusCode::SEE_OTHER, "{}", answer.body);
    let review = journaled(retail, &field_of(&fields, "request"))["outcome"]["digest"]
        .as_str()
        .unwrap()
        .to_owned();
    (review, fields)
}

fn task_fields(task: &str) -> Vec<(&'static str, &str)> {
    vec![
        ("repository", "https://github.com/OpenAgentsInc/example"),
        ("commit", "cccccccccccccccccccccccccccccccccccccccc"),
        ("task", task),
        ("checks", "cargo test -p parser"),
        ("max_seconds", "600"),
    ]
}

/// The confirmation form for exactly `review`.
fn confirm_fields(html: &str, action: &str, review: &str) -> Vec<(String, String)> {
    let form = html
        .split("<form ")
        .skip(1)
        .map(|form| form.split("</form>").next().unwrap())
        .find(|form| form.contains(&format!("action=\"{action}\"")) && form.contains(review))
        .unwrap_or_else(|| panic!("no confirmation for {review}"));
    hidden(&format!("<form {form}</form>"), action)
}

async fn confirm(
    fixture: &Fixture,
    cookies: &Cookies,
    id: &str,
    review: &str,
) -> (Answer, Vec<(String, String)>) {
    let action = format!("{PAGE}/{id}/confirm");
    let page = get(fixture, cookies, PAGE).await;
    let fields = confirm_fields(&page.body, &action, review);
    let all: Vec<(&str, &str)> = fields
        .iter()
        .map(|(n, v)| (n.as_str(), v.as_str()))
        .chain([("custody", "service")])
        .collect();
    (post(fixture, cookies, &action, &all).await, fields)
}

/// Quote and confirm one purchase; returns its execution.
async fn bought(fixture: &Fixture, cookies: &Cookies, retail: &Retail, id: &str) -> String {
    let (review, _) = quoted(fixture, cookies, retail, id).await;
    let (answer, fields) = confirm(fixture, cookies, id, &review).await;
    assert_eq!(answer.status, StatusCode::SEE_OTHER, "{}", answer.body);
    journaled(retail, &field_of(&fields, "request"))["outcome"]["execution"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn purchase(id: &str, execution: &str) -> String {
    format!("{PAGE}/{id}/purchases/{execution}")
}

impl Retail {
    fn resource(&self, execution: &str) -> String {
        let task = retail_cloud::dispatch::task_id(execution);
        self.runtime
            .provider
            .active()
            .into_iter()
            .find(|r| self.runtime.owner.status(r, &task).ok().flatten().is_some())
            .expect("dispatched sandbox")
    }

    fn settle(&self) {
        for n in 0..8 {
            self.service.tick(now() as i64 + 40 + n).unwrap();
        }
    }

    fn balance(&self) -> Value {
        self.service
            .call(
                "buyer",
                &bearer("buyer"),
                serde_json::from_value(json!({"op":"account"})).unwrap(),
                now() as i64,
            )
            .unwrap()["result"]["balance"]
            .clone()
    }
}

fn line<'a>(html: &'a str, label: &str) -> &'a str {
    html.split(&format!("<dt>{label}</dt><dd>"))
        .nth(1)
        .unwrap_or_else(|| panic!("no {label}: {html}"))
        .split("</dd>")
        .next()
        .unwrap()
}

async fn alice(fixture: &Fixture) -> Cookies {
    let mut cookies = login(fixture, "alice").await;
    choose_personal(fixture, &mut cookies).await;
    cookies
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn purchase_progress_artifacts_and_receipt_recover_one_canonical_record() {
    let mut fixture = fixture().await;
    let retail = retail();
    retail.attach(
        &mut fixture,
        &[
            ALICE,
            ("narrowed", "alice", "alice-personal", 3, "buyer", true),
        ],
    );
    let cookies = alice(&fixture).await;
    let invoice = funded(&fixture, &cookies, &retail, "alice-retail", "1000").await;
    keyed(&fixture, &cookies, "alice-retail").await;
    let execution = bought(&fixture, &cookies, &retail, "alice-retail").await;
    retail.dispatch(&execution);
    let resource = retail.resource(&execution);
    let task = retail_cloud::dispatch::task_id(&execution);
    retail.runtime.owner.emit(&resource, &task, "step one <b>");
    retail.runtime.owner.emit(&resource, &task, "step two");

    // The list and the retail page both lead to the same purchase.
    let list = get(
        &fixture,
        &cookies,
        &format!("{PAGE}/alice-retail/purchases"),
    )
    .await;
    assert_eq!(list.status, StatusCode::OK, "{}", list.body);
    private(&list);
    assert!(list.body.contains(&purchase("alice-retail", &execution)));
    assert!(list.body.contains("approved on this page"));
    let page = get(&fixture, &cookies, PAGE).await;
    assert!(page.body.contains(&purchase("alice-retail", &execution)));

    let detail = get(&fixture, &cookies, &purchase("alice-retail", &execution)).await;
    assert_eq!(detail.status, StatusCode::OK, "{}", detail.body);
    private(&detail);
    no_secret(&detail.body);
    assert!(detail.body.contains("step one &lt;b&gt;"));
    assert!(detail.body.contains("step two"));
    assert!(line(&detail.body, "Payment").contains("Quote maximum reserved"));
    assert!(line(&detail.body, "Acceptance").starts_with("Not recorded"));
    assert!(line(&detail.body, "Publication").starts_with("Not published"));
    assert!(line(&detail.body, "Approval").starts_with("Confirmed on this page"));
    assert_eq!(line(&detail.body, "Native account"), "retail-customer");
    assert_eq!(line(&detail.body, "Sandbox"), resource);
    assert!(detail.body.contains("Public GitHub source"));
    assert!(
        detail
            .body
            .contains(&format!("action=\"{PAGE}/alice-retail/cancel\""))
    );

    // A site restart recovers the same record and reads on from its cursor.
    retail.reload(&mut fixture);
    retail.runtime.owner.emit(&resource, &task, "step three");
    let again = get(&fixture, &cookies, &purchase("alice-retail", &execution)).await;
    assert_eq!(again.status, StatusCode::OK, "{}", again.body);
    assert_eq!(again.body.matches("step one").count(), 1);
    assert!(again.body.contains("step three"));
    assert_eq!(
        line(&again.body, "Funded request"),
        line(&detail.body, "Funded request")
    );

    // Observation-only scope sees the purchase without any control.
    let narrowed = get(&fixture, &cookies, &purchase("narrowed", &execution)).await;
    assert_eq!(narrowed.status, StatusCode::OK, "{}", narrowed.body);
    assert!(!narrowed.body.contains("/cancel\""));
    assert!(line(&narrowed.body, "Approval").contains("another client"));

    // Another account reaches neither the list, the purchase, nor its artifacts.
    let bob = login(&fixture, "bob").await;
    for path in [
        format!("{PAGE}/alice-retail/purchases"),
        purchase("alice-retail", &execution),
        format!("{}/artifacts/patch", purchase("alice-retail", &execution)),
    ] {
        assert_eq!(
            get(&fixture, &bob, &path).await.status,
            StatusCode::FORBIDDEN,
            "{path}"
        );
    }

    // Completion, settlement, artifacts, and cleanup.
    retail.runtime.owner.set_status(
        &resource,
        &task,
        TaskStatus::Ended {
            end: retail_cloud::dispatch::ExecutorEnd::Completed,
            patch: Some(retail_cloud::sha256_hex(b"patch\n")),
            checks: vec![],
        },
    );
    retail.runtime.provider.set_usage(&resource, 30);
    retail.settle();
    let done = get(&fixture, &cookies, &purchase("alice-retail", &execution)).await;
    assert_eq!(done.status, StatusCode::OK, "{}", done.body);
    let charged = line(&done.body, "Payment").to_owned();
    assert!(charged.starts_with("Charged"), "{charged}");
    assert!(line(&done.body, "Completion").starts_with("Ended"));
    assert!(line(&done.body, "Acceptance").starts_with("Not recorded"));
    assert!(line(&done.body, "Publication").starts_with("Not published"));
    assert!(done.body.contains("Cleanup confirmed"), "{}", done.body);
    assert!(
        !done
            .body
            .contains(&format!("action=\"{PAGE}/alice-retail/cancel\""))
    );
    let artifact = format!("{}/artifacts/patch", purchase("alice-retail", &execution));
    assert!(done.body.contains(&format!("href=\"{artifact}\"")));
    let read = get(&fixture, &cookies, &artifact).await;
    assert_eq!(read.status, StatusCode::OK, "{}", read.body);
    private(&read);
    assert!(read.body.contains("matches the retained manifest"));
    assert!(read.body.contains("Reading it is not acceptance"));
    assert_eq!(
        get(
            &fixture,
            &cookies,
            &format!("{}/artifacts/other", purchase("alice-retail", &execution))
        )
        .await
        .status,
        StatusCode::CONFLICT
    );

    // Duplicate payment evidence or repeated settlement credits nothing more.
    let balance = retail.balance();
    retail.fund(&invoice);
    retail.settle();
    assert_eq!(retail.balance(), balance);
    retail.reload(&mut fixture);
    let after = get(&fixture, &cookies, &purchase("alice-retail", &execution)).await;
    assert_eq!(after.status, StatusCode::OK, "{}", after.body);
    assert_eq!(line(&after.body, "Payment"), charged);
    assert_eq!(after.body.matches("step one").count(), 1);
    assert_eq!(retail.runtime.owner.started(), 1);
    assert_eq!(retail.runtime.provider.create_calls(), 1);
    assert_eq!(retail.wallet.issued(), 1);

    // A purchase part that differs from the retained one is refused, not shown.
    for record in std::fs::read_dir(retail.root.join("site").join("requests"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.to_string_lossy().ends_with(".purchase.json"))
    {
        let mut retained: Value = serde_json::from_slice(&std::fs::read(&record).unwrap()).unwrap();
        retained["binding"]["quote"]["max_sats"] = json!(1);
        std::fs::write(&record, serde_json::to_vec(&retained).unwrap()).unwrap();
    }
    let changed = get(&fixture, &cookies, &purchase("alice-retail", &execution)).await;
    assert_eq!(changed.status, StatusCode::CONFLICT, "{}", changed.body);
    assert!(changed.body.contains("Purchase record changed"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stop_unknown_meter_and_cleanup_uncertainty_stay_held_until_evidenced() {
    let mut fixture = fixture().await;
    let retail = retail();
    retail.attach(&mut fixture, &[ALICE]);
    let cookies = alice(&fixture).await;
    funded(&fixture, &cookies, &retail, "alice-retail", "1000").await;
    keyed(&fixture, &cookies, "alice-retail").await;
    let execution = bought(&fixture, &cookies, &retail, "alice-retail").await;
    retail.dispatch(&execution);
    let resource = retail.resource(&execution);
    let path = purchase("alice-retail", &execution);

    // The stop control on the purchase returns to the same purchase.
    let detail = get(&fixture, &cookies, &path).await;
    retail.runtime.provider.set_usage(&resource, 20);
    retail.runtime.provider.set_usage_unreadable(true);
    retail.runtime.provider.set_unreachable(true);
    let (stopped, _) = submit(
        &fixture,
        &cookies,
        &detail.body,
        &format!("{PAGE}/alice-retail/cancel"),
        &[],
    )
    .await;
    assert_eq!(stopped.status, StatusCode::SEE_OTHER, "{}", stopped.body);
    assert_eq!(stopped.headers[header::LOCATION], path.as_str());
    retail.settle();
    let held = get(&fixture, &cookies, &path).await;
    assert_eq!(held.status, StatusCode::OK, "{}", held.body);
    assert!(
        line(&held.body, "Payment").contains("funds remain held"),
        "{}",
        held.body
    );
    assert!(!held.body.contains("Cleanup confirmed"));
    assert!(line(&held.body, "Stop").starts_with("cancelled"));
    assert!(retail.balance()["held_msat"].as_i64().unwrap() > 0);

    // A restart keeps the uncertainty; only evidence resolves it.
    retail.reload(&mut fixture);
    let restarted = get(&fixture, &cookies, &path).await;
    assert!(line(&restarted.body, "Payment").contains("funds remain held"));
    retail.runtime.provider.set_unreachable(false);
    retail.runtime.provider.set_usage_unreadable(false);
    retail.settle();
    let settled = get(&fixture, &cookies, &path).await;
    assert_eq!(settled.status, StatusCode::OK, "{}", settled.body);
    assert!(line(&settled.body, "Payment").starts_with("Charged"));
    assert!(
        settled.body.contains("Cleanup confirmed"),
        "{}",
        settled.body
    );
    assert_eq!(retail.balance()["held_msat"], 0);
    assert_eq!(retail.runtime.owner.started(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn lost_dispatch_and_provider_loss_never_replace_the_admitted_sandbox() {
    let mut fixture = fixture().await;
    let retail = retail();
    retail.attach(&mut fixture, &[ALICE]);
    let cookies = alice(&fixture).await;
    funded(&fixture, &cookies, &retail, "alice-retail", "1000").await;
    keyed(&fixture, &cookies, "alice-retail").await;
    let execution = bought(&fixture, &cookies, &retail, "alice-retail").await;
    retail.runtime.owner.lose_next_ack();
    retail.dispatch(&execution);
    let path = purchase("alice-retail", &execution);
    let first = get(&fixture, &cookies, &path).await;
    assert_eq!(first.status, StatusCode::OK, "{}", first.body);
    let resource = retail.resource(&execution);
    retail.settle();
    assert_eq!(retail.runtime.owner.started(), 1);

    // Provider loss after the executor started ends this purchase; nothing replaces it.
    retail.runtime.provider.lose(&resource);
    retail.settle();
    retail.reload(&mut fixture);
    let lost = get(&fixture, &cookies, &path).await;
    assert_eq!(lost.status, StatusCode::OK, "{}", lost.body);
    assert!(
        line(&lost.body, "Completion").contains("Provider lost"),
        "{}",
        lost.body
    );
    assert_eq!(line(&lost.body, "Sandbox"), resource);
    assert_eq!(retail.runtime.provider.create_calls(), 1);
    assert_eq!(retail.runtime.owner.started(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_spending_changed_quotes_and_revoked_rights_are_refused() {
    let mut fixture = fixture().await;
    let retail = retail();
    retail.attach(
        &mut fixture,
        &[
            ALICE,
            ("alice-second", "alice", "alice-personal", 3, "buyer", false),
        ],
    );
    let cookies = alice(&fixture).await;
    // Enough for one quote maximum, not two.
    funded(&fixture, &cookies, &retail, "alice-retail", "200").await;
    keyed(&fixture, &cookies, "alice-retail").await;
    keyed(&fixture, &cookies, "alice-second").await;
    let (first, quote_fields) = quoted(&fixture, &cookies, &retail, "alice-retail").await;
    let (second, _) = quoted(&fixture, &cookies, &retail, "alice-second").await;

    // A changed quote under the same request identity conflicts.
    let mut changed: Vec<(&str, &str)> = quote_fields
        .iter()
        .map(|(n, v)| (n.as_str(), v.as_str()))
        .collect();
    changed.extend(task_fields("Delete the repository."));
    assert_eq!(
        post(
            &fixture,
            &cookies,
            &format!("{PAGE}/alice-retail/quote"),
            &changed
        )
        .await
        .status,
        StatusCode::CONFLICT
    );
    // A review digest this page never showed cannot be confirmed.
    let page = get(&fixture, &cookies, PAGE).await;
    let action = format!("{PAGE}/alice-retail/confirm");
    let mut forged = confirm_fields(&page.body, &action, &first);
    for (name, value) in &mut forged {
        if name == "review" {
            *value = second.clone();
        }
    }
    let forged: Vec<(&str, &str)> = forged
        .iter()
        .map(|(n, v)| (n.as_str(), v.as_str()))
        .chain([("custody", "service")])
        .collect();
    assert_eq!(
        post(&fixture, &cookies, &action, &forged).await.status,
        StatusCode::CONFLICT
    );

    // Two concurrent confirmations cannot both reserve their maximum.
    let (a, b) = tokio::join!(
        confirm(&fixture, &cookies, "alice-retail", &first),
        confirm(&fixture, &cookies, "alice-second", &second),
    );
    let statuses = [a.0.status, b.0.status];
    assert_eq!(
        statuses
            .iter()
            .filter(|s| **s == StatusCode::SEE_OTHER)
            .count(),
        1,
        "{statuses:?} {} {}",
        a.0.body,
        b.0.body
    );
    assert!(
        statuses.contains(&StatusCode::PAYMENT_REQUIRED),
        "{statuses:?}"
    );
    let balance = retail.balance();
    assert_eq!(balance["held_msat"], 124_000);
    assert_eq!(balance["available_msat"], 76_000);
    assert_eq!(retail.wallet.issued(), 1);
    let (won, fields) = if a.0.status == StatusCode::SEE_OTHER {
        ("alice-retail", a.1)
    } else {
        ("alice-second", b.1)
    };
    let execution = journaled(&retail, &field_of(&fields, "request"))["outcome"]["execution"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        get(&fixture, &cookies, &purchase(won, &execution))
            .await
            .status,
        StatusCode::OK
    );

    // Revoked native rights refuse observation, stop, and new spending.
    let mut ledger = Ledger::open(&retail.root.join("compute.sqlite")).unwrap();
    ledger.revoke_principal("buyer", now() as i64).unwrap();
    assert_eq!(
        get(&fixture, &cookies, &purchase(won, &execution))
            .await
            .status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        get(&fixture, &cookies, &format!("{PAGE}/{won}/purchases"))
            .await
            .status,
        StatusCode::FORBIDDEN
    );
    let page = get(&fixture, &cookies, PAGE).await;
    assert!(
        !page
            .body
            .contains(&format!("action=\"{PAGE}/{won}/cancel\""))
    );
    assert!(
        !page
            .body
            .contains(&format!("action=\"{PAGE}/{won}/top-up\""))
    );
    assert_eq!(retail.wallet.issued(), 1);
}
