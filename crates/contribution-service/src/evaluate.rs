//! Native protected replay, current permissions, and REV-25 cost joins.

use crate::{Result, source, types::*};
use gym::{
    admission,
    commitment::Commitment,
    row::Row,
    sales_evidence::digest,
    sales_finance,
    suite::{LockedLedger, Suite},
};
use pay_ledger::markets::contribution::Acceptance;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};
use tenancy::training::{self, Corpus, Recipe, Role, Trial, TrialOutcome};

pub const ARTIFACT_CONTRACT: &str =
    "tenant-checkpoint:adapter,head,tokenizer:protected-accuracy-v1";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Report {
    pub schema: String,
    pub frozen: String,
    pub evaluation: String,
    pub acceptance: String,
    pub artifact: String,
    pub recipe: String,
    pub metric: String,
    pub partition: String,
    pub improvement: i64,
    pub all_in_msat: i64,
    pub costs: BTreeMap<CostClass, i64>,
    pub finance_manifest: String,
    pub decision: String,
    pub serving_activated: bool,
}
pub(crate) struct Verified {
    pub frozen: Frozen,
    pub current: Current,
    pub accepted: Acceptance,
    pub report: Report,
    pub admitted_until: i64,
}

fn hash_set(values: &BTreeSet<String>) -> String {
    digest(&serde_json::to_vec(values).expect("source groups serialize"))
}
pub fn plan_policy(plan: &admission::Plan) -> String {
    let mut value = serde_json::to_value(plan).expect("native plan serializes");
    value["digest"] = "".into();
    value["candidate"]["identity"]["artifact_signature"] = "candidate-artifact".into();
    digest(&nostr::contracts::jcs(&value).expect("native plan canonicalizes"))
}
fn phase(root: &Path, source: &Phase) -> Result<(Vec<Row>, Commitment)> {
    source::read(root, &source.store)?;
    let values = gym::store::Store::at(source::path(root, &source.store.path)?)
        .verified_rows()
        .map_err(|_| "protected Gym store chain is invalid")?;
    if values.is_empty() || values.len() > 8192 {
        return Err("protected store coverage is absent or oversized".into());
    }
    let rows: Vec<Row> = values
        .into_iter()
        .map(|v| serde_json::from_value(v).map_err(|_| "invalid protected Gym row".to_string()))
        .collect::<Result<_>>()?;
    let commitment: Commitment = source::json(root, &source.commitment)?;
    if commitment.digest != commitment.compute_digest()
        || !gym::commitment::check(&commitment, &rows).is_empty()
    {
        return Err("protected report does not commit the complete Gym store".into());
    }
    source::read(root, &source.store)?;
    Ok((rows, commitment))
}
fn labels(rows: &[Row], suite: &Suite) -> Result<()> {
    for row in rows {
        row.check()
            .map_err(|_| "invalid protected evaluation answer")?;
        let item = suite
            .items
            .iter()
            .find(|i| i.id == row.item_id)
            .ok_or("protected row names an absent item")?;
        if row.suite != suite.name
            || row.suite_digest != suite.digest
            || row.split != item.partition.as_str()
            || row.family != item.family
        {
            return Err("protected row changed workload or partition".into());
        }
        if row.answered {
            let selected = row
                .selected
                .as_deref()
                .ok_or("protected accuracy requires an explicit selected answer")?;
            if row.correct != Some(selected == item.truth) {
                return Err("protected row correctness disagrees with the retained label".into());
            }
            let distribution = row
                .distribution
                .as_ref()
                .ok_or("protected answer probabilities are absent")?;
            let choices = item
                .question
                .as_ref()
                .and_then(|value| value.get("choices"))
                .and_then(|v| v.as_array())
                .ok_or("the supported accuracy workload requires explicit Choice options")?;
            let options: BTreeSet<_> = choices.iter().filter_map(|v| v.as_str()).collect();
            if item.family != "choice"
                || options.len() != choices.len()
                || options != distribution.keys().map(String::as_str).collect()
                || row.raw_top != distribution.get(selected).copied()
                || distribution
                    .values()
                    .any(|p| !p.is_finite() || !(0.0..=1.0).contains(p))
                || (distribution.values().sum::<f64>() - 1.0).abs() > 1e-9
            {
                return Err(
                    "protected Choice probabilities are invalid or differ from the selected answer"
                        .into(),
                );
            }
        }
    }
    Ok(())
}
fn corpus_suite(corpus: &Corpus, suite: &Suite) -> Result<()> {
    corpus
        .validate()
        .map_err(|_| "corpus provenance or partition validation failed")?;
    if corpus.tombstone.is_some() {
        return Err("protected corpus is tombstoned".into());
    }
    let items: Vec<_> = corpus
        .items
        .iter()
        .filter(|i| i.partition != Role::Training)
        .collect();
    if items.len() != suite.items.len() {
        return Err("corpus and evaluation suite coverage differ".into());
    }
    for item in &suite.items {
        let c = items
            .iter()
            .find(|c| c.id == item.id)
            .ok_or("suite item is absent from corpus")?;
        if c.partition.as_str() != item.partition.as_str()
            || c.state != item.state
            || c.label != item.truth
            || c.question != item.question
        {
            return Err("suite changed protected corpus data, label, or partition".into());
        }
    }
    Ok(())
}
fn corpus_separation(corpus: &Corpus, transfer: &Corpus) -> Result<()> {
    let groups: BTreeSet<_> = corpus.items.iter().map(|i| &i.group).collect();
    if transfer.items.iter().any(|i| groups.contains(&i.group)) {
        return Err("primary and transfer source groups overlap".into());
    }
    // Reuse the native near-duplicate and four-partition checks across both
    // corpora, including calibration/development/locked boundaries.
    let mut combined = corpus.clone();
    combined.items.extend(transfer.items.clone());
    combined.digest = Corpus::digest_of(&combined.items);
    combined
        .validate()
        .map_err(|_| "combined primary and transfer partitions leak")?;
    Ok(())
}
fn groups(corpora: [&Corpus; 2]) -> (BTreeSet<String>, BTreeSet<String>) {
    let mut training = BTreeSet::new();
    let mut evaluation = BTreeSet::new();
    for corpus in corpora {
        for item in &corpus.items {
            if item.partition == Role::Training {
                training.insert(item.group.clone());
            } else {
                evaluation.insert(item.group.clone());
            }
        }
    }
    (training, evaluation)
}
fn accuracy(rows: &[Row], door: &str) -> Result<(i64, i64)> {
    let rows: Vec<_> = rows.iter().filter(|r| r.door == door).collect();
    if rows.is_empty() {
        return Err("accuracy coverage is absent".into());
    }
    Ok((
        rows.iter().filter(|r| r.correct == Some(true)).count() as i64,
        rows.len() as i64,
    ))
}
fn rights(config: &Config, frozen: &Frozen, corpora: [&Corpus; 2], now: i64) -> Result<i64> {
    if frozen.rights.is_empty() || frozen.rights.len() > 256 {
        return Err("current provenance permissions are absent or oversized".into());
    }
    let mut grants = Vec::new();
    for pin in &frozen.rights {
        let (_, grant): (_, Rights) = source::signed(
            &source::bytes(&config.protected_root, &pin.file)?,
            &pin.issuer,
            now,
        )?;
        if grant.schema != "openagents.contribution-rights.v1" || grant.expires_at <= now {
            return Err("contribution provenance permission is expired".into());
        }
        grants.push(grant);
    }
    for corpus in corpora {
        for item in &corpus.items {
            let grant = grants
                .iter()
                .find(|g| {
                    g.permission == item.provenance.permission
                        && g.source == item.provenance.source
                        && g.group == item.group
                        && g.license == item.provenance.license
                        && g.corpus == corpus.digest
                })
                .ok_or("exact current provenance permission is absent")?;
            let action = if item.partition == Role::Training {
                "train"
            } else {
                "evaluate"
            };
            if !grant.actions.iter().any(|a| a == action) {
                return Err("required corpus right is absent".into());
            }
        }
    }
    let licenses: BTreeSet<_> = corpora
        .into_iter()
        .flat_map(|c| c.items.iter().map(|i| i.provenance.license.clone()))
        .collect();
    if frozen.terms.license != hash_set(&licenses) {
        return Err("frozen license identities differ".into());
    }
    grants
        .iter()
        .map(|grant| grant.expires_at)
        .min()
        .ok_or("current rights deadline is absent".into())
}
fn costs(
    config: &Config,
    frozen: &Frozen,
    evaluation: &Evaluation,
    trials: &[Trial],
) -> Result<(BTreeMap<CostClass, i64>, String)> {
    let bytes = source::read(&config.protected_root, &evaluation.finance)?;
    let finance = sales_finance::rebuild(&config.protected_root, &bytes)
        .map_err(|_| "REV-25 customer evidence or required costs are unavailable")?;
    let offer = finance
        .manifest
        .offers
        .iter()
        .find(|o| o.id == evaluation.finance_offer)
        .ok_or("contribution customer evidence is absent")?;
    let view = finance
        .offers
        .iter()
        .find(|o| o.offer == offer.id)
        .ok_or("contribution operating view is absent")?;
    if !finance.inventory_complete
        || !finance.manifest.gaps.is_empty()
        || !view.missing_cost_classes.is_empty()
        || view.costs.iter().any(|c| {
            c.unknown_items != 0 || c.basis != sales_finance::Basis::Billed || c.unit != "msat"
        })
    {
        return Err(
            "contribution requires complete known billed costs in exact millisatoshis".into(),
        );
    }
    if evaluation.costs.is_empty() || evaluation.costs.len() > 512 {
        return Err("contribution cost join is absent or oversized".into());
    }
    let mut seen = BTreeSet::new();
    let mut coverage = BTreeSet::new();
    let mut result = BTreeMap::<CostClass, i64>::new();
    for binding in &evaluation.costs {
        if !seen.insert(&binding.expense) {
            return Err("the same expense cannot fund two cost classes".into());
        }
        let expense = offer
            .entries
            .iter()
            .flat_map(|e| &e.expenses)
            .find(|e| e.id == binding.expense)
            .ok_or("joined contribution expense is absent")?;
        if expense.basis != sales_finance::Basis::Billed
            || expense.unit != "msat"
            || expense.price.is_some()
        {
            return Err("contribution cost is unknown, estimated, or converted".into());
        }
        let line: CostLine = source::json(
            &config.protected_root,
            expense.evidence.as_ref().ok_or("cost bill is absent")?,
        )?;
        if line.schema != "openagents.contribution-cost-line.v1"
            || line.expense != binding.expense
            || line.obligation != frozen.terms.obligation
            || line.class != binding.class
            || line.trial != binding.trial
            || line.amount_msat < 0
            || u64::try_from(line.amount_msat).ok() != expense.amount
        {
            return Err(
                "cost bill does not bind the exact obligation, class, trial, and amount".into(),
            );
        }
        if let Some(index) = binding.trial {
            if index == 0 || index > trials.len() {
                return Err("cost names an absent trial".into());
            }
            coverage.insert((index, binding.class));
        }
        let total = result.entry(binding.class).or_default();
        *total = total
            .checked_add(line.amount_msat)
            .ok_or("contribution cost overflow")?;
    }
    let all_expenses: BTreeSet<_> = offer
        .entries
        .iter()
        .flat_map(|e| &e.expenses)
        .map(|e| &e.id)
        .collect();
    if seen != all_expenses || COST_CLASSES.iter().any(|class| !result.contains_key(class)) {
        return Err("contribution cost inventory is incomplete".into());
    }
    for (index, trial) in trials.iter().enumerate() {
        for class in [CostClass::Training, CostClass::Compute] {
            if !coverage.contains(&(index + 1, class)) {
                return Err("every trial needs its known training and compute costs".into());
            }
        }
        if trial.outcome != TrialOutcome::Kept
            && !coverage.contains(&(index + 1, CostClass::FailedAttempt))
        {
            return Err("rejected and failed trial costs are absent".into());
        }
    }
    let total = result.values().try_fold(0i64, |a, b| {
        a.checked_add(*b).ok_or("contribution cost overflow")
    })?;
    if total > frozen.max_all_in_msat {
        return Err("contribution cost budget exceeded".into());
    }
    Ok((result, finance.manifest_digest))
}

