//! WEB-14 module acceptance: pipeline, evidence and claims, pilots and
//! delivery, invoices and fulfillment, journeys and weekly review, and the
//! scoped audit, against the real owner adapter over loopback HTTP with a
//! synthetic private pipeline, evidence, and credentials.

use super::*;
use coder::task::sales::claims::{
    ClaimInput, Evidence as ClaimEvidence, Pin, Purpose, Readiness, SOURCE_SCHEMA, Scope,
    SourceInput,
};
use receipts::service_sale::{Disposition, PaymentInput};

mod service_fixture {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../receipts/tests/support/service_sale.rs"
    ));
}
use service_fixture::{Comparison, retain};

const OWNER_BEARER: &str = "synthetic-sales-site-bearer-owner";
const UNRELATED: &str = "unrelated@synthetic.invalid";

struct Commercial {
    owner: Owner,
    other: String,
}

/// The same retained path and digest in the claims evidence type.
fn same<T: serde::de::DeserializeOwned>(r: receipts::service_sale::Reference) -> T {
    serde_json::from_value(serde_json::to_value(r).unwrap()).unwrap()
}

fn admin(owner: &Owner) -> (Store, coder::task::sales::Access) {
    let mut store = Store::open(&owner.root.join("host")).unwrap();
    let access = store
        .authenticate(&Store::read_credential(&owner.root.join("operator")).unwrap())
        .unwrap();
    (store, access)
}

