//! Immutable catalog imports and restart-safe objective assignment intents.
use super::*;
use grid_foreman::objectives::{CATALOG, Catalog, digest};
use openagents_sdk::agents::AgentDefinition;

const BOOK_KEY: &str = "grid-foreman/objectives/attempts.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Exploration {
    pub objective: String,
    pub revision: String,
    pub request_id: String,
}
#[derive(Clone, Serialize, Deserialize)]
struct Attempt {
    reference: Exploration,
    at: u64,
    owner: String,
    create: CreateAssignment,
    definition: AgentDefinition,
}
#[derive(Default, Serialize, Deserialize)]
struct Attempts {
    attempts: Vec<Attempt>,
}

async fn load(store: &Store) -> Result<(Attempts, Option<String>), &'static str> {
    match store
        .read_key(BOOK_KEY)
        .await
        .map_err(|_| "foreman_objectives_unreadable")?
    {
        Some((bytes, generation)) => Ok((
            serde_json::from_slice(&bytes).map_err(|_| "foreman_objectives_unreadable")?,
            Some(generation),
        )),
        None => Ok((Attempts::default(), None)),
    }
}

/// Import only the catalog shipped by the operator, never visitor-supplied text.
/// Preserve each exact revision before any attempt refers to it.
pub(super) async fn import(store: &Store) -> Result<Catalog, &'static str> {
    let catalog = Catalog::parse(CATALOG.as_bytes()).map_err(|_| "foreman_catalog_invalid")?;
    let bytes = serde_json::to_vec(&catalog).map_err(|_| "foreman_catalog_invalid")?;
    let key = format!(
        "grid-foreman/objectives/catalog-{}.json",
        catalog.revision()
    );
    match store
        .read_key(&key)
        .await
        .map_err(|_| "foreman_objectives_unreadable")?
    {
        Some((saved, _)) if saved == bytes => {}
        Some(_) => return Err("foreman_catalog_conflict"),
        None => {
            if store.write_key(&key, bytes.clone(), None).await.is_err() {
                let (saved, _) = store
                    .read_key(&key)
                    .await
                    .ok()
                    .flatten()
                    .ok_or("foreman_objectives_unreadable")?;
                if saved != bytes {
                    return Err("foreman_catalog_conflict");
                }
            }
        }
    }
    Ok(catalog)
}
fn active(state: AssignmentState) -> bool {
    matches!(state, AssignmentState::Queued | AssignmentState::Running)
}
fn choose(
    catalog: &Catalog,
    attempts: &Attempts,
    now: u64,
    state: impl Fn(&str) -> Option<AssignmentState>,
) -> Option<Exploration> {
    // An intent with an unknown assignment must be retried with its original ID.
    if let Some(a) = attempts
        .attempts
        .iter()
        .find(|a| state(&a.reference.request_id).is_none())
    {
        return Some(a.reference.clone());
    }
    if attempts
        .attempts
        .iter()
        .any(|a| state(&a.reference.request_id).is_some_and(active))
    {
        return None;
    }
    let objective = catalog
        .objectives
        .iter()
        .filter(|o| o.desired_state == "active")
        .filter_map(|o| {
            let last = attempts
                .attempts
                .iter()
                .filter(|a| a.reference.objective == o.id)
                .map(|a| a.at)
                .max();
            if last.is_some_and(|at| {
                now.saturating_sub(at) < catalog.selection.review_interval_seconds
            }) {
                return None;
            }
            let priority = match o.priority.as_str() {
                "high" => 0,
                "normal" => 1,
                _ => 2,
            };
            Some((priority, last.unwrap_or(0), &o.id))
        })
        .min_by_key(|(priority, last, _)| (*priority, *last))?;
    let revision = catalog.revision();
    let request_id = digest(
        format!(
            "foreman-objective-v1\0{revision}\0{}\0{}",
            objective.2,
            attempts.attempts.len()
        )
        .as_bytes(),
    );
    Some(Exploration {
        objective: objective.2.clone(),
        revision,
        request_id,
    })
}

