//! Project and operator-job presentation from admitted native records.
//! The owning application supplies source identities, bounded pages, and gates.

use crate::control::Gate;
use crate::workspace::Palette;
use rust_native::style::{Color, Space, Style};
use rust_native::{Axis, Element, Node, TextRole, View};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoordinationIntent {
    ReviewSubmission,
    ContinueJob,
    RequestStop,
    ReviewCandidate,
    ApplyArtifacts,
    PublishCandidate,
    Detach,
}

#[derive(Clone, Copy)]
pub struct Source<'a> {
    pub key: &'a str,
    pub host: &'a str,
    pub generation: u64,
    pub workspace: &'a str,
    /// An admitted native record alias, never a filesystem path or URL.
    pub record: &'a str,
    pub revision: &'a str,
    pub source_digest: &'a str,
}

pub struct Project<'a> {
    pub source: Source<'a>,
    pub title: &'a str,
    pub repository: &'a str,
    pub goals: &'a [Goal<'a>],
    pub issues: &'a [Issue<'a>],
    pub capacity: &'a str,
    pub review_backpressure: &'a str,
}

pub struct Goal<'a> {
    pub key: &'a str,
    pub id: &'a str,
    pub title: &'a str,
    pub state: &'a str,
    pub tasks: &'a [&'a str],
    pub blockers: &'a [&'a str],
}

pub struct Dependency<'a> {
    pub issue: u64,
    pub repository: Option<&'a str>,
    pub state: &'a str,
}

pub struct Issue<'a> {
    pub key: &'a str,
    pub number: u64,
    pub title: &'a str,
    pub state: &'a str,
    pub status: &'a str,
    pub version: &'a str,
    /// The native claim observation, independent of assignees and issue text.
    pub claim: &'a str,
    pub dependencies: &'a [Dependency<'a>],
    pub blockers: &'a [&'a str],
    pub worktrees: &'a [&'a str],
}

pub struct Artifact<'a> {
    pub key: &'a str,
    pub label: &'a str,
    pub digest: &'a str,
    pub retention: &'a str,
    pub state: &'a str,
}

pub struct Attempt<'a> {
    pub key: &'a str,
    pub id: &'a str,
    pub state: &'a str,
    pub executor: &'a str,
    pub requested_model: Option<&'a str>,
    pub served_model: Option<&'a str>,
    pub usage: Option<&'a Value>,
    pub cost: Option<&'a str>,
    pub continuation: Option<&'a str>,
    pub stop_request: &'a str,
    pub delivery: &'a str,
    pub publish: &'a str,
    pub cleanup: &'a str,
}

pub struct Job<'a> {
    pub source: Source<'a>,
    pub title: &'a str,
    pub state: &'a str,
    pub executor: &'a str,
    pub placement: &'a str,
    pub requested_model: Option<&'a str>,
    pub served_model: Option<&'a str>,
    pub usage: Option<&'a Value>,
    pub cost: Option<&'a str>,
    pub continuation: Option<&'a str>,
    pub stop_request: &'a str,
    pub delivery: &'a str,
    pub publish: &'a str,
    pub cleanup: &'a str,
    pub artifacts: &'a [Artifact<'a>],
    pub attempts: &'a [Attempt<'a>],
}

pub struct Controls<'a> {
    pub key: &'a str,
    pub submission: Gate<'a>,
    pub continuation: Gate<'a>,
    pub stop: Gate<'a>,
    pub review: Gate<'a>,
    pub apply: Gate<'a>,
    pub publish: Gate<'a>,
    pub detach: Gate<'a>,
}

fn text(
    key: impl Into<String>,
    value: impl Into<String>,
    role: TextRole,
    color: Color,
) -> Node<CoordinationIntent> {
    Node {
        key: key.into(),
        style: Style {
            foreground: Some(color),
            ..Style::default()
        },
        element: Element::Text {
            value: value.into(),
            role,
        },
    }
}

