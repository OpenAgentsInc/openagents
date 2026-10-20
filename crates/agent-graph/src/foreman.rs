//! Foreman's graph: [`lab::of`] over `agent_lab::templates::foreman()`,
//! with the lessons each part answers, the replay that tests it, and what
//! we measure it by.

use agent_lab::{replay, templates};

use crate::catalog::capitalized;
use crate::lab::{self, Notes};
use crate::{Accent, Detail, GraphSpec, Link, Unit};

/// Foreman's lessons (`docs/foreman/lessons-learned.md`), by number.
pub const LESSONS: [(&str, &str); 13] = [
    ("L1", "Coordination ran where no one could see it"),
    ("L2", "Foreman's own turn hung"),
    ("L3", "The stall rule only watched"),
    ("L4", "Check-ins were never switched on"),
    ("L5", "Foreman's watch died with its process"),
    ("L6", "Agents handed off code that never compiled"),
    ("L7", "Branches were merged one at a time by hand"),
    ("L8", "Builds ran on the owner's Mac"),
    ("L9", "A red main was compared by hand"),
    ("L10", "Hand-written rules were wrong"),
    ("L11", "Probes changed production"),
    ("L12", "Actions without a record"),
    ("L13", "Nothing measured Foreman"),
];

/// The lessons each part answers, by unit ID. The tests unit answers the
/// lessons its replay scenarios name.
pub const PART_LESSONS: [(&str, &[&str]); 9] = [
    ("runtime", &["L1", "L2"]),
    ("instructions", &["L6", "L7", "L8"]),
    ("tools", &["L10"]),
    ("sources", &["L11"]),
    ("check-ins", &["L4", "L5"]),
    ("authority", &["L3"]),
    ("memory", &["L12"]),
    ("stalls", &["L3"]),
    ("measures", &["L13"]),
];

/// A lesson's title by its number.
pub fn lesson_title(number: &str) -> &'static str {
    LESSONS
        .iter()
        .find(|(n, _)| *n == number)
        .map_or("", |(_, title)| title)
}

/// A replay scenario's title without its roadmap items: "Item 1: a hung
/// tool call is caught (2:11 PM)" reads "A hung tool call is caught (2:11 PM)".
pub fn scenario_title(title: &str) -> String {
    let short = title
        .split_once(": ")
        .filter(|(head, _)| head.starts_with("Item"))
        .map_or(title, |(_, rest)| rest);
    capitalized(short)
}

/// The lessons the replay's scenarios answer, in order.
pub fn replay_lessons() -> Vec<String> {
    let mut lessons: Vec<String> = replay::day()
        .scenarios
        .iter()
        .flat_map(|s| s.lessons.iter().cloned())
        .collect();
    lessons.sort_by_key(|l| l.trim_start_matches('L').parse::<u32>().unwrap_or(0));
    lessons.dedup();
    lessons
}

/// What we measure Foreman by, and what we have measured so far.
pub fn measures() -> Vec<Detail> {
    vec![
        Detail::fact(
            "Code complete to close",
            "Minutes from the last push to a closed issue",
        ),
        Detail::fact("Stall minutes", "Minutes each chat sat with no progress"),
        Detail::text(
            "On 2026-10-17, issues coordinated by hand took 4 h 47 min to 5 h 17 min from \
             code complete to close. Foreman's six parallel chats closed #12018 in 21 \
             minutes, #12022 in 33 and #12019 in 35.",
        ),
        Detail::text(
            "Over 2026-10-18 and 19, 91 issues closed with a median of 2.3 hours from \
             filing. Only 3 of 380 commits went through the landing queue, and after an \
             account switch every Foreman turn ended empty for 35 minutes with no alarm.",
        ),
    ]
}

