//! Private feedback and candidate retrievers. Only replayed training outcomes
//! contribute targets; a teacher's opinion is a separate diagnostic field.

use crate::{
    demand::Partition,
    evaluation::{Origin, Plan, Purpose, Report, VerifiedReplay},
    recipes::Artifact,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Partitions {
    pub schema: String,
    pub repository: String,
    pub workspace: String,
    pub training_cutoff_ms: u64,
    pub groups: BTreeMap<String, Partition>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Observation {
    pub id: String,
    pub question_digest: String,
    pub group: Option<String>,
    pub status: String,
    pub required_roles_complete: bool,
    pub elapsed_ms: u64,
    pub at_ms: Option<u64>,
    pub linked_navigation_calls: Option<usize>,
    pub linked_navigation_ms: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Feedback {
    pub schema: String,
    pub repository: String,
    pub workspace: String,
    pub observations: Vec<Observation>,
    pub notices: Vec<String>,
}

pub fn collect(
    directory: &Path,
    partitions: &Partitions,
    corpus: Option<&crate::episodes::Corpus>,
) -> Result<Feedback, String> {
    if partitions.schema != "openagents.task-partitions.v1" {
        return Err("Choose a frozen task-partition manifest.".into());
    }
    let mut questions = BTreeMap::<String, BTreeSet<String>>::new();
    let mut navigation = BTreeMap::<String, (usize, Option<u64>)>::new();
    if let Some(corpus) = corpus {
        for e in &corpus.episodes {
            let operations = e
                .operations
                .iter()
                .filter(|o| {
                    matches!(
                        o.activity,
                        crate::episodes::Activity::Search
                            | crate::episodes::Activity::Read
                            | crate::episodes::Activity::List
                            | crate::episodes::Activity::History
                    )
                })
                .collect::<Vec<_>>();
            let duration = operations
                .iter()
                .map(|o| o.duration_ms)
                .collect::<Option<Vec<_>>>()
                .map(|v| v.iter().sum::<u64>());
            let entry = navigation
                .entry(crate::digest(e.question.as_bytes()))
                .or_default();
            entry.0 = entry.0.max(operations.len());
            entry.1 = entry.1.zip(duration).map(|(a, b)| a.max(b)).or(duration);
            questions
                .entry(crate::digest(e.question.as_bytes()))
                .or_default()
                .insert(e.family.clone());
        }
    }
    let mut feedback = Feedback {
        schema: "openagents.task-feedback.v1".into(),
        repository: partitions.repository.clone(),
        workspace: partitions.workspace.clone(),
        observations: vec![],
        notices: vec![],
    };
    let mut seen = BTreeSet::new();
    let mut count = 0;
    for entry in std::fs::read_dir(directory).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if !path
            .file_name()
            .and_then(|p| p.to_str())
            .is_some_and(|n| n.starts_with("lookup-") && n.ends_with(".json"))
        {
            continue;
        }
        count += 1;
        if count > 100000 {
            return Err("The selected feedback directory exceeds its entry limit.".into());
        }
        if path.is_symlink()
            || std::fs::metadata(&path).map_err(|e| e.to_string())?.len() > 128 * 1024
        {
            feedback
                .notices
                .push("An oversized or linked observation was skipped.".into());
            continue;
        }
        let value: Value = crate::read_json(&path)?;
        if value["schema"] != "openagents.lookup-observation.v1"
            || value["repository"] != partitions.repository
            || value["workspace"] != partitions.workspace
        {
            feedback
                .notices
                .push("An observation from another source or workspace was skipped.".into());
            continue;
        }
        let id = atif::digest(&value);
        if !seen.insert(id.clone()) {
            continue;
        }
        let question = value["question_digest"]
            .as_str()
            .ok_or("An observation has no question digest.")?
            .to_owned();
        let group = questions
            .get(&question)
            .filter(|g| g.len() == 1)
            .and_then(|g| g.first())
            .filter(|g| partitions.groups.contains_key(*g))
            .cloned();
        feedback.observations.push(Observation {
            id,
            linked_navigation_calls: navigation.get(&question).map(|n| n.0),
            linked_navigation_ms: navigation.get(&question).and_then(|n| n.1),
            question_digest: question,
            group,
            status: value["status"].as_str().unwrap_or("unknown").into(),
            required_roles_complete: value["required_roles_complete"].as_bool().unwrap_or(false),
            elapsed_ms: value["elapsed_ms"].as_u64().unwrap_or(0),
            at_ms: value["at_ms"].as_u64(),
        });
    }
    feedback.observations.sort_by(|a, b| {
        b.linked_navigation_ms
            .unwrap_or(b.elapsed_ms)
            .cmp(&a.linked_navigation_ms.unwrap_or(a.elapsed_ms))
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(feedback)
}

#[derive(Clone, Debug, Serialize)]
pub struct Target {
    artifact_digest: String,
    question: String,
    recipe: String,
    group: String,
    receipt_digest: String,
    fixture: bool,
    /// A separately retained diagnostic, never a training label or feature.
    teacher_hint: Option<f64>,
}
impl Target {
    pub fn set_teacher_hint(&mut self, value: Option<f64>) {
        self.teacher_hint = value;
    }
}

pub fn admit(
    replay: VerifiedReplay,
    partitions: &Partitions,
    allow_fixture: bool,
) -> Result<Vec<Target>, String> {
    let case = &replay.case;
    if partitions.schema != "openagents.task-partitions.v1"
        || partitions.repository != case.index.snapshot.repository
        || partitions.workspace != case.index.snapshot.workspace
        || partitions.groups.get(&case.family) != Some(&Partition::Training)
        || case.partition != Partition::Training
        || replay.trial.finished_ms == 0
        || replay.trial.finished_ms > partitions.training_cutoff_ms
    {
        return Err("Only outcomes in the frozen training partition and time cut may become learning targets.".into());
    }
    let fixture = replay.trial.engine.origin == Origin::Fixture;
    if fixture && !allow_fixture {
        return Err("Fixture outcomes cannot supply production training targets.".into());
    }
    let expected: BTreeSet<_> = case.companion_paths.iter().collect();
    let receipt_digest =
        atif::digest(&serde_json::to_value(&replay.receipt).map_err(|e| e.to_string())?);
    let mut targets = Vec::new();
    for recipe in &case.index.recipes {
        let overlap = recipe
            .roles
            .iter()
            .filter(|r| r.required)
            .flat_map(|r| &r.references)
            .any(|r| expected.contains(&r.path));
        if overlap {
            targets.push(Target {
                artifact_digest: atif::digest(
                    &serde_json::to_value(&case.index).map_err(|e| e.to_string())?,
                ),
                question: case.task.clone(),
                recipe: recipe.id.clone(),
                group: case.family.clone(),
                receipt_digest: receipt_digest.clone(),
                fixture,
                teacher_hint: None,
            });
        }
    }
    // These targets predict verified required-source overlap, not causal use
    // of a recipe or an independent proof of every semantic claim in it.
    Ok(targets)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selector {
    pub schema: String,
    pub repository: String,
    pub workspace: String,
    pub artifact_digest: String,
    pub partition_digest: String,
    pub training_digest: String,
    pub training_groups: BTreeSet<String>,
    pub weights: BTreeMap<String, BTreeMap<String, f64>>,
    pub fixture_only: bool,
}

pub fn train(
    artifact: &Artifact,
    partitions: &Partitions,
    targets: &[Target],
) -> Result<Selector, String> {
    if targets.is_empty()
        || targets.len() > 100000
        || partitions.schema != "openagents.task-partitions.v1"
        || artifact.snapshot.repository != partitions.repository
        || artifact.snapshot.workspace != partitions.workspace
    {
        return Err("Choose verified training targets for this repository and workspace.".into());
    }
    let known: BTreeSet<_> = artifact.recipes.iter().map(|r| r.id.as_str()).collect();
    let mut grouped = BTreeMap::<(String, String), BTreeSet<String>>::new();
    let mut groups = BTreeSet::new();
    for target in targets {
        if target.artifact_digest
            != atif::digest(&serde_json::to_value(artifact).map_err(|e| e.to_string())?)
            || partitions.groups.get(&target.group) != Some(&Partition::Training)
            || !known.contains(target.recipe.as_str())
        {
            return Err("A training target belongs to a held-out group or unknown recipe.".into());
        }
        groups.insert(target.group.clone());
        grouped
            .entry((target.group.clone(), target.recipe.clone()))
            .or_default()
            .extend(terms(&target.question));
    }
    let mut weights = BTreeMap::<String, BTreeMap<String, f64>>::new();
    for ((_, recipe), terms) in &grouped {
        for term in terms {
            *weights
                .entry(recipe.clone())
                .or_default()
                .entry(term.clone())
                .or_default() += 1.0;
        }
    }
    for values in weights.values_mut() {
        let norm = values.values().map(|v| v * v).sum::<f64>().sqrt();
        for value in values.values_mut() {
            *value /= norm.max(1.0);
        }
    }
    let training_digest=atif::digest(&json!(targets.iter().map(|t|json!({"question_digest":crate::digest(t.question.as_bytes()),"recipe":t.recipe,"group":t.group,"receipt_digest":t.receipt_digest})).collect::<Vec<_>>()));
    Ok(Selector {
        schema: "openagents.task-selector.v1".into(),
        repository: partitions.repository.clone(),
        workspace: partitions.workspace.clone(),
        artifact_digest: atif::digest(&serde_json::to_value(artifact).map_err(|e| e.to_string())?),
        partition_digest: atif::digest(
            &serde_json::to_value(partitions).map_err(|e| e.to_string())?,
        ),
        training_digest,
        training_groups: groups,
        weights,
        fixture_only: targets.iter().any(|t| t.fixture),
    })
}

fn terms(question: &str) -> BTreeSet<String> {
    crate::demand::normalize(question)
        .split_whitespace()
        .filter(|t| t.len() > 1)
        .map(str::to_owned)
        .collect()
}

impl Selector {
    pub fn rank(
        &self,
        artifact: &Artifact,
        question: &str,
        candidates: &[String],
    ) -> Result<Vec<(String, f64)>, String> {
        if self.schema != "openagents.task-selector.v1"
            || self.repository != artifact.snapshot.repository
            || self.workspace != artifact.snapshot.workspace
            || self.artifact_digest
                != atif::digest(&serde_json::to_value(artifact).map_err(|e| e.to_string())?)
        {
            return Err("The selector was trained for a different recipe artifact.".into());
        }
        let q = terms(question);
        let mut ranked = Vec::new();
        for id in candidates {
            if !artifact.recipes.iter().any(|r| r.id == *id) {
                return Err("A selection candidate is not in the task index.".into());
            }
            if let Some(weights) = self.weights.get(id) {
                if weights.values().any(|v| !v.is_finite() || *v < 0.0) {
                    return Err("The selector contains invalid weights.".into());
                }
                let score = q
                    .iter()
                    .map(|t| weights.get(t).copied().unwrap_or(0.0))
                    .sum::<f64>();
                if score > 0.0 {
                    ranked.push((id.clone(), score));
                }
            }
        }
        ranked.sort_by(|(a, x), (b, y)| y.total_cmp(x).then_with(|| a.cmp(b)));
        Ok(ranked)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestCase {
    pub question: String,
    pub family: String,
    pub expected: BTreeSet<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectorReceipt {
    pub schema: String,
    pub candidate_digest: String,
    pub baseline_digest: Option<String>,
    pub artifact_digest: String,
    pub partition_digest: String,
    pub dataset_digest: String,
    pub evaluation_digest: String,
    pub minimum_gain: f64,
    pub candidate_recall: f64,
    pub baseline_recall: f64,
    pub passed: bool,
}

pub fn compare(
    candidate: &Selector,
    baseline: Option<&Selector>,
    artifact: &Artifact,
    partitions: &Partitions,
    cases: &[TestCase],
    evaluation_plan: &Plan,
    evaluation: &Report,
    minimum_gain: f64,
) -> Result<SelectorReceipt, String> {
    if cases.is_empty()
        || !minimum_gain.is_finite()
        || !(0.0..=1.0).contains(&minimum_gain)
        || candidate.partition_digest
            != atif::digest(&serde_json::to_value(partitions).map_err(|e| e.to_string())?)
    {
        return Err("Choose a frozen held-out selector dataset and gain target.".into());
    }
    candidate.rank(artifact, "", &[])?;
    let checked = crate::evaluation::report(evaluation_plan, evaluation.trials.clone())?;
    if checked.plan_digest != evaluation.plan_digest
        || checked.promotion_passed != evaluation.promotion_passed
        || evaluation_plan.purpose != Purpose::Evaluation
    {
        return Err("The edit-quality receipt does not match its frozen evaluation.".into());
    }
    let candidate_digest =
        atif::digest(&serde_json::to_value(candidate).map_err(|e| e.to_string())?);
    let same_model = evaluation_plan
        .arms
        .iter()
        .find(|a| a.id == evaluation_plan.candidate)
        .zip(
            evaluation_plan
                .arms
                .iter()
                .find(|a| a.id == evaluation_plan.baseline),
        )
        .is_some_and(|(a, b)| {
            a.model == b.model
                && matches!(a.context, crate::evaluation::ContextKind::Recipes)
                && matches!(b.context, crate::evaluation::ContextKind::Recipes)
        });
    let actual_candidate: BTreeSet<_> = evaluation
        .trials
        .iter()
        .filter(|t| t.arm == evaluation_plan.candidate)
        .map(|t| t.engine.model.as_str())
        .collect();
    let actual_baseline: BTreeSet<_> = evaluation
        .trials
        .iter()
        .filter(|t| t.arm == evaluation_plan.baseline)
        .map(|t| t.engine.model.as_str())
        .collect();
    let frozen_candidate = evaluation_plan
        .selectors
        .get(&evaluation_plan.candidate)
        .map(|m| atif::digest(&serde_json::to_value(m).unwrap_or_default()));
    let frozen_baseline = evaluation_plan
        .selectors
        .get(&evaluation_plan.baseline)
        .map(|m| atif::digest(&serde_json::to_value(m).unwrap_or_default()));
    let baseline_digest =
        baseline.map(|m| atif::digest(&serde_json::to_value(m).unwrap_or_default()));
    if !same_model
        || actual_candidate.len() != 1
        || actual_candidate != actual_baseline
        || frozen_candidate.as_deref() != Some(&candidate_digest)
        || frozen_baseline != baseline_digest
        || evaluation_plan.selector_dataset_digest.as_deref()
            != Some(&atif::digest(
                &serde_json::to_value(cases).map_err(|e| e.to_string())?,
            ))
        || evaluation_plan.selector_minimum_gain != Some(minimum_gain)
        || partitions.training_cutoff_ms > evaluation_plan.index_cutoff_ms
        || evaluation_plan.cases.iter().any(|c| {
            atif::digest(&serde_json::to_value(&c.index).unwrap_or_default())
                != candidate.artifact_digest
                || candidate.training_groups.contains(&c.family)
                || partitions.groups.get(&c.family) != Some(&c.partition)
        })
    {
        return Err("The frozen edit experiment must compare these exact selectors, artifact, model, and target after the training cut.".into());
    }
    let ids: Vec<_> = artifact.recipes.iter().map(|r| r.id.clone()).collect();
    let mut families = BTreeMap::<String, (usize, usize, usize)>::new();
    for case in cases {
        if candidate.training_groups.contains(&case.family)
            || baseline.is_some_and(|b| b.training_groups.contains(&case.family))
            || !matches!(
                partitions.groups.get(&case.family),
                Some(Partition::Development | Partition::Evaluation | Partition::Locked)
            )
            || case.expected.is_empty()
            || case.expected.iter().any(|id| !ids.contains(id))
        {
            return Err("Selector evaluation must use held-out families and known targets.".into());
        }
        let request = crate::lookup::Request {
            question: case.question.clone(),
            operation: None,
            snapshot: artifact.snapshot.clone(),
            max_bytes: 16000,
            conditions: BTreeMap::new(),
            file_candidates: vec![],
            query_vector: None,
            recipe_vectors: BTreeMap::new(),
            decision: None,
        };
        let mut candidate_request = request.clone();
        apply(candidate, artifact, &mut candidate_request)?;
        let candidate_id = crate::lookup::query(artifact, &candidate_request)?
            .candidates
            .first()
            .map(|c| c.id.clone());
        let mut baseline_request = request;
        if let Some(b) = baseline {
            apply(b, artifact, &mut baseline_request)?;
        }
        let baseline_id = crate::lookup::query(artifact, &baseline_request)?
            .candidates
            .first()
            .map(|c| c.id.clone());
        let counts = families.entry(case.family.clone()).or_default();
        counts.0 += usize::from(candidate_id.is_some_and(|id| case.expected.contains(&id)));
        counts.1 += usize::from(baseline_id.is_some_and(|id| case.expected.contains(&id)));
        counts.2 += 1;
    }
    let candidate_recall = families
        .values()
        .map(|(a, _, n)| *a as f64 / *n as f64)
        .sum::<f64>()
        / families.len() as f64;
    let baseline_recall = families
        .values()
        .map(|(_, b, n)| *b as f64 / *n as f64)
        .sum::<f64>()
        / families.len() as f64;
    let evaluation_digest =
        atif::digest(&serde_json::to_value(evaluation).map_err(|e| e.to_string())?);
    Ok(SelectorReceipt {
        schema: "openagents.task-selector-comparison.v1".into(),
        candidate_digest: atif::digest(
            &serde_json::to_value(candidate).map_err(|e| e.to_string())?,
        ),
        baseline_digest: baseline
            .map(|b| atif::digest(&serde_json::to_value(b).unwrap_or_default())),
        artifact_digest: candidate.artifact_digest.clone(),
        partition_digest: candidate.partition_digest.clone(),
        dataset_digest: atif::digest(&serde_json::to_value(cases).map_err(|e| e.to_string())?),
        evaluation_digest,
        minimum_gain,
        candidate_recall,
        baseline_recall,
        passed: checked.promotion_passed
            && families.len() >= evaluation_plan.min_families
            && !candidate.fixture_only
            && candidate_recall - baseline_recall >= minimum_gain,
    })
}

pub fn promote(
    directory: &Path,
    candidate: &Selector,
    receipt: &SelectorReceipt,
    current: Option<&str>,
    artifact: &Artifact,
    partitions: &Partitions,
    cases: &[TestCase],
    plan: &Plan,
    evaluation: &Report,
) -> Result<String, String> {
    if current.is_some_and(|id| !valid_id(id)) {
        return Err("Choose a known selector digest.".into());
    }
    let baseline = if let Some(id) = current {
        let model: Selector = crate::read_json(&directory.join(format!("selector-{id}.json")))?;
        if atif::digest(&serde_json::to_value(&model).map_err(|e| e.to_string())?) != id {
            return Err("The active selector does not match its digest.".into());
        }
        Some(model)
    } else {
        None
    };
    let repeated = compare(
        candidate,
        baseline.as_ref(),
        artifact,
        partitions,
        cases,
        plan,
        evaluation,
        receipt.minimum_gain,
    )?;
    if !repeated.passed
        || atif::digest(&serde_json::to_value(&repeated).map_err(|e| e.to_string())?)
            != atif::digest(&serde_json::to_value(receipt).map_err(|e| e.to_string())?)
    {
        return Err("The candidate has no passing comparison for these exact inputs.".into());
    }
    crate::private::write(
        directory,
        "selector-store.json",
        b"{\"schema\":\"openagents.task-selector-store.v1\"}",
    )?;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let lock = options
        .open(directory.join("selector.lock"))
        .map_err(|e| e.to_string())?;
    lock.try_lock()
        .map_err(|_| "Another process is updating the selector.")?;
    let active = if directory.join("selector-active.json").exists() {
        Some(crate::read_json::<String>(
            &directory.join("selector-active.json"),
        )?)
    } else {
        None
    };
    if active.as_deref() != current || receipt.baseline_digest.as_deref() != current {
        return Err("The active selector changed after comparison.".into());
    }
    check_installed(directory, &candidate.artifact_digest)?;
    let id = &receipt.candidate_digest;
    crate::private::write(
        directory,
        &format!("selector-{id}.json"),
        &serde_json::to_vec(candidate).map_err(|e| e.to_string())?,
    )?;
    crate::private::write(
        directory,
        &format!("selector-{id}-receipt.json"),
        &serde_json::to_vec(receipt).map_err(|e| e.to_string())?,
    )?;
    crate::private::write(
        directory,
        "selector-active.json",
        &serde_json::to_vec(id).map_err(|e| e.to_string())?,
    )?;
    Ok(id.clone())
}

pub fn rollback(directory: &Path, digest: &str, expected: &str) -> Result<(), String> {
    if ![digest, expected]
        .iter()
        .all(|id| id.len() == 64 && id.chars().all(|c| c.is_ascii_hexdigit()))
    {
        return Err("Choose known selector digests.".into());
    }
    let model: Selector = crate::read_json(&directory.join(format!("selector-{digest}.json")))?;
    if atif::digest(&serde_json::to_value(&model).map_err(|e| e.to_string())?) != digest {
        return Err("The saved selector does not match its digest.".into());
    }
    let receipt: SelectorReceipt =
        crate::read_json(&directory.join(format!("selector-{digest}-receipt.json")))?;
    if !receipt.passed || receipt.candidate_digest != digest || model.fixture_only {
        return Err("The saved selector was never admitted for activation.".into());
    }
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let lock = options
        .open(directory.join("selector.lock"))
        .map_err(|e| e.to_string())?;
    lock.try_lock()
        .map_err(|_| "Another process is updating the selector.")?;
    check_installed(directory, &model.artifact_digest)?;
    let active: String = crate::read_json(&directory.join("selector-active.json"))?;
    if active != expected {
        return Err("The active selector changed before rollback.".into());
    }
    crate::private::write(
        directory,
        "selector-active.json",
        &serde_json::to_vec(digest).map_err(|e| e.to_string())?,
    )
}

fn valid_id(id: &str) -> bool {
    id.len() == 64 && id.chars().all(|c| c.is_ascii_hexdigit())
}

/// Rerank only the lookup's closed candidate set; ties keep its existing order.
pub fn apply(
    selector: &Selector,
    artifact: &Artifact,
    request: &mut crate::lookup::Request,
) -> Result<(), String> {
    request.decision = None;
    if selector.repository != artifact.snapshot.repository
        || selector.workspace != artifact.snapshot.workspace
    {
        return Err("The selector belongs to another repository or workspace.".into());
    }
    if selector.artifact_digest
        != atif::digest(&serde_json::to_value(artifact).map_err(|e| e.to_string())?)
    {
        return Ok(());
    }
    let initial = crate::lookup::query(artifact, request)?;
    let ids = initial
        .candidates
        .iter()
        .take(5)
        .map(|c| c.id.clone())
        .collect::<Vec<_>>();
    let ranked = selector.rank(artifact, &request.question, &ids)?;
    if let Some((id, score)) = ranked.first() {
        if ranked.get(1).is_none_or(|(_, next)| score > next) {
            request.decision = Some(crate::lookup::Decision {
                question_digest: crate::digest(request.question.as_bytes()),
                artifact_digest: initial.artifact_digest,
                chosen: Some(id.clone()),
            });
        }
    }
    Ok(())
}

/// Runtime selection is optional. A missing, stale, or invalid candidate falls back.
pub fn active(directory: &Path, artifact: &Artifact) -> Result<Option<Selector>, String> {
    if !directory.join("selector-active.json").exists() {
        return Ok(None);
    }
    let id: String = crate::read_json(&directory.join("selector-active.json"))?;
    if !valid_id(&id) {
        return Err("The saved selector ID is invalid.".into());
    }
    let model: Selector = crate::read_json(&directory.join(format!("selector-{id}.json")))?;
    let receipt: SelectorReceipt =
        crate::read_json(&directory.join(format!("selector-{id}-receipt.json")))?;
    if model.fixture_only
        || !receipt.passed
        || receipt.candidate_digest != id
        || atif::digest(&serde_json::to_value(&model).map_err(|e| e.to_string())?) != id
        || receipt.artifact_digest != model.artifact_digest
        || receipt.partition_digest != model.partition_digest
    {
        return Err("The saved selector has no matching activation receipt.".into());
    }
    model.rank(artifact, "", &[])?;
    Ok(Some(model))
}

/// Conservatively discard learned derivatives when any private source is withdrawn.
pub(crate) fn withdraw_all(directory: &Path) -> Result<(), String> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let lock = options
        .open(directory.join("selector.lock"))
        .map_err(|e| e.to_string())?;
    lock.try_lock()
        .map_err(|_| "Another process is updating the selector.")?;
    for entry in std::fs::read_dir(directory).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path
            .file_name()
            .and_then(|p| p.to_str())
            .is_some_and(|n| n.starts_with("selector-") && n.ends_with(".json"))
        {
            std::fs::remove_file(path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn check_installed(directory: &Path, artifact_digest: &str) -> Result<(), String> {
    let id: String = crate::read_json(&directory.join("active.json"))?;
    if !valid_id(&id) {
        return Err("Install this recipe artifact before activating its selector.".into());
    }
    let bundle: crate::store::Bundle = crate::read_json(&directory.join(format!("{id}.json")))?;
    if crate::digest(&serde_json::to_vec(&bundle).map_err(|e| e.to_string())?) != id
        || atif::digest(&serde_json::to_value(&bundle.artifact).map_err(|e| e.to_string())?)
            != artifact_digest
    {
        return Err("The installed recipe artifact changed before selector activation.".into());
    }
    Ok(())
}