fn stack(
    key: impl Into<String>,
    children: Vec<Node<CoordinationIntent>>,
) -> Node<CoordinationIntent> {
    Node {
        key: key.into(),
        style: Style {
            gap: Some(Space::Sm),
            ..Style::default()
        },
        element: Element::Stack {
            axis: Axis::Vertical,
            children,
        },
    }
}

fn reported(value: &str) -> &str {
    if value.is_empty() { "Unknown" } else { value }
}

fn optional(value: Option<&str>) -> &str {
    value.filter(|value| !value.is_empty()).unwrap_or("Unknown")
}

fn fact(
    key: &str,
    suffix: &str,
    label: &str,
    value: &str,
    palette: Palette,
) -> Node<CoordinationIntent> {
    text(
        format!("{key}:{suffix}"),
        format!("{label}: {}", reported(value)),
        TextRole::Body,
        palette.text,
    )
}

fn list_fact(
    key: &str,
    suffix: &str,
    label: &str,
    values: &[&str],
    palette: Palette,
) -> Node<CoordinationIntent> {
    fact(
        key,
        suffix,
        label,
        &if values.is_empty() {
            "Unknown".into()
        } else {
            values.join(" · ")
        },
        palette,
    )
}

fn source(value: Source<'_>, palette: Palette) -> Vec<Node<CoordinationIntent>> {
    let key = value.key;
    [
        ("host", "Host", value.host),
        (
            "generation",
            "Host generation",
            &value.generation.to_string(),
        ),
        ("workspace", "Workspace", value.workspace),
        ("record", "Record", value.record),
        ("revision", "Revision", value.revision),
    ]
    .into_iter()
    .map(|(suffix, label, value)| fact(key, suffix, label, value, palette))
    .collect()
}

fn goal_node(value: &Goal<'_>, palette: Palette) -> Node<CoordinationIntent> {
    let key = value.key;
    stack(
        key,
        vec![
            text(
                format!("{key}:title"),
                value.title,
                TextRole::Heading,
                palette.heading,
            ),
            fact(key, "id", "Goal", value.id, palette),
            fact(key, "state", "Goal state", value.state, palette),
            list_fact(key, "tasks", "Linked tasks", value.tasks, palette),
            list_fact(key, "blockers", "Goal blockers", value.blockers, palette),
        ],
    )
}

pub fn goal(value: &Goal<'_>, palette: Palette) -> View<CoordinationIntent> {
    View::new_v3("project-goal", 1, goal_node(value, palette))
}

fn issue_node(value: &Issue<'_>, palette: Palette) -> Node<CoordinationIntent> {
    let key = value.key;
    let mut children = vec![
        text(
            format!("{key}:title"),
            value.title,
            TextRole::Heading,
            palette.heading,
        ),
        fact(
            key,
            "number",
            "Issue",
            &format!("#{}", value.number),
            palette,
        ),
        fact(key, "state", "Tracker state", value.state, palette),
        fact(key, "status", "Status", value.status, palette),
        fact(key, "version", "Issue version", value.version, palette),
        fact(key, "claim", "Claimed by", value.claim, palette),
        list_fact(key, "blockers", "Issue blockers", value.blockers, palette),
        list_fact(
            key,
            "worktrees",
            "Worktree ownership",
            value.worktrees,
            palette,
        ),
    ];
    if value.dependencies.is_empty() {
        children.push(fact(
            key,
            "dependencies",
            "Dependencies",
            "Unknown",
            palette,
        ));
    } else {
        for (index, dependency) in value.dependencies.iter().enumerate() {
            children.push(fact(
                key,
                &format!("dependency-{index}"),
                "Dependency",
                &format!(
                    "{} #{} · {}",
                    optional(dependency.repository),
                    dependency.issue,
                    reported(dependency.state)
                ),
                palette,
            ));
        }
    }
    stack(key, children)
}

pub fn issue(value: &Issue<'_>, palette: Palette) -> View<CoordinationIntent> {
    View::new_v3("project-issue", 1, issue_node(value, palette))
}

