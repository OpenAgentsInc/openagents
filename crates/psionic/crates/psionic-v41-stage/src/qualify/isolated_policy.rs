//! Independent original-input stage envelopes and prospective confirmation.
use super::*;
use psionic_models::{
    deepseek_v41_stage::{V41StageGeometry, stage_contract},
    stage_state::{RowRule, SectionKind, WireDtype},
};

fn hash(v: &Value) -> bool {
    v.as_str()
        .is_some_and(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
}

fn layout(stage: u64, position: u64, tokens: u64) -> Vec<Value> {
    let mut g = V41StageGeometry::flash();
    g.residual_dtype = WireDtype::F32;
    if stage == 3 {
        return vec![
            json!({"kind":SectionKind::Logits,"dtype":WireDtype::F32,"shape":[1,g.vocabulary],"first_row":0}),
        ];
    }
    let next = stage as u16 + 1;
    stage_contract(&g, next, 10 * u32::from(next)..10 * (u32::from(next) + 1))
        .sections
        .into_iter()
        .map(|s| {
            let (rows, first) = match s.rows {
                RowRule::PerToken => (tokens, 0),
                RowRule::Fixed(n) => (u64::from(n), 0),
                RowRule::Append { ratio } => {
                    let r = u64::from(ratio);
                    ((position + tokens) / r - position / r, position / r)
                }
            };
            let mut shape = vec![rows as u32];
            shape.extend(s.inner);
            json!({"kind":s.kind,"dtype":s.dtype,"shape":shape,"first_row":first})
        })
        .collect()
}

fn key(stage: u64, mode: &str, kind: &Value) -> Result<String, Refused> {
    Ok(format!(
        "{stage}/{mode}/{}",
        serde_json::to_string(kind).map_err(|e| refused(e.to_string()))?
    ))
}
fn number(v: &Value) -> Result<f64, Refused> {
    v.as_f64()
        .filter(|x| x.is_finite() && *x >= 0.0)
        .ok_or_else(|| refused("missing, negative, or nonfinite isolated metric"))
}

fn complete(r: &Value) -> bool {
    let Some(stage) = r["stage"].as_u64().filter(|s| *s < 4) else {
        return false;
    };
    r["cases"].as_array().is_some_and(|cases| {
        !cases.is_empty()
            && cases.iter().all(|c| {
                let Some(generated) = c["generated_tokens"].as_u64().filter(|n| *n <= 32768) else {
                    return false;
                };
                let expected: BTreeSet<_> = (0..=generated)
                    .map(|s| ("decode", s))
                    .chain(std::iter::once(("replay", 0)))
                    .collect();
                let Some(prompt) = c["prompt_tokens"].as_u64().filter(|n| *n > 0 && *n <= 8192)
                else {
                    return false;
                };
                prompt + generated <= 8192
                    && c["steps"].as_array().is_some_and(|steps| {
                        steps.len() == expected.len()
                            && steps
                                .iter()
                                .filter_map(|s| Some((s["mode"].as_str()?, s["step"].as_u64()?)))
                                .collect::<BTreeSet<_>>()
                                == expected
                            && steps.iter().all(|s| {
                                let at = s["step"].as_u64().unwrap_or(u64::MAX);
                                let (position, tokens) = if s["mode"] == "replay" {
                                    (0, prompt + generated)
                                } else if at == 0 {
                                    (0, prompt)
                                } else {
                                    (prompt + at - 1, 1)
                                };
                                let expected = layout(stage, position, tokens);
                                hash(&s["input_message_sha256"])
                                    && s["position"].as_u64() == Some(position)
                                    && s["output"].as_array().is_some_and(|out| {
                                        out.len() == expected.len()
                                            && expected.iter().all(|e| {
                                                out.iter()
                                                    .filter(|o| {
                                                        ["kind", "dtype", "shape", "first_row"]
                                                            .iter()
                                                            .all(|k| o[*k] == e[*k])
                                                    })
                                                    .count()
                                                    == 1
                                            })
                                            && out.iter().all(|o| hash(&o["sha256"]))
                                    })
                            })
                    })
            })
    })
}

pub(super) fn derive(reference: &[Value], index: DecisionRule) -> Result<Value, Refused> {
    let mut limits = BTreeMap::<String, Value>::new();
    let mut stages = BTreeSet::new();
    let mut coverage = BTreeMap::<u64, (BTreeSet<String>, u64)>::new();
    for r in reference {
        if !complete(r)
            || r["schema"] != "psionic.deepseek_v41.isolated_recheck.v1"
            || r["profile"] != "cpu-bf16"
            || r["backend"] != "cpu"
            || !r["control"].is_null()
            || r["source_report_sha256"]
                .as_str()
                .is_none_or(|s| s.len() != 64)
        {
            return Err(refused(
                "isolated calibration takes rechecked, uncontrolled plain BF16 CPU records",
            ));
        }
        let stage = r["stage"]
            .as_u64()
            .filter(|s| *s < 4)
            .ok_or_else(|| refused("invalid calibration stage"))?;
        stages.insert(stage);
        for c in r["cases"].as_array().into_iter().flatten() {
            let cover = coverage.entry(stage).or_default();
            cover.0.insert(c["case"].as_str().unwrap_or("?").to_owned());
            cover.1 = cover.1.max(c["prompt_tokens"].as_u64().unwrap_or(0));
            for s in c["steps"].as_array().into_iter().flatten() {
                let mode = s["mode"]
                    .as_str()
                    .filter(|m| matches!(*m, "decode" | "replay"))
                    .ok_or_else(|| refused("missing calibration mode"))?;
                for output in s["output"].as_array().into_iter().flatten() {
                    let k = key(stage, mode, &output["kind"])?;
                    let m = &output["metrics"];
                    if m.get("max_abs").is_some() {
                        if m["values"].as_u64() == Some(0) {
                            continue;
                        }
                        let a = HEADROOM * number(&m["max_abs"])?;
                        let l = HEADROOM * number(&m["relative_l2"])?;
                        let slot = limits
                            .entry(k)
                            .or_insert(json!({"max_abs":0.0,"relative_l2":0.0}));
                        slot["max_abs"] = json!(number(&slot["max_abs"])?.max(a));
                        slot["relative_l2"] = json!(number(&slot["relative_l2"])?.max(l));
                    } else if m.get("changed_fraction").is_some() {
                        let f = (HEADROOM * number(&m["changed_fraction"])?).clamp(1.0 / 33.0, 1.0);
                        let n = m["max_row_substitutions"]
                            .as_u64()
                            .ok_or_else(|| refused("missing isolated index substitutions"))?
                            .max(1);
                        let slot = limits
                            .entry(k)
                            .or_insert(json!({"changed_fraction":0.0,"max_substitutions":0}));
                        slot["changed_fraction"] = json!(number(&slot["changed_fraction"])?.max(f));
                        slot["max_substitutions"] =
                            json!(slot["max_substitutions"].as_u64().unwrap_or(0).max(n));
                    } else if m["passed"] != true {
                        return Err(refused("an exact stage boundary reference failed"));
                    }
                }
            }
        }
    }
    if stages != BTreeSet::from([0, 1, 2, 3])
        || limits.is_empty()
        || coverage
            .values()
            .any(|(cases, longest)| cases.len() < 2 || *longest < 2700)
    {
        return Err(refused(
            "isolated calibration needs all four stages with short and near-capacity original-input cases",
        ));
    }
    let value = json!({"schema":"psionic.deepseek_v41.isolated_policy.v1","headroom":HEADROOM,
        "rationale":"Independent plain BF16 CPU outputs on original oracle stage inputs; per stage, mode, and section. Both maximum absolute error and relative L2 must pass. Index decisions additionally obey the frozen CPU-derived margin and regret budget. Candidates and token history are exact at the 8192-context launch scope.","limits":limits});
    for r in reference {
        let failures = violations(&value, index, r);
        if !failures.is_empty() {
            return Err(refused(format!(
                "independent isolated reference fails its derived rule: {}",
                failures.join("; ")
            )));
        }
    }
    Ok(value)
}

fn violations(policy: &Value, index: DecisionRule, report: &Value) -> Vec<String> {
    let mut out = Vec::new();
    let Some(stage) = report["stage"].as_u64().filter(|s| *s < 4) else {
        return vec!["missing stage".into()];
    };
    for c in report["cases"].as_array().into_iter().flatten() {
        for s in c["steps"].as_array().into_iter().flatten() {
            let mode = s["mode"].as_str().unwrap_or("?");
            for output in s["output"].as_array().into_iter().flatten() {
                let k = match key(stage, mode, &output["kind"]) {
                    Ok(k) => k,
                    Err(e) => {
                        out.push(e.to_string());
                        continue;
                    }
                };
                let m = &output["metrics"];
                let limit = &policy["limits"][&k];
                let label = format!("{} {mode} step {} {k}", c["case"], s["step"]);
                if m.get("max_abs").is_some() {
                    if m["values"].as_u64() == Some(0) {
                        continue;
                    }
                    for metric in ["max_abs", "relative_l2"] {
                        match (number(&m[metric]), number(&limit[metric])) {
                            (Ok(v), Ok(bound)) if v <= bound => {}
                            _ => out.push(format!(
                                "{label}: {metric} exceeds its frozen limit or is missing"
                            )),
                        }
                    }
                } else if m.get("changed_fraction").is_some() {
                    let fraction = number(&m["changed_fraction"]);
                    let bound = number(&limit["changed_fraction"]);
                    if !matches!((fraction,bound),(Ok(f),Ok(b)) if f<=b) {
                        out.push(format!(
                            "{label}: changed-decision fraction exceeds its limit"
                        ));
                    }
                    let cap = limit["max_substitutions"].as_u64().unwrap_or(0);
                    let epsilon = index.epsilon.unwrap_or(f64::NAN);
                    let changes = m["changes"].as_array();
                    if m["changed_rows"].as_u64() != changes.map(|c| c.len() as u64)
                        || !epsilon.is_finite()
                    {
                        out.push(format!("{label}: missing index decision evidence"));
                    }
                    for change in changes.into_iter().flatten() {
                        let n = change["substitutions"].as_u64().unwrap_or(u64::MAX);
                        if change["valid"] != true
                            || n == 0
                            || n > cap
                            || !number(&change["boundary_gap"]).is_ok_and(|g| g <= 2.0 * epsilon)
                            || change["regret"]
                                .as_f64()
                                .is_none_or(|r| !r.is_finite() || r > 2.0 * epsilon * n as f64)
                        {
                            out.push(format!(
                                "{label}: index margin, regret, or substitution cap failed"
                            ));
                        }
                    }
                } else if m["passed"] != true {
                    out.push(format!("{label}: exact boundary check failed"));
                }
            }
        }
    }
    out
}

pub(super) fn confirmation_violations(
    document: &Value,
    policy: &Policy,
    reports: &[Value],
    lengths: &BTreeMap<String, (u64, u64)>,
) -> Vec<String> {
    let scope = &document["release_scope"];
    let isolated = &document["isolated_stages"];
    if isolated["schema"] != "psionic.deepseek_v41.isolated_policy.v1" || scope.is_null() {
        return vec!["missing frozen isolated policy or release scope".into()];
    }
    let Some(frozen_cases) = scope["cases"].as_array() else {
        return vec!["missing frozen cases".into()];
    };
    let cases: BTreeSet<_> = frozen_cases.iter().filter_map(Value::as_str).collect();
    let mut stages = BTreeSet::new();
    let mut failures = Vec::new();
    for r in reports {
        let stage = r["stage"].as_u64().unwrap_or(u64::MAX);
        if !complete(r)
            || !stages.insert(stage)
            || r["schema"] != "psionic.deepseek_v41.isolated_recheck.v1"
            || r["manifest"] != scope["manifest"]
            || r["weights"] != scope["weights"]
            || r["profile"] != scope["numerical_profile"]
            || r["build"]["bundle_sha256"] != scope["stage_bundle_sha256"]
            || r["execution_config_sha256"] != scope["execution_config_sha256"]
            || r["corpus_sha256"] != scope["corpus_sha256"]
            || !hash(&r["oracle_manifest_sha256"])
            || !r["control"].is_null()
            || r["cases"].as_array().is_none_or(|cs| {
                cs.len() != cases.len()
                    || cs
                        .iter()
                        .filter_map(|c| c["case"].as_str())
                        .collect::<BTreeSet<_>>()
                        != cases
                    || cs.iter().any(|c| {
                        lengths.get(c["case"].as_str().unwrap_or("?")).is_none_or(
                            |(prompt, generated)| {
                                c["prompt_tokens"].as_u64() != Some(*prompt)
                                    || c["generated_tokens"].as_u64() != Some(*generated)
                            },
                        )
                    })
            })
        {
            failures.push(format!("stage {stage}: confirmation identity, corpus, or coverage differs from the frozen scope"));
        }
        failures.extend(violations(isolated, policy.index, r));
    }
    if stages != BTreeSet::from([0, 1, 2, 3]) {
        failures.push("confirmation needs exactly all four stages".into());
    }
    let manifests: BTreeSet<_> = reports
        .iter()
        .map(|r| r["oracle_manifest_sha256"].as_str())
        .collect();
    if manifests.len() != 1 {
        failures.push("isolated stages do not share one oracle capture".into());
    }
    failures
}

pub(super) fn run(args: &[String]) -> Result<(), Refused> {
    let mut flags = BTreeMap::new();
    let chunks = args.chunks_exact(2);
    if !chunks.remainder().is_empty() {
        return Err(refused("isolated-policy needs flag/value pairs"));
    }
    for p in chunks {
        if ![
            "--policy",
            "--reference",
            "--candidate",
            "--weights",
            "--oracle",
            "--manifest",
            "--corpus",
            "--out",
        ]
        .contains(&p[0].as_str())
            || flags.insert(p[0].as_str(), p[1].as_str()).is_some()
        {
            return Err(refused("unknown or repeated isolated-policy flag"));
        }
    }
    let value = |key: &str| {
        flags
            .get(key)
            .copied()
            .ok_or_else(|| refused(format!("{key} is required")))
    };
    let path = Path::new(value("--policy")?);
    let bytes = std::fs::read(path).map_err(|e| refused(e.to_string()))?;
    let mut document: Value = serde_json::from_slice(&bytes).map_err(|e| refused(e.to_string()))?;
    let policy = Policy::parse(std::str::from_utf8(&bytes).map_err(|e| refused(e.to_string()))?)
        .map_err(|e| refused(e.to_string()))?;
    let load = |paths: &str, manifest: &str| -> Result<Vec<Value>, Refused> {
        paths
            .split(',')
            .map(|p| {
                isolated_check::evaluate(
                    Path::new(value("--weights")?),
                    manifest,
                    Path::new(value("--oracle")?),
                    Path::new(p),
                )
            })
            .collect()
    };
    let result = if let Some(paths) = flags.get("--reference") {
        if flags.contains_key("--candidate") {
            return Err(refused("derive and confirmation are separate commands"));
        }
        if document.get("release_scope").is_some() || document.get("isolated_stages").is_some() {
            return Err(refused("cannot change a frozen scope or isolated policy"));
        }
        let references = load(paths, value("--manifest")?)?;
        let mut isolated = derive(&references, policy.index)?;
        isolated["reference_report_sha256"] = json!(
            references
                .iter()
                .map(|r| &r["source_report_sha256"])
                .collect::<Vec<_>>()
        );
        document["isolated_stages"] = isolated;
        document
    } else {
        let scope = &document["release_scope"];
        let isolated = &document["isolated_stages"];
        if isolated["schema"] != "psionic.deepseek_v41.isolated_policy.v1" || scope.is_null() {
            return Err(refused(
                "confirmation needs a frozen isolated policy and release scope",
            ));
        }
        let lengths = scope::confirmation_inputs(
            &document,
            Path::new(value("--corpus")?),
            Path::new(value("--oracle")?),
        )?;
        let reports = load(
            value("--candidate")?,
            scope["manifest"]
                .as_str()
                .ok_or_else(|| refused("missing frozen manifest"))?,
        )?;
        let failures = confirmation_violations(&document, &policy, &reports, &lengths);
        let manifests: BTreeSet<_> = reports
            .iter()
            .filter_map(|r| r["oracle_manifest_sha256"].as_str())
            .collect();
        json!({"schema":"psionic.deepseek_v41.isolated_qualification.v1","passed":failures.is_empty(),"violations":failures,
            "policy_sha256":hex::encode(Sha256::digest(&bytes)),"release_scope":scope,"oracle_manifest_sha256":manifests.iter().next(),"reports":reports})
    };
    std::fs::write(
        value("--out")?,
        serde_json::to_vec_pretty(&result).map_err(|e| refused(e.to_string()))?,
    )
    .map_err(|e| refused(e.to_string()))?;
    if result["passed"] == false {
        return Err(refused("isolated confirmation failed"));
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn test_steps(stage: u64, prompt: u64, generated: u64) -> Vec<Value> {
    (0..=generated).map(|s| ("decode", s)).chain(std::iter::once(("replay", 0))).map(|(mode, at)| {
        let (position, tokens) = if mode == "replay" {(0, prompt + generated)} else if at == 0 {(0, prompt)} else {(prompt + at - 1, 1)};
        let output: Vec<_> = layout(stage, position, tokens).into_iter().map(|mut o| {
            o["sha256"] = json!("b".repeat(64));
            o["metrics"] = if matches!(o["dtype"].as_str(), Some("F32" | "BF16")) {
                let n:u64 = o["shape"].as_array().unwrap().iter().map(|n| n.as_u64().unwrap()).product();
                json!({"values":n,"max_abs":0.1,"relative_l2":0.01})
            } else if o["kind"].get("IndexSelection").is_some() {
                json!({"rows":tokens,"changed_rows":1,"changed_fraction":1.0/tokens as f64,"max_row_substitutions":1,
                    "changes":[{"valid":true,"substitutions":1,"boundary_gap":0.01,"regret":0.01}]})
            } else { json!({"passed":true}) };
            o
        }).collect();
        json!({"mode":mode,"step":at,"position":position,"input_message_sha256":"a".repeat(64),"output":output})
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rule() -> DecisionRule {
        DecisionRule {
            epsilon: Some(0.02),
            max_substitutions: 1,
            max_substituted_fraction: Some(0.1),
        }
    }
    fn reference(stage: u64) -> Value {
        let cases: Vec<_> = [38,2726].into_iter().map(|n| json!({"case":format!("c{n}"),"prompt_tokens":n,"generated_tokens":2,"steps":test_steps(stage,n,2)})).collect();
        json!({"schema":"psionic.deepseek_v41.isolated_recheck.v1","stage":stage,"profile":"cpu-bf16","backend":"cpu","control":null,"source_report_sha256":"e".repeat(64),"cases":cases})
    }
    #[test]
    fn original_input_envelopes_do_not_compensate_for_failed_metrics() {
        let refs: Vec<_> = (0..4).map(reference).collect();
        let policy = derive(&refs, rule()).unwrap();
        assert!(violations(&policy, rule(), &refs[0]).is_empty());
        let mut candidate = refs[0].clone();
        candidate["passed"] = json!(true);
        candidate["cases"][0]["steps"][0]["output"][0]["metrics"]["relative_l2"] = json!(0.03);
        assert!(!violations(&policy, rule(), &candidate).is_empty());
        candidate = refs[0].clone();
        candidate["cases"][0]["steps"][0]["output"][0]["metrics"]["max_abs"] = json!(0.3);
        assert!(!violations(&policy, rule(), &candidate).is_empty());
        for (kind, metric, value) in [
            ("IndexSelection", "regret", json!(0.5)),
            ("EngramHistory", "passed", json!(false)),
        ] {
            candidate = refs[0].clone();
            let o = candidate["cases"][0]["steps"][0]["output"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|o| o["kind"] == kind || o["kind"].get(kind).is_some())
                .unwrap();
            if kind == "IndexSelection" {
                o["metrics"]["changes"][0][metric] = value;
            } else {
                o["metrics"][metric] = value;
            }
            assert!(!violations(&policy, rule(), &candidate).is_empty());
        }
        let mut bad = refs.clone();
        bad[0]["profile"] = json!("cuda-device-sm_120");
        assert!(derive(&bad, rule()).is_err());
        assert!(derive(&refs[..3], rule()).is_err());
        let mut incomplete = refs;
        incomplete[0]["cases"][0]["steps"] = json!([]);
        assert!(derive(&incomplete, rule()).is_err());
    }
    #[test]
    fn missing_and_duplicate_boundary_sections_fail_even_with_passing_metrics() {
        let r = reference(0);
        assert!(complete(&r));
        let mut bad = r.clone();
        bad["cases"][0]["steps"][0]["output"]
            .as_array_mut()
            .unwrap()
            .remove(0);
        assert!(!complete(&bad));
        let mut bad = r.clone();
        bad["cases"][0]["steps"][0]["output"][0] = bad["cases"][0]["steps"][0]["output"][1].clone();
        assert!(!complete(&bad));
        let mut bad = r;
        bad["cases"][0]["steps"][0]["output"][0]["shape"] = json!([1]);
        assert!(!complete(&bad));
    }
}
