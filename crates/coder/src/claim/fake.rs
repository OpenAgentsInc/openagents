//! An in-memory GitHub for claim tests: issues with comments and
//! assignees, optionally on one project with a status field, and the
//! repository's project board. Every write lands in its state, so a test
//! reads back what a claim wrote.

use std::collections::BTreeMap;
use std::sync::Mutex;

use super::{Comment, Hub, Item, Project, Queued};

/// One issue's state.
#[derive(Clone, Debug, Default)]
pub struct State {
    pub comments: Vec<Comment>,
    pub assignees: Vec<String>,
    pub labels: Vec<String>,
    pub open: bool,
    /// The issue's status on the project, when it is on it.
    pub on_project: bool,
    pub status: Option<String>,
    pub status_at: u64,
    /// Issues that block this one.
    pub blocked_by: Vec<u64>,
}

/// GitHub as a claim sees it.
#[derive(Debug)]
pub struct Fake {
    pub viewer: String,
    /// The project's status options; `None` is a repository without a project.
    pub options: Option<Vec<String>>,
    pub issues: Mutex<BTreeMap<u64, State>>,
    /// Project order of the issues on the project.
    pub order: Mutex<Vec<u64>>,
    /// The clock writes are stamped with.
    pub now: u64,
}

impl Fake {
    /// A repository without a project.
    #[must_use]
    pub fn plain(viewer: &str) -> Self {
        Fake {
            viewer: viewer.into(),
            options: None,
            issues: Mutex::new(BTreeMap::new()),
            order: Mutex::new(Vec::new()),
            now: 1_000,
        }
    }

    /// A repository with one project whose status field has `options`.
    #[must_use]
    pub fn with_project(viewer: &str, options: &[&str]) -> Self {
        Fake {
            options: Some(options.iter().map(|option| (*option).to_owned()).collect()),
            ..Fake::plain(viewer)
        }
    }

    /// Adds an open issue; on the project with `status` when the
    /// repository has one and `status` is given.
    pub fn issue(&self, number: u64, status: Option<&str>, labels: &[&str], blocked_by: &[u64]) {
        let on_project = self.options.is_some() && status.is_some();
        self.issues.lock().unwrap().insert(
            number,
            State {
                open: true,
                on_project,
                status: status.map(str::to_owned),
                labels: labels.iter().map(|label| (*label).to_owned()).collect(),
                blocked_by: blocked_by.to_vec(),
                ..State::default()
            },
        );
        if on_project {
            self.order.lock().unwrap().push(number);
        }
    }

    /// One issue's state.
    #[must_use]
    pub fn state(&self, number: u64) -> State {
        self.issues
            .lock()
            .unwrap()
            .get(&number)
            .cloned()
            .unwrap_or_default()
    }

    fn with<T>(&self, number: u64, f: impl FnOnce(&mut State) -> T) -> Result<T, String> {
        self.issues
            .lock()
            .unwrap()
            .get_mut(&number)
            .map(f)
            .ok_or_else(|| format!("no issue #{number}"))
    }
}

impl Hub for Fake {
    fn comment(&self, _: &str, number: u64, body: &str) -> Result<(), String> {
        let at = self.now;
        self.with(number, |state| {
            state.comments.push(Comment {
                body: body.into(),
                at,
            });
        })
    }

    fn comments(&self, _: &str, number: u64) -> Result<Vec<Comment>, String> {
        self.with(number, |state| state.comments.clone())
    }

    fn labeled(&self, _: &str, label: &str) -> Result<Vec<u64>, String> {
        Ok(self
            .issues
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, state)| state.open && state.labels.iter().any(|have| have == label))
            .map(|(number, _)| *number)
            .collect())
    }

    fn viewer(&self) -> Result<String, String> {
        Ok(self.viewer.clone())
    }

    fn assign(&self, _: &str, number: u64, login: &str, add: bool) -> Result<(), String> {
        self.with(number, |state| {
            state.assignees.retain(|have| have != login);
            if add {
                state.assignees.push(login.into());
            }
        })
    }

    fn items(&self, _: &str, number: u64, field: &str) -> Result<Vec<Item>, String> {
        let Some(options) = &self.options else {
            return Ok(Vec::new());
        };
        let has_field = field == Project::default().field;
        self.with(number, |state| {
            if !state.on_project {
                return Vec::new();
            }
            vec![Item {
                project: "project-1".into(),
                project_title: "Board".into(),
                item: format!("item-{number}"),
                field: has_field.then(|| "field-status".into()),
                options: options
                    .iter()
                    .map(|name| (name.clone(), format!("option-{name}")))
                    .collect(),
                status: state.status.clone(),
                status_at: state.status_at,
            }]
        })
    }

    fn set_status(&self, item: &Item, option: &str) -> Result<(), String> {
        let number: u64 = item
            .item
            .strip_prefix("item-")
            .and_then(|number| number.parse().ok())
            .ok_or("no such item")?;
        let name = option
            .strip_prefix("option-")
            .ok_or("no such option")?
            .to_owned();
        let at = self.now;
        self.with(number, |state| {
            state.status = Some(name);
            state.status_at = at;
        })
    }

    fn board(&self, _: &str, _: &Project) -> Result<Option<Vec<Queued>>, String> {
        if self.options.is_none() {
            return Ok(None);
        }
        let issues = self.issues.lock().unwrap();
        Ok(Some(
            self.order
                .lock()
                .unwrap()
                .iter()
                .filter_map(|number| {
                    let state = issues.get(number)?;
                    Some(Queued {
                        number: *number,
                        open: state.open,
                        status: state.status.clone(),
                        labels: state.labels.clone(),
                        blocked: state
                            .blocked_by
                            .iter()
                            .any(|blocker| issues.get(blocker).is_none_or(|b| b.open)),
                    })
                })
                .collect(),
        ))
    }
}
