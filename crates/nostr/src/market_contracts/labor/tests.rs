use super::*;
use secp256k1::{Keypair, Secp256k1, SecretKey};
use serde_json::json;
use std::cell::Cell;

fn public(n: u8) -> String {
    Keypair::from_secret_key(
        &Secp256k1::new(),
        &SecretKey::from_byte_array([n; 32]).unwrap(),
    )
    .x_only_public_key()
    .0
    .to_string()
}
fn parties() -> Parties {
    Parties {
        buyer: public(1),
        provider: public(2),
        worker: public(3),
    }
}
fn reference(value: &Value, schema: &str) -> Value {
    let bytes = jcs(value).unwrap();
    json!({"digest":digest_bytes(&bytes),"size":bytes.len(),"media_type":"application/json","schema":schema})
}
fn dummy(schema: &str) -> Value {
    reference(&json!({"v":schema}), schema)
}
fn definition(n: u8, name: &str) -> Value {
    json!({"id":format!("{}:fixture/{name}",public(n)),"artifact":dummy("openagents.cap.v1")})
}
fn policy() -> Value {
    json!({"v":ACCEPTANCE_POLICY_SCHEMA,"requires":[],"checker":definition(1,"checker"),"lock":dummy("openagents.lock.v1"),"criteria":["tests","scope"],"rule":"all-pass-v1"})
}
fn rights() -> Value {
    json!({"v":RIGHTS_SCHEMA,"requires":[],"license":dummy("fixture.license.v1"),"input_use":"perform-and-review-order","output_use":"review-only","publication":"deny","training":"separate-grant","evaluation_reuse":"deny","redistribution":"deny","recipients":[public(1),public(2),public(3),public(4),public(5)],"retention":"through-market-retain-until"})
}
fn terms() -> Value {
    let schema = json!({"type":"object"});
    let mut schema_ref = reference(&schema, "https://json-schema.org/draft/2020-12/schema");
    schema_ref["media_type"] = json!("application/schema+json");
    json!({"v":LABOR_TERMS_SCHEMA,"requires":[],"task_frame":dummy("openagents.task-frame.v1"),"execution":{"target":definition(2,"work"),"lock":dummy("openagents.lock.v1"),"input":dummy("fixture.input.v1"),"context":dummy("openagents.context.v1"),"requirements":dummy("fixture.requirements.v1"),"bounds":[]},"deliverables":[{"id":"patch","schema":schema_ref,"max_bytes":4096}],"reviewer":public(4),"acceptance_policy":reference(&policy(),ACCEPTANCE_POLICY_SCHEMA),"resolver":public(5),"resolver_policy":"labor-evidence-v1","max_reworks":0,"rework_due_at":null,"dispute_due_at":420,"resolution_due_at":450,"cancellation":"evaluate-delivered-work-v1","partial_delivery":"no-partial-payment-v1","buyer_unavailable":"resolver-required-v1","rights":reference(&rights(),RIGHTS_SCHEMA),"role_relationships":(1..=5).map(|n|json!({"pubkey":public(n),"operator":format!("operator-{n}")})).collect::<Vec<_>>()})
}
fn market(t: &Value) -> Terms {
    Terms {
        profile: PROFILE.into(),
        profile_terms: parse_artifact(&reference(t, LABOR_TERMS_SCHEMA)).unwrap(),
        buyer: public(1),
        provider: public(2),
        worker: public(3),
        price_msat: 0,
        fee_limit_msat: 0,
        payment_profile: FREE_PROFILE.into(),
        network: None,
        quote_expires_at: 200,
        order_confirm_by: 220,
        delivery_due_at: 300,
        review_due_at: 400,
        payment_due_at: 500,
        retain_until: 600,
    }
}