/// Foreman's notes: lessons, the replay inside `tests`, and `measures`.
pub fn notes() -> Notes {
    let day = replay::day();
    let mut tags: Vec<(String, Vec<String>)> = PART_LESSONS
        .iter()
        .map(|(unit, lessons)| {
            (
                (*unit).to_owned(),
                lessons.iter().map(|l| (*l).to_owned()).collect(),
            )
        })
        .collect();
    tags.push(("tests".into(), replay_lessons()));
    let replay = Unit::new(
        "replay",
        format!("Replay of {}", day.id),
        format!("{} moments", day.scenarios.len()),
        Accent::Success,
    )
    .inside("tests")
    .detail(Detail::text(format!(
        "{} moments from the day we learned most from, each scored by its deadline. A new \
         revision is saved only if every case passes.",
        day.scenarios.len()
    )))
    .detail(Detail::List {
        label: None,
        items: day
            .scenarios
            .iter()
            .map(|s| {
                let title = scenario_title(&s.title);
                if s.lessons.is_empty() {
                    title
                } else {
                    format!("{title} ({})", s.lessons.join(", "))
                }
            })
            .collect(),
    });
    let mut measures = Unit::new("measures", "Measures", "time to close", Accent::Danger)
        .input("score", "score", "score");
    measures.details = self::measures();
    Notes {
        tags,
        units: vec![replay, measures],
        links: vec![Link::new(("tests", "score"), ("measures", "score"))],
    }
}

/// Foreman's graph, from the definition this build carries.
pub fn graph() -> GraphSpec {
    lab::of(&templates::foreman(), "foreman", &notes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog;

    #[test]
    fn foremans_graph_is_valid_and_names_every_tool_its_definition_names() {
        let graph = graph();
        graph.validate().unwrap();
        let text = lab::text(&templates::foreman());
        let chips: Vec<&str> = graph
            .units
            .iter()
            .filter(|u| u.parent.as_deref() == Some("tools"))
            .flat_map(|u| &u.details)
            .flat_map(|d| match d {
                Detail::Chips { label: None, items } => items.iter().map(String::as_str).collect(),
                _ => vec![],
            })
            .collect();
        for (_, names) in catalog::TOOLS {
            for name in names {
                assert!(
                    catalog::named(&text, name),
                    "Foreman no longer names {name}"
                );
                assert!(chips.contains(name), "the graph leaves out {name}");
            }
        }
        for command in catalog::COMMANDS {
            assert!(
                catalog::named(&text, command),
                "Foreman no longer runs {command}"
            );
            assert!(chips.contains(&command), "the graph leaves out {command}");
        }
        for chip in &chips {
            assert!(
                catalog::named(&text, chip),
                "the graph names {chip}, Foreman doesn't"
            );
        }
        for (place, _, via) in catalog::SOURCES {
            assert!(via.iter().any(|v| catalog::named(&text, v)), "{place}");
        }
    }

    #[test]
    fn foremans_graph_has_every_part_and_nests_its_skills_and_rungs() {
        let graph = graph();
        for id in [
            "runtime",
            "check-ins",
            "sources",
            "instructions",
            "tools",
            "authority",
            "memory",
            "stalls",
            "tests",
            "measures",
        ] {
            assert!(graph.unit(id).is_some(), "{id}");
            assert_eq!(graph.unit(id).unwrap().parent, None, "{id}");
        }
        assert_eq!(
            graph.children(Some("instructions")).count(),
            templates::FOREMAN_SKILLS.len()
        );
        assert_eq!(graph.children(Some("stalls")).count(), 3);
        assert_eq!(
            graph.children(Some("sources")).count(),
            catalog::SOURCES.len()
        );
        let foreman = templates::foreman();
        let runtime = graph.unit("runtime").unwrap();
        assert!(runtime.line.contains(&foreman.draft.spec.runtime));
        assert_eq!(
            graph.unit("tests").unwrap().line,
            format!(
                "{} cases",
                foreman.suite.cases.len() + replay::day().scenarios.len()
            )
        );
    }

    #[test]
    fn foremans_graph_reads_plainly() {
        let graph = graph();
        for line in graph.visible_text() {
            let found = oa_copy::violations(line, &[]);
            assert!(found.is_empty(), "{line}: {found:?}");
            assert!(!line.to_lowercase().contains("zeron"), "{line}");
        }
    }

    #[test]
    fn the_json_names_the_schema_and_reads_back() {
        let graph = graph();
        let json = serde_json::to_string(&graph).unwrap();
        let back: GraphSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(back, graph);
    }
}
