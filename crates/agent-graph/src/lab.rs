//! An Agent Lab experiment as a graph.
//!
//! [`of`] works for any experiment: the agent's runtime is the center, its
//! instructions and check-ins feed it, the places it reads send it facts,
//! its tool calls pass through its authority, its decisions go to memory and
//! back, and its replies go to the suite that tests it. A part the
//! definition doesn't have is left out. [`Notes`] adds what a definition
//! can't say about itself, such as the lessons a part answers.

use agent_contract::{Access, GrantScope, NEVER_GRANTED_REFUSAL, WakeEvent};
use agent_lab::{Check, Component, Experiment};

use crate::catalog::{self, capitalized, named, plain, plain_line};
use crate::{Accent, Detail, GraphSpec, Link, Unit};

/// The stall window a computer uses when a definition sets none
/// (`DEFAULT_STALL_MINUTES` in the app's `agent_wakes`).
pub const HOST_STALL_MINUTES: u32 = 20;

/// What a graph adds to a definition.
#[derive(Clone, Debug, Default)]
pub struct Notes {
    /// Labels for a unit, by unit ID.
    pub tags: Vec<(String, Vec<String>)>,
    /// More units, such as a replay inside `tests` or a `measures` unit.
    pub units: Vec<Unit>,
    pub links: Vec<Link>,
}

/// The events that wake an agent, in plain words.
pub fn wake_words(event: WakeEvent) -> &'static str {
    match event {
        WakeEvent::ChatFinished => "a chat finishes",
        WakeEvent::ChatErrored => "a chat errors",
        WakeEvent::ChatNeedsAnswer => "a chat needs an answer",
        WakeEvent::IssueOpened => "an issue opens",
        WakeEvent::IssueClosed => "an issue closes",
        WakeEvent::ClaimStale => "a claim goes stale",
        WakeEvent::ChatStalled => "a chat stalls",
        WakeEvent::ClaimNoProgress => "a claim makes no progress",
        WakeEvent::Other => "something newer",
    }
}

/// The wakes a computer sends when a definition names none.
pub const DEFAULT_WAKES: [WakeEvent; 8] = [
    WakeEvent::ChatFinished,
    WakeEvent::ChatErrored,
    WakeEvent::ChatNeedsAnswer,
    WakeEvent::IssueOpened,
    WakeEvent::IssueClosed,
    WakeEvent::ClaimStale,
    WakeEvent::ChatStalled,
    WakeEvent::ClaimNoProgress,
];

/// A grant in plain words.
pub fn grant_words(scope: GrantScope) -> &'static str {
    match scope {
        GrantScope::ChatMessage => "Message a chat",
        GrantScope::ChatStart => "Start a chat",
        GrantScope::ChatSteer => "Steer a chat",
        GrantScope::ChatStop => "Stop a chat",
        GrantScope::LandSubmit => "Send a change to the landing queue",
        GrantScope::WorkDispatch => "Start issue work on the pool",
        GrantScope::LogWrite => "Keep its decision log",
    }
}

/// A length of time in plain words.
pub fn minutes(seconds: u64) -> String {
    if seconds % 60 == 0 {
        format!("{} minutes", seconds / 60)
    } else {
        format!("{seconds} seconds")
    }
}

/// The skills and memory texts of an experiment, by name.
fn skills(experiment: &Experiment) -> Vec<(&str, &str)> {
    experiment
        .draft
        .components
        .iter()
        .filter_map(|c| match c {
            Component::Skill { name, text, .. } => Some((name.as_str(), text.as_str())),
            _ => None,
        })
        .collect()
}

/// All of an agent's own text: its instructions and every skill or memory.
pub fn text(experiment: &Experiment) -> String {
    let mut text = experiment.draft.spec.instructions.clone();
    for component in &experiment.draft.components {
        if let Component::Skill { text: t, .. } | Component::Memory { text: t, .. } = component {
            text.push('\n');
            text.push_str(t);
        }
    }
    text
}

/// A unit ID from a name: lower case, words joined by `-`.
fn slug(name: &str) -> String {
    name.split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join("-")
}

