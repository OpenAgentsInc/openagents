//! Ready-made experiments that a person can start from instead of a blank draft.
//!
//! Each one is published as an agent definition (`api::PublishedAgent`) that
//! anyone can fetch, test and save as their own (#12048). The copies here are
//! what a build falls back to when it can't reach the published one, and what
//! the publisher sends as a new revision.
use super::*;

/// The published ID of the official Foreman. It is the digest of the private
/// request ID of its first publish ([`api::published_id`]), so nobody else
/// can publish under it.
pub const FOREMAN_ID: &str = "04288eb136553674824afea5992734e56e9ede0508bf8a931b9d5023d5255a2c";

/// Foreman's short instructions. Its playbook is in [`FOREMAN_SKILLS`].
pub const FOREMAN_INSTRUCTIONS: &str = include_str!("templates/foreman/instructions.md");

/// Foreman's playbook as Agent Lab skills, in order: name and text.
pub const FOREMAN_SKILLS: [(&str, &str); 8] = [
    (
        "Where I look",
        include_str!("templates/foreman/where-i-look.md"),
    ),
    (
        "How I reason",
        include_str!("templates/foreman/how-i-reason.md"),
    ),
    (
        "What I may change",
        include_str!("templates/foreman/what-i-may-change.md"),
    ),
    (
        "Dispatching issue work",
        include_str!("templates/foreman/dispatching.md"),
    ),
    (
        "Upgrading myself",
        include_str!("templates/foreman/upgrading-myself.md"),
    ),
    (
        "Staying responsive",
        include_str!("templates/foreman/staying-responsive.md"),
    ),
    ("Check-ins", include_str!("templates/foreman/check-ins.md")),
    ("Stalls", include_str!("templates/foreman/stalls.md")),
];

/// A skill component for a draft, with the digest Agent Lab checks.
pub fn skill(name: &str, text: &str) -> Component {
    Component::Skill {
        name: name.into(),
        text: text.into(),
        sha256: lab_kit::digest(&json!({"name": name, "text": text})),
    }
}

/// The templates Agent Lab offers to start from: each one's published ID and
/// the copy in this build.
pub fn offered() -> Vec<(&'static str, Experiment)> {
    vec![(FOREMAN_ID, foreman())]
}

/// Foreman (#11926): an agent that reasons over every open issue and every
/// active agent. It runs on Claude Code, which the app host gives its chat,
/// device and session tools and its signed-in `gh`. Its suite checks that it
/// reaches GitHub, the app, and the claim-to-chat join before anyone relies
/// on it; the two reviewed cases judge the answers people actually ask for.
/// The host and project stay unchosen: the person picks the computer.
/// The trial starts with no project files, because Foreman reads GitHub and
/// the app rather than the checkout.
pub fn foreman() -> Experiment {
    Experiment {
        title: "Foreman".into(),
        draft: Draft {
            spec: AgentSpec {
                name: "Foreman".into(),
                description: "Keeps the whole picture of open issues and active agents: what's idle, what's colliding, what's stuck, and what to do next.".into(),
                instructions: FOREMAN_INSTRUCTIONS.trim_end().into(),
                runtime: "claude-code".into(),
                model: Some("claude-opus-5-5".into()),
                access: agent_contract::Access::HostDefault,
                fallbacks: vec![],
                // Hosts turn on 10-minute check-ins and standing authority
                // for it (#12030), and every shell call stops after 2
                // minutes, so one probe with no timeout can't hang a turn
                // (#12029).
                autonomy: Some(agent_contract::Autonomy {
                    check_ins: true,
                    every_minutes: Some(10),
                    authority: true,
                    // The host's 20-minute default stall window.
                    stall_minutes: None,
                    shell_timeout_secs: Some(120),
                }),
            },
            source: None,
            components: FOREMAN_SKILLS
                .iter()
                .map(|(name, text)| skill(name, text.trim_end()))
                .collect(),
        },
        suite: Suite {
            title: "Foreman reach and judgment".into(),
            partition: Partition::Development,
            cases: vec![
                Case {
                    id: "github".into(),
                    title: "Reaches GitHub issues".into(),
                    task: "Find the OpenAgentsInc/openagents issue whose title starts with \"Epic: OpenAgents Coder for iPhone\". Reply with only its number, written like #123.".into(),
                    check: Check::Contains {
                        expected: "#11840".into(),
                    },
                },
                Case {
                    id: "app".into(),
                    title: "Reaches the app's agent tools".into(),
                    task: "Use your OpenAgents tools to find out which harness runs your own chat. Reply with only the harness id.".into(),
                    check: Check::Contains {
                        expected: "claude-code".into(),
                    },
                },
                Case {
                    id: "claim".into(),
                    title: "Reads claim markers".into(),
                    task: "Issue #11859 in OpenAgentsInc/openagents has an issue claim comment. Reply with only the value of the session field in its claim marker, in the form harness:id.".into(),
                    check: Check::Contains {
                        expected: "codex:01a14115-9212-7051-bd07-7aac3430a81b".into(),
                    },
                },
                Case {
                    id: "status".into(),
                    title: "Holistic status".into(),
                    task: "Give me the state of OpenAgents work right now: which epics have no agent on them, which idle issues can start now, and anything stuck. Keep it under 3,000 characters.".into(),
                    check: Check::Review {
                        criteria: "Every claim is backed by a claim, chat or commit with its time; idle issues are split into can-start-now, waiting and parked with reasons; the answer leads with the conclusion and ends with a recommendation.".into(),
                    },
                },
                Case {
                    id: "collisions".into(),
                    title: "Finds collisions and drift".into(),
                    task: "Are any two agents working on the same issue, and does the OpenAgents board disagree with what agents are actually doing? Name the chats and issues. Keep it under 3,000 characters.".into(),
                    check: Check::Review {
                        criteria: "Collisions name both chats or sessions and the issue; board drift cites the status and the contrary evidence; it says when it found none and what it could not reach.".into(),
                    },
                },
            ],
        },
        compare: None,
        host: "choose".into(),
        project: "choose".into(),
        repetitions: 1,
        max_seconds: 900,
        no_project_files: true,
    }
}
