//! Ready-made experiments that a person can start from instead of a blank draft.
use super::*;

/// Foreman's instructions: the whole picture of open issues and active agents.
pub const FOREMAN_INSTRUCTIONS: &str = include_str!("templates/foreman.md");

/// Foreman (#11926): an agent that reasons over every open issue and every
/// active agent. It runs on Claude Code, which the app host gives its chat,
/// device and session tools and its signed-in `gh`. Its suite checks that it
/// reaches GitHub, the app, and the claim-to-chat join before anyone relies
/// on it; the two reviewed cases judge the answers people actually ask for.
/// The host and project stay unchosen: the person picks the computer.
pub fn foreman() -> Experiment {
    Experiment {
        title: "Foreman".into(),
        draft: Draft {
            spec: AgentSpec {
                name: "Foreman".into(),
                description: "Keeps the whole picture of open issues and active agents: what's idle, what's colliding, what's stuck, and what to do next.".into(),
                instructions: FOREMAN_INSTRUCTIONS.into(),
                runtime: "claude-code".into(),
                model: Some("claude-opus-5-5".into()),
                access: agent_contract::Access::HostDefault,
            },
            source: None,
            components: vec![],
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
    }
}