#[test]
fn labor_terms_check_exact_roles_disclosure_and_deadlines() {
    let t = terms();
    let m = market(&t);
    let parsed = parse_labor_terms(&jcs(&t).unwrap(), &parties(), Some(&m)).unwrap();
    assert_eq!(parsed.max_reworks, 0);
    assert_eq!(parsed.deliverables[0].max_bytes, 4096);
    for (name, value) in [
        ("reviewer", json!(public(2))),
        ("resolver", json!(public(1))),
        ("max_reworks", json!(4)),
        ("rework_due_at", json!(350)),
        ("dispute_due_at", json!(399)),
        ("resolution_due_at", json!(500)),
        ("cancellation", json!("cancel-erases-debt")),
        ("partial_delivery", json!("prorate")),
        ("unknown_semantics", json!(true)),
        ("requires", json!(["future"])),
    ] {
        let mut changed = t.clone();
        changed[name] = value;
        assert!(
            parse_labor_terms(&jcs(&changed).unwrap(), &parties(), Some(&m)).is_err(),
            "accepted {name}"
        );
    }
    let mut changed = t.clone();
    changed["max_reworks"] = json!(1);
    changed["rework_due_at"] = json!(350);
    assert!(parse_labor_terms(&jcs(&changed).unwrap(), &parties(), Some(&m)).is_ok());
    changed["rework_due_at"] = json!(400);
    assert!(parse_labor_terms(&jcs(&changed).unwrap(), &parties(), Some(&m)).is_err());
    let mut wrong = m;
    wrong.worker = public(6);
    assert!(parse_labor_terms(&jcs(&t).unwrap(), &parties(), Some(&wrong)).is_err());
}

#[test]
fn deliverable_and_role_lists_are_closed_bounded_and_unique() {
    let t = terms();
    let mut bad = t.clone();
    bad["deliverables"] = json!([]);
    assert!(parse_labor_terms(&jcs(&bad).unwrap(), &parties(), None).is_err());
    bad = t.clone();
    bad["deliverables"][0]["max_bytes"] = json!(0);
    assert!(parse_labor_terms(&jcs(&bad).unwrap(), &parties(), None).is_err());
    bad = t.clone();
    bad["deliverables"]
        .as_array_mut()
        .unwrap()
        .push(t["deliverables"][0].clone());
    assert!(parse_labor_terms(&jcs(&bad).unwrap(), &parties(), None).is_err());
    bad = t.clone();
    bad["role_relationships"].as_array_mut().unwrap().pop();
    assert!(parse_labor_terms(&jcs(&bad).unwrap(), &parties(), None).is_err());
    bad = t.clone();
    bad["role_relationships"]
        .as_array_mut()
        .unwrap()
        .push(t["role_relationships"][0].clone());
    assert!(parse_labor_terms(&jcs(&bad).unwrap(), &parties(), None).is_err());
    bad = t;
    bad["execution"]["grant"] = json!("run-anything");
    assert!(parse_labor_terms(&jcs(&bad).unwrap(), &parties(), None).is_err());
}

#[test]
fn acceptance_policy_requires_exact_all_pass_criterion_set() {
    let p = policy();
    assert_eq!(
        parse_acceptance_policy(&jcs(&p).unwrap()).unwrap().criteria,
        vec!["tests", "scope"]
    );
    for (name, value) in [
        ("criteria", json!(["tests", "tests"])),
        ("criteria", json!([])),
        ("criteria", json!(["bad/slug"])),
        ("rule", json!("majority")),
        ("grant", json!(true)),
    ] {
        let mut bad = p.clone();
        bad[name] = value;
        assert!(parse_acceptance_policy(&jcs(&bad).unwrap()).is_err());
    }
}

#[test]
fn reuse_is_restriction_not_automatic_grant() {
    let r = rights();
    let parsed = parse_rights(&jcs(&r).unwrap()).unwrap();
    assert_eq!(parsed.training, Reuse::SeparateGrant);
    assert_eq!(parsed.publication, Reuse::Deny);
    for (name, value) in [
        ("training", json!("allow")),
        ("publication", json!(true)),
        ("input_use", json!("resell")),
        ("retention", json!("forever")),
        ("recipients", json!([])),
        ("hidden_permission", json!("yes")),
    ] {
        let mut bad = r.clone();
        bad[name] = value;
        assert!(parse_rights(&jcs(&bad).unwrap()).is_err());
    }
}

