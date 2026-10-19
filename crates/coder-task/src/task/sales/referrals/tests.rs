use super::*;
use tenancy::accounts::referrals::{Capture, Kind, Outcome, CONSENT};

#[test]
fn canonical_assisted_source_preserves_identity_consent_and_current_pipeline_rights() {
    let (dir, mut pipeline, owner, credential) = fixture();
    let path = dir.path().join("canonical-accounts");
    let accounts = tenancy::Accounts::install(&path).unwrap();
    let referrer_account = accounts.create_account("Referrer", &[]).unwrap();
    let referrer = accounts.create_referrer(&referrer_account.id, Kind::Partner, "Partner").unwrap();
    let link = accounts.issue_referral_link(&referrer_account.id, &referrer.id).unwrap();
    let (buyer, source) = accounts.create_account_acquired("Buyer", &Capture { request: "assisted-customer".into(), token: Some(link.token), consent: true, consent_version: Some(CONSENT.into()) }).unwrap();
    let mut create: Command = serde_json::from_slice(&create("intro-lead")).unwrap();
    if let Operation::Create { input, .. } = &mut create.operation { input.details.account = buyer.id.clone(); }
    let lead = pipeline.apply(&owner, &serde_json::to_vec(&create).unwrap()).unwrap();
    let reader = grant(&dir, &mut pipeline, &owner, "reader", Role::Reader);
    let bytes = command("bind-source", Some(&lead.lead), lead.revision, Operation::RecordAcquisition { accounts_directory: path.display().to_string() });
    assert!(pipeline.apply(&reader, &bytes).is_err());
    let bound = pipeline.apply(&owner, &bytes).unwrap();
    assert_eq!(pipeline.apply(&owner, &bytes).unwrap(), bound);
    let record = pipeline.show(&owner, &lead.lead).unwrap().acquisition.unwrap();
    assert_eq!(record.source, source);
    assert_eq!(record.source.referrer.as_ref().unwrap().id, referrer.id);
    assert!(pipeline.show(&reader, &lead.lead).is_err());
    let replacement = command("replace-source", Some(&lead.lead), bound.revision, Operation::RecordAcquisition { accounts_directory: path.display().to_string() });
    assert!(pipeline.apply(&owner, &replacement).is_err());
    let mut changed = details();
    changed.account = referrer_account.id.clone();
    assert!(pipeline.apply(&owner, &command("move-account", Some(&lead.lead), bound.revision, Operation::Update { details: changed })).is_err());
    accounts.disable_referral_links(&referrer_account.id, &referrer.id).unwrap();
    drop(pipeline);
    let mut reopened = Store::open_with_clock(&dir.path().join("host"), now).unwrap();
    let owner = reopened.authenticate(&Store::read_credential(&credential).unwrap()).unwrap();
    assert_eq!(reopened.show(&owner, &lead.lead).unwrap().acquisition.unwrap(), record);
    assert_eq!(source.outcome, Outcome::Captured);
}