pub(crate) fn verify(
    config: &Config,
    ledger: &pay_ledger::Ledger,
    node: &str,
    now: i64,
) -> Result<Verified> {
    let (frozen_event, frozen): (_, Frozen) = source::signed(
        &source::read(&config.protected_root, &config.frozen)?,
        &config.authority,
        now,
    )?;
    frozen
        .terms
        .validate(now)
        .map_err(|_| "contribution terms are expired or invalid")?;
    if frozen.schema != SCHEMA
        || frozen.terms.funding_authority != config.authority
        || frozen.terms.funding_authority == frozen.terms.beneficiary
        || frozen_event.created_at as i64 != frozen.terms.committed_at
        || frozen.platform_fee_msat < 0
        || frozen.max_all_in_msat <= 0
        || frozen.terms.artifact_contract != digest(ARTIFACT_CONTRACT.as_bytes())
    {
        return Err("unsupported or changed frozen contribution terms".into());
    }
    let (_, current): (_, Current) = source::signed(
        &source::bytes(&config.protected_root, &config.current)?,
        &config.authority,
        now,
    )?;
    if current.schema != "openagents.contribution-current.v1"
        || current.frozen != frozen_event.id
        || !current.enabled
        || current.expires_at <= now
        || current.central_node != node
    {
        return Err("current exact contribution funding admission is unavailable".into());
    }
    let payee = ledger
        .payee(&frozen.terms.beneficiary)
        .map_err(|_| "central destination is unavailable")?
        .ok_or("central contributor destination is absent")?;
    if payee.destination_kind != current.destination_kind
        || payee.destination_value != current.destination_value
        || payee.verified_at > now
        || payee.verified_at < frozen.terms.committed_at
    {
        return Err("current central contributor destination differs".into());
    }
    let corpus: Corpus = source::json(&config.protected_root, &frozen.corpus)?;
    let transfer_corpus: Corpus = source::json(&config.protected_root, &frozen.transfer_corpus)?;
    let recipe: Recipe = source::json(&config.protected_root, &frozen.recipe)?;
    let attribution: Attribution = source::json(&config.protected_root, &frozen.attribution)?;
    if frozen.terms.attribution != frozen.attribution.sha256
        || attribution.schema != "openagents.contribution-attribution.v1"
        || attribution.beneficiary != frozen.terms.beneficiary
        || attribution.corpus != corpus.digest
        || attribution.transfer_corpus != transfer_corpus.digest
    {
        return Err(
            "frozen attribution differs from the beneficiary and exact corpus identities".into(),
        );
    }
    recipe
        .verify()
        .map_err(|_| "frozen native recipe changed")?;
    source::artifact(&config.protected_root, &frozen.baseline)?;
    if recipe.base_model.signature != format!("sha256:{}", frozen.baseline.sha256) {
        return Err("the frozen baseline artifact changed".into());
    }
    if recipe.metric.statistic != "accuracy"
        || recipe.created > gym::eval::utc_from_unix(frozen.terms.committed_at as u64)
        || !recipe.metric.baseline.is_finite()
        || !recipe.metric.min_margin.is_finite()
        || frozen.terms.source != corpus.digest.trim_start_matches("sha256:")
    {
        return Err("only the frozen protected-accuracy recipe is supported".into());
    }
    corpus_separation(&corpus, &transfer_corpus)?;
    let (source_groups, evaluation_groups) = groups([&corpus, &transfer_corpus]);
    if !source_groups.is_disjoint(&evaluation_groups)
        || frozen.terms.source_group != hash_set(&source_groups)
        || frozen.terms.evaluation_group != hash_set(&evaluation_groups)
    {
        return Err("source/evaluation groups overlap or changed".into());
    }
    let rights_deadline = rights(config, &frozen, [&corpus, &transfer_corpus], now)?;
    let admitted_until = frozen
        .terms
        .expires_at
        .min(current.expires_at)
        .min(rights_deadline);
    let (evaluation_event, evaluation): (_, Evaluation) = source::signed(
        &source::read(&config.protected_root, &config.evaluation)?,
        &frozen.terms.protected_evaluator,
        now,
    )?;
    if evaluation.schema != "openagents.contribution-evaluation.v1"
        || evaluation.frozen != frozen_event.id
        || evaluation.evaluated_at <= frozen.terms.committed_at
        || evaluation_event.created_at as i64 != evaluation.evaluated_at
    {
        return Err("protected evaluator chronology differs".into());
    }
    let candidate: training::CandidateDoc =
        source::json(&config.protected_root, &evaluation.candidate)?;
    candidate
        .verify()
        .map_err(|_| "the native candidate seal changed")?;
    if candidate.created < gym::eval::utc_from_unix(frozen.terms.committed_at as u64)
        || candidate.created > gym::eval::utc_from_unix(evaluation.evaluated_at as u64)
    {
        return Err("candidate seal chronology differs".into());
    }
    if !evaluation
        .candidate
        .path
        .starts_with("training/candidates/")
        || evaluation.candidate.path != format!("training/candidates/{}.json", candidate.name)
        || candidate.name.contains('/')
        || candidate.name.contains("..")
    {
        return Err("candidate must resolve to the existing protected training book".into());
    }
    if candidate.identities.recipe_digest != recipe.digest
        || candidate.identities.corpus_digest != corpus.digest
        || candidate.identities.base_model.signature != recipe.base_model.signature
        || candidate.identities.base_model.id != recipe.base_model.id
    {
        return Err("candidate changed corpus, recipe, or baseline".into());
    }
    for (reference, parent, name) in [
        (&frozen.corpus, "corpora", &corpus.name),
        (&frozen.transfer_corpus, "corpora", &transfer_corpus.name),
        (&frozen.recipe, "recipes", &recipe.name),
    ] {
        if name.contains('/')
            || name.contains("..")
            || reference.path != format!("training/{parent}/{name}.json")
        {
            return Err(
                "native corpus and recipe must resolve to the existing protected training book"
                    .into(),
            );
        }
    }
    let trials_bytes = source::read(&config.protected_root, &evaluation.trials)?;
    if evaluation.trials.path != "training/trials.jsonl" {
        return Err("trial history must resolve to the existing training book".into());
    }
    let trials: Vec<Trial> = std::str::from_utf8(&trials_bytes)
        .map_err(|_| "invalid trial history")?
        .lines()
        .map(|line| serde_json::from_str(line).map_err(|_| "malformed trial history".to_string()))
        .collect::<Result<_>>()?;
    if trials.is_empty()
        || trials.len() > 256
        || trials.len() > recipe.trials_max as usize
        || trials.iter().any(|t| {
            t.v != training::TRIAL_SCHEMA
                || t.recipe_digest != recipe.digest
                || !recipe.seeds.contains(&t.seed)
                || t.recorded_at <= gym::eval::utc_from_unix(frozen.terms.committed_at as u64)
                || t.recorded_at > gym::eval::utc_from_unix(evaluation.evaluated_at as u64)
        })
    {
        return Err("trial inventory, recipe, seed, or chronology differs".into());
    }
    let kept = trials
        .get(
            candidate
                .evidence
                .trial
                .checked_sub(1)
                .ok_or("candidate has no kept trial")? as usize,
        )
        .filter(|t| t.outcome == TrialOutcome::Kept)
        .ok_or("candidate has no kept trial")?;
    if evaluation.artifacts.len() != 3 {
        return Err("exact adapter, head, and tokenizer bytes are required".into());
    }
    for (name, pin) in [
        ("adapter", &candidate.identities.adapter),
        ("head", &candidate.identities.head),
        ("tokenizer", &candidate.identities.tokenizer),
    ] {
        let reference = evaluation
            .artifacts
            .get(name)
            .ok_or("candidate artifact is absent")?;
        source::artifact(&config.worker_root, reference)?;
        if *pin != format!("sha256:{}", reference.sha256) || kept.artifacts.get(name) != Some(pin) {
            return Err("candidate artifact differs from the sealed kept trial".into());
        }
    }
    let suite = Suite::load(
        std::str::from_utf8(&source::read(&config.protected_root, &evaluation.suite)?)
            .map_err(|_| "invalid protected suite")?,
    )
    .map_err(|_| "protected suite is invalid")?;
    let transfer_suite = Suite::load(
        std::str::from_utf8(&source::read(
            &config.protected_root,
            &evaluation.transfer_suite,
        )?)
        .map_err(|_| "invalid transfer suite")?,
    )
    .map_err(|_| "transfer suite is invalid")?;
    corpus_suite(&corpus, &suite)?;
    corpus_suite(&transfer_corpus, &transfer_suite)?;
    let plan = admission::Plan::parse(
        std::str::from_utf8(&source::read(&config.protected_root, &evaluation.plan)?)
            .map_err(|_| "invalid native plan")?,
    )
    .map_err(|_| "native plan is invalid")?;
    if plan_policy(&plan) != frozen.plan_policy
        || frozen.terms.evaluation_policy != frozen.plan_policy
        || plan.candidate.identity.artifact_signature != candidate.signature
        || plan.base.identity.artifact_signature != recipe.base_model.signature
        || plan.base.identity.model != recipe.base_model.id
        || plan.candidate.identity.model != recipe.base_model.id
        || plan.candidate.identity.adapter != candidate.name
        || plan.rule.metric_order.first().map(|m| m.metric) != Some(gym::ab::Metric::Accuracy)
    {
        return Err("native plan changed the frozen artifact, recipe, or accuracy policy".into());
    }
    let dev = phase(&config.protected_root, &evaluation.development)?;
    let locked = phase(&config.protected_root, &evaluation.locked)?;
    let transfer = phase(&config.protected_root, &evaluation.transfer)?;
    labels(&dev.0, &suite)?;
    labels(&locked.0, &suite)?;
    labels(&transfer.0, &transfer_suite)?;
    source::read(&config.protected_root, &evaluation.locked_ledger)?;
    let locked_ledger = LockedLedger::at(source::path(
        &config.protected_root,
        &evaluation.locked_ledger.path,
    )?);
    let sides = |rows: &[Row]| {
        (
            rows.iter()
                .filter(|r| r.door == plan.base.door)
                .cloned()
                .collect::<Vec<_>>(),
            rows.iter()
                .filter(|r| r.door == plan.candidate.door)
                .cloned()
                .collect::<Vec<_>>(),
        )
    };
    let ds = sides(&dev.0);
    let ls = sides(&locked.0);
    let ts = sides(&transfer.0);
    let deployment: gym::gate::Deployment =
        source::json(&config.protected_root, &evaluation.deployment)?;
    let evidence = admission::Evidence {
        reports: admission::Reports {
            development: Some(admission::ReportEvidence {
                rows: &dev.0,
                commitment: &dev.1,
            }),
            locked: Some(admission::ReportEvidence {
                rows: &locked.0,
                commitment: &locked.1,
            }),
            transfer: Some(admission::ReportEvidence {
                rows: &transfer.0,
                commitment: &transfer.1,
            }),
        },
        suite: &suite,
        development: admission::Side {
            base: &ds.0,
            candidate: &ds.1,
            store_head: dev.1.head.clone(),
        },
        locked: Some(admission::Locked {
            base: &ls.0,
            candidate: &ls.1,
            ledger: &locked_ledger,
            store_head: locked.1.head.clone(),
        }),
        transfer: Some(admission::Transfer {
            suite: &transfer_suite,
            base: &ts.0,
            candidate: &ts.1,
            store_head: transfer.1.head.clone(),
        }),
        deployment: Some(deployment),
        decided_at: gym::eval::utc_from_unix(evaluation.evaluated_at as u64),
        commitment: Some(dev.1.digest.clone()),
    };
    let record = tenancy::admission::Record::evaluate(&plan, &evidence)
        .map_err(|_| "native protected replay failed")?;
    record
        .admitted()
        .map_err(|_| "native protected comparison does not accept the candidate")?;
    let base = accuracy(&locked.0, &plan.base.door)?;
    let winner = accuracy(&locked.0, &plan.candidate.door)?;
    let db = accuracy(&dev.0, &plan.base.door)?;
    let dc = accuracy(&dev.0, &plan.candidate.door)?;
    if base.1 != winner.1
        || db.1 != dc.1
        || db.0 as f64 / db.1 as f64 != recipe.metric.baseline
        || dc.0 as f64 / dc.1 as f64 - recipe.metric.baseline < recipe.metric.min_margin
        || kept.metrics.get("accuracy") != Some(&(dc.0 as f64 / dc.1 as f64))
        || candidate.evidence.metrics.get("accuracy") != kept.metrics.get("accuracy")
    {
        return Err("the measured accuracy does not meet the frozen recipe".into());
    }
    let improvement = winner
        .0
        .checked_sub(base.0)
        .filter(|v| *v > 0)
        .ok_or("protected confirmation has no accepted improvement")?;
    let (costs, finance_manifest) = costs(config, &frozen, &evaluation, &trials)?;
    let (consent_event, consent): (_, Consent) = source::signed(
        &source::read(&config.protected_root, &config.acceptance)?,
        &frozen.terms.acceptance_authority,
        now,
    )?;
    if consent.schema != "openagents.contribution-acceptance.v1"
        || consent.frozen != frozen_event.id
        || consent.evaluation != evaluation_event.id
        || consent.decision != record.reference()
        || consent.artifact != candidate.signature
        || consent.recipe != recipe.digest
        || consent.improvement != improvement
        || consent.accepted_at < evaluation.evaluated_at
        || consent.accepted_at > now
        || consent_event.created_at as i64 != consent.accepted_at
    {
        return Err("independent acceptance does not bind the recomputed result".into());
    }
    let report = Report {
        schema: "openagents.contribution-service-report.v1".into(),
        frozen: frozen_event.id,
        evaluation: evaluation_event.id.clone(),
        acceptance: consent_event.id.clone(),
        artifact: candidate.signature.clone(),
        recipe: recipe.digest,
        metric: "accuracy".into(),
        partition: "locked_confirmation".into(),
        improvement,
        all_in_msat: costs.values().sum(),
        costs,
        finance_manifest,
        decision: record.reference().into(),
        serving_activated: false,
    };
    let accepted = Acceptance {
        terms_fingerprint: frozen
            .terms
            .fingerprint()
            .map_err(|_| "invalid frozen terms")?,
        artifact: candidate.signature.trim_start_matches("sha256:").into(),
        evaluation_receipt: evaluation_event.id,
        acceptance_receipt: consent_event.id,
        improvement,
        evaluated_at: evaluation.evaluated_at,
        accepted_at: consent.accepted_at,
    };
    Ok(Verified {
        frozen,
        current,
        accepted,
        report,
        admitted_until,
    })
}

