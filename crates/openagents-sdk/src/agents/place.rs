//! Where a saved agent runs when asked: an online computer of the account
//! with the agent's runtime and a project on it (#11934). The CLI's
//! `openagents agents ask` and the website chat's agents door choose the
//! same way.
use super::AgentDefinition;
use crate::agent_lab::Host;

/// Why no computer can run the agent now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unplaced {
    /// No online computer has the agent's runtime (or, for a Rust runner,
    /// its installed runner), or none matches the named computer.
    NoComputer,
    /// The computer is online but has no project, or none matches the
    /// named project.
    NoProject,
}

/// Choose the computer and project. `computer` and `project` narrow the
/// choice to an exact name or ID (names compare without regard to case).
/// Without `project`, the project named `prefer` comes first, then the
/// computer seen most recently and its first project.
pub fn place(
    hosts: &[Host],
    agent: &AgentDefinition,
    computer: Option<&str>,
    project: Option<&str>,
    prefer: Option<&str>,
) -> Result<(Host, String), Unplaced> {
    let same = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
    let runs = |h: &&Host| {
        if agent.spec.runtime == "sdk" {
            agent
                .spec
                .model
                .as_ref()
                .is_some_and(|m| h.runners.contains(m))
        } else {
            h.runtimes.iter().any(|r| *r == agent.spec.runtime)
        }
    };
    let mut fit: Vec<&Host> = hosts
        .iter()
        .filter(runs)
        .filter(|h| computer.is_none_or(|c| h.id == c || same(&h.name, c)))
        .collect();
    if fit.is_empty() {
        return Err(Unplaced::NoComputer);
    }
    fit.sort_by(|a, b| b.seen_ms.cmp(&a.seen_ms));
    let pick = |h: &Host, wanted: &str| {
        h.projects
            .iter()
            .find(|p| p.id == wanted || same(&p.name, wanted))
            .map(|p| p.id.clone())
    };
    if let Some(wanted) = project {
        return fit
            .iter()
            .find_map(|h| pick(h, wanted).map(|p| ((*h).clone(), p)))
            .ok_or(Unplaced::NoProject);
    }
    if let Some(wanted) = prefer
        && let Some(found) = fit
            .iter()
            .find_map(|h| pick(h, wanted).map(|p| ((*h).clone(), p)))
    {
        return Ok(found);
    }
    fit.iter()
        .find_map(|h| h.projects.first().map(|p| ((*h).clone(), p.id.clone())))
        .ok_or(Unplaced::NoProject)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_lab::Project;
    use crate::agents::{Access, AgentSpec};

    fn agent(runtime: &str, model: Option<&str>) -> AgentDefinition {
        AgentDefinition {
            id: "a".repeat(64),
            revision: 1,
            spec: AgentSpec {
                name: "Foreman".into(),
                description: String::new(),
                instructions: "Look.".into(),
                runtime: runtime.into(),
                model: model.map(Into::into),
                access: Access::HostDefault,
            },
            deleted: false,
        }
    }
    fn host(id: &str, seen: u64, runtimes: &[&str], projects: &[&str]) -> Host {
        Host {
            id: id.into(),
            name: format!("{id} Mac"),
            projects: projects
                .iter()
                .map(|p| Project {
                    id: format!("{id}-{p}"),
                    name: (*p).into(),
                })
                .collect(),
            runtimes: runtimes.iter().map(|r| (*r).into()).collect(),
            runners: vec!["reviewer".into()],
            tools: vec![],
            seen_ms: seen,
        }
    }

    #[test]
    fn chooses_a_computer_with_the_runtime_and_the_preferred_project() {
        let hosts = vec![
            host("old", 1, &["claude-code"], &["openagents"]),
            host("new", 2, &["claude-code"], &["scratch"]),
            host("codex", 3, &["codex"], &["openagents"]),
        ];
        let a = agent("claude-code", None);
        let at = |c, p, prefer| place(&hosts, &a, c, p, prefer).map(|(h, p)| (h.id, p));
        assert_eq!(
            at(None, None, Some("openagents")),
            Ok(("old".into(), "old-openagents".into()))
        );
        assert_eq!(
            at(None, None, None),
            Ok(("new".into(), "new-scratch".into()))
        );
        assert_eq!(
            at(Some("OLD MAC"), None, None),
            Ok(("old".into(), "old-openagents".into()))
        );
        assert_eq!(
            at(None, Some("Scratch"), None),
            Ok(("new".into(), "new-scratch".into()))
        );
        assert_eq!(at(None, Some("missing"), None), Err(Unplaced::NoProject));
        assert_eq!(at(Some("codex"), None, None), Err(Unplaced::NoComputer));
        assert_eq!(
            place(&[host("x", 1, &["claude-code"], &[])], &a, None, None, None)
                .map(|(h, p)| (h.id, p)),
            Err(Unplaced::NoProject)
        );
        assert!(place(&hosts, &agent("sdk", Some("reviewer")), None, None, None).is_ok());
        assert_eq!(
            place(&hosts, &agent("sdk", Some("other")), None, None, None).map(|(h, p)| (h.id, p)),
            Err(Unplaced::NoComputer)
        );
    }
}
