use super::*;

fn policy() -> Policy {
    Policy {
        schema: POLICY_SCHEMA.into(),
        version: "test".into(),
        distribution: DistributionLimits {
            total_variation: Some(0.05),
            jensen_shannon: Some(0.01),
            reference_logprob_error: Some(0.2),
            case_mean_total_variation: Some(0.02),
            tail_total_variation: Some(0.03),
            tail_fraction: Some(0.25),
        },
        top1: DecisionRule {
            epsilon: Some(0.05),
            max_substitutions: 1,
            max_substituted_fraction: Some(0.5),
        },
        router: DecisionRule {
            epsilon: Some(1e-5),
            max_substitutions: 1,
            max_substituted_fraction: Some(0.01),
        },
        index: DecisionRule {
            epsilon: None,
            max_substitutions: 2,
            max_substituted_fraction: None,
        },
        router_layer_mismatch: None,
        router_score_budget: None,
        index_out_of_scope: None,
    }
}

#[test]
fn identical_logits_have_zero_error_and_an_offset_is_not_a_change() -> Result<(), QualifyError> {
    let a = [1.0f32, 2.0, 3.0, -4.0];
    let d = compare_logits(&a, &a, 2)?;
    assert_eq!(d.total_variation, 0.0);
    assert!(d.jensen_shannon < 1e-15);
    assert_eq!(d.reference_logprob_error, 0.0);
    assert_eq!((d.oracle_top1, d.candidate_top1), (2, 2));
    assert_eq!(d.oracle_top1_gap, 1.0);
    // A uniform offset leaves probabilities and centered logits unchanged.
    let shifted: Vec<f32> = a.iter().map(|x| x + 7.0).collect();
    let d = compare_logits(&a, &shifted, 0)?;
    assert!(d.total_variation < 1e-12 && d.centered_logit_error < 1e-6);
    Ok(())
}

#[test]
fn distribution_metrics_match_a_hand_computed_pair() -> Result<(), QualifyError> {
    // p = (0.5, 0.5), q = (0.75, 0.25) from logits ln-ratios.
    let p = [0.0f32, 0.0];
    let q = [3f32.ln(), 0.0];
    let d = compare_logits(&p, &q, 0)?;
    assert!((d.total_variation - 0.25).abs() < 1e-6);
    let m = [0.625f64, 0.375];
    let kl = |x: [f64; 2]| x[0] * (x[0] / m[0]).ln() + x[1] * (x[1] / m[1]).ln();
    let js = 0.5 * kl([0.5, 0.5]) + 0.5 * kl([0.75, 0.25]);
    assert!((d.jensen_shannon - js).abs() < 1e-6);
    assert!((d.reference_logprob_error - (0.75f64.ln() - 0.5f64.ln())).abs() < 1e-6);
    Ok(())
}

#[test]
fn malformed_logits_fail() {
    assert!(compare_logits(&[1.0, 2.0], &[1.0], 0).is_err());
    assert!(compare_logits(&[1.0, f32::NAN], &[1.0, 2.0], 0).is_err());
    assert!(compare_logits(&[1.0, 2.0], &[1.0, 2.0], 2).is_err());
    assert!(compare_logits(&[], &[], 0).is_err());
}