pub(super) async fn next(
    store: &Store,
    now: u64,
    state: impl Fn(&str) -> Option<AssignmentState>,
) -> Result<Option<Exploration>, &'static str> {
    let catalog = import(store).await?;
    let (attempts, _) = load(store).await?;
    Ok(choose(&catalog, &attempts, now, state))
}

/// Reserved intents also count toward caps, even when a response or log write is lost.
pub(super) async fn entries(store: &Store) -> Result<Vec<Entry>, &'static str> {
    Ok(load(store)
        .await?
        .0
        .attempts
        .iter()
        .map(|a| Entry {
            at: a.at,
            action: grid_foreman::EXPLORE.into(),
            issue: 0,
            result: "started".into(),
            assignment: Some(a.reference.request_id.clone()),
            objective: Some(a.reference.objective.clone()),
            ..Entry::default()
        })
        .collect())
}

/// Persist the exact assignment before dispatch; a restart cannot rebrief it or
/// turn a lost response into a second chat.
pub(super) async fn start(
    store: &Store,
    reference: &Exploration,
    worker: Worker,
    now: u64,
    allow_new: bool,
) -> Result<(String, String), &'static str> {
    for _ in 0..8 {
        let (mut attempts, generation) = load(store).await?;
        if let Some(a) = attempts.attempts.iter().find(|a| &a.reference == reference) {
            if a.owner != worker.0 {
                return Err("foreman_objective_conflict");
            }
            let made = crate::agent_assignments::mutate(
                store,
                &a.owner,
                Change::Create {
                    body: a.create.clone(),
                    definition: a.definition.clone(),
                },
            )
            .await
            .map_err(|_| "foreman_unsetup")?
            .map_err(|_| "foreman_objective_conflict")?;
            return Ok((made.id, made.chat));
        }
        if !allow_new {
            return Err("foreman_backlog_unknown");
        }
        if attempts.attempts.len() >= 4096 {
            return Err("foreman_cap");
        }
        let catalog = import(store).await?;
        let (book, _) = crate::agent_assignments::load(store, &worker.0)
            .await
            .map_err(|_| "foreman_unsetup")?;
        if choose(&catalog, &attempts, now, |id| {
            book.assignments.get(id).map(|a| a.state)
        })
        .as_ref()
            != Some(reference)
        {
            return Err("foreman_objective_conflict");
        }
        let objective = catalog
            .objective(&reference.objective)
            .ok_or("foreman_catalog_invalid")?;
        let definition =
            match crate::custom_agents::definition(store, &worker.0, &worker.1, None).await {
                Ok(Ok(d)) if !d.deleted => d,
                _ => return Err("foreman_unsetup"),
            };
        if definition
            .spec
            .autonomy
            .as_ref()
            .is_some_and(|a| a.authority)
        {
            return Err("foreman_worker_unsafe");
        }
        let create = CreateAssignment {
            request_id: reference.request_id.clone(),
            spec: AssignmentSpec {
                agent: worker.1.clone(),
                agent_revision: definition.revision,
                host: worker.2.clone(),
                project: worker.3.clone(),
                task: super::dispatch_brief(catalog.brief(
                    objective,
                    &reference.revision,
                    &reference.request_id,
                ))?,
            },
            chat: None,
        };
        attempts.attempts.push(Attempt {
            reference: reference.clone(),
            at: now,
            owner: worker.0.clone(),
            create,
            definition,
        });
        let bytes = serde_json::to_vec(&attempts).map_err(|_| "foreman_catalog_invalid")?;
        if store
            .write_key(BOOK_KEY, bytes, generation.as_deref())
            .await
            .is_ok()
        {
            // Resume through the same path after the durable reservation.
            continue;
        }
    }
    Err("foreman_objective_conflict")
}