pub fn project(value: &Project<'_>, palette: Palette) -> View<CoordinationIntent> {
    let key = value.source.key;
    let mut children = vec![text(
        format!("{key}:title"),
        value.title,
        TextRole::Heading,
        palette.heading,
    )];
    children.extend(source(value.source, palette));
    children.extend([
        fact(key, "repository", "Repository", value.repository, palette),
        fact(key, "capacity", "Current capacity", value.capacity, palette),
        fact(
            key,
            "review",
            "Review queue",
            value.review_backpressure,
            palette,
        ),
    ]);
    children.extend(value.goals.iter().map(|value| goal_node(value, palette)));
    children.extend(value.issues.iter().map(|value| issue_node(value, palette)));
    View::new_v3("native-project", 1, stack(key, children))
}

#[allow(clippy::too_many_arguments)]
fn execution_facts(
    key: &str,
    requested_model: Option<&str>,
    served_model: Option<&str>,
    usage: Option<&Value>,
    cost: Option<&str>,
    continuation: Option<&str>,
    stop_request: &str,
    delivery: &str,
    publish: &str,
    cleanup: &str,
    palette: Palette,
) -> Vec<Node<CoordinationIntent>> {
    let mut children = [
        (
            "requested-model",
            "Requested model",
            optional(requested_model),
        ),
        ("served-model", "Served model", optional(served_model)),
        ("cost", "Cost", optional(cost)),
        ("continuation", "Session", optional(continuation)),
        ("stop", "Stop request", stop_request),
        ("delivery", "Delivery", delivery),
        ("publish", "Publication", publish),
        ("cleanup", "Cleanup", cleanup),
    ]
    .into_iter()
    .map(|(suffix, label, value)| fact(key, suffix, label, value, palette))
    .collect::<Vec<_>>();
    children.push(text(
        format!("{key}:usage"),
        usage.map_or_else(
            || "Usage: Unknown".into(),
            |usage| format!("Usage: {usage}"),
        ),
        TextRole::Code,
        palette.text,
    ));
    children
}

fn attempt_node(value: &Attempt<'_>, palette: Palette) -> Node<CoordinationIntent> {
    let key = value.key;
    let mut children = vec![
        text(
            format!("{key}:title"),
            "Earlier attempt",
            TextRole::Heading,
            palette.heading,
        ),
        fact(key, "id", "Attempt", value.id, palette),
        fact(key, "state", "Attempt state", value.state, palette),
        fact(key, "executor", "Attempt executor", value.executor, palette),
    ];
    children.extend(execution_facts(
        key,
        value.requested_model,
        value.served_model,
        value.usage,
        value.cost,
        value.continuation,
        value.stop_request,
        value.delivery,
        value.publish,
        value.cleanup,
        palette,
    ));
    stack(key, children)
}

pub fn attempt(value: &Attempt<'_>, palette: Palette) -> View<CoordinationIntent> {
    View::new_v3("operator-job-attempt", 1, attempt_node(value, palette))
}

fn artifact_node(value: &Artifact<'_>, palette: Palette) -> Node<CoordinationIntent> {
    let key = value.key;
    stack(
        key,
        vec![
            text(
                format!("{key}:title"),
                value.label,
                TextRole::Heading,
                palette.heading,
            ),
            fact(key, "retention", "Kept", value.retention, palette),
            fact(key, "state", "Artifact state", value.state, palette),
        ],
    )
}

