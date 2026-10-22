//! Produce a Pylon numerical claim only from successful, matching gate reports.
use super::*;

fn hash(v: &Value) -> bool {
    v.as_str()
        .is_some_and(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
}
fn same_source(source: &Value, g1: &Value) -> bool {
    [
        "profile",
        "stage_bundle_sha256",
        "execution_config_sha256",
        "manifest",
    ]
    .iter()
    .all(|k| source[*k] == g1[*k])
        && source["launch"] == "pinned"
        && source["expected_bundle_sha256"] == g1["stage_bundle_sha256"]
}

fn check(
    policy: &Policy,
    document: &Value,
    g1: &Value,
    g2: &Value,
    answers: &Value,
    g4: &Value,
    suite: &Value,
    lengths: &BTreeMap<String, (u64, u64)>,
) -> Result<Value, Refused> {
    let d = policy.distribution;
    let valid = |v: Option<f64>| v.is_some_and(|n| n.is_finite() && n >= 0.0);
    if ![
        d.total_variation,
        d.jensen_shannon,
        d.reference_logprob_error,
        d.case_mean_total_variation,
        d.tail_total_variation,
        d.tail_fraction,
        policy.top1.epsilon,
        policy.top1.max_substituted_fraction,
        policy.index.epsilon,
        policy.index.max_substituted_fraction,
    ]
    .into_iter()
    .all(valid)
        || policy
            .router_score_budget
            .as_ref()
            .is_none_or(|b| b.len() != 40 || b.iter().any(|n| !n.is_finite() || *n < 0.0))
        || policy.router_layer_mismatch.as_ref().is_none_or(|b| {
            b.len() != 40 || b.iter().any(|n| !n.is_finite() || !(0.0..=1.0).contains(n))
        })
    {
        return Err(refused(
            "numerical publication needs every distribution, router, and index rule frozen",
        ));
    }
    let scope = &document["release_scope"];
    if scope["schema"] != "psionic.deepseek_v41.release_scope.v1"
        || !hash(&scope["stage_bundle_sha256"])
        || !hash(&scope["execution_config_sha256"])
        || !hash(&scope["corpus_sha256"])
    {
        return Err(refused(
            "numerical publication needs a complete release scope",
        ));
    }
    let names: BTreeSet<_> = scope["cases"]
        .as_array()
        .ok_or_else(|| refused("missing release corpus"))?
        .iter()
        .filter_map(Value::as_str)
        .collect();
    if names.is_empty()
        || g1["schema"] != "psionic.deepseek_v41.g1_v2_report.v1"
        || g1["release"] != true
        || g1["release_passed"] != true
        || g1["passed"] != true
        || g1["observe"] != true
        || g1["launch"] != "pinned"
        || !g1["worker_stop_error"].is_null()
        || !g1["control"].is_null()
        || g1["release_refusals"]
            .as_array()
            .is_none_or(|r| !r.is_empty())
        || !judge_report(policy, g1).is_empty()
        || policy.index_out_of_scope.is_some()
        || policy.index.epsilon.is_none()
        || g1["weights"] != scope["weights"]
        || g1["manifest"] != scope["manifest"]
        || g1["profile"] != scope["numerical_profile"]
        || g1["stage_bundle_sha256"] != scope["stage_bundle_sha256"]
        || g1["expected_bundle_sha256"] != scope["stage_bundle_sha256"]
        || g1["execution_config_sha256"].as_array().is_none_or(|cs| {
            cs.len() != 4 || cs.iter().any(|c| c != &scope["execution_config_sha256"])
        })
        || g1["corpus_sha256"] != scope["corpus_sha256"]
    {
        return Err(refused(
            "G1 is incomplete, failed, or differs from the frozen release scope",
        ));
    }
    let cases = g1["cases"]
        .as_array()
        .ok_or_else(|| refused("missing G1 cases"))?;
    if cases.len() != names.len()
        || cases
            .iter()
            .filter_map(|c| c["case"].as_str())
            .collect::<BTreeSet<_>>()
            != names
        || cases.iter().any(|c| {
            let frozen = lengths.get(c["case"].as_str().unwrap_or("?"));
            frozen.is_none_or(|(prompt, generated)| {
                c["prompt_tokens"].as_u64() != Some(*prompt)
                    || c["generated_tokens"].as_u64() != Some(*generated)
                    || !scope::complete_router(&c["router"], *prompt, *generated)
            }) || c["replay_equals_forced_decode"] != true
                || c["exact"]["passed"] != true
                || c["exact"]["checked"].as_u64().unwrap_or(0) == 0
                || c["index"]["complete"] != true
                || !index::violations(policy.index, &c["index"]).is_empty()
                || c["router"]["layers"].as_array().is_none_or(|ls| {
                    ls.len() != 40
                        || ls
                            .iter()
                            .filter_map(|l| l["layer"].as_u64())
                            .collect::<BTreeSet<_>>()
                            != (0..40).collect::<BTreeSet<_>>()
                        || ls.iter().any(|l| {
                            l["score_error"]["from"] != "stage"
                                || l["score_error"]["rows"].as_u64().unwrap_or(0) == 0
                        })
                })
                || c["router"]["layers_compared"] != 40
                || c["router"]["layers_with_stage_scores"] != 40
                || c["modes"].as_array().is_none_or(|ms| {
                    ms.len() != 2
                        || ms
                            .iter()
                            .filter_map(|m| m["mode"].as_str())
                            .collect::<BTreeSet<_>>()
                            != BTreeSet::from(["decode", "replay"])
                        || ms.iter().any(|m| {
                            frozen.is_none_or(|(_, generated)| {
                                !scope::complete_positions(m, *generated)
                            }) || m["missing"].as_array().is_none_or(|v| !v.is_empty())
                        })
                })
        })
    {
        return Err(refused(
            "G1 lacks complete numerical, exact, or same-mode evidence",
        ));
    }
    if g4["schema"] != "psionic.deepseek_v41.isolated_qualification.v1"
        || g4["passed"] != true
        || g4["violations"].as_array().is_none_or(|v| !v.is_empty())
        || g4["release_scope"] != *scope
        || g4["oracle_manifest_sha256"] != g1["oracle_manifest_sha256"]
        || g4["policy_sha256"] != g1["policy_sha256"]
        || g4["reports"].as_array().is_none_or(|rs| {
            rs.len() != 4
                || rs
                    .iter()
                    .filter_map(|r| r["stage"].as_u64())
                    .collect::<BTreeSet<_>>()
                    != BTreeSet::from([0, 1, 2, 3])
        })
    {
        return Err(refused(
            "G4 is incomplete, failed, or names a different policy or oracle",
        ));
    }
    let isolated_reports = g4["reports"].as_array().unwrap();
    if !isolated_policy::confirmation_violations(document, policy, isolated_reports, lengths)
        .is_empty()
    {
        return Err(refused(
            "G4 measurements do not pass the frozen isolated policy",
        ));
    }
    let tasks = suite["cases"]
        .as_array()
        .filter(|c| !c.is_empty())
        .ok_or_else(|| refused("empty G2 suite"))?;
    let rows = g2["rows"]
        .as_array()
        .ok_or_else(|| refused("missing G2 rows"))?;
    let raw = answers["answers"]
        .as_array()
        .ok_or_else(|| refused("missing G2 answers"))?;
    let task_ids: BTreeSet<_> = tasks.iter().filter_map(|c| c["id"].as_str()).collect();
    if task_ids.len() != tasks.len()
        || rows.len() != tasks.len()
        || raw.len() != tasks.len()
        || rows
            .iter()
            .filter_map(|r| r["case"].as_str())
            .collect::<BTreeSet<_>>()
            != task_ids
        || raw
            .iter()
            .filter_map(|r| r["case"].as_str())
            .collect::<BTreeSet<_>>()
            != task_ids
        || g2["schema"] != "psionic.deepseek_v41.g2_v2_report.v1"
        || answers["schema"] != "psionic.deepseek_v41.g2_v2_answers.v1"
        || !answers["worker_stop_error"].is_null()
        || g2["passed"] != true
        || g2["floor_held"] != true
        || g2["missing"].as_array().is_none_or(|v| !v.is_empty())
        || g2["tasks"].as_u64() != Some(tasks.len() as u64)
        || !same_source(&g2["source"], g1)
        || !same_source(answers, g1)
        || g2["source"]["cap"] != answers["cap"]
        || answers["cap"].as_u64().is_none_or(|n| n == 0 || n > 4096)
        || rows.iter().any(|r| {
            r["runtime_must_pass_held"] != true
                || r["runtime_checks"].as_array().is_none_or(|cs| {
                    cs.iter()
                        .any(|c| c["must_pass"] == true && c["passed"] != true)
                })
        })
    {
        return Err(refused(
            "G2 is incomplete, failed, or uses another worker configuration",
        ));
    }
    for task in tasks {
        let answer = raw.iter().find(|a| a["case"] == task["id"]).unwrap();
        if answer["ended"] != true
            || answer["finish"] != "stop"
            || task["thinking_mode"] == "thinking"
                && answer["reasoning"]
                    .as_str()
                    .is_none_or(|s| s.trim().is_empty())
        {
            return Err(refused(
                "a G2 answer did not finish or lacks its required reasoning",
            ));
        }
    }
    let runtime = rows.iter().filter(|r| r["runtime_passed"] == true).count();
    let oracle = rows.iter().filter(|r| r["oracle_passed"] == true).count();
    let floor = document["g2_quality"]["minimum_oracle_passes"]
        .as_u64()
        .filter(|n| *n > 0)
        .ok_or_else(|| refused("G2 needs a frozen oracle quality floor"))?;
    if runtime < oracle
        || oracle < (floor as usize)
        || g2["runtime_passes"].as_u64() != Some(runtime as u64)
        || g2["oracle_passes"].as_u64() != Some(oracle as u64)
    {
        return Err(refused(
            "G2 pass counts do not meet the frozen comparison and oracle floor",
        ));
    }
    let limits = g1["stage_limits"]
        .as_array()
        .filter(|v| v.len() == 4 && v.iter().all(|l| l == &v[0]))
        .ok_or_else(|| refused("G1 stage limits differ"))?;
    let context = limits[0]["max_context_tokens"]
        .as_u64()
        .filter(|n| *n > 0)
        .ok_or_else(|| refused("missing G1 context limit"))?;
    let input = limits[0]["max_input_tokens"]
        .as_u64()
        .filter(|n| *n > 0 && *n < context)
        .ok_or_else(|| refused("missing G1 prompt limit"))?;
    for field in [
        "stage_bundle_sha256",
        "oracle_manifest_sha256",
        "policy_sha256",
    ] {
        if !hash(&g1[field]) {
            return Err(refused("missing G1 evidence identity"));
        }
    }
    Ok(
        json!({"schema":"openagents.deepseek_v41.numerical_qualification.v1","artifact":g1["weights"],"manifest":g1["manifest"],"profile":g1["profile"],"bundle":g1["stage_bundle_sha256"],"execution_config_sha256":scope["execution_config_sha256"],
        "max_context_tokens":context,"max_input_tokens":input,"max_output_tokens":answers["cap"]}),
    )
}

pub(super) fn run(args: &[String]) -> Result<(), Refused> {
    let chunks = args.chunks_exact(2);
    if !chunks.remainder().is_empty() {
        return Err(refused("qualification-receipt needs flag/value pairs"));
    }
    let mut flags = BTreeMap::new();
    for p in chunks {
        if ![
            "--policy",
            "--g1",
            "--g2",
            "--g2-answers",
            "--g2-suite",
            "--g2-checker",
            "--g4",
            "--g4-records",
            "--weights",
            "--oracle",
            "--corpus",
            "--out",
        ]
        .contains(&p[0].as_str())
            || flags.insert(p[0].as_str(), p[1].as_str()).is_some()
        {
            return Err(refused("unknown or repeated qualification-receipt flag"));
        }
    }
    let path = |key: &str| {
        flags
            .get(key)
            .copied()
            .map(Path::new)
            .ok_or_else(|| refused(format!("{key} is required")))
    };
    let load = |key: &str| -> Result<Value, Refused> {
        serde_json::from_slice(&std::fs::read(path(key)?).map_err(|e| refused(e.to_string()))?)
            .map_err(|e| refused(e.to_string()))
    };
    let document = load("--policy")?;
    let g1 = load("--g1")?;
    let g2 = load("--g2")?;
    let answers = load("--g2-answers")?;
    let g4 = load("--g4")?;
    let suite = load("--g2-suite")?;
    let policy =
        Policy::parse(&serde_json::to_string(&document).map_err(|e| refused(e.to_string()))?)
            .map_err(|e| refused(e.to_string()))?;
    if g1["policy_sha256"] != checked_sha256(path("--policy")?)?
        || g2["suite_sha256"] != checked_sha256(path("--g2-suite")?)?
        || g2["checker_sha256"] != checked_sha256(path("--g2-checker")?)?
        || g2["answers_sha256"] != checked_sha256(path("--g2-answers")?)?
        || g2["suite_sha256"] != document["acceptance_tools"]["g2_suite_sha256"]
        || g2["checker_sha256"] != document["acceptance_tools"]["g2_checker_sha256"]
    {
        return Err(refused(
            "gate reports do not match frozen policy, suite, checker, or answer bytes",
        ));
    }
    let lengths = scope::confirmation_inputs(&document, path("--corpus")?, path("--oracle")?)?;
    if g1["oracle_manifest_sha256"] != checked_sha256(&path("--oracle")?.join("manifest.json"))? {
        return Err(refused("gate reports name a different oracle capture"));
    }
    let raw = flags
        .get("--g4-records")
        .ok_or_else(|| refused("--g4-records is required"))?;
    let mut rechecked = BTreeMap::new();
    for source in raw.split(',') {
        let r = isolated_check::evaluate(
            path("--weights")?,
            document["release_scope"]["manifest"]
                .as_str()
                .ok_or_else(|| refused("missing frozen manifest"))?,
            path("--oracle")?,
            Path::new(source),
        )?;
        let stage = r["stage"]
            .as_u64()
            .ok_or_else(|| refused("missing raw G4 stage"))?;
        if rechecked.insert(stage, r).is_some() {
            return Err(refused("duplicate raw G4 stage"));
        }
    }
    if rechecked.len() != 4
        || g4["reports"].as_array().is_none_or(|reports| {
            reports.len() != 4
                || reports
                    .iter()
                    .any(|r| r["stage"].as_u64().and_then(|s| rechecked.get(&s)) != Some(r))
        })
    {
        return Err(refused(
            "G4 report differs from rechecked original-input output bytes",
        ));
    }
    let mut receipt = check(
        &policy, &document, &g1, &g2, &answers, &g4, &suite, &lengths,
    )?;
    receipt["evidence"] = json!({"G1":checked_sha256(path("--g1")?)?,"G2":checked_sha256(path("--g2")?)?,"G4":checked_sha256(path("--g4")?)?});
    std::fs::write(
        path("--out")?,
        serde_json::to_vec_pretty(&receipt).map_err(|e| refused(e.to_string()))?,
    )
    .map_err(|e| refused(e.to_string()))?;
    println!(
        "{}",
        json!({"purpose":"numerical-qualification-claim","out":path("--out")?})
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Policy, Value, Value, Value, Value, Value, Value) {
        let mut policy = pending_policy();
        policy.version = "2.4".into();
        policy.distribution = q::DistributionLimits {
            total_variation: Some(0.1),
            jensen_shannon: Some(0.1),
            reference_logprob_error: Some(0.1),
            case_mean_total_variation: Some(0.1),
            tail_total_variation: Some(0.1),
            tail_fraction: Some(0.1),
        };
        policy.top1 = DecisionRule {
            epsilon: Some(0.1),
            max_substitutions: 1,
            max_substituted_fraction: Some(0.1),
        };
        policy.index = policy.top1;
        policy.router_score_budget = Some(vec![0.1; 40]);
        policy.router_layer_mismatch = Some(vec![0.1; 40]);
        policy.index_out_of_scope = None;
        let scope = json!({"schema":"psionic.deepseek_v41.release_scope.v1","cases":["x"],"weights":format!("sha256:{}","a".repeat(64)),"manifest":format!("sha256:{}","b".repeat(64)),
            "numerical_profile":"cuda-fp8-mma-tp8-sm_120","stage_bundle_sha256":"c".repeat(64),"execution_config_sha256":"d".repeat(64),"corpus_sha256":"e".repeat(64)});
        let metrics = json!({"total_variation":0.0,"jensen_shannon":0.0,"reference_logprob_error":0.0,"centered_logit_error":0.0,"oracle_top1":0,"candidate_top1":0,"oracle_top1_gap":1.0,"top1_regret":0.0});
        let modes: Vec<_> = ["decode", "replay"]
            .into_iter()
            .map(
                |m| json!({"mode":m,"positions":scope::comparison_labels(2).into_iter().map(|label|json!({"label":label,"metrics":metrics})).collect::<Vec<_>>(),"missing":[]}),
            )
            .collect();
        let layers:Vec<_>=(0..40).map(|l|json!({"layer":l,"same":3,"substituted":0,"failed":0,"unresolved":0,"mismatch":0.0,"score_error":{"from":"stage","rows":3,"max":0.0}})).collect();
        let index = json!({"complete":true,"local_changed":0,"score_error_max":0.0,"kinds":{"indices":{"rows":1,"changed":0},"candidate_blocks":{"rows":1,"changed":0}},"changes":[]});
        let g1 = json!({"schema":"psionic.deepseek_v41.g1_v2_report.v1","release":true,"release_passed":true,"passed":true,"observe":true,"launch":"pinned","control":null,"release_refusals":[],
            "weights":scope["weights"],"manifest":scope["manifest"],"profile":scope["numerical_profile"],"stage_bundle_sha256":scope["stage_bundle_sha256"],"expected_bundle_sha256":scope["stage_bundle_sha256"],
            "execution_config_sha256":vec![scope["execution_config_sha256"].clone();4],"corpus_sha256":scope["corpus_sha256"],"oracle_manifest_sha256":"f".repeat(64),"policy_sha256":"0".repeat(64),
            "stage_limits":vec![json!({"max_context_tokens":8192,"max_input_tokens":2872});4],
            "cases":[{"case":"x","prompt_tokens":1,"generated_tokens":2,"modes":modes,"replay_equals_forced_decode":true,"exact":{"passed":true,"checked":1},"index":index,"router":{"rows":120,"same":120,"substituted":0,"failed":0,"unresolved":0,"layers_compared":40,"layers_with_stage_scores":40,"layers":layers,"changes":[]}}]});
        let mut limits = serde_json::Map::new();
        let reports:Vec<_>=(0..4).map(|stage|{
            let steps = isolated_policy::test_steps(stage, 1, 2);
            for s in &steps {for o in s["output"].as_array().unwrap() {
                limits.insert(format!("{stage}/{}/{}", s["mode"].as_str().unwrap(), serde_json::to_string(&o["kind"]).unwrap()),json!({"max_abs":0.2,"relative_l2":0.1,"changed_fraction":1.0,"max_substitutions":1}));
            }}
            json!({"schema":"psionic.deepseek_v41.isolated_recheck.v1","stage":stage,"weights":scope["weights"],"manifest":scope["manifest"],"profile":scope["numerical_profile"],"build":{"bundle_sha256":scope["stage_bundle_sha256"]},
                "execution_config_sha256":scope["execution_config_sha256"],"corpus_sha256":scope["corpus_sha256"],"oracle_manifest_sha256":g1["oracle_manifest_sha256"],"control":null,
                "cases":[{"case":"x","prompt_tokens":1,"generated_tokens":2,"steps":steps}]})
        }).collect();
        let g4 = json!({"schema":"psionic.deepseek_v41.isolated_qualification.v1","passed":true,"violations":[],"release_scope":scope,"oracle_manifest_sha256":g1["oracle_manifest_sha256"],"policy_sha256":g1["policy_sha256"],"reports":reports});
        let mut source = g1.clone();
        source["cap"] = json!(4096);
        let g2 = json!({"schema":"psionic.deepseek_v41.g2_v2_report.v1","passed":true,"floor_held":true,"missing":[],"tasks":1,"source":source,"oracle_passes":1,"runtime_passes":1,
            "rows":[{"case":"t","runtime_must_pass_held":true,"runtime_checks":[{"must_pass":true,"passed":true}],"runtime_passed":true,"oracle_passed":true}]});
        let mut answers = source;
        answers["schema"] = json!("psionic.deepseek_v41.g2_v2_answers.v1");
        answers["answers"] = json!([{"case":"t","ended":true,"finish":"stop","reasoning":"Reason.","content":"Answer."}]);
        let suite = json!({"cases":[{"id":"t","thinking_mode":"thinking"}]});
        let document = json!({"release_scope":scope,"isolated_stages":{"schema":"psionic.deepseek_v41.isolated_policy.v1","limits":limits},"g2_quality":{"minimum_oracle_passes":1}});
        (policy, document, g1, g2, answers, g4, suite)
    }
    fn lengths() -> BTreeMap<String, (u64, u64)> {
        BTreeMap::from([("x".into(), (1, 2))])
    }
    #[test]
    fn publication_requires_successful_matching_and_rejudged_evidence() {
        let (p, doc, g1, g2, answers, g4, suite) = fixture();
        check(&p, &doc, &g1, &g2, &answers, &g4, &suite, &lengths()).unwrap();
        let mut bad = g1.clone();
        bad["release_passed"] = json!(false);
        assert!(check(&p, &doc, &bad, &g2, &answers, &g4, &suite, &lengths()).is_err());
        let mut bad = g1.clone();
        bad["cases"][0]["index"]["local_changed"] = json!(1);
        assert!(check(&p, &doc, &bad, &g2, &answers, &g4, &suite, &lengths()).is_err());
        let mut bad = g2.clone();
        bad["source"]["execution_config_sha256"] = json!([]);
        assert!(check(&p, &doc, &g1, &bad, &answers, &g4, &suite, &lengths()).is_err());
        let mut bad = g2.clone();
        bad["rows"][0]["runtime_checks"][0]["passed"] = json!(false);
        assert!(check(&p, &doc, &g1, &bad, &answers, &g4, &suite, &lengths()).is_err());
        let mut bad = g4.clone();
        bad["reports"][2]["cases"][0]["steps"][0]["output"][0]["metrics"]["max_abs"] = json!(1.0);
        assert!(check(&p, &doc, &g1, &g2, &answers, &bad, &suite, &lengths()).is_err());
        let mut bad = answers;
        bad["answers"][0]["reasoning"] = json!("");
        assert!(check(&p, &doc, &g1, &g2, &bad, &g4, &suite, &lengths()).is_err());
    }
    #[test]
    fn missing_oracle_hash_wrong_weights_and_cropped_continuations_cannot_publish() {
        let (p, doc, g1, g2, answers, g4, suite) = fixture();
        for field in ["weights", "oracle_manifest_sha256"] {
            let mut bad = g4.clone();
            for stage in 1..4 {
                bad["reports"][stage].as_object_mut().unwrap().remove(field);
            }
            assert!(
                check(&p, &doc, &g1, &g2, &answers, &bad, &suite, &lengths()).is_err(),
                "{field}"
            );
            let mut bad = g4.clone();
            bad["reports"][2][field] = json!("9".repeat(64));
            assert!(
                check(&p, &doc, &g1, &g2, &answers, &bad, &suite, &lengths()).is_err(),
                "{field}"
            );
        }
        let mut bad = g4;
        for r in bad["reports"].as_array_mut().unwrap() {
            r["cases"][0]["generated_tokens"] = json!(0);
            let stage = r["stage"].as_u64().unwrap();
            r["cases"][0]["steps"] = json!(isolated_policy::test_steps(stage, 1, 0));
        }
        assert!(check(&p, &doc, &g1, &g2, &answers, &bad, &suite, &lengths()).is_err());
    }
    #[test]
    fn every_frozen_g1_position_and_both_lengths_are_required() {
        let (p, doc, g1, g2, answers, g4, suite) = fixture();
        check(&p, &doc, &g1, &g2, &answers, &g4, &suite, &lengths()).unwrap();
        for field in ["prompt_tokens", "generated_tokens"] {
            let mut bad = g1.clone();
            bad["cases"][0].as_object_mut().unwrap().remove(field);
            assert!(check(&p, &doc, &bad, &g2, &answers, &g4, &suite, &lengths()).is_err());
            for value in [0, 999] {
                bad["cases"][0][field] = json!(value);
                assert!(check(&p, &doc, &bad, &g2, &answers, &g4, &suite, &lengths()).is_err());
            }
        }
        for mode in 0..2 {
            for labels in [
                vec!["last"],
                vec!["last", "g1"],
                vec!["last", "g1", "g1"],
                vec!["last", "g1", "g3"],
            ] {
                let mut bad = g1.clone();
                let metrics = bad["cases"][0]["modes"][mode]["positions"][0]["metrics"].clone();
                bad["cases"][0]["modes"][mode]["positions"] = json!(
                    labels
                        .into_iter()
                        .map(|label| json!({"label":label,"metrics":metrics}))
                        .collect::<Vec<_>>()
                );
                assert!(check(&p, &doc, &bad, &g2, &answers, &g4, &suite, &lengths()).is_err());
            }
        }
        let mut bad = g1.clone();
        bad["cases"][0]["modes"][1]["positions"][1]["metrics"] = json!({});
        assert!(check(&p, &doc, &bad, &g2, &answers, &g4, &suite, &lengths()).is_err());
        let mut bad = g1;
        let router = &mut bad["cases"][0]["router"];
        router["rows"] = json!(40);
        router["same"] = json!(40);
        for l in router["layers"].as_array_mut().unwrap() {
            l["same"] = json!(1);
            l["score_error"]["rows"] = json!(1);
        }
        assert!(check(&p, &doc, &bad, &g2, &answers, &g4, &suite, &lengths()).is_err());
    }
}
