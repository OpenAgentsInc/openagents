//! Prospective index budgets from independently recorded plain BF16 CPU runs.
use super::*;

pub(super) fn derive(
    reference: &[Value],
    controls: &[Value],
    index_reference: &[Value],
) -> Result<(Policy, Value), Refused> {
    if index_reference.is_empty() {
        return Err(refused(
            "rule 2.4 needs complete independent index references",
        ));
    }
    if reference
        .iter()
        .any(|r| !r["control"].is_null() || r["backend"] != "cpu")
    {
        return Err(refused("rule 2.4 takes uncontrolled CPU references"));
    }
    let names: BTreeSet<_> = controls
        .iter()
        .filter_map(|r| r["control"].as_str())
        .collect();
    if ![
        "wrong-weights",
        "wrong-routing",
        "wrong-rope",
        "stale-cache",
    ]
    .into_iter()
    .all(|c| names.contains(c))
    {
        return Err(refused(
            "rule 2.4 needs all four numerical negative controls",
        ));
    }
    let (mut policy, mut evidence) = derive_policy_v23(reference, controls)?;
    let (mut error, mut fraction, mut most) = (0.0_f64, 0.0_f64, 1_usize);
    let mut cases = BTreeSet::new();
    for r in index_reference {
        let source = &r["source"];
        let name = source["case"]
            .as_str()
            .ok_or_else(|| refused("missing reference case"))?;
        if source["profile"] != "cpu-bf16"
            || !cases.insert(name)
            || !reference.iter().any(|base| {
                base["stage_bundle_sha256"] == source["stage_bundle_sha256"]
                    && base["cases"]
                        .as_array()
                        .is_some_and(|cs| cs.iter().any(|c| c["case"] == name))
            })
            || r["recording_manifest_sha256"]
                .as_str()
                .is_none_or(|s| s.len() != 64)
            || r["oracle_manifest_sha256"]
                .as_str()
                .is_none_or(|s| s.len() != 64)
        {
            return Err(refused(
                "index reference inputs, profile, or bundle do not match the CPU runs",
            ));
        }
        let i = &r["index"];
        if i["complete"] != true || i["local_changed"].as_u64() != Some(0) || i["error"].is_string()
        {
            return Err(refused(
                "incomplete or internally inconsistent reference index observations",
            ));
        }
        let e = i["score_error_max"]
            .as_f64()
            .filter(|e| e.is_finite() && *e >= 0.0)
            .ok_or_else(|| refused("invalid reference index score error"))?;
        error = error.max(e);
        for kind in ["indices", "candidate_blocks"] {
            let s = &i["kinds"][kind];
            let rows = s["rows"]
                .as_u64()
                .filter(|n| *n > 0)
                .ok_or_else(|| refused("missing index decision rows"))?;
            let changed = s["changed"]
                .as_u64()
                .filter(|n| *n <= rows)
                .ok_or_else(|| refused("invalid changed-row count"))?;
            let n = s["max_row_substitutions"]
                .as_u64()
                .filter(|n| *n <= 512)
                .ok_or_else(|| refused("invalid substitution count"))?;
            fraction = fraction.max(changed as f64 / rows as f64);
            most = most.max(n as usize);
        }
    }
    // These are the existing prospective headroom and case aggregation rules.
    // Each changed 512-way selection counts as one changed decision; IDs are
    // not pooled into the denominator. Candidate blocks are judged separately.
    policy.version = "2.4".into();
    policy.index = DecisionRule {
        epsilon: Some(HEADROOM * error),
        max_substitutions: most,
        max_substituted_fraction: Some((HEADROOM * fraction).clamp(1.0 / 33.0, 1.0)),
    };
    policy.index_out_of_scope = None;
    for r in index_reference {
        let failures = index::violations(policy.index, &r["index"]);
        if !failures.is_empty() {
            return Err(refused(format!(
                "the independent index reference fails its derived rule: {}",
                failures.join("; ")
            )));
        }
    }
    // Older references contributed distribution and router measurements only.
    // Recheck those under the resulting policy, without fabricating index data.
    let mut distribution_policy = policy.clone();
    distribution_policy.index_out_of_scope =
        Some("checked separately from complete recorded rows".into());
    let checks = self_check(&distribution_policy, reference, controls)?;
    evidence["rule"] = json!("qualify::calibration::derive (rule 2.4)");
    evidence["rationale"] = json!(
        "Plain BF16 CPU portability envelope plus complete CPU index observations, frozen before fresh confirmation. No CUDA candidate measurement determines a threshold. Every changed index set must satisfy the score budget, stable-boundary, regret, and per-row substitution limits. The fraction of decisions with any changed ID is checked separately for index selections and candidate blocks."
    );
    evidence["index_reference_cases"] = json!(cases);
    evidence["index_reference_max"] =
        json!({"score_error":error,"changed_decision_fraction":fraction,"row_substitutions":most});
    evidence["index_reference_manifests"]=json!(index_reference.iter().map(|r|json!({"recording":r["recording_manifest_sha256"],"oracle":r["oracle_manifest_sha256"],"source":r["source"]})).collect::<Vec<_>>());
    evidence["self_check"] = checks;
    Ok((policy, evidence))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn reference() -> Value {
        let layers:Vec<_>=(0..LAYERS).map(|l|json!({"layer":l,"mismatch":0.0,"max_row_substitutions":1,"score_error":{"rows":20,"max":0.01}})).collect();
        json!({"profile":"cpu-bf16","backend":"cpu","control":null,"stage_bundle_sha256":"a".repeat(64),"cases":[{
            "case":"c","router":{"layers":layers,"changes":[]},"modes":[{"mode":"decode","positions":[{"label":"last","metrics":{
                "total_variation":0.01,"jensen_shannon":0.001,"reference_logprob_error":0.01,"centered_logit_error":0.01,
                "oracle_top1":0,"candidate_top1":0,"oracle_top1_gap":1.0,"top1_regret":0.0}}]}]}]})
    }
    fn controls() -> Vec<Value> {
        [
            "wrong-weights",
            "wrong-routing",
            "wrong-rope",
            "stale-cache",
        ]
        .into_iter()
        .map(|name| {
            let mut r = reference();
            r["control"] = json!(name);
            r["cases"][0]["modes"][0]["positions"][0]["metrics"]["total_variation"] = json!(0.9);
            r
        })
        .collect()
    }
    fn index() -> Value {
        json!({"source":{"case":"c","profile":"cpu-bf16","stage_bundle_sha256":"a".repeat(64)},
            "recording_manifest_sha256":"b".repeat(64),"oracle_manifest_sha256":"c".repeat(64),
            "index":{"complete":true,"local_changed":0,"score_error_max":0.01,
                "kinds":{"indices":{"rows":20,"changed":1,"max_row_substitutions":1},"candidate_blocks":{"rows":20,"changed":0,"max_row_substitutions":0}},
                "changes":[{"valid":true,"substitutions":1,"boundary_gap":0.01,"regret":0.01}]}})
    }
    #[test]
    fn derives_only_from_complete_independent_cpu_and_checks_its_reference() {
        let (p, e) = derive(&[reference()], &controls(), &[index()]).unwrap();
        assert_eq!(p.version, "2.4");
        assert_eq!(p.index.epsilon, Some(0.02));
        assert_eq!(p.index.max_substituted_fraction, Some(0.1));
        assert!(p.index_out_of_scope.is_none());
        assert_eq!(e["self_check"]["reference_passes"], true);
        let mut gpu = index();
        gpu["source"]["profile"] = json!("cuda-device-sm_120");
        assert!(derive(&[reference()], &controls(), &[gpu]).is_err());
        let mut bad = index();
        bad["index"]["local_changed"] = json!(1);
        assert!(derive(&[reference()], &controls(), &[bad]).is_err());
        let mut inconsistent = index();
        inconsistent["index"]["changes"][0]["regret"] = json!(0.9);
        assert!(derive(&[reference()], &controls(), &[inconsistent]).is_err());
        assert!(derive(&[reference()], &[], &[index()]).is_err());
        assert!(derive(&[reference()], &controls(), &[]).is_err());
    }
}