/// The owner host with one accepted service sale, an unknown and a paid
/// payment verification, an unrelated customer's record, a withdrawn claim,
/// and a weekly review manifest.
async fn commercial() -> Commercial {
    let owner = owner().await;
    let evidence = owner.root.join("evidence");
    private_dir(&evidence);
    let at = now();
    let comparison = Comparison {
        manifest: retain(&evidence, "comparison.json", b"synthetic manifest"),
        report: retain(&evidence, "comparison-report.json", b"synthetic report"),
        candidate: retain(&evidence, "candidate.patch", b"synthetic candidate"),
        check: retain(&evidence, "independent-check", b"synthetic check"),
        decision: retain(&evidence, "buyer-decision", b"synthetic decision"),
        frozen_checks: vec![retain(&evidence, "frozen-command", b"synthetic frozen")],
    };
    let admission = service_fixture::admission(
        &evidence,
        at,
        &owner.lead,
        "synthetic-account",
        "offer-v1",
        comparison,
    );
    let (mut store, operator) = admin(&owner);
    let command = |id: &str, revision: u64, operation: Value| {
        serde_json::to_vec(&json!({"schema":coder::task::sales::COMMAND_SCHEMA,"id":id,
            "lead":owner.lead,"expected_revision":revision,"operation":operation}))
        .unwrap()
    };
    store
        .apply_with_evidence_root(
            &operator,
            &command(
                "admit-sale",
                1,
                json!({"kind":"record_service_sale","admission":admission}),
            ),
            Some(&evidence),
        )
        .unwrap();
    for (n, (id, disposition)) in [
        ("payment-unknown", Disposition::Unknown),
        ("payment-paid", Disposition::Paid),
    ]
    .into_iter()
    .enumerate()
    {
        let paid = disposition == Disposition::Paid;
        let payment = PaymentInput {
            disposition,
            external_reference: paid.then(|| "synthetic-external-payment".into()),
            paid_minor: paid.then_some(25000),
            reversed_minor: None,
            evidence: retain(&evidence, id, id.as_bytes()),
        };
        store
            .apply_with_evidence_root(
                &operator,
                &command(
                    id,
                    2 + n as u64,
                    json!({"kind":"reconcile_service_payment","sale":"synthetic-sale","payment":payment}),
                ),
                Some(&evidence),
            )
            .unwrap();
    }
    // An unrelated customer's record the scoped audit must exclude.
    let unrelated = json!({"schema":coder::task::sales::COMMAND_SCHEMA,"id":"unrelated-lead","lead":null,"expected_revision":0,"operation":{"kind":"create","ownership_acceptance":"operator accepted responsibility","input":{
        "contact":format!("email:{UNRELATED}"),"source":"synthetic unrelated introduction","source_at":at-20,
        "details":{"account":"unrelated-account","jurisdiction":"synthetic jurisdiction record",
        "permission":{"state":"granted","reference":"synthetic-consent-v1","recorded_at":at-10,"expires_at":at+86_400,"channels":["email"]},
        "workflow":"an unrelated synthetic workflow","baseline_reference":"private-baseline-reference",
        "data":{"recipients":["human:operator"],"permitted_use":"one agreed pilot","retain_until":at+2*86_400},
        "stage":"new","next":{"description":"first contact","due_at":at+3600},"customer_decision":null,"readers":[]}}}});
    let other = store
        .apply(&operator, &serde_json::to_vec(&unrelated).unwrap())
        .unwrap()
        .lead;
    // One reviewed capability claim, then withdrawn.
    let claims = owner.root.join("claims");
    private_dir(&claims);
    let source = SourceInput {
        schema: SOURCE_SCHEMA.into(),
        pin: Pin {
            id: "supported".into(),
            revision: 1,
        },
        scope: Scope {
            product: "Coder".into(),
            offer_version: coder::task::sales::intake::OFFER.into(),
            release: "a".repeat(40),
        },
        root: claims.clone(),
        evidence: ClaimEvidence::Capability {
            contract: same(retain(
                &claims,
                "contract",
                b"synthetic maintained contract",
            )),
            reviewed_fact: "Coder retains a checked patch for this scoped workflow.".into(),
        },
        readiness: Readiness::Implemented,
        limits: vec!["One declared repository, checks, supported client, and payer.".into()],
        review: same(retain(&claims, "source-review", b"synthetic source review")),
        expires_at: at + 3600,
        activation: None,
    };
    store.review_claim_source(&operator, source).unwrap();
    let claim = ClaimInput {
        pin: Pin {
            id: "fact".into(),
            revision: 1,
        },
        source: Pin {
            id: "supported".into(),
            revision: 1,
        },
        purpose: Purpose::Capability,
        playbook: same(retain(&claims, "playbook", b"reviewed playbook v1")),
        expires_at: at + 3600,
        review: same(retain(&claims, "claim-review", b"synthetic claim review")),
    };
    store.review_claim(&operator, claim.clone()).unwrap();
    store
        .withdraw_claim_revision(&operator, &claim.pin, false, "synthetic withdrawal")
        .unwrap();
    drop(store);

    // A completed seven-day weekly window with one explicit gap.
    let weekly = owner.root.join("weekly");
    private_dir(&weekly);
    let week = 7 * 24 * 60 * 60;
    private_file(
        &owner.root.join("weekly.json"),
        &serde_json::to_vec(
            &json!({"schema":"openagents.gym.sales-weekly.v1","owner":"operator",
            "period_start":at-week-100,"period_end":at-100,"generated_at":at-50,"journeys":[],
            "finance":null,"gaps":["synthetic-gap-no-provider-observations"]}),
        )
        .unwrap(),
    );

    // The adapter: the writer binding and a separate owner binding.
    let config = owner.root.join("remote.json");
    let document = json!({"schema":remote::CONFIG_SCHEMA,"root":owner.root.join("host"),
        "journal":owner.root.join("journal"),"evidence":evidence,
        "weekly":{"input":owner.root.join("weekly.json"),"evidence_root":weekly},
        "bindings":[{"id":"alice-sales","account":"alice","workspace":"alice-personal","members_epoch":3,
        "principal":"writer-a","credential":owner.root.join("writer-a"),
        "client_digest":hex_digest(SITE_BEARER.as_bytes()),"effects":["update"]},
        {"id":"alice-owner","account":"alice","workspace":"alice-personal","members_epoch":3,
        "principal":"operator","credential":owner.root.join("operator"),
        "client_digest":hex_digest(OWNER_BEARER.as_bytes()),"effects":[]}]});
    private_file(&config, &serde_json::to_vec(&document).unwrap());
    Commercial { owner, other }
}

