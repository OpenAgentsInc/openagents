use super::*;
use crate::task::agent_key::FileKeys;
use crate::task::sales::{COMMAND_SCHEMA, Command, Details, Input, Operation};
use serde_json::json;
use tenancy::accounts::referrals::{CONSENT, Capture};

mod sources {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../receipts/tests/support/service_sale.rs"
    ));
}

fn now() -> u64 {
    1000
}

struct Fixture {
    work: tempfile::TempDir,
    store: Store,
    owner: Access,
    reader: Access,
    arthur: Anchor,
    vanna: Anchor,
}

fn native(root: &Path, work: &Path, name: &str) {
    let native = agent::Store::with_keys(root, name, std::sync::Arc::new(FileKeys)).unwrap();
    let record = native
        .open_as(work, now(), crate::task::agent::preset(name))
        .unwrap();
    let record = native.ensure_key(record, now()).unwrap();
    native
        .attest(
            record,
            &secp256k1::SecretKey::from_byte_array([17; 32]).unwrap(),
            now() + 5000,
            now(),
        )
        .unwrap();
}

fn fixture() -> Fixture {
    let work = tempfile::tempdir().unwrap();
    let root = work.path().join("host");
    let mut store = Store::open_with_clock(&root, now).unwrap();
    store
        .initialize("operator", &work.path().join("owner"))
        .unwrap();
    let owner = store
        .authenticate(&Store::read_credential(&work.path().join("owner")).unwrap())
        .unwrap();
    let reader_path = work.path().join("reader");
    store
        .issue(&owner, "reader", Role::Reader, &reader_path)
        .unwrap();
    let reader = store
        .authenticate(&Store::read_credential(&reader_path).unwrap())
        .unwrap();
    native(&root, work.path(), "arthur");
    native(&root, work.path(), "vanna");
    let arthur = store.sales_agent_anchor(&owner, "arthur").unwrap();
    let vanna = store.sales_agent_anchor(&owner, "vanna").unwrap();
    Fixture {
        work,
        store,
        owner,
        reader,
        arthur,
        vanna,
    }
}

fn binding(f: &Fixture, desk: Desk) -> Binding {
    Binding {
        schema: SCHEMA.into(),
        desk,
        revision: 1,
        anchor: match desk {
            Desk::Arthur => f.arthur.clone(),
            Desk::Vanna => f.vanna.clone(),
        },
        owner_credential: f.work.path().join("owner"),
        growth_owner: "growth-owner".into(),
        daily_usd_millionths: 1_000_000,
    }
}

fn configure(f: &mut Fixture, desk: Desk) -> String {
    let b = binding(f, desk);
    f.store
        .configure_desk(&f.owner, &b, &b.sha256().unwrap())
        .unwrap()
}

fn details(account: &str) -> Details {
    serde_json::from_value(json!({
        "account":account,"jurisdiction":"synthetic jurisdiction",
        "permission":{"state":"granted","reference":"synthetic-consent","recorded_at":999,"expires_at":1500,"channels":["email"]},
        "workflow":"synthetic accepted workflow","baseline_reference":"synthetic baseline",
        "data":{"recipients":["human:operator"],"permitted_use":"private partner preparation","retain_until":2000},
        "stage":"qualified","next":{"description":"review private terms","due_at":1200},"customer_decision":null,"readers":[]
    }))
    .unwrap()
}

fn lead(f: &mut Fixture, id: &str, account: &str) -> (String, u64) {
    let command = Command {
        schema: COMMAND_SCHEMA.into(),
        id: id.into(),
        lead: None,
        expected_revision: 0,
        operation: Operation::Create {
            input: Input {
                contact: format!("email:{id}-private-buyer@example.invalid"),
                source: "synthetic direct permission".into(),
                source_at: 999,
                details: details(account),
            },
            ownership_acceptance: "operator accepted responsibility".into(),
        },
    };
    let r = f
        .store
        .apply(&f.owner, &serde_json::to_vec(&command).unwrap())
        .unwrap();
    (r.lead, r.revision)
}

fn apply(
    f: &mut Fixture,
    id: &str,
    lead: &str,
    revision: u64,
    operation: Operation,
) -> Result<Receipt> {
    let command = Command {
        schema: COMMAND_SCHEMA.into(),
        id: id.into(),
        lead: Some(lead.into()),
        expected_revision: revision,
        operation,
    };
    f.store
        .apply(&f.owner, &serde_json::to_vec(&command).unwrap())
}