/// A complete fresh GitHub read is required to call a backlog empty.
pub(super) async fn eligible_issues() -> Result<Vec<u64>, &'static str> {
    // Share one read across public polls and command checks. Errors expire too;
    // a previous success is never substituted for a failed refresh.
    type Cached = Option<(std::time::Instant, Result<Vec<u64>, &'static str>)>;
    static CACHE: tokio::sync::Mutex<Cached> = tokio::sync::Mutex::const_new(None);
    let mut cache = CACHE.lock().await;
    if let Some((at, result)) = cache.as_ref()
        && at.elapsed().as_secs() < 60
    {
        return result.clone();
    }
    let result = read_eligible_issues().await;
    *cache = Some((std::time::Instant::now(), result.clone()));
    result
}

async fn read_eligible_issues() -> Result<Vec<u64>, &'static str> {
    let catalog = Catalog::parse(CATALOG.as_bytes()).map_err(|_| "foreman_catalog_invalid")?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|_| "foreman_backlog_unknown")?;
    let token = std::env::var("OPENAGENTS_FOREMAN_GITHUB_TOKEN").ok();
    let mut eligible = Vec::new();
    let mut pull_requests = Vec::new();
    for page in 1..=10 {
        let value = crate::grid_foreman::get_json(
            &client,
            &format!(
                "https://api.github.com/repos/{}/issues?state=open&per_page=100&page={page}",
                catalog.repository
            ),
            &[],
            token.as_deref(),
        )
        .await
        .map_err(|_| "foreman_backlog_unknown")?;
        let rows = value.as_array().ok_or("foreman_backlog_unknown")?;
        for row in rows {
            let number = row["number"].as_u64().ok_or("foreman_backlog_unknown")?;
            if row.get("pull_request").is_some() {
                pull_requests.push(row["body"].as_str().unwrap_or_default().to_owned());
                continue;
            }
            if catalog.selection.excluded_issues.contains(&number)
                || row["issue_dependencies_summary"]["blocked_by"]
                    .as_u64()
                    .is_some_and(|n| n > 0)
            {
                continue;
            }
            let assigned = row["assignees"]
                .as_array()
                .ok_or("foreman_backlog_unknown")?;
            let labels = row["labels"].as_array().ok_or("foreman_backlog_unknown")?;
            if assigned.is_empty()
                && !labels.iter().any(|l| {
                    l["name"].as_str().is_some_and(|name| {
                        grid_foreman::DENY_LABELS
                            .iter()
                            .chain(["blocked", "waiting-on-owner", "deferred"].iter())
                            .any(|deny| name.eq_ignore_ascii_case(deny))
                    })
                })
            {
                if !claimed(&client, &catalog.repository, number, token.as_deref()).await? {
                    eligible.push(number);
                }
            }
        }
        if rows.len() < 100 {
            // A PR mentioning an issue needs follow-up, not another assignment.
            eligible.retain(|number| {
                !pull_requests.iter().any(|body| {
                    body.contains(&format!("#{number}"))
                        || body.contains(&format!("/issues/{number}"))
                })
            });
            eligible.sort_unstable();
            return Ok(eligible);
        }
    }
    Err("foreman_backlog_unknown")
}

