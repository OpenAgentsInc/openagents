//! `evidence.probes.planner`: which typed, read-only operations the host
//! runs before the executor starts.
//!
//! Probing is two components, measured apart: this planner chooses what
//! to run, and the capture selector
//! ([`crate::component::evidence::probe_keep`]) chooses which finished
//! observations reach the briefing. A better selector cannot recover the
//! cost of probes that already ran.
//!
//! The planner reads [`Facts`], which [`facts`] gathers from directory
//! entries and metadata alone, and returns [`Planned`] operations; nothing
//! runs until [`crate::ops::run_all`]. [`plan`] is a function from a
//! serializable input to a serializable output, so it runs on a fixture
//! without an episode.

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::ops::{GitQuery, ListFilter, Operation, Scope, ToolQuery};
use crate::record::Implementation;

/// What a path the instruction names is on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Dir,
    File,
    Missing,
}

/// One path the instruction names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Named {
    pub path: String,
    pub kind: Kind,
    /// Whether it is inside a Git work tree.
    pub repo: bool,
}

/// What the planner knows about the workspace, gathered without running
/// anything that could change it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Facts {
    pub workdir: String,
    pub is_git: bool,
    /// README files at the top of the working directory.
    pub readmes: Vec<String>,
    pub named: Vec<Named>,
}

/// The most paths from the instruction the planner probes.
pub const NAMED_MAX: usize = 6;

/// Gathers [`Facts`] by reading directory entries and metadata only.
#[must_use]
pub fn facts(workdir: &Path, instruction: &str) -> Facts {
    let mut readmes: Vec<String> = std::fs::read_dir(workdir)
        .map(|entries| {
            entries
                .flatten()
                .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .filter(|name| name.to_lowercase().starts_with("readme"))
                .collect()
        })
        .unwrap_or_default();
    readmes.sort();
    let named = crate::ops::named_paths(instruction, NAMED_MAX)
        .into_iter()
        .map(|path| {
            let on_disk = Path::new(&path);
            let kind = if on_disk.is_dir() {
                Kind::Dir
            } else if on_disk.is_file() {
                Kind::File
            } else {
                Kind::Missing
            };
            let repo = kind == Kind::Dir && in_work_tree(on_disk);
            Named { path, kind, repo }
        })
        .collect();
    Facts {
        workdir: workdir.to_string_lossy().into_owned(),
        is_git: in_work_tree(workdir),
        readmes,
        named,
    }
}

/// Whether `dir` is inside a Git work tree, found by looking for a `.git`
/// entry in it or an ancestor, without running Git.
fn in_work_tree(dir: &Path) -> bool {
    let mut current = dir.canonicalize().ok();
    while let Some(path) = current {
        if path.join(".git").exists() {
            return true;
        }
        current = path.parent().map(Path::to_path_buf);
    }
    false
}

/// The planner's parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanParams {
    /// Probe v2: Git questions in repositories the instruction names.
    pub v2: bool,
    /// Whether to list the working directory one level deep beside its
    /// three-level listing, as the original battery did.
    pub shallow_listing: bool,
}

impl Default for PlanParams {
    fn default() -> Self {
        Self {
            v2: true,
            shallow_listing: false,
        }
    }
}

/// The planner's identity and parameters, digested.
#[must_use]
pub fn implementation(params: PlanParams) -> Implementation {
    Implementation::new(
        "evidence.probes.planner",
        "typed battery v2",
        &json!({
            "params": params,
            "named_max": NAMED_MAX,
            "operations": "list depth 3 (150), readme heads (120 lines), test listing depth 4 (40), python3 --version, pip list (60), git status/branches/log 40/reflog 40/stashes in a work tree, named directories listed (200) and, with v2, their repositories' git state, named files read (200 lines)",
        }),
    )
}

/// One operation the planner chose, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Planned {
    pub id: String,
    pub operation: Operation,
    pub reason: String,
}

/// The probe planner: the operations to run, in order.
#[must_use]
pub fn plan(facts: &Facts, params: PlanParams) -> Vec<Planned> {
    let mut out: Vec<Planned> = Vec::new();
    let mut add = |operation: Operation, reason: &str| {
        if out.iter().any(|p| p.operation == operation) {
            return;
        }
        out.push(Planned {
            id: format!("op_{}", out.len()),
            operation,
            reason: reason.to_string(),
        });
    };
    if params.shallow_listing {
        add(
            Operation::List {
                path: facts.workdir.clone(),
                depth: 1,
                max_entries: 200,
                filter: ListFilter::All,
            },
            "the working directory's top level",
        );
    }
    add(
        Operation::List {
            path: facts.workdir.clone(),
            depth: 3,
            max_entries: 150,
            filter: ListFilter::All,
        },
        "the working directory's layout",
    );
    for readme in &facts.readmes {
        add(
            Operation::Read {
                path: readme.clone(),
                max_lines: 120,
                max_bytes: 32 * 1024,
            },
            "the repository's own description",
        );
    }
    add(
        Operation::List {
            path: facts.workdir.clone(),
            depth: 4,
            max_entries: 40,
            filter: ListFilter::Tests,
        },
        "tests the task may be checked against",
    );
    add(
        Operation::Tool {
            query: ToolQuery::Python,
        },
        "the Python version",
    );
    add(
        Operation::Tool {
            query: ToolQuery::PipList { max_lines: 60 },
        },
        "installed Python packages",
    );
    if facts.is_git {
        for query in git_battery() {
            add(
                Operation::Git {
                    repo: ".".to_string(),
                    query,
                },
                "the working directory's Git state",
            );
        }
    }
    for named in &facts.named {
        match named.kind {
            Kind::Dir => {
                add(
                    Operation::List {
                        path: named.path.clone(),
                        depth: 1,
                        max_entries: 200,
                        filter: ListFilter::All,
                    },
                    "a directory the task names",
                );
                if params.v2 && named.repo && !facts.is_git {
                    for query in [
                        GitQuery::Status,
                        GitQuery::Log { count: 40 },
                        GitQuery::Reflog { count: 40 },
                        GitQuery::Branches,
                    ] {
                        add(
                            Operation::Git {
                                repo: named.path.clone(),
                                query,
                            },
                            "a repository the task names",
                        );
                    }
                }
            }
            Kind::File => add(
                Operation::Read {
                    path: named.path.clone(),
                    max_lines: 200,
                    max_bytes: 64 * 1024,
                },
                "a file the task names",
            ),
            Kind::Missing => {}
        }
    }
    out
}