/// The numbered steps of a stall skill: "1. **Message it** (rung 1, ...):
/// ask what it needs." gives ("Message it", "Ask what it needs.").
pub(crate) fn rungs(skill: &str) -> Vec<(String, String)> {
    skill
        .lines()
        .filter_map(|line| {
            let (n, rest) = line.split_once(". ")?;
            n.trim().parse::<u8>().ok()?;
            let title = rest
                .strip_prefix("**")
                .and_then(|r| r.split_once("**"))
                .map(|(t, _)| t.to_owned());
            let what = rest.split_once("): ").map(|(_, w)| w).unwrap_or(rest);
            Some((
                title.unwrap_or_else(|| plain_line(rest)),
                capitalized(&plain_line(what)),
            ))
        })
        .collect()
}

/// `experiment` as a graph under `id`.
pub fn of(experiment: &Experiment, id: &str, notes: &Notes) -> GraphSpec {
    let spec = &experiment.draft.spec;
    let text = text(experiment);
    let autonomy = spec.autonomy.clone().unwrap_or_default();
    let skills = skills(experiment);
    let model = spec
        .model
        .clone()
        .unwrap_or_else(|| "the runtime's own".into());
    let mut graph = GraphSpec::new(id, spec.name.clone(), spec.description.clone());
    let mut runtime = Unit::new(
        "runtime",
        spec.name.clone(),
        format!("{} · {model}", spec.runtime),
        Accent::Core,
    );

    // Check-ins wake the agent on a schedule and on events.
    let checks_in = autonomy.check_ins || !autonomy.wakes.is_empty();
    if checks_in {
        let wakes: Vec<&str> = if autonomy.wakes.is_empty() {
            DEFAULT_WAKES.iter().map(|w| wake_words(*w)).collect()
        } else {
            autonomy.wakes.iter().map(|w| wake_words(*w)).collect()
        };
        let schedule = match (autonomy.check_ins, autonomy.every_minutes) {
            (true, Some(m)) => format!("Every {m} minutes"),
            (true, None) => "On the computer's schedule".to_owned(),
            (false, _) => "Off".to_owned(),
        };
        let mut unit = Unit::new(
            "check-ins",
            "Check-ins",
            autonomy
                .every_minutes
                .map_or_else(|| "when asked".to_owned(), |m| format!("every {m} minutes")),
            Accent::Info,
        )
        .output("wake", "wake", "wake")
        .detail(Detail::fact("On a schedule", schedule));
        if let Some(secs) = autonomy.coalesce_secs {
            unit = unit.detail(Detail::fact("Waits for quiet", minutes(u64::from(secs))));
        }
        unit = unit
            .detail(Detail::Chips {
                label: Some("It also wakes when".into()),
                items: wakes.into_iter().map(str::to_owned).collect(),
            })
            .detail(Detail::text(
                "The computer, not the agent's own process, keeps the schedule, so a \
                 restart doesn't lose it.",
            ));
        graph.units.push(unit);
    }

    // Where it looks: one unit per place, inside a graph unit.
    let sources = catalog::sources(&text);
    if !sources.is_empty() {
        graph.units.push(
            Unit::new(
                "sources",
                "Where I look",
                format!("{} places", sources.len()),
                Accent::Success,
            )
            .detail(Detail::text(
                "It gathers fresh facts before every answer, and says plainly what it \
                 couldn't reach instead of guessing.",
            )),
        );
        for (place, what, via) in &sources {
            let unit_id = format!("source-{}", slug(place));
            graph.units.push(
                Unit::new(&unit_id, *place, via.join(", "), Accent::Success)
                    .inside("sources")
                    .output("facts", "facts", "facts")
                    .detail(Detail::text(*what))
                    .detail(Detail::Chips {
                        label: Some("Read with".into()),
                        items: via.iter().map(|v| (*v).to_owned()).collect(),
                    }),
            );
            graph
                .links
                .push(Link::new((&unit_id, "facts"), ("runtime", "facts")));
        }
    }

    // Instructions, with each skill inside.
    let paragraphs: Vec<&str> = spec
        .instructions
        .split("\n\n")
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    let mut instructions = Unit::new(
        "instructions",
        "Instructions",
        if skills.is_empty() {
            format!("{} words", spec.instructions.split_whitespace().count())
        } else {
            format!("{} skills", skills.len())
        },
        Accent::Discovery,
    )
    .output("prompt", "prompt", "prompt");
    if let Some(identity) = paragraphs.first() {
        instructions = instructions.detail(Detail::Quote {
            text: plain_line(identity),
        });
    }
    if let Some(rule) = paragraphs
        .iter()
        .find_map(|p| p.strip_prefix("**")?.split_once("**").map(|(r, _)| r))
    {
        instructions = instructions.detail(Detail::text(rule));
    }
    instructions = instructions.detail(Detail::fact(
        "Length",
        format!("{} words in all", text.split_whitespace().count()),
    ));
    graph.units.push(instructions);
    for (at, (name, body)) in skills.iter().enumerate() {
        let first = body
            .split("\n\n")
            .map(str::trim)
            .find(|p| !p.is_empty())
            .unwrap_or("");
        graph.units.push(
            Unit::new(
                &format!("skill-{}", at + 1),
                plain(name),
                format!("{} words", body.split_whitespace().count()),
                Accent::Discovery,
            )
            .inside("instructions")
            .detail(Detail::Quote {
                text: plain_line(first),
            }),
        );
    }
    graph
        .links
        .push(Link::new(("instructions", "prompt"), ("runtime", "prompt")));

    // Tools, grouped, and the commands it runs.
    let tools = catalog::tools(&text);
    let commands = catalog::commands(&text);
    let calls = !tools.is_empty() || !commands.is_empty();
    let remembers = named(&text, "record_decision");
    if calls {
        let count: usize = tools.iter().map(|(_, n)| n.len()).sum();
        graph.units.push(Unit::new(
            "tools",
            "Tools",
            format!("{count} app tools"),
            Accent::Warning,
        ));
        let mut groups: Vec<(String, &str, Vec<&str>)> = tools
            .iter()
            .map(|(group, names)| (format!("tools-{}", slug(group)), *group, names.clone()))
            .collect();
        if !commands.is_empty() {
            groups.push((
                "tools-commands".into(),
                "Commands it runs",
                commands.clone(),
            ));
        }
        let scopes: Vec<&str> = catalog::MACHINE_SCOPES
            .iter()
            .copied()
            .filter(|s| text.contains(s))
            .collect();
        for (unit_id, group, names) in &groups {
            let mut unit = Unit::new(
                unit_id,
                *group,
                match names.len() {
                    1 => names[0].to_owned(),
                    n => format!("{n} tools"),
                },
                Accent::Warning,
            )
            .inside("tools")
            .input("call", "call", "call")
            .detail(Detail::Chips {
                label: None,
                items: names.iter().map(|n| (*n).to_owned()).collect(),
            });
            if *group == "This computer" && !scopes.is_empty() {
                unit = unit.detail(Detail::Chips {
                    label: Some("Each needs its own grant".into()),
                    items: scopes.iter().map(|s| (*s).to_owned()).collect(),
                });
            }
            if names.contains(&"record_decision") {
                unit = unit.output("record", "record", "decision");
            }
            graph.units.push(unit);
        }
        // Every call passes through what the agent may do.
        let mut authority = Unit::new(
            "authority",
            "Authority",
            format!("{} grants", GrantScope::ALL.len()),
            Accent::Danger,
        )
        .input("call", "call", "call")
        .output("allowed", "allowed", "call")
        .detail(Detail::text(if autonomy.authority {
            "It asks for standing authority, so it acts on its check-ins without asking \
             first. Without a grant it only looks and reports."
        } else {
            "It acts only when someone asks."
        }))
        .detail(Detail::fact(
            "Access",
            match spec.access {
                Access::HostDefault => "What the computer allows by default",
                Access::ReadOnly => "Read only",
                Access::WorkspaceWrite => "It may write in its project",
            },
        ))
        .detail(Detail::Chips {
            label: Some("Grants".into()),
            items: GrantScope::ALL
                .iter()
                .map(|s| grant_words(*s).to_owned())
                .collect(),
        })
        .detail(Detail::text(NEVER_GRANTED_REFUSAL));
        if let Some(secs) = autonomy.shell_timeout_secs {
            authority =
                authority.detail(Detail::fact("Longest shell call", minutes(u64::from(secs))));
        }
        graph.units.push(authority);
        runtime = runtime.output("call", "call", "call");
        graph
            .links
            .push(Link::new(("runtime", "call"), ("authority", "call")));
        for (unit_id, _, _) in &groups {
            graph
                .links
                .push(Link::new(("authority", "allowed"), (unit_id, "call")));
        }
    }

    // Memory: decisions recorded, then read back before acting.
    if remembers && calls {
        let working = skills
            .iter()
            .flat_map(|(_, body)| body.lines())
            .find_map(|l| {
                l.strip_prefix("- ")
                    .filter(|l| l.starts_with("My working state"))
            });
        let mut memory = Unit::new("memory", "Memory", "a decision log", Accent::Discovery)
            .input("record", "record", "decision")
            .output("log", "log", "decision")
            .detail(Detail::text(
                "Every action that changes something gets its own record_decision, and every \
                 check-in ends with one more: what it saw, decided and did. It reads that log \
                 before acting, so it doesn't repeat work.",
            ));
        if let Some(line) = working {
            memory = memory.detail(Detail::Quote {
                text: plain_line(line),
            });
        }
        graph.units.push(memory);
        runtime = runtime.input("memory", "memory", "decision");
        graph
            .links
            .push(Link::new(("tools-memory", "record"), ("memory", "record")).feedback());
        graph
            .links
            .push(Link::new(("memory", "log"), ("runtime", "memory")));
    }

    // The stall ladder: each rung inside, one after the other.
    let ladder = skills
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("stalls"))
        .map(|(_, body)| rungs(body))
        .unwrap_or_default();
    let watches = !ladder.is_empty();
    if watches {
        let stall = autonomy.stall_minutes.unwrap_or(HOST_STALL_MINUTES);
        let reads_app = graph.unit("source-the-app").is_some();
        let mut unit = Unit::new(
            "stalls",
            "Stall ladder",
            format!("{} rungs", ladder.len()),
            Accent::Warning,
        )
        .output("wake", "wake", "wake")
        .detail(Detail::text(format!(
            "Work is stalled after {stall} minutes with no output, commit, push, comment or \
             build. Then it climbs the rungs. Any progress starts the ladder over."
        )));
        if reads_app {
            unit = unit.input("activity", "activity", "facts");
            graph.links.push(Link::new(
                ("source-the-app", "facts"),
                ("stalls", "activity"),
            ));
        }
        graph.units.push(unit);
        for (at, (title, what)) in ladder.iter().enumerate() {
            let unit_id = format!("stall-{}", at + 1);
            graph.units.push(
                Unit::new(
                    &unit_id,
                    title.clone(),
                    format!("Rung {}", at + 1),
                    Accent::Warning,
                )
                .inside("stalls")
                .detail(Detail::text(what.clone())),
            );
        }
        graph
            .links
            .push(Link::new(("stalls", "wake"), ("runtime", "wake")));
    }
    if checks_in {
        graph
            .links
            .push(Link::new(("check-ins", "wake"), ("runtime", "wake")));
    }

    // The runtime itself.
    if checks_in || watches {
        runtime = runtime.input("wake", "wake", "wake");
    }
    runtime = runtime.input("prompt", "prompt", "prompt");
    if !sources.is_empty() {
        runtime = runtime.input("facts", "facts", "facts");
    }
    // Inputs in the order they are drawn: wake, prompt, facts, memory.
    runtime.inputs.sort_by_key(|p| match p.id.as_str() {
        "wake" => 0,
        "prompt" => 1,
        "facts" => 2,
        _ => 3,
    });
    let fallbacks: Vec<String> = spec.fallbacks.iter().map(|f| f.runtime.clone()).collect();
    runtime = runtime
        .output("reply", "reply", "reply")
        .detail(Detail::fact("Runtime", spec.runtime.clone()))
        .detail(Detail::fact("Model", model))
        .detail(Detail::fact(
            "If the runtime isn't ready",
            if fallbacks.is_empty() {
                "No fallback: it runs only where its runtime is ready.".to_owned()
            } else {
                format!("Tries {}.", fallbacks.join(", "))
            },
        ))
        .detail(Detail::text(
            "It runs as a chat in the OpenAgents app, on a computer you choose, so you can \
             watch and steer it from the desktop, the phone or the web.",
        ));
    graph.units.insert(0, runtime);

    // The suite that tests it.
    let suite = &experiment.suite;
    if !suite.cases.is_empty() {
        let automatic = suite
            .cases
            .iter()
            .filter(|c| !matches!(c.check, Check::Review { .. }))
            .count();
        graph.units.push(
            Unit::new("tests", "Tested by", "", Accent::Success)
                .input("reply", "reply", "reply")
                .output("score", "score", "score"),
        );
        graph.units.push(
            Unit::new(
                "suite",
                suite.title.clone(),
                format!("{} cases", suite.cases.len()),
                Accent::Success,
            )
            .inside("tests")
            .detail(Detail::text(format!(
                "{automatic} checked automatically, {} reviewed by a person. Each trial gets {}{}.",
                suite.cases.len() - automatic,
                minutes(experiment.max_seconds),
                if experiment.no_project_files {
                    " and no project files"
                } else {
                    ""
                }
            )))
            .detail(Detail::List {
                label: None,
                items: suite
                    .cases
                    .iter()
                    .map(|c| match c.check {
                        Check::Review { .. } => format!("{} (reviewed)", c.title),
                        _ => c.title.clone(),
                    })
                    .collect(),
            }),
        );
        graph
            .links
            .push(Link::new(("runtime", "reply"), ("tests", "reply")));
    }

    graph.units.extend(notes.units.iter().cloned());
    graph.links.extend(notes.links.iter().cloned());
    // Every case inside `tests` is one item of a child's list.
    let cases: usize = graph
        .children(Some("tests"))
        .flat_map(|u| &u.details)
        .map(|d| match d {
            Detail::List { items, .. } => items.len(),
            _ => 0,
        })
        .sum();
    if let Some(tests) = graph.units.iter_mut().find(|u| u.id == "tests") {
        tests.line = format!("{cases} cases");
    }
    for (unit_id, tags) in &notes.tags {
        if let Some(unit) = graph.units.iter_mut().find(|u| &u.id == unit_id) {
            unit.tags.extend(tags.iter().cloned());
        }
    }
    graph
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rungs_keep_their_bold_names_and_what_they_do() {
        let skill = "Intro.\n\n1. **Message it** (rung 1, at the window): ask what it needs.\n2. **Tell the owner** (rung 2): lead with the issue.\n";
        assert_eq!(
            rungs(skill),
            vec![
                ("Message it".to_owned(), "Ask what it needs.".to_owned()),
                (
                    "Tell the owner".to_owned(),
                    "Lead with the issue.".to_owned()
                ),
            ]
        );
    }

    #[test]
    fn a_bare_experiment_still_draws_its_runtime_and_instructions() {
        let mut experiment = agent_lab::templates::foreman();
        experiment.draft.components.clear();
        experiment.draft.spec.instructions = "Answer questions about the weather.".into();
        experiment.draft.spec.autonomy = None;
        experiment.suite.cases.clear();
        let graph = of(&experiment, "weather", &Notes::default());
        graph.validate().unwrap();
        let ids: Vec<&str> = graph.units.iter().map(|u| u.id.as_str()).collect();
        assert_eq!(ids, ["runtime", "instructions"]);
        assert_eq!(graph.links.len(), 1);
    }
}