#[cfg(test)]
mod separation_tests {
    use super::*;

    #[test]
    fn transfer_cannot_reuse_groups_or_leak_across_primary_partitions() {
        let item = |id: &str, group: &str, role: Role, text: &str| {
            serde_json::from_value(
            serde_json::json!({"id":id,"group":group,"partition":role,"state":text,"question":{"type":"choice","question":"Choose","choices":["yes","no"]},"label":"yes","provenance":{"source":"fixture","license":"fixture","permission":"fixture"}})
        ).unwrap()
        };
        let corpus = |items: Vec<training::CorpusItem>| Corpus {
            v: training::CORPUS_SCHEMA.into(),
            workspace: "fixture".into(),
            name: "fixture".into(),
            created: "2026-10-07".into(),
            label_rules: None,
            retention: training::Retention {
                days: 30,
                access: "operator".into(),
                artifacts: "digests-only".into(),
            },
            digest: Corpus::digest_of(&items),
            items,
            tombstone: None,
        };
        let primary = corpus(vec![
            item(
                "p-train",
                "p-train",
                Role::Training,
                "primary training unique",
            ),
            item(
                "p-locked",
                "p-locked",
                Role::Locked,
                "protected unique locked text",
            ),
        ]);
        let mut transfer = corpus(vec![
            item(
                "t-train",
                "t-train",
                Role::Training,
                "transfer distinct learning",
            ),
            item(
                "t-dev",
                "t-dev",
                Role::Development,
                "evaluation other language",
            ),
        ]);
        assert!(corpus_separation(&primary, &transfer).is_ok());
        transfer.items[1].group = "p-locked".into();
        transfer.digest = Corpus::digest_of(&transfer.items);
        assert!(
            corpus_separation(&primary, &transfer)
                .unwrap_err()
                .contains("groups")
        );
        transfer.items[1].group = "t-dev".into();
        transfer.items[1].state = primary.items[1].state.clone();
        transfer.digest = Corpus::digest_of(&transfer.items);
        assert!(
            corpus_separation(&primary, &transfer)
                .unwrap_err()
                .contains("leak")
        );
    }
}
