//! Bounded task lookup. Retrieval scores are not calibrated probabilities.

use crate::recipes::{Artifact, Check, Recipe, Reference, Snapshot};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub question: String,
    pub operation: Option<String>,
    pub snapshot: Snapshot,
    pub max_bytes: usize,
    #[serde(default)]
    pub conditions: BTreeMap<String, bool>,
    #[serde(default)]
    pub file_candidates: Vec<String>,
    #[serde(default)]
    pub query_vector: Option<Vec<f32>>,
    #[serde(default)]
    pub recipe_vectors: BTreeMap<String, Vec<f32>>,
    #[serde(default)]
    pub decision: Option<Decision>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Decision {
    pub question_digest: String,
    pub artifact_digest: String,
    pub chosen: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Matched,
    Ambiguous,
    Stale,
    NoMatch,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Evidence {
    pub path: String,
    pub symbol: Option<String>,
    pub start: u32,
    pub end: u32,
    pub content_digest: String,
}
impl From<&Reference> for Evidence {
    fn from(r: &Reference) -> Self {
        Self {
            path: r.path.clone(),
            symbol: r.symbol.clone(),
            start: r.start,
            end: r.end,
            content_digest: r.content_digest.clone(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Role {
    pub id: String,
    pub purpose: String,
    pub required: bool,
    pub when: Option<String>,
    pub references: Vec<Evidence>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Candidate {
    pub id: String,
    pub operation: String,
    pub description: String,
    pub score: f64,
    pub roles: Vec<Role>,
    pub checks: Vec<Check>,
    pub examples: Vec<Evidence>,
    pub new_files: Vec<serde_json::Value>,
    pub omitted_roles: Vec<String>,
    pub unresolved_conditions: Vec<String>,
    pub limitations: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Answer {
    pub schema: String,
    pub status: Status,
    pub artifact_digest: String,
    pub candidates: Vec<Candidate>,
    pub required_roles_complete: bool,
    pub omitted_candidates: usize,
    pub next_step: String,
}

fn tokens(value: &str) -> BTreeSet<String> {
    crate::demand::normalize(value)
        .split_whitespace()
        .filter(|s| {
            !matches!(
                *s,
                "a" | "an"
                    | "the"
                    | "how"
                    | "do"
                    | "i"
                    | "can"
                    | "to"
                    | "of"
                    | "in"
                    | "is"
                    | "where"
            )
        })
        .map(|s| {
            if s.len() > 3 {
                s.trim_end_matches('s').into()
            } else {
                s.into()
            }
        })
        .collect()
}

fn cosine(a: &[f32], b: &[f32]) -> Option<f64> {
    if a.len() != b.len()
        || a.is_empty()
        || a.len() > 4096
        || a.iter().chain(b).any(|v| !v.is_finite())
    {
        return None;
    }
    let dot = a
        .iter()
        .zip(b)
        .map(|(x, y)| *x as f64 * *y as f64)
        .sum::<f64>();
    let norm = a.iter().map(|v| (*v as f64).powi(2)).sum::<f64>().sqrt()
        * b.iter().map(|v| (*v as f64).powi(2)).sum::<f64>().sqrt();
    if norm == 0.0 { None } else { Some(dot / norm) }
}

fn view(recipe: &Recipe, score: f64, request: &Request) -> Candidate {
    let roles = recipe
        .roles
        .iter()
        .filter(|r| {
            r.when
                .as_ref()
                .is_none_or(|c| request.conditions.get(c) != Some(&false))
        })
        .map(|r| Role {
            id: r.id.clone(),
            purpose: r.purpose.clone(),
            required: r.required,
            when: r.when.clone(),
            references: r.references.iter().map(Evidence::from).collect(),
        })
        .collect();
    Candidate{id:recipe.id.clone(),operation:recipe.operation.clone(),description:recipe.description.clone(),score,roles,
        checks:recipe.checks.clone(),examples:recipe.examples.iter().map(Evidence::from).collect(),
        new_files:recipe.new_files.iter().map(|n|serde_json::json!({"directory":n.directory,"purpose":n.purpose,"integration":Evidence::from(&n.integration)})).collect(),
        omitted_roles:vec![],unresolved_conditions:recipe.roles.iter().filter_map(|r|r.when.as_ref()).filter(|c|!request.conditions.contains_key(*c)).cloned().collect(),limitations:recipe.limitations.clone()}
}

pub fn query(artifact: &Artifact, request: &Request) -> Result<Answer, String> {
    if request.question.trim().is_empty()
        || request.question.len() > 16384
        || !(1024..=128 * 1024).contains(&request.max_bytes)
    {
        return Err("Choose a question and a context budget between 1 and 128 KiB.".into());
    }
    let artifact_digest = atif::digest(&serde_json::to_value(artifact).map_err(|e| e.to_string())?);
    let mut answer = Answer {
        schema: "openagents.code-lookup.v1".into(),
        status: Status::NoMatch,
        artifact_digest,
        candidates: vec![],
        required_roles_complete: false,
        omitted_candidates: 0,
        next_step: "Use graph_symbol or search to find a source entry point.".into(),
    };
    if artifact.snapshot != request.snapshot {
        answer.status = Status::Stale;
        answer.next_step="Refresh this index for the current repository revision and workspace, or use the graph.".into();
        return Ok(answer);
    }
    let q = tokens(&request.question);
    let mut frequencies = BTreeMap::<String, usize>::new();
    for recipe in &artifact.recipes {
        for term in tokens(&format!(
            "{} {} {}",
            recipe.description,
            recipe.capability,
            recipe.aliases.join(" ")
        )) {
            *frequencies.entry(term).or_default() += 1;
        }
    }
    let mut scored = Vec::new();
    for recipe in &artifact.recipes {
        if request
            .operation
            .as_ref()
            .is_some_and(|o| *o != recipe.operation)
        {
            continue;
        }
        if recipe.exclusions.iter().any(|e| {
            let excluded = tokens(e);
            !excluded.is_empty() && excluded.is_subset(&q)
        }) {
            continue;
        }
        let text = std::iter::once(recipe.description.clone())
            .chain(std::iter::once(recipe.capability.clone()))
            .chain(recipe.aliases.clone())
            .chain(
                recipe
                    .roles
                    .iter()
                    .flat_map(|r| r.references.iter().filter_map(|r| r.symbol.clone())),
            )
            .collect::<Vec<_>>()
            .join(" ");
        let terms = tokens(&text);
        let lexical = q
            .intersection(&terms)
            .map(|term| {
                let frequency = frequencies.get(term).copied().unwrap_or(0);
                ((artifact.recipes.len() + 1) as f64 / (frequency + 1) as f64).ln() + 1.0
            })
            .sum::<f64>();
        let exact = recipe.aliases.iter().any(|a| tokens(a) == q);
        let vector = request
            .query_vector
            .as_ref()
            .zip(request.recipe_vectors.get(&recipe.id))
            .and_then(|(a, b)| cosine(a, b))
            .unwrap_or(0.0)
            .max(0.0);
        let files = recipe
            .roles
            .iter()
            .flat_map(|r| &r.references)
            .filter(|r| request.file_candidates.contains(&r.path))
            .count();
        let score = lexical + vector + files.min(8) as f64 + if exact { 100.0 } else { 0.0 };
        if score > 0.0 {
            scored.push((recipe, score));
        }
    }
    scored.sort_by(|(a, x), (b, y)| y.total_cmp(x).then_with(|| a.id.cmp(&b.id)));
    if let Some(decision) = &request.decision {
        if decision.question_digest != crate::digest(request.question.as_bytes())
            || decision.artifact_digest != answer.artifact_digest
        {
            return Err("The selection belongs to another question or index.".into());
        }
        if let Some(id) = &decision.chosen {
            let Some(at) = scored.iter().take(5).position(|(r, _)| &r.id == id) else {
                return Err("The selected recipe was not a retrieved candidate.".into());
            };
            let chosen = scored.remove(at);
            scored.clear();
            scored.push(chosen);
        } else {
            scored.clear();
        }
    }
    if scored.is_empty() {
        return Ok(answer);
    }
    let ambiguous = scored
        .get(1)
        .is_some_and(|(r, s)| *s == scored[0].1 && r.operation != scored[0].0.operation);
    answer.status = if ambiguous {
        Status::Ambiguous
    } else {
        Status::Matched
    };
    answer.next_step="Read the referenced source and preserve required companion roles; run the listed checks for the change.".into();
    let count = scored.len();
    for (recipe, score) in scored.iter().take(5) {
        let mut candidate = view(recipe, *score, request);
        answer.candidates.push(candidate.clone());
        if serde_json::to_vec(&answer)
            .map_err(|e| e.to_string())?
            .len()
            > request.max_bytes - 128
        {
            answer.candidates.pop();
            candidate.examples.clear();
            candidate.new_files.clear();
            candidate.limitations.clear();
            let optional: Vec<_> = candidate
                .roles
                .iter()
                .filter(|r| !r.required)
                .map(|r| r.id.clone())
                .collect();
            candidate.roles.retain(|r| r.required);
            candidate.omitted_roles.extend(optional);
            answer.candidates.push(candidate.clone());
            if serde_json::to_vec(&answer)
                .map_err(|e| e.to_string())?
                .len()
                > request.max_bytes - 128
            {
                answer.candidates.pop();
                if answer.candidates.is_empty() {
                    candidate
                        .omitted_roles
                        .extend(candidate.roles.iter().map(|r| r.id.clone()));
                    candidate.roles.clear();
                    candidate.checks.clear();
                    answer.candidates.push(candidate);
                    if serde_json::to_vec(&answer)
                        .map_err(|e| e.to_string())?
                        .len()
                        > request.max_bytes - 128
                    {
                        answer.candidates.clear();
                    }
                }
                break;
            }
        }
    }
    answer.omitted_candidates = count - answer.candidates.len();
    answer.required_roles_complete = !answer.candidates.is_empty()
        && answer.candidates.iter().all(|c| {
            let recipe = artifact
                .recipes
                .iter()
                .find(|r| r.id == c.id)
                .expect("retrieved recipe exists");
            c.unresolved_conditions.is_empty()
                && recipe
                    .roles
                    .iter()
                    .filter(|r| {
                        r.required
                            && r.when
                                .as_ref()
                                .is_none_or(|k| request.conditions.get(k) != Some(&false))
                    })
                    .all(|r| c.roles.iter().any(|shown| shown.id == r.id))
        });
    if !answer.required_roles_complete {
        answer.status = Status::Ambiguous;
        answer.next_step="Resolve applicability conditions or increase the context budget, then inspect the graph for missing roles.".into();
    }
    if serde_json::to_vec(&answer)
        .map_err(|e| e.to_string())?
        .len()
        > request.max_bytes
    {
        return Err("The lookup could not fit its result in the requested budget.".into());
    }
    Ok(answer)
}
