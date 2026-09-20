//! Regression tests for what a gate digest identifies.
//!
//! A digest covers the parts of a gate that decide — the schema, the id,
//! the rule's tag, every bound's value, basis, and evidence, the enums that
//! say what a statistic covers, and a pending measurement's identity — and
//! none of the prose that explains them. Historical digests recorded under
//! an earlier encoding attribute only through `previously` bindings whose
//! stored identity snapshot still matches the live rule.
//! `docs/gym/gate-digests.md` states the policy these tests pin.

use std::path::Path;

use gym::gate::{
    self, Basis, Bound, Evidence, Gate, GateError, GatedPercentile, Pending, Rule, VarianceBasis,
};

/// The digest `openagents.gym.gate.v1` recorded for `probability-v1`, and
/// the value the committed `support-v2-three-way` rows pin. `previously`
/// carries it forward so those rows still attribute.
const PROBABILITY_V1_LEGACY: &str =
    "gate:368cefd18f308119008db3099c8415af380c9d096d4cbcb6997196bfc6d82013";

/// The gates committed to `crates/gym/gates/`.
const SHIPPED: [&str; 3] = ["decision-v1", "deployment-v1", "probability-v1"];

fn gate(id: &str) -> Gate {
    gate::load(id).unwrap_or_else(|error| panic!("{id} loads: {error}"))
}

/// Every bound a rule carries, mutably, so one edit can visit all of them.
fn bounds_mut(rule: &mut Rule) -> Vec<&mut Bound> {
    match rule {
        Rule::Decision(rule) => vec![&mut rule.min_items, &mut rule.gain_standard_errors],
        Rule::Probability(rule) => vec![
            &mut rule.min_items,
            &mut rule.min_ece_reduction,
            &mut rule.max_brier_increase,
        ],
        Rule::Deployment(rule) => vec![
            &mut rule.min_calls,
            &mut rule.latency_block_sigma_relative,
            &mut rule.regression_sigmas,
        ],
    }
}

/// The gap a rule records, mutably.
fn pending_mut(rule: &mut Rule) -> Option<&mut Pending> {
    match rule {
        Rule::Decision(rule) => rule.pending_measurement.as_mut(),
        Rule::Probability(rule) => rule.pending_measurement.as_mut(),
        Rule::Deployment(rule) => rule.pending_measurement.as_mut(),
    }
}

#[test]
fn every_shipped_gate_loads_under_v2_and_digests() {
    for id in SHIPPED {
        let gate = gate(id);
        assert_eq!(gate.schema, gate::SCHEMA, "{id}");
        assert_eq!(gate.id, id);
        let digest = gate.digest();
        assert!(
            digest.starts_with("gate:")
                && digest.len() == 69
                && digest[5..]
                    .chars()
                    .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)),
            "{id}: {digest} is not gate:<sha256>"
        );
    }
}

#[test]
fn the_shipped_gates_are_three_different_rules() {
    let digests: Vec<String> = SHIPPED.iter().map(|id| gate(id).digest()).collect();
    for (index, digest) in digests.iter().enumerate() {
        assert!(
            !digests[..index].contains(digest),
            "two shipped gates share a digest"
        );
    }
}

#[test]
fn editing_the_question_does_not_re_identify_a_gate() {
    for id in SHIPPED {
        let gate = gate(id);
        let mut edited = gate.clone();
        edited.question = "A different sentence for the same rule.".into();
        assert_eq!(gate.digest(), edited.digest(), "{id}");
    }
}

#[test]
fn editing_a_bounds_prose_does_not_re_identify_a_gate() {
    for id in SHIPPED {
        let gate = gate(id);
        let mut edited = gate.clone();
        for bound in bounds_mut(&mut edited.rule) {
            bound.why = "docs/moved/elsewhere.md says so, punctuated differently!".into();
        }
        assert_eq!(gate.digest(), edited.digest(), "{id}");
    }
}