impl Commercial {
    fn attach(&self, fixture: &mut Fixture) {
        let site = self.owner.root.join("site");
        private_dir(&site);
        for (name, bearer) in [("bearer", SITE_BEARER), ("owner-bearer", OWNER_BEARER)] {
            private_file(&site.join(name), bearer.as_bytes());
        }
        let delegation = |id: &str, bearer: &str| {
            json!({"id":id,"account":"alice","workspace":"alice-personal","members_epoch":3,
                "endpoint":self.owner.url,"binding":id,"bearer_file":site.join(bearer),
                "development_loopback":true})
        };
        let path = site.join("sales.json");
        private_file(
            &path,
            &serde_json::to_vec(&json!({"schema":crate::cloud::sales::SCHEMA,"directory":site,
                "delegations":[delegation("alice-sales","bearer"),delegation("alice-owner","owner-bearer")]}))
            .unwrap(),
        );
        fixture.config.cloud_sales = Some(Arc::new(
            crate::cloud::sales::Delegations::load(&path).unwrap(),
        ));
        fixture.site = crate::router(fixture.config.clone());
    }
}

/// The page offers no effect form on any sales path.
fn no_effects(body: &str) {
    assert!(
        !body.contains("method=\"post\" action=\"/cloud/app/sales"),
        "a module offered an effect"
    );
}