async fn claimed(
    client: &reqwest::Client,
    repository: &str,
    issue: u64,
    token: Option<&str>,
) -> Result<bool, &'static str> {
    let mut comments = Vec::new();
    for page in 1..=10 {
        let rows = crate::grid_foreman::get_json(client,
            &format!("https://api.github.com/repos/{repository}/issues/{issue}/comments?per_page=100&page={page}"),
            &[], token).await.map_err(|_| "foreman_backlog_unknown")?;
        let rows = rows.as_array().ok_or("foreman_backlog_unknown")?;
        for row in rows {
            comments.push(coder::claim::Comment {
                body: row["body"]
                    .as_str()
                    .ok_or("foreman_backlog_unknown")?
                    .to_owned(),
                at: row["created_at"]
                    .as_str()
                    .and_then(coder::claim::iso_seconds)
                    .ok_or("foreman_backlog_unknown")?,
            });
        }
        if rows.len() < 100 {
            return Ok(coder::claim::held(
                issue,
                &comments,
                &[],
                crate::chat_store::now_unix(),
                coder_lease::claims::DEFAULT_CLAIM_AGE.as_secs() / 3600,
                &coder::claim::Project::default(),
            )
            .is_some());
        }
    }
    Err("foreman_backlog_unknown")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn import_and_reserved_dispatch_survive_restarts_and_lost_replies() {
        let directory = tempfile::tempdir().unwrap();
        let store = Store::local(directory.path().to_owned());
        let owner = crate::chat_store::account_owner("objective-fixture");
        let definition = crate::custom_agents::mutate(
            &store,
            &owner,
            crate::custom_agents::Mutation::Create {
                request_id: "1".repeat(64),
                spec: worker_spec(),
            },
        )
        .await
        .unwrap()
        .unwrap();
        let catalog = import(&store).await.unwrap();
        assert_eq!(catalog.revision(), import(&store).await.unwrap().revision());
        let first = next(&store, 1000, |_| None).await.unwrap().unwrap();
        let worker = (
            owner.clone(),
            definition.id.clone(),
            "fixture-host".into(),
            Some("fixture-project".into()),
        );
        assert_eq!(
            start(&store, &first, worker.clone(), 1000, false).await,
            Err("foreman_backlog_unknown")
        );
        assert!(entries(&store).await.unwrap().is_empty());
        let (left, right) = tokio::join!(
            start(&store, &first, worker.clone(), 1000, true),
            start(&store, &first, worker.clone(), 1000, true)
        );
        let initial = left.unwrap();
        assert_eq!(initial, right.unwrap());
        // Lose both the HTTP reply and the normal audit entry; reopen the store.
        let reopened = Store::local(directory.path().to_owned());
        assert_eq!(
            next(&reopened, 1001, |_| None).await.unwrap(),
            Some(first.clone())
        );
        let recovered = start(&reopened, &first, worker.clone(), 1002, false)
            .await
            .unwrap();
        assert_eq!(initial, recovered);
        let (book, _) = crate::agent_assignments::load(&reopened, &owner)
            .await
            .unwrap();
        assert_eq!(book.assignments.len(), 1);
        assert_eq!(entries(&reopened).await.unwrap().len(), 1);
        assert!(
            next(&reopened, 1003, |_| Some(AssignmentState::Running))
                .await
                .unwrap()
                .is_none()
        );
        let following = next(&reopened, 1004, |_| Some(AssignmentState::Completed))
            .await
            .unwrap()
            .unwrap();
        assert_ne!(first.objective, following.objective);
        assert_ne!(first.request_id, following.request_id);
        let mut stale = following.clone();
        stale.revision = "0".repeat(64);
        assert_eq!(
            start(&reopened, &stale, worker, 1005, true).await,
            Err("foreman_objective_conflict")
        );
        let (_, generation) = load(&reopened).await.unwrap();
        reopened
            .write_key(BOOK_KEY, b"bad json".to_vec(), generation.as_deref())
            .await
            .unwrap();
        assert!(next(&reopened, 1006, |_| None).await.is_err());
    }

    #[test]
    fn selections_are_stable_fair_and_bounded() {
        let c = Catalog::parse(CATALOG.as_bytes()).unwrap();
        let empty = Attempts::default();
        let first = choose(&c, &empty, 1000, |_| None).unwrap();
        assert_eq!(Some(first.clone()), choose(&c, &empty, 1001, |_| None));
        let mut c2 = c.clone();
        c2.objectives
            .iter_mut()
            .for_each(|o| o.desired_state = "proposed".into());
        assert!(choose(&c2, &empty, 1000, |_| None).is_none());
        assert_ne!(first.revision, c2.revision());
    }
}
