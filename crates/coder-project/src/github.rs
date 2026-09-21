//! The project controller consumes Coder's approved tracker adapter.

use coder::{capability, tracker};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    pub owner: String,
    pub repository: String,
    pub project: u32,
}

impl Scope {
    pub fn validate(&self) -> Result<(), String> {
        for name in [&self.owner, &self.repository] {
            if name.is_empty()
                || name.len() > 100
                || !name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
            {
                return Err(
                    "GitHub scope names must be bounded ASCII repository identifiers".into(),
                );
            }
        }
        if self.project == 0 {
            return Err("GitHub project number must be positive".into());
        }
        Ok(())
    }
    #[must_use]
    pub fn repository_name(&self) -> String {
        format!("{}/{}", self.owner, self.repository)
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Blocker {
    pub repository: String,
    pub number: u64,
    pub closed: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct Issue {
    pub number: u64,
    pub updated_at: String,
    pub body_digest: String,
    pub closed: bool,
    pub blockers: Vec<Blocker>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Snapshot {
    pub issues: BTreeMap<u64, Issue>,
    pub source_digest: String,
    pub default_branch_revision: String,
    pub ignored_non_issues: usize,
    pub ignored_other_repositories: usize,
}

/// Acquire through the pinned `github-project` capability and project scope.
pub async fn fetch(scope: &Scope, repository: &Path) -> Result<Snapshot, String> {
    scope.validate()?;
    let scope = scope.clone();
    let repository = repository.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let entry = capability::Entry::load(
            &repository.join("capabilities/github-project.json"),
            capability::Source::Repository,
        )?;
        let scope_record =
            tracker::Scope::github(&scope.owner, &scope.repository, u64::from(scope.project));
        let mut environment: Vec<_> = ["GH_TOKEN", "GITHUB_TOKEN"]
            .into_iter()
            .filter_map(|name| std::env::var(name).ok().map(|value| (name.into(), value)))
            .collect();
        environment.extend([
            ("GH_HOST".into(), "github.com".into()),
            ("GH_PROMPT_DISABLED".into(), "1".into()),
            ("GH_DEBUG".into(), String::new()),
        ]);
        let pinned = tracker::github::acquire(
            &entry,
            &capability::Trust::operator(),
            &repository,
            &scope_record,
            &tracker::Limits::bounded(),
            &tracker::TaskMap::empty(),
            &environment,
        )?;
        pinned.check_scope(&scope_record)?;
        let mut snapshot = Snapshot {
            issues: BTreeMap::new(),
            source_digest: pinned.digest(),
            default_branch_revision: pinned.base.revision.clone(),
            ignored_non_issues: pinned.skipped.len(),
            ignored_other_repositories: 0,
        };
        for issue in pinned.issues {
            if issue.repo != scope.repository_name() {
                snapshot.ignored_other_repositories += 1;
                continue;
            }
            if !issue.blocked_by_complete {
                return Err("project dependency observations are incomplete".into());
            }
            let blockers = issue
                .blocked_by
                .into_iter()
                .map(|dependency| Blocker {
                    repository: dependency.repo.unwrap_or_else(|| scope.repository_name()),
                    number: dependency.number,
                    closed: dependency.state == "closed",
                })
                .collect();
            snapshot.issues.insert(
                issue.number,
                Issue {
                    number: issue.number,
                    updated_at: issue.updated,
                    body_digest: issue.body_digest,
                    closed: issue.state == "closed",
                    blockers,
                },
            );
        }
        Ok(snapshot)
    })
    .await
    .map_err(|_| "project acquisition worker stopped".to_string())?
}