fn checked(answer: &Answer) {
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    private(answer);
    no_secret(&answer.body);
    assert!(!answer.body.contains(OWNER_BEARER));
    assert!(!answer.body.contains(CONTACT));
    assert!(!answer.body.contains(UNRELATED));
    no_effects(&answer.body);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sales_modules_show_separate_exact_records_and_offer_no_effects() {
    let mut fixture = fixture().await;
    let mut cookies = login(&fixture, "alice").await;
    choose_personal(&fixture, &mut cookies).await;
    let commercial = commercial().await;
    commercial.attach(&mut fixture);
    let lead = commercial.owner.lead.clone();

    // The pipeline links every module for each delegation.
    let index = get(&fixture, &cookies, PAGE).await;
    assert_eq!(index.status, StatusCode::OK, "{}", index.body);
    for module in [
        "Evidence and claims",
        "Pilots and delivery",
        "Invoices and fulfillment",
        "Journeys and weekly review",
    ] {
        assert!(index.body.contains(module), "{module}");
    }

    // Pilots and delivery: exact facts and each separate record, pinned kits.
    let pilots = get(&fixture, &cookies, &format!("{PAGE}/alice-sales/pilots")).await;
    checked(&pilots);
    for line in [
        "synthetic-sale",
        "Customer decision maker",
        "buyer",
        "all failed, repair, and retry attempts included",
        "Agreement acceptance",
        "Customer acceptance",
        "Support acceptance",
        "No separate reuse, training, public example, or marketing permission",
        "openagents.sales.pilot-kit.v1",
        "openagents.sales.delivery-kit.v1",
        "real customer qualified: false",
        "No cleanup recorded yet",
    ] {
        assert!(pilots.body.contains(line), "{line}");
    }

    // Delivery: support from the exact handoff; no cleanup is recorded, so
    // every planned item reads as not yet removed.
    let delivery = get(
        &fixture,
        &cookies,
        &format!("{PAGE}/alice-sales/leads/{lead}/services/synthetic-sale"),
    )
    .await;
    checked(&delivery);
    assert!(delivery.body.contains("one synthetic correction"));
    assert!(delivery.body.contains("No cleanup has been recorded yet."));
    assert!(delivery.body.contains("temporary-credentials"));
    assert!(delivery.body.contains("Not yet removed"));
    assert!(!delivery.body.contains("Unverified"));
    assert!(!delivery.body.contains("Removed "));
    assert!(!delivery.body.contains("synthetic support contact"));

    // Invoices: the invoice, every verification including unknown, no form.
    let invoices = get(&fixture, &cookies, &format!("{PAGE}/alice-sales/invoices")).await;
    checked(&invoices);
    for line in [
        "synthetic-invoice",
        "USD 250.00",
        "<td>Unknown</td>",
        "<td>Paid</td>",
        "an invoice is not a payment",
    ] {
        assert!(
            invoices.body.to_lowercase().contains(&line.to_lowercase()),
            "{line}"
        );
    }

    // The writer reads no owner register, review, or audit.
    let evidence = get(&fixture, &cookies, &format!("{PAGE}/alice-sales/evidence")).await;
    checked(&evidence);
    assert!(evidence.body.contains("owner-only"));
    let journeys = get(&fixture, &cookies, &format!("{PAGE}/alice-sales/journeys")).await;
    checked(&journeys);
    assert!(journeys.body.contains("No consented journey"));
    assert!(journeys.body.contains("owner-only"));
    let audit = get(
        &fixture,
        &cookies,
        &format!("{PAGE}/alice-sales/leads/{lead}/audit"),
    )
    .await;
    checked(&audit);
    assert!(audit.body.contains("owner-only"));

    // The owner binding: claim register with withdrawal and history.
    let evidence = get(&fixture, &cookies, &format!("{PAGE}/alice-owner/evidence")).await;
    checked(&evidence);
    assert!(evidence.body.contains("fact r1"));
    assert!(evidence.body.contains("Withdrawn"));
    assert!(evidence.body.contains("withdraw claim:fact:1"));
    // The weekly review is rebuilt from current sources, gaps included.
    let journeys = get(&fixture, &cookies, &format!("{PAGE}/alice-owner/journeys")).await;
    checked(&journeys);
    assert!(
        journeys
            .body
            .contains("synthetic-gap-no-provider-observations"),
        "{}",
        journeys.body
    );
    // The audit for one record excludes the unrelated customer's record.
    let audit = get(
        &fixture,
        &cookies,
        &format!("{PAGE}/alice-owner/leads/{lead}/audit"),
    )
    .await;
    checked(&audit);
    assert!(audit.body.contains("<td>admin</td>") || audit.body.contains("<td>operator</td>"));
    assert!(!audit.body.contains(&commercial.other[..16]));

    // Suppression fences the record's sale out of every module.
    let (mut store, operator) = admin(&commercial.owner);
    let revision = store.show(&operator, &lead).unwrap().revision;
    store
        .apply(
            &operator,
            &serde_json::to_vec(&json!({"schema":coder::task::sales::COMMAND_SCHEMA,
                "id":"suppress-lead","lead":lead,"expected_revision":revision,
                "operation":{"kind":"suppress","reference":"synthetic opt-out"}}))
            .unwrap(),
        )
        .unwrap();
    drop(store);
    for module in ["pilots", "invoices"] {
        let page = get(&fixture, &cookies, &format!("{PAGE}/alice-sales/{module}")).await;
        checked(&page);
        assert!(!page.body.contains("synthetic-sale"), "{module}");
    }
    let gone = get(
        &fixture,
        &cookies,
        &format!("{PAGE}/alice-sales/leads/{lead}/services/synthetic-sale"),
    )
    .await;
    assert_ne!(gone.status, StatusCode::OK);
    assert!(!gone.body.contains("one synthetic correction"));

    // A revoked writer credential refuses every module without content.
    commercial.owner.revoke();
    let refused = get(&fixture, &cookies, &format!("{PAGE}/alice-sales/pilots")).await;
    assert_ne!(refused.status, StatusCode::OK);
    assert!(!refused.body.contains("synthetic-invoice"));
}