#[test]
fn desk_bindings_need_the_desks_own_crew_anchor_and_exact_owner_approval() {
    let mut f = fixture();
    let mut wrong = binding(&f, Desk::Arthur);
    wrong.anchor = f.vanna.clone();
    assert!(wrong.sha256().is_err(), "Arthur cannot bind Vanna's anchor");
    let mut rich = binding(&f, Desk::Vanna);
    rich.daily_usd_millionths = 6_000_000;
    assert!(
        rich.sha256().is_err(),
        "a desk cannot exceed the floor ceiling"
    );
    let b = binding(&f, Desk::Arthur);
    assert!(f.store.configure_desk(&f.owner, &b, "0").is_err());
    assert!(
        f.store
            .configure_desk(&f.reader, &b, &b.sha256().unwrap())
            .is_err()
    );
    let sha = configure(&mut f, Desk::Arthur);
    assert_eq!(sha, b.sha256().unwrap());
    assert!(
        f.store.configure_desk(&f.owner, &b, &sha).is_err(),
        "revision 1 cannot be approved twice"
    );
    assert!(f.store.partner_brief(&f.reader).is_err());
    assert!(
        f.store.attribution_view(&f.owner).is_err(),
        "Vanna has no binding yet"
    );
    drop(f.store);
    let mut reopened = Store::open_with_clock(&f.work.path().join("host"), now).unwrap();
    let owner = reopened
        .authenticate(&Store::read_credential(&f.work.path().join("owner")).unwrap())
        .unwrap();
    assert_eq!(reopened.partner_brief(&owner).unwrap().binding_sha256, sha);
}

#[test]
fn arthur_brief_cites_offerings_and_states_the_money_limitation() {
    let mut f = fixture();
    configure(&mut f, Desk::Arthur);
    let (lead_id, revision) = lead(&mut f, "partner-lead", "synthetic-account");
    let root = f.work.path().join("evidence");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::set_permissions(&root, std::os::unix::fs::PermissionsExt::from_mode(0o700)).unwrap();
    let mut proposal = partners::Proposal {
        id: "disc-1".into(),
        recipient_human: "operator".into(),
        expires_at: 1300,
        next: NextAction {
            description: "prepare one consented introduction".into(),
            due_at: 1100,
        },
        terms: partners::Terms::Discovery {
            brief: sources::retain(&root, "brief", b"public offering summary"),
            permitted_use: "private partner preparation".into(),
        },
        consent: sources::retain(&root, "consent", b"consent"),
        provenance: sources::retain(&root, "provenance", b"provenance"),
        approval: Reference {
            path: "placeholder.json".into(),
            sha256: "0".repeat(64),
        },
        commission: None,
    };
    let digest = f
        .store
        .partner_digest(&f.owner, &lead_id, &proposal)
        .unwrap()["proposal_sha256"]
        .as_str()
        .unwrap()
        .to_owned();
    proposal.approval = sources::doc(
        &root,
        "disc-1-approval.json",
        json!({"schema":"openagents.sales.partner-approval.v1",
        "pipeline_lead":lead_id,"assignment":"disc-1","proposal_sha256":digest,"approved_by":"operator","approved_at":1000,"allow_private_assignment":true}),
    );
    let command = Command {
        schema: COMMAND_SCHEMA.into(),
        id: "propose".into(),
        lead: Some(lead_id.clone()),
        expected_revision: revision,
        operation: Operation::ProposePartner { proposal },
    };
    f.store
        .apply_with_evidence_root(
            &f.owner,
            &serde_json::to_vec(&command).unwrap(),
            Some(&root),
        )
        .unwrap();
    let brief = f.store.partner_brief(&f.owner).unwrap();
    assert_eq!(brief.offerings.len(), 1);
    assert_eq!(brief.offerings[0].kind, "discovery");
    assert_eq!(brief.offerings[0].status, partners::Status::Proposed);
    assert_eq!(brief.limits, Limits::default());
    assert!(!brief.limits.commissions_available && !brief.limits.payouts_available);
    assert_eq!(brief.disclosure, Limits::DISCLOSURE);
    let text = serde_json::to_string(&brief).unwrap();
    assert!(!text.contains("private-buyer"), "contacts stay private");
    assert!(!text.contains("synthetic-account"), "accounts stay private");
    assert!(!text.contains(&lead_id), "raw lead ids stay private");
    let memory = f.store.desk_memory(&f.owner, Desk::Arthur).unwrap();
    assert_eq!(
        memory.references,
        vec![brief.offerings[0].lead_reference.clone()]
    );
    assert!(f.store.desk_memory(&f.owner, Desk::Vanna).is_err());
}

