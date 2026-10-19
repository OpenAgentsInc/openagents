//! Where a saved agent runs when asked: an online computer of the account
//! with the agent's runtime and a project on it (#11934). The CLI's
//! `openagents agents ask` and the website chat's agents door choose the
//! same way.
use super::AgentDefinition;
use crate::agent_lab::Host;

/// Runtimes a host may pick for an agent that leaves the choice to it.
pub const HOST_DEFAULTS: [&str; 3] = ["claude-code", "codex", "opencode"];

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
    // The position of the first runtime in the agent's order this computer
    // has: computers with an earlier choice come first.
    let order = agent.spec.runtime_order();
    let rank = |h: &Host| -> Option<usize> {
        if agent.spec.runtime == "sdk" {
            agent
                .spec
                .model
                .as_ref()
                .is_some_and(|m| h.runners.contains(m))
                .then_some(0)
        } else if order.is_empty() {
            h.runtimes
                .iter()
                .any(|r| HOST_DEFAULTS.contains(&r.as_str()))
                .then_some(0)
        } else {
            order
                .iter()
                .position(|c| h.runtimes.iter().any(|r| *r == c.runtime))
        }
    };
    // An agent that wakes on its own belongs on a computer that stays up
    // (#12050), so a desktop restart can't end its check-in.
    let keeps_running = agent.spec.autonomy.as_ref().is_some_and(|a| {
        a.check_ins || a.authority || a.every_minutes.is_some() || !a.wakes.is_empty()
    });
    let mut fit: Vec<(usize, &Host)> = hosts
        .iter()
        .filter(|h| computer.is_none_or(|c| h.id == c || same(&h.name, c)))
        .filter_map(|h| rank(h).map(|r| (r, h)))
        .collect();
    if fit.is_empty() {
        return Err(Unplaced::NoComputer);
    }
    fit.sort_by(|a, b| {
        let up = |h: &Host| !(keeps_running && h.always_on);
        up(a.1)
            .cmp(&up(b.1))
            .then(a.0.cmp(&b.0))
            .then(b.1.seen_ms.cmp(&a.1.seen_ms))
    });
    let fit: Vec<&Host> = fit.into_iter().map(|(_, h)| h).collect();
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
                fallbacks: vec![],
                autonomy: None,
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
            always_on: false,
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

    #[test]
    fn computers_with_an_earlier_runtime_come_first_and_any_takes_a_default() {
        let hosts = vec![
            host("codex", 3, &["codex"], &["openagents"]),
            host("claude", 1, &["claude-code"], &["openagents"]),
            host("cursor", 5, &["cursor"], &["openagents"]),
        ];
        let mut a = agent("claude-code", None);
        a.spec.fallbacks = vec![crate::agents::RuntimeChoice {
            runtime: "codex".into(),
            model: None,
        }];
        let id = |a: &AgentDefinition, hosts: &[Host]| {
            place(hosts, a, None, None, None).map(|(h, _)| h.id)
        };
        assert_eq!(id(&a, &hosts), Ok("claude".into()));
        assert_eq!(id(&a, &hosts[..1]), Ok("codex".into()));
        assert_eq!(id(&a, &hosts[2..]), Err(Unplaced::NoComputer));
        let any = agent(crate::agents::ANY_RUNTIME, None);
        assert_eq!(id(&any, &hosts), Ok("codex".into()));
        assert_eq!(id(&any, &hosts[2..]), Err(Unplaced::NoComputer));
    }

    #[test]
    fn an_agent_that_wakes_on_its_own_goes_to_a_computer_that_stays_up() {
        let mut cloud = host("cloud", 1, &["claude-code"], &["openagents"]);
        cloud.always_on = true;
        let hosts = vec![host("mac", 9, &["claude-code"], &["openagents"]), cloud];
        let id = |a: &AgentDefinition| place(&hosts, a, None, None, None).map(|(h, _)| h.id);
        let mut a = agent("claude-code", None);
        assert_eq!(id(&a), Ok("mac".into()));
        a.spec.autonomy = Some(crate::agents::Autonomy {
            check_ins: true,
            ..Default::default()
        });
        assert_eq!(id(&a), Ok("cloud".into()));
        // Naming the computer still wins.
        let named = place(&hosts, &a, Some("mac"), None, None).map(|(h, _)| h.id);
        assert_eq!(named, Ok("mac".into()));
    }
}