#[test]
fn editing_a_pending_why_does_not_re_identify_a_gate() {
    for id in SHIPPED {
        let gate = gate(id);
        assert!(
            gate.rule.pending_measurement().is_some(),
            "{id} records the measurement it is missing"
        );
        let mut edited = gate.clone();
        if let Some(pending) = pending_mut(&mut edited.rule) {
            pending.why = "A different sentence; the record moved to docs/elsewhere.md.".into();
        }
        assert_eq!(gate.digest(), edited.digest(), "{id}");
    }
}

#[test]
fn editing_the_comment_does_not_re_identify_a_gate() {
    for id in SHIPPED {
        let gate = gate(id);
        let mut edited = gate.clone();
        edited.comment = Some(serde_json::Value::String(
            "a different commentary, with a different doc path".into(),
        ));
        assert_eq!(gate.digest(), edited.digest(), "{id}");
    }
}

#[test]
fn a_threshold_change_is_a_different_gate() {
    for id in SHIPPED {
        let gate = gate(id);
        let mut edited = gate.clone();
        let bound = bounds_mut(&mut edited.rule).swap_remove(0);
        bound.value = bound.value.map(|value| value + 1.0);
        assert_ne!(gate.digest(), edited.digest(), "{id}: a new bound is a new rule");
    }
}

#[test]
fn relabelling_a_bounds_basis_is_a_different_gate() {
    for id in SHIPPED {
        let gate = gate(id);
        let mut edited = gate.clone();
        let bound = bounds_mut(&mut edited.rule).swap_remove(0);
        bound.basis = match bound.basis {
            Basis::Derived => Basis::Tuned,
            _ => Basis::Derived,
        };
        assert_ne!(gate.digest(), edited.digest(), "{id}");
    }
}

#[test]
fn renaming_the_gate_is_a_different_gate() {
    for id in SHIPPED {
        let gate = gate(id);
        let mut edited = gate.clone();
        edited.id = format!("{id}-renamed");
        assert_ne!(gate.digest(), edited.digest(), "{id}");
    }
}

#[test]
fn changing_a_rules_evidence_is_a_different_gate() {
    for id in SHIPPED {
        let gate = gate(id);
        let mut edited = gate.clone();
        let bound = bounds_mut(&mut edited.rule).swap_remove(0);
        bound.evidence.push(Evidence::Measurement { id: "a-record-the-bound-never-read".into() });
        assert_ne!(
            gate.digest(),
            edited.digest(),
            "{id}: a bound resting on a different record is a different rule"
        );
    }
}

#[test]
fn changing_pending_identity_is_a_different_gate() {
    for id in SHIPPED {
        let gate = gate(id);
        let mut edited = gate.clone();
        let pending = pending_mut(&mut edited.rule)
            .unwrap_or_else(|| panic!("{id} records the measurement it is missing"));
        pending.quantity = "a quantity nobody is missing".into();
        assert_ne!(gate.digest(), edited.digest(), "{id}: the quantity");

        let mut edited = gate.clone();
        let pending = pending_mut(&mut edited.rule).expect("the gap is recorded");
        pending.issue = Some("openagents#9999".into());
        assert_ne!(gate.digest(), edited.digest(), "{id}: the issue that takes it");
    }
}

#[test]
fn the_enums_that_say_what_a_statistic_covers_are_identity() {
    let decision = gate("decision-v1");
    let mut edited = decision.clone();
    let Rule::Decision(rule) = &mut edited.rule else {
        panic!("decision-v1 carries a decision rule");
    };
    rule.variance_basis = VarianceBasis::ItemSamplingAndTrialResampling;
    assert_ne!(decision.digest(), edited.digest(), "variance_basis");

    let deployment = gate("deployment-v1");
    let mut edited = deployment.clone();
    let Rule::Deployment(rule) = &mut edited.rule else {
        panic!("deployment-v1 carries a deployment rule");
    };
    rule.gated_percentile = GatedPercentile::P50;
    assert_ne!(deployment.digest(), edited.digest(), "gated_percentile");
}

#[test]
fn the_schema_is_inside_the_identity() {
    for id in SHIPPED {
        let gate = gate(id);
        let mut edited = gate.clone();
        edited.schema = "openagents.gym.gate.v3".into();
        assert_ne!(
            gate.digest(),
            edited.digest(),
            "{id}: a schema transition is an identity transition"
        );
    }
}