fn git_battery() -> [GitQuery; 5] {
    [
        GitQuery::Status,
        GitQuery::Branches,
        GitQuery::Log { count: 40 },
        GitQuery::Reflog { count: 40 },
        GitQuery::Stashes,
    ]
}

/// The scope a plan runs under: the working directory, plus every existing
/// path the instruction names, for reading only.
#[must_use]
pub fn scope(facts: &Facts, workdir: &Path) -> Scope {
    let mut scope = Scope::new(workdir);
    for named in &facts.named {
        match named.kind {
            Kind::Dir => scope.allow_read(Path::new(&named.path)),
            Kind::File => scope.allow_read(Path::new(&named.path)),
            Kind::Missing => {}
        }
    }
    scope
}

/// The planner's input as a fixture holds it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanInput {
    pub facts: Facts,
    #[serde(default)]
    pub params: PlanParams,
}

/// What the probe battery's command strings become under the typed
/// planner: the labels of the equivalent operations, or `None` when the
/// planner drops the command on purpose because another operation covers
/// it.
#[must_use]
pub fn equivalent(command: &str, workdir: &str) -> Option<Vec<String>> {
    let labels = match command {
        "pwd && ls -la" => return None,
        c if c.starts_with("find . -maxdepth 3 ") => vec![format!("list {workdir} (depth 3)")],
        c if c.starts_with("for f in README*") => vec!["read README".to_string()],
        c if c.starts_with("find . -maxdepth 4 ") => {
            vec![format!("list test files under {workdir} (depth 4)")]
        }
        c if c.starts_with("python3 --version") => {
            vec!["python3 --version".to_string(), "pip list".to_string()]
        }
        c if c.starts_with("ls -la ") => vec![format!("list {}", &c["ls -la ".len()..])],
        c if c.starts_with("head -200 ") => {
            vec![format!(
                "read {} (first 200 lines)",
                &c["head -200 ".len()..]
            )]
        }
        other => vec![other.to_string()],
    };
    Some(labels)
}

/// The facts a retained probe battery implies, for a fixture: the working
/// directory from `pwd`, a work tree when `git status` ran, a README when
/// the README probe printed something, and each named path's kind from
/// whether it was listed, read, or asked about with Git. A named path the
/// battery neither listed nor read reads as missing.
#[must_use]
pub fn facts_from_battery(instruction: &str, probes: &[(String, String)]) -> Facts {
    let ran = |command: &str| probes.iter().any(|(c, _)| c == command);
    let workdir = probes
        .iter()
        .find(|(c, _)| c == "pwd && ls -la")
        .and_then(|(_, output)| output.lines().next())
        .map_or("/app".to_string(), |line| line.trim().to_string());
    let readmes = if probes
        .iter()
        .any(|(c, _)| c.starts_with("for f in README*"))
    {
        vec!["README".to_string()]
    } else {
        Vec::new()
    };
    let named = crate::ops::named_paths(instruction, NAMED_MAX)
        .into_iter()
        .map(|path| {
            let kind = if ran(&format!("ls -la {path}")) {
                Kind::Dir
            } else if ran(&format!("head -200 {path}")) {
                Kind::File
            } else {
                Kind::Missing
            };
            let repo = ran(&format!("git -C {path} status"));
            Named { path, kind, repo }
        })
        .collect();
    Facts {
        workdir,
        is_git: ran("git status"),
        readmes,
        named,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_planner_probes_what_the_workspace_and_task_name() {
        let (dir, _) = crate::collect::tests::fixture();
        let data = dir.path().join("data");
        let instruction = format!(
            "Count the errors in {} and write /nonexistent/out.csv. See {}/log.txt.",
            data.display(),
            data.display()
        );
        let facts = facts(dir.path(), &instruction);
        assert!(facts.is_git);
        assert_eq!(facts.readmes, ["README.md"]);
        assert_eq!(facts.named.len(), 3);
        assert_eq!(facts.named[1].kind, Kind::Missing);
        let planned = plan(&facts, PlanParams::default());
        let labels: Vec<String> = planned.iter().map(|p| p.operation.label()).collect();
        assert!(labels.iter().any(|l| l == "git status"));
        assert!(labels.iter().any(|l| l.starts_with("read README.md")));
        assert!(labels.iter().any(|l| l.contains("log.txt")));
        assert!(!labels.iter().any(|l| l.contains("nonexistent")));
        // No shallow listing beside the deep one unless asked.
        assert_eq!(
            labels
                .iter()
                .filter(|l| l.starts_with("list ") && !l.contains("test"))
                .count(),
            2
        );
        // Every planned path is inside the plan's scope.
        let scope = scope(&facts, dir.path());
        for p in &planned {
            assert!(scope.check(&p.operation).is_ok(), "{}", p.operation.label());
        }
    }
}