#[test]
fn decisions_follow_the_margin_rule() {
    let rule = policy().router;
    let scores: BTreeMap<u32, f64> = [(1, 0.9), (2, 0.8), (3, 0.795), (4, 0.1)]
        .into_iter()
        .collect();
    let check = |candidate: &[u32], gap: Option<f64>, scores: Option<&BTreeMap<u32, f64>>| {
        rule.check(&Selection {
            oracle: &[1, 2],
            candidate,
            boundary_gap: gap,
            oracle_scores: scores,
            universe: 384,
        })
    };
    assert_eq!(check(&[2, 1], Some(1.0), None), Decision::Same);
    // A stable boundary must not change.
    assert!(matches!(
        check(&[1, 3], Some(1.0), Some(&scores)),
        Decision::Failed { .. }
    ));
    // Inside the band: one swap with small regret passes ...
    let near = Some(1e-5);
    let tight: BTreeMap<u32, f64> = [(1, 0.9), (2, 0.8), (3, 0.799_995)].into_iter().collect();
    assert!(matches!(
        check(&[1, 3], near, Some(&tight)),
        Decision::Substituted { count: 1, .. }
    ));
    // ... but not a swap with an expert far below the boundary,
    assert!(matches!(
        check(&[1, 4], near, Some(&scores)),
        Decision::Failed { .. }
    ));
    // nor two swaps, nor an unknown gap or score.
    assert!(matches!(
        check(&[3, 4], near, Some(&scores)),
        Decision::Failed { .. }
    ));
    assert!(matches!(
        check(&[1, 3], None, Some(&scores)),
        Decision::Unresolved { .. }
    ));
    assert!(matches!(
        check(&[1, 3], near, None),
        Decision::Unresolved { .. }
    ));
    // Structural errors always fail.
    assert!(matches!(
        check(&[1, 1], near, Some(&scores)),
        Decision::Failed { .. }
    ));
    assert!(matches!(
        check(&[1, 999], near, Some(&scores)),
        Decision::Failed { .. }
    ));
    assert!(matches!(
        check(&[1], near, Some(&scores)),
        Decision::Failed { .. }
    ));
    // The substitution cap holds without a budget too.
    let unbudgeted = DecisionRule {
        epsilon: None,
        ..rule
    };
    assert!(matches!(
        unbudgeted.check(&Selection {
            oracle: &[1, 2],
            candidate: &[3, 4],
            boundary_gap: None,
            oracle_scores: None,
            universe: 384
        }),
        Decision::Failed { .. }
    ));
    // No budget: unresolved, never passed.
    let pending = DecisionRule {
        epsilon: None,
        ..rule
    };
    assert!(matches!(
        pending.check(&Selection {
            oracle: &[1, 2],
            candidate: &[1, 3],
            boundary_gap: near,
            oracle_scores: Some(&tight),
            universe: 384
        }),
        Decision::Unresolved { .. }
    ));
}

#[test]
fn positions_and_cases_fail_pend_and_pass() -> Result<(), QualifyError> {
    let p = policy();
    let o = [0.0f32, 1.0, 5.0, 2.0];
    let good = judge_position(&p, "g1", compare_logits(&o, &[0.0, 1.0, 5.001, 2.0], 2)?);
    assert!(
        good.failures.is_empty() && good.pending.is_empty(),
        "{good:?}"
    );
    // A different, confidently chosen top-1 fails.
    let wrong = judge_position(&p, "g2", compare_logits(&o, &[0.0, 9.0, 5.0, 2.0], 2)?);
    assert!(wrong.failures.iter().any(|f| f.starts_with("top-1")));
    assert!(
        wrong
            .failures
            .iter()
            .any(|f| f.starts_with("total_variation"))
    );
    let case = judge_case(&p, "c", "decode", vec![good.clone()], vec![]);
    assert!(case.passed, "{case:?}");
    let missing = judge_case(&p, "c", "decode", vec![good.clone()], vec!["g32".into()]);
    assert!(!missing.passed);
    let failed = judge_case(&p, "c", "decode", vec![good.clone(), wrong], vec![]);
    assert!(!failed.passed);
    let mut unfrozen = p.clone();
    unfrozen.distribution.case_mean_total_variation = None;
    let pending = judge_case(&unfrozen, "c", "decode", vec![good], vec![]);
    assert!(
        !pending.passed
            && pending
                .pending
                .contains(&"case_mean_total_variation".into())
    );
    assert!(!judge_case(&p, "c", "decode", vec![], vec![]).passed);
    Ok(())
}

#[test]
fn policy_parse_checks_the_schema() {
    let text = serde_json::to_string(&policy()).expect("policy json");
    assert_eq!(Policy::parse(&text), Ok(policy()));
    assert!(Policy::parse(&text.replace(POLICY_SCHEMA, "other")).is_err());
}