#[test]
fn the_recorded_v1_digest_still_attributes_to_probability_v1() {
    let gate = gate("probability-v1");
    let alias = gate
        .previously
        .iter()
        .find(|alias| alias.digest == PROBABILITY_V1_LEGACY)
        .expect("probability-v1 carries the digest the v1 encoding recorded");
    assert!(
        alias
            .recorded_in
            .iter()
            .any(|record| record.ends_with("support-v2-three-way.jsonl")),
        "the binding names a record that carries the digest"
    );
    assert!(
        gate.has_digest(PROBABILITY_V1_LEGACY),
        "a row pinned to the recorded digest attributes to this rule"
    );
    assert_ne!(
        gate.digest(),
        PROBABILITY_V1_LEGACY,
        "the recorded digest is an alias, not the current encoding's output"
    );
    assert!(gate.has_digest(&gate.digest()));
}

#[test]
fn a_digest_no_gate_recorded_does_not_attribute() {
    let foreign = "gate:0000000000000000000000000000000000000000000000000000000000000001";
    for id in SHIPPED {
        let gate = gate(id);
        assert!(!gate.has_digest(foreign), "{id}");
        assert!(!gate.has_digest("not-a-digest"), "{id}");
        assert!(!gate.has_digest(""), "{id}");
    }
}

#[test]
fn decision_and_deployment_carried_no_recorded_digest_forward() {
    // The v1 store pinned rows to probability-v1's digest alone; the other
    // two gates recorded no digest under the old encoding, so they carry no
    // aliases and nothing written before this schema attributes to them.
    for id in ["decision-v1", "deployment-v1"] {
        let gate = gate(id);
        assert!(gate.previously.is_empty(), "{id}");
        assert!(!gate.has_digest(PROBABILITY_V1_LEGACY), "{id}");
    }
}

/// A minimal decision gate, with `previously` spliced in verbatim.
fn fixture(previously: &str) -> String {
    r#"{
        "schema": "openagents.gym.gate.v2",
        "id": "alias-v1",
        "question": "Does the binding hold?",
        PREVIOUSLY
        "rule": {
            "decides": "decision",
            "min_items": { "value": 10, "basis": "derived", "why": "a floor" },
            "gain_standard_errors": { "value": 2.0, "basis": "convention", "why": "a multiple" },
            "variance_basis": "item_sampling",
            "pending_measurement": null
        }
    }"#
    .replace("PREVIOUSLY", previously)
}

/// The identity projection the fixture's rule produces — the `equivalent`
/// document a correct `previously` binding carries.
const FIXTURE_IDENTITY: &str = r#"{
    "identity_schema": "openagents.gym.gate-identity.v1",
    "schema": "openagents.gym.gate.v2",
    "id": "alias-v1",
    "rule": {
        "decides": "decision",
        "min_items": { "value": 10, "basis": "derived", "evidence": [] },
        "gain_standard_errors": { "value": 2.0, "basis": "convention", "evidence": [] },
        "variance_basis": "item_sampling",
        "pending_measurement": null
    }
}"#;

/// A binding over the fixture gate, with the fields spliced in verbatim.
fn binding(digest: &str, equivalent: &str, recorded_in: &str) -> String {
    format!(
        r#""previously": [
            {{
                "digest": "{digest}",
                "equivalent": {equivalent},
                "recorded_in": {recorded_in}
            }}
        ],"#
    )
}

#[test]
fn a_correctly_bound_alias_attributes() {
    let recorded = format!("gate:{}", "a".repeat(64));
    let source = fixture(&binding(
        &recorded,
        FIXTURE_IDENTITY,
        r#"["a-store-that-recorded-it.jsonl"]"#,
    ));
    let gate = Gate::from_json(&source, Path::new("alias-v1.json")).expect("the binding holds");
    assert!(gate.has_digest(&recorded));
    assert!(!gate.has_digest(&format!("gate:{}", "b".repeat(64))));
}