struct Host {
    blobs: BTreeMap<String, Vec<u8>>,
    allowed: bool,
    calls: Cell<usize>,
}
impl Host {
    fn new(t: &Value, p: &Value, r: &Value, allowed: bool) -> Self {
        let mut blobs = BTreeMap::new();
        for v in [t, p, r] {
            let bytes = jcs(v).unwrap();
            blobs.insert(digest_bytes(&bytes), bytes);
        }
        Self {
            blobs,
            allowed,
            calls: Cell::new(0),
        }
    }
}
impl ClosureAdmission for Host {
    fn resolve(&self, reference: &ArtifactRef) -> Result<Vec<u8>, ContractError> {
        self.blobs.get(&reference.digest).cloned().ok_or_else(|| {
            ContractError::new(RefusalCode::ContentUnavailable, "fixture dependency")
        })
    }
    fn check(
        &self,
        _: &Parties,
        _: &LaborTerms,
        _: &AcceptancePolicy,
        _: &Rights,
        _: Option<&DefinitionRef>,
    ) -> Result<(), ContractError> {
        self.calls.set(self.calls.get() + 1);
        if self.allowed {
            Ok(())
        } else {
            Err(not_admitted("fixture host rejects unsupported closure"))
        }
    }
}

#[test]
fn market_adapter_requires_host_closure_admission_after_exact_local_checks() {
    let t = terms();
    let p = policy();
    let r = rights();
    let h = Host::new(&t, &p, &r, true);
    let profile = LaborProfile::new(parties(), &h).unwrap();
    profile.validate_request(&market(&t).profile_terms).unwrap();
    assert_eq!(h.calls.get(), 1);
    profile
        .validate_terms(
            &market(&t),
            &parse_definition(&definition(2, "work")).unwrap(),
        )
        .unwrap();
    assert_eq!(h.calls.get(), 2);
    let h = Host::new(&t, &p, &r, false);
    let profile = LaborProfile::new(parties(), &h).unwrap();
    assert_eq!(
        profile
            .validate_request(&market(&t).profile_terms)
            .unwrap_err()
            .code,
        RefusalCode::NotAdmitted
    );
}

#[test]
fn missing_tampered_or_undisclosable_closure_never_reaches_host_admission() {
    let t = terms();
    let p = policy();
    let r = rights();
    let mut h = Host::new(&t, &p, &r, true);
    h.blobs.remove(&digest_bytes(&jcs(&p).unwrap()));
    let profile = LaborProfile::new(parties(), &h).unwrap();
    assert_eq!(
        profile
            .validate_request(&market(&t).profile_terms)
            .unwrap_err()
            .code,
        RefusalCode::ContentUnavailable
    );
    assert_eq!(h.calls.get(), 0);
    let mut h = Host::new(&t, &p, &r, true);
    h.blobs
        .insert(digest_bytes(&jcs(&p).unwrap()), b"{}".to_vec());
    assert_eq!(
        LaborProfile::new(parties(), &h)
            .unwrap()
            .validate_request(&market(&t).profile_terms)
            .unwrap_err()
            .code,
        RefusalCode::IdentityMismatch
    );
    let mut r = rights();
    r["recipients"].as_array_mut().unwrap().pop();
    let mut t = terms();
    t["rights"] = reference(&r, RIGHTS_SCHEMA);
    let h = Host::new(&t, &p, &r, true);
    assert_eq!(
        LaborProfile::new(parties(), &h)
            .unwrap()
            .validate_request(&market(&t).profile_terms)
            .unwrap_err()
            .code,
        RefusalCode::NotAdmitted
    );
    assert_eq!(h.calls.get(), 0);
}