pub fn artifacts(values: &[Artifact<'_>], palette: Palette) -> View<CoordinationIntent> {
    View::new_v3(
        "operator-job-artifacts",
        1,
        stack(
            "operator-job-artifacts",
            values
                .iter()
                .map(|value| artifact_node(value, palette))
                .collect(),
        ),
    )
}

pub fn job(value: &Job<'_>, palette: Palette) -> View<CoordinationIntent> {
    let key = value.source.key;
    let mut children = vec![text(
        format!("{key}:title"),
        value.title,
        TextRole::Heading,
        palette.heading,
    )];
    children.extend(source(value.source, palette));
    children.extend([
        fact(key, "state", "Status", value.state, palette),
        fact(key, "executor", "Engine", value.executor, palette),
        fact(key, "placement", "Runs on", value.placement, palette),
    ]);
    children.extend(execution_facts(
        key,
        value.requested_model,
        value.served_model,
        value.usage,
        value.cost,
        value.continuation,
        value.stop_request,
        value.delivery,
        value.publish,
        value.cleanup,
        palette,
    ));
    children.extend(
        value
            .attempts
            .iter()
            .map(|value| attempt_node(value, palette)),
    );
    children.extend(
        value
            .artifacts
            .iter()
            .map(|value| artifact_node(value, palette)),
    );
    View::new_v3("operator-job", 1, stack(key, children))
}

pub fn controls(value: &Controls<'_>, palette: Palette) -> View<CoordinationIntent> {
    let key = value.key;
    let mut children = Vec::new();
    for (suffix, label, gate, intent) in [
        (
            "submit",
            "Review submission",
            value.submission,
            CoordinationIntent::ReviewSubmission,
        ),
        (
            "continue",
            "Review next turn",
            value.continuation,
            CoordinationIntent::ContinueJob,
        ),
        ("stop", "Stop", value.stop, CoordinationIntent::RequestStop),
        (
            "review",
            "Review changes",
            value.review,
            CoordinationIntent::ReviewCandidate,
        ),
        (
            "apply",
            "Apply changes",
            value.apply,
            CoordinationIntent::ApplyArtifacts,
        ),
        (
            "publish",
            "Publish changes",
            value.publish,
            CoordinationIntent::PublishCandidate,
        ),
        (
            "detach",
            "Stop watching",
            value.detach,
            CoordinationIntent::Detach,
        ),
    ] {
        children.push(Node {
            key: format!("{key}:{suffix}:activate"),
            style: Style {
                foreground: Some(palette.text),
                border: Some(palette.border),
                ..Style::default()
            },
            element: Element::Button {
                label: label.into(),
                enabled: gate.enabled,
                intent,
                icon: None,
                shortcut: None,
            },
        });
        if let Some(reason) = gate.reason {
            children.push(text(
                format!("{key}:{suffix}:reason"),
                reason,
                TextRole::Status,
                palette.secondary,
            ));
        }
    }
    View::new_v3("coordination-controls", 1, stack(key, children))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_native::Activation;

    fn palette() -> Palette {
        let white = Color::rgb(255, 255, 255);
        Palette {
            text: white,
            heading: white,
            secondary: white,
            border: white,
        }
    }
    fn source() -> Source<'static> {
        Source {
            key: "source",
            host: "native-host",
            generation: 7,
            workspace: "checkout",
            record: "admitted-alias",
            revision: "original-revision",
            source_digest: "original-source-digest",
        }
    }
    #[test]
    fn graph_rows_preserve_claims_dependencies_and_unknown_admission() {
        let dependencies = [Dependency {
            issue: 12,
            repository: Some("org/repository"),
            state: "Unknown",
        }];
        let issues = [Issue {
            key: "issue-13",
            number: 13,
            title: "Run everything automatically",
            state: "Open",
            status: "Ready",
            version: "version-13",
            claim: "Held by a different native actor",
            dependencies: &dependencies,
            blockers: &["Dependency observation incomplete"],
            worktrees: &[],
        }];
        let goals = [Goal {
            key: "goal-1",
            id: "goal-original",
            title: "Original goal",
            state: "Planning",
            tasks: &["native-task"],
            blockers: &[],
        }];
        let view = project(
            &Project {
                source: source(),
                title: "Native project",
                repository: "org/repository",
                goals: &goals,
                issues: &issues,
                capacity: "",
                review_backpressure: "Review lane full",
            },
            palette(),
        )
        .validate()
        .unwrap();
        let json = serde_json::to_string(view.view()).unwrap();
        for label in [
            "Run everything automatically",
            "Held by a different native actor",
            "org/repository #12 · Unknown",
            "Worktree ownership: Unknown",
            "Current capacity: Unknown",
            "Review queue: Review lane full",
        ] {
            assert!(json.contains(label), "{label}");
        }
        assert!(!json.contains("\"intent\":"));
    }
    #[test]
    fn job_outcomes_do_not_supply_cost_served_model_delivery_or_cleanup() {
        let usage = serde_json::json!({"tokens":321,"cost":null});
        let view = job(
            &Job {
                source: source(),
                title: "Original operator job",
                state: "Completed",
                executor: "Native executor label",
                placement: "Granted pool",
                requested_model: Some("requested-model"),
                served_model: None,
                usage: Some(&usage),
                cost: None,
                continuation: None,
                stop_request: "Requested, acknowledgment unknown",
                delivery: "Unknown",
                publish: "Not published",
                cleanup: "Unknown",
                artifacts: &[],
                attempts: &[],
            },
            palette(),
        )
        .validate()
        .unwrap();
        let json = serde_json::to_string(view.view()).unwrap();
        for label in [
            "Status: Completed",
            "Requested model: requested-model",
            "Served model: Unknown",
            "Cost: Unknown",
            "Stop request: Requested, acknowledgment unknown",
            "Delivery: Unknown",
            "Publication: Not published",
            "Cleanup: Unknown",
            "321",
        ] {
            assert!(json.contains(label), "{label}");
        }
        assert!(!json.contains("$0"));
    }
    #[test]
    fn attempts_and_artifacts_retain_original_separate_sources() {
        let attempts = [Attempt {
            key: "attempt-1",
            id: "original-attempt",
            state: "Failed",
            executor: "Original executor",
            requested_model: Some("asked"),
            served_model: Some("served"),
            usage: None,
            cost: None,
            continuation: Some("original-session"),
            stop_request: "None recorded",
            delivery: "Unknown",
            publish: "Unknown",
            cleanup: "Incomplete",
        }];
        let artifacts = [Artifact {
            key: "artifact-1",
            label: "Original candidate",
            digest: "original-digest",
            retention: "Retained by native job owner",
            state: "Recorded",
        }];
        let view = job(
            &Job {
                source: source(),
                title: "Later turn",
                state: "Running",
                executor: "Current executor",
                placement: "Explicit placement",
                requested_model: None,
                served_model: None,
                usage: None,
                cost: None,
                continuation: None,
                stop_request: "Unknown",
                delivery: "Unknown",
                publish: "Unknown",
                cleanup: "Unknown",
                artifacts: &artifacts,
                attempts: &attempts,
            },
            palette(),
        )
        .validate()
        .unwrap();
        let json = serde_json::to_string(view.view()).unwrap();
        for label in [
            "Attempt: original-attempt",
            "Attempt state: Failed",
            "Attempt executor: Original executor",
            "Served model: served",
            "Session: original-session",
            "Kept: Retained by native job owner",
            "Cleanup: Incomplete",
        ] {
            assert!(json.contains(label), "{label}");
        }
    }
    #[test]
    fn native_gates_keep_stop_review_apply_publish_and_detach_distinct() {
        let denied = Gate {
            enabled: false,
            reason: Some("Current native policy refuses this operation."),
        };
        let allowed = Gate {
            enabled: true,
            reason: None,
        };
        let view = controls(
            &Controls {
                key: "controls",
                submission: denied,
                continuation: denied,
                stop: denied,
                review: denied,
                apply: denied,
                publish: denied,
                detach: allowed,
            },
            palette(),
        )
        .validate()
        .unwrap();
        let activation = |node: &str| Activation {
            instance: view.view().instance.clone(),
            revision: view.view().revision,
            node: node.into(),
        };
        for suffix in ["submit", "continue", "stop", "review", "apply", "publish"] {
            assert!(
                view.activate(&activation(&format!("controls:{suffix}:activate")))
                    .is_err()
            );
        }
        assert_eq!(
            view.activate(&activation("controls:detach:activate"))
                .unwrap(),
            &CoordinationIntent::Detach
        );
    }
}