#[test]
fn a_binding_to_a_stale_identity_is_refused() {
    // The snapshot says min_items 11 and the file says 10: the binding was
    // reviewed against a rule this file no longer is.
    let recorded = format!("gate:{}", "a".repeat(64));
    let stale = FIXTURE_IDENTITY.replace("\"value\": 10", "\"value\": 11");
    let source = fixture(&binding(
        &recorded,
        &stale,
        r#"["a-store-that-recorded-it.jsonl"]"#,
    ));
    let error = Gate::from_json(&source, Path::new("alias-v1.json")).unwrap_err();
    assert!(matches!(error, GateError::Invalid { .. }), "{error}");
}

#[test]
fn a_binding_with_a_malformed_digest_is_refused() {
    let source = fixture(&binding(
        "the old one",
        FIXTURE_IDENTITY,
        r#"["a-store-that-recorded-it.jsonl"]"#,
    ));
    let error = Gate::from_json(&source, Path::new("alias-v1.json")).unwrap_err();
    assert!(matches!(error, GateError::Invalid { .. }), "{error}");
}

#[test]
fn a_binding_without_record_provenance_is_refused() {
    // A digest-shaped string is not proof the digest was recorded; the
    // binding has to name a record that carries it.
    let recorded = format!("gate:{}", "a".repeat(64));
    let source = fixture(&binding(&recorded, FIXTURE_IDENTITY, "[]"));
    let error = Gate::from_json(&source, Path::new("alias-v1.json")).unwrap_err();
    assert!(matches!(error, GateError::Invalid { .. }), "{error}");
}

#[test]
fn a_bare_string_in_previously_is_refused() {
    let source = fixture(r#""previously": ["the old one"],"#);
    let error = Gate::from_json(&source, Path::new("alias-v1.json")).unwrap_err();
    assert!(matches!(error, GateError::Parse { .. }), "{error}");
}

#[test]
fn a_v1_document_is_refused_by_name_not_as_a_syntax_error() {
    // The lenient read of `pending_measurement` exists so this file reaches
    // the schema check: a v1 gate is the wrong schema, not a parse failure.
    let source = r#"{
        "schema": "openagents.gym.gate.v1",
        "id": "legacy-v1",
        "question": "Does a v1 document parse far enough to be refused by name?",
        "rule": {
            "decides": "decision",
            "min_items": { "value": 10, "basis": "derived", "why": "a floor" },
            "gain_standard_errors": { "value": 2.0, "basis": "convention", "why": "a multiple" },
            "variance_basis": "item_sampling",
            "pending_measurement": "openagents#9370"
        }
    }"#;
    let error = Gate::from_json(source, Path::new("legacy-v1.json")).unwrap_err();
    assert!(matches!(error, GateError::Schema { .. }), "{error}");
}

#[test]
fn a_v2_pending_string_is_refused_for_carrying_no_why() {
    // Under v2 a bare string parses into a Pending with no `why` and fails
    // validation, which is what keeps a v2 file honest about the gap's
    // identity.
    let source = r#"{
        "schema": "openagents.gym.gate.v2",
        "id": "gap-v1",
        "question": "Does a bare pending string pass?",
        "rule": {
            "decides": "decision",
            "min_items": { "value": 10, "basis": "derived", "why": "a floor" },
            "gain_standard_errors": { "value": 2.0, "basis": "convention", "why": "a multiple" },
            "variance_basis": "item_sampling",
            "pending_measurement": "openagents#9370"
        }
    }"#;
    let error = Gate::from_json(source, Path::new("gap-v1.json")).unwrap_err();
    assert!(matches!(error, GateError::Invalid { .. }), "{error}");
}

#[test]
fn evidence_names_a_record_not_the_file_holding_it() {
    // A path is not an identity: moving the documentation tree must not
    // re-identify a rule, so the validator refuses one.
    for bad in ["docs/gym/measurements/2026-09-19-suite-v2-scores.md", "measurements/foo"] {
        let mut edited = gate("probability-v1");
        let bound = bounds_mut(&mut edited.rule).swap_remove(0);
        bound.evidence = vec![Evidence::Measurement { id: bad.into() }];
        let error = edited.validate().unwrap_err();
        assert!(matches!(error, GateError::Invalid { .. }), "{bad}: {error}");
    }
}