#[test]
fn vanna_attributes_outside_referrers_only_and_flags_abuse_without_payout_authority() {
    let mut f = fixture();
    configure(&mut f, Desk::Vanna);
    let path = f.work.path().join("canonical-accounts");
    let accounts = tenancy::Accounts::install(&path).unwrap();
    let directory = path.display().to_string();
    let acquired = |accounts: &tenancy::Accounts, kind: Kind, label: &str| {
        let referrer_account = accounts.create_account(label, &[]).unwrap();
        let referrer = match kind {
            Kind::Agent => accounts
                .create_sales_referrer(&referrer_account.id, label)
                .unwrap(),
            _ => accounts
                .create_referrer(&referrer_account.id, kind, label)
                .unwrap(),
        };
        let link = accounts
            .issue_referral_link(&referrer_account.id, &referrer.id)
            .unwrap();
        let (buyer, _) = accounts
            .create_account_acquired(
                &format!("{label} buyer"),
                &Capture {
                    request: format!("assisted-{}", label.to_ascii_lowercase()),
                    token: Some(link.token),
                    consent: true,
                    consent_version: Some(CONSENT.into()),
                },
            )
            .unwrap();
        (buyer.id, referrer.id)
    };
    let (partner_buyer, _) = acquired(&accounts, Kind::Partner, "Partner");
    let (agent_buyer, _) = acquired(&accounts, Kind::Agent, "Paul");
    let (plain, _) = accounts
        .create_account_acquired(
            "Plain buyer",
            &Capture {
                request: "assisted-plain".into(),
                token: None,
                consent: false,
                consent_version: None,
            },
        )
        .unwrap();
    let plain = plain.id;
    let mut bound = Vec::new();
    for (id, account) in [
        ("ref-partner", partner_buyer),
        ("ref-agent", agent_buyer),
        ("ref-none", plain),
    ] {
        let (lead_id, revision) = lead(&mut f, id, &account);
        apply(
            &mut f,
            &format!("{id}-bind"),
            &lead_id,
            revision,
            Operation::RecordAcquisition {
                accounts_directory: directory.clone(),
            },
        )
        .unwrap();
        bound.push(lead_id);
    }
    let view = f.store.attribution_view(&f.owner).unwrap();
    assert_eq!(view.rows.len(), 3);
    assert!(!view.payout_authority);
    assert_eq!(view.limits, Limits::default());
    let by_outcome = |earns: Earns| view.rows.iter().filter(|r| r.earns == earns).count();
    assert_eq!(by_outcome(Earns::ReferralWhenTermsPublish), 1);
    assert_eq!(
        by_outcome(Earns::Nothing),
        1,
        "our own agent's link earns nothing"
    );
    assert_eq!(by_outcome(Earns::NotAttributed), 1);
    assert_eq!(view.under_review, 0);
    let text = serde_json::to_string(&view).unwrap();
    for id in &bound {
        assert!(!text.contains(id.as_str()));
    }
    assert!(!text.contains("private-buyer") && !text.contains("Partner buyer"));
    assert!(f.store.attribution_view(&f.reader).is_err());
}

#[test]
fn self_referral_and_malformed_sources_enter_bounded_review() {
    let mut f = fixture();
    configure(&mut f, Desk::Vanna);
    let (lead_id, _) = lead(&mut f, "sybil", "sybil-account");
    let mut next = f.store.state.clone();
    let found = next.leads.get_mut(&lead_id).unwrap();
    found.acquisition = Some(referrals::Introduction {
        source: tenancy::accounts::referrals::Source {
            schema: tenancy::accounts::referrals::SCHEMA.into(),
            account: "sybil-account".into(),
            request: "sybil".into(),
            outcome: Outcome::Captured,
            referrer: Some(tenancy::accounts::referrals::Identity {
                id: "sybil-account".into(),
                version: 1,
                kind: Kind::Person,
                source_only: false,
            }),
            consent_version: Some(CONSENT.into()),
            captured_at: 999,
        },
        accounts_revision: format!("sha256:{}", "0".repeat(64)),
        recorded_by: "operator".into(),
        recorded_at: 999,
    });
    f.store.persist(next).unwrap();
    let view = f.store.attribution_view(&f.owner).unwrap();
    assert_eq!(view.under_review, 1);
    assert_eq!(view.rows[0].findings, vec![Finding::SelfReferral]);
    assert_eq!(view.rows[0].earns, Earns::ReferralWhenTermsPublish);
    assert!(!view.payout_authority);
}