#[test]
fn no_policy_edit_inherits_the_previous_encodings_identity() {
    // Every kind of semantic edit breaks the binding: the stored snapshot
    // no longer matches the live projection, so the alias stops attributing
    // even though the digest string is untouched.
    let mut edited = gate("probability-v1");
    let Rule::Probability(rule) = &mut edited.rule else {
        unreachable!()
    };
    rule.min_ece_reduction.value = Some(0.2);
    assert!(!edited.has_digest(PROBABILITY_V1_LEGACY), "a moved threshold");

    let mut edited = gate("probability-v1");
    let Rule::Probability(rule) = &mut edited.rule else {
        unreachable!()
    };
    rule.min_ece_reduction.basis = Basis::Derived;
    assert!(!edited.has_digest(PROBABILITY_V1_LEGACY), "a relabelled basis");

    let mut edited = gate("probability-v1");
    let Rule::Probability(rule) = &mut edited.rule else {
        unreachable!()
    };
    rule.min_items
        .evidence
        .push(Evidence::Measurement { id: "a-record-the-bound-never-read".into() });
    assert!(!edited.has_digest(PROBABILITY_V1_LEGACY), "a different evidence set");

    let mut edited = gate("probability-v1");
    pending_mut(&mut edited.rule).expect("the gap is recorded").quantity =
        "a quantity nobody is missing".into();
    assert!(!edited.has_digest(PROBABILITY_V1_LEGACY), "a different pending quantity");

    let mut edited = gate("probability-v1");
    pending_mut(&mut edited.rule).expect("the gap is recorded").issue =
        Some("openagents#9999".into());
    assert!(!edited.has_digest(PROBABILITY_V1_LEGACY), "a different pending issue");
}

#[test]
fn prose_edits_keep_the_previous_encodings_identity() {
    // The binding covers the identity, and prose is outside it, so an edit
    // that leaves the rule alone leaves the attribution alone.
    let mut gate = gate("probability-v1");
    gate.question = "A different sentence for the same rule.".into();
    gate.comment = Some(serde_json::Value::String("moved commentary".into()));
    for bound in bounds_mut(&mut gate.rule) {
        bound.why = "a different explanation".into();
    }
    pending_mut(&mut gate.rule).expect("the gap is recorded").why =
        "a different interim story".into();
    assert!(gate.has_digest(PROBABILITY_V1_LEGACY));
}

#[test]
fn a_pending_object_with_an_unknown_field_is_refused() {
    // Unknown fields fail closed: a new semantic field must not be silently
    // dropped on the way to the digest.
    let source = r#"{
        "schema": "openagents.gym.gate.v2",
        "id": "gap-v1",
        "question": "Does an unknown pending field pass?",
        "rule": {
            "decides": "decision",
            "min_items": { "value": 10, "basis": "derived", "why": "a floor" },
            "gain_standard_errors": { "value": 2.0, "basis": "convention", "why": "a multiple" },
            "variance_basis": "item_sampling",
            "pending_measurement": {
                "quantity": "the suite's trial-to-trial resampling variance",
                "issue": "openagents#9370",
                "why": "the standard error covers item sampling only",
                "owner": "whoever claims it"
            }
        }
    }"#;
    let error = Gate::from_json(source, Path::new("gap-v1.json")).unwrap_err();
    assert!(matches!(error, GateError::Parse { .. }), "{error}");
}

#[test]
fn a_changed_policy_cannot_inherit_the_previous_encodings_identity() {
    let mut gate = gate("probability-v1");
    assert!(gate.has_digest(PROBABILITY_V1_LEGACY));
    let Rule::Probability(rule) = &mut gate.rule else { unreachable!() };
    rule.max_brier_increase.value = Some(rule.max_brier_increase.value.unwrap() + 0.01);
    assert!(!gate.has_digest(PROBABILITY_V1_LEGACY),
        "a new policy must not attribute historical rows through an unchanged alias list");
}
