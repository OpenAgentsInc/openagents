//! Granted computer and task controls over application-supplied native standing.
//! The application owns admission, custody, requests, and original evidence.

use crate::workspace::Palette;
use rust_native::style::{Color, Space, Style};
use rust_native::{Axis, Element, Node, TextRole, View};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Activation resolves against the application's current view and authority.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControlIntent {
    /// Confirm the exact operation supplied by the owning application.
    Confirm,
    Invitation,
    ConfirmEnrollment,
    Prompt,
    Queue,
    Send,
    Steer,
    Cancel,
    Review,
    Publish,
    Detach,
}

#[derive(Clone, Copy)]
pub enum Capacity<'a> {
    Unknown,
    Reported { load: &'a str, available: &'a str },
}

pub struct Host<'a> {
    pub key: &'a str,
    pub host: &'a str,
    pub generation: u64,
    pub workspace: &'a str,
    pub route: &'a str,
    pub version: Option<&'a str>,
    pub capabilities: &'a [&'a str],
    pub capacity: Capacity<'a>,
    pub providers: &'a [&'a str],
    pub observed_at: Option<u64>,
}

#[derive(Clone, Copy)]
pub struct Grant<'a> {
    pub key: &'a str,
    pub host: &'a str,
    pub generation: u64,
    pub workspace: &'a str,
    pub device: &'a str,
    pub id: &'a str,
    pub epoch: u64,
    pub expires_at: u64,
    pub rights: &'a [&'a str],
    pub scope: &'a str,
    pub custody: &'a str,
    pub consent: &'a str,
}

/// The application supplies the exact native gate, including refusal reasons.
#[derive(Clone, Copy)]
pub struct Gate<'a> {
    pub enabled: bool,
    pub reason: Option<&'a str>,
}

pub struct EnrollmentReview<'a> {
    pub key: &'a str,
    pub grant: Grant<'a>,
    pub account: &'a str,
    pub account_workspace: &'a str,
    pub disclosure: &'a str,
    pub confirm: Gate<'a>,
}

#[derive(Clone, Copy)]
pub struct Target<'a> {
    pub host: &'a str,
    pub generation: u64,
    pub workspace: &'a str,
    pub task: Option<&'a str>,
    pub revision: Option<u64>,
    pub attempt: Option<u64>,
}

pub struct Composer<'a> {
    pub key: &'a str,
    pub target: Target<'a>,
    pub engine: Option<&'a str>,
    pub engine_readiness: &'a str,
    pub max_bytes: usize,
    pub enabled: bool,
    pub reason: Option<&'a str>,
}

pub struct Controls<'a> {
    pub key: &'a str,
    pub target: Target<'a>,
    pub queue: Gate<'a>,
    pub send: Gate<'a>,
    pub steer: Gate<'a>,
    pub cancel: Gate<'a>,
    pub review: Gate<'a>,
    pub publish: Gate<'a>,
    pub detach: Gate<'a>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReceiptState {
    Prepared,
    Unknown,
    Answered,
    Refused,
}

pub struct Receipt<'a> {
    pub key: &'a str,
    pub request: &'a str,
    pub request_digest: &'a str,
    pub principal: &'a str,
    pub target: Target<'a>,
    pub state: ReceiptState,
    pub operation: &'a str,
    /// Bounded, original native outcome, admitted by the application.
    pub outcome: Option<&'a Value>,
}

fn text(
    key: impl Into<String>,
    value: impl Into<String>,
    role: TextRole,
    color: Color,
) -> Node<ControlIntent> {
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

fn stack(key: impl Into<String>, children: Vec<Node<ControlIntent>>) -> Node<ControlIntent> {
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

fn fact(
    key: &str,
    suffix: &str,
    label: &str,
    value: &str,
    palette: Palette,
) -> Node<ControlIntent> {
    text(
        format!("{key}:{suffix}"),
        format!("{label}: {value}"),
        TextRole::Body,
        palette.text,
    )
}

fn target(value: Target<'_>, key: &str, palette: Palette) -> Vec<Node<ControlIntent>> {
    let mut rows = vec![
        fact(key, "host", "Host", value.host, palette),
        fact(
            key,
            "generation",
            "Host generation",
            &value.generation.to_string(),
            palette,
        ),
        fact(key, "workspace", "Host workspace", value.workspace, palette),
    ];
    if let Some(task) = value.task {
        rows.push(fact(key, "task", "Task", task, palette));
        rows.push(fact(
            key,
            "revision",
            "Task revision",
            &value
                .revision
                .map_or_else(|| "Not recorded".into(), |value| value.to_string()),
            palette,
        ));
        rows.push(fact(
            key,
            "attempt",
            "Attempt",
            &value
                .attempt
                .map_or_else(|| "Not recorded".into(), |value| value.to_string()),
            palette,
        ));
    }
    rows
}

fn action(
    key: &str,
    label: &str,
    gate: Gate<'_>,
    intent: ControlIntent,
    palette: Palette,
) -> Node<ControlIntent> {
    let mut rows = vec![Node {
        key: format!("{key}:activate"),
        style: Style {
            foreground: Some(palette.text),
            border: Some(palette.border),
            ..Style::default()
        },
        element: Element::Button {
            label: label.into(),
            enabled: gate.enabled,
            icon: None,
            shortcut: None,
            intent,
        },
    }];
    if let Some(reason) = gate.reason {
        rows.push(text(
            format!("{key}:reason"),
            reason,
            TextRole::Status,
            palette.secondary,
        ));
    }
    stack(key, rows)
}

/// One confirmation control for an application-owned, scoped native form.
pub fn submit(key: &str, label: &str, gate: Gate<'_>, palette: Palette) -> View<ControlIntent> {
    View::new_v3(
        "cloud-scoped-submit",
        1,
        action(key, label, gate, ControlIntent::Confirm, palette),
    )
}

pub fn computer(value: &Host<'_>, palette: Palette) -> View<ControlIntent> {
    let key = value.key;
    let mut rows = vec![text(
        format!("{key}:title"),
        "Computer",
        TextRole::Heading,
        palette.heading,
    )];
    rows.extend(target(
        Target {
            host: value.host,
            generation: value.generation,
            workspace: value.workspace,
            task: None,
            revision: None,
            attempt: None,
        },
        key,
        palette,
    ));
    rows.push(fact(key, "route", "Route", value.route, palette));
    rows.push(fact(
        key,
        "version",
        "Reported version",
        value.version.unwrap_or("Unknown"),
        palette,
    ));
    rows.push(fact(
        key,
        "capabilities",
        "Current capabilities",
        &if value.capabilities.is_empty() {
            "Unknown".into()
        } else {
            value.capabilities.join(", ")
        },
        palette,
    ));
    match value.capacity {
        Capacity::Unknown => rows.push(fact(key, "capacity", "Capacity", "Unknown", palette)),
        Capacity::Reported { load, available } => {
            rows.push(fact(key, "load", "Reported load", load, palette));
            rows.push(fact(
                key,
                "capacity",
                "Available capacity",
                available,
                palette,
            ));
        }
    }
    rows.push(fact(
        key,
        "providers",
        "Provider readiness",
        &if value.providers.is_empty() {
            "Unknown".into()
        } else {
            value.providers.join(", ")
        },
        palette,
    ));
    rows.push(fact(
        key,
        "observed",
        "Last checked (Unix time)",
        &value
            .observed_at
            .map_or_else(|| "Unknown".into(), |value| value.to_string()),
        palette,
    ));
    View::new_v3("cloud-computer", 1, stack(key, rows))
}

fn grant_node(value: &Grant<'_>, palette: Palette) -> Node<ControlIntent> {
    let key = value.key;
    let mut rows = vec![text(
        format!("{key}:title"),
        "Device access",
        TextRole::Heading,
        palette.heading,
    )];
    rows.extend(target(
        Target {
            host: value.host,
            generation: value.generation,
            workspace: value.workspace,
            task: None,
            revision: None,
            attempt: None,
        },
        key,
        palette,
    ));
    for (suffix, label, value) in [
        ("id", "Grant", value.id),
        ("device", "Device", value.device),
        ("scope", "Scope", value.scope),
        ("custody", "Keys held by", value.custody),
        ("consent", "Consent", value.consent),
    ] {
        rows.push(fact(key, suffix, label, value, palette));
    }
    rows.push(fact(
        key,
        "expiry",
        "Access expires (Unix time)",
        &value.expires_at.to_string(),
        palette,
    ));
    rows.push(fact(
        key,
        "rights",
        "Allowed actions",
        &if value.rights.is_empty() {
            "None reported".into()
        } else {
            value.rights.join(", ")
        },
        palette,
    ));
    stack(key, rows)
}

pub fn grant(value: &Grant<'_>, palette: Palette) -> View<ControlIntent> {
    View::new_v3("cloud-grant", 1, grant_node(value, palette))
}

pub fn enrollment_review(value: &EnrollmentReview<'_>, palette: Palette) -> View<ControlIntent> {
    let key = value.key;
    let rows = vec![
        text(
            format!("{key}:title"),
            "Review this connection",
            TextRole::Heading,
            palette.heading,
        ),
        fact(key, "account", "Account", value.account, palette),
        fact(
            key,
            "account-workspace",
            "Account workspace",
            value.account_workspace,
            palette,
        ),
        grant_node(&value.grant, palette),
        text(
            format!("{key}:disclosure"),
            value.disclosure,
            TextRole::Body,
            palette.text,
        ),
        action(
            &format!("{key}:confirm"),
            "Confirm and connect",
            value.confirm,
            ControlIntent::ConfirmEnrollment,
            palette,
        ),
    ];
    View::new_v3("cloud-enrollment-review", 1, stack(key, rows))
}

/// Secret invitation text stays in the platform field and starts empty.
pub fn invitation(key: &str, gate: Gate<'_>, palette: Palette) -> View<ControlIntent> {
    let mut rows = vec![Node {
        key: format!("{key}:invitation"),
        style: Style {
            foreground: Some(palette.text),
            border: Some(palette.border),
            ..Style::default()
        },
        element: Element::Field {
            label: "Host invitation".into(),
            value: String::new(),
            placeholder: "Paste the invitation".into(),
            secret: true,
            multiline: false,
            enabled: gate.enabled,
            max_bytes: 8192,
            on_change: ControlIntent::Invitation,
        },
    }];
    if let Some(reason) = gate.reason {
        rows.push(text(
            format!("{key}:reason"),
            reason,
            TextRole::Status,
            palette.secondary,
        ));
    }
    View::new_v3("cloud-host-invitation", 1, stack(key, rows))
}

pub fn composer(value: &Composer<'_>, palette: Palette) -> View<ControlIntent> {
    let key = value.key;
    let mut rows = vec![text(
        format!("{key}:title"),
        "Task request",
        TextRole::Heading,
        palette.heading,
    )];
    rows.extend(target(value.target, key, palette));
    rows.push(fact(
        key,
        "engine",
        "Requested engine",
        value.engine.unwrap_or("Choose an engine"),
        palette,
    ));
    rows.push(fact(
        key,
        "engine-readiness",
        "Engine readiness",
        value.engine_readiness,
        palette,
    ));
    rows.push(Node {
        key: format!("{key}:prompt"),
        style: Style {
            foreground: Some(palette.text),
            border: Some(palette.border),
            ..Style::default()
        },
        element: Element::Field {
            label: "Task".into(),
            value: String::new(),
            placeholder: "Describe the work".into(),
            secret: false,
            multiline: true,
            enabled: value.enabled,
            max_bytes: value.max_bytes,
            on_change: ControlIntent::Prompt,
        },
    });
    if let Some(reason) = value.reason {
        rows.push(text(
            format!("{key}:reason"),
            reason,
            TextRole::Status,
            palette.secondary,
        ));
    }
    View::new_v3("cloud-task-composer", 1, stack(key, rows))
}

pub fn controls(value: &Controls<'_>, palette: Palette) -> View<ControlIntent> {
    let key = value.key;
    let mut rows = vec![text(
        format!("{key}:title"),
        "Task controls",
        TextRole::Heading,
        palette.heading,
    )];
    rows.extend(target(value.target, key, palette));
    for (suffix, label, gate, intent) in [
        ("queue", "Queue task", value.queue, ControlIntent::Queue),
        ("send", "Send task", value.send, ControlIntent::Send),
        ("steer", "Steer task", value.steer, ControlIntent::Steer),
        ("cancel", "Cancel task", value.cancel, ControlIntent::Cancel),
        (
            "review",
            "Review changes",
            value.review,
            ControlIntent::Review,
        ),
        (
            "publish",
            "Publish changes",
            value.publish,
            ControlIntent::Publish,
        ),
        (
            "detach",
            "Stop watching",
            value.detach,
            ControlIntent::Detach,
        ),
    ] {
        rows.push(action(
            &format!("{key}:{suffix}"),
            label,
            gate,
            intent,
            palette,
        ));
    }
    View::new_v3("cloud-task-controls", 1, stack(key, rows))
}

pub fn receipt(value: &Receipt<'_>, palette: Palette) -> View<ControlIntent> {
    let key = value.key;
    let mut rows = vec![text(
        format!("{key}:title"),
        "Request",
        TextRole::Heading,
        palette.heading,
    )];
    rows.extend(target(value.target, key, palette));
    for (suffix, label, value) in [
        ("request", "Request ID", value.request),
        ("principal", "Requested by", value.principal),
        ("operation", "Requested operation", value.operation),
    ] {
        rows.push(fact(key, suffix, label, value, palette));
    }
    rows.push(fact(
        key,
        "state",
        "Request state",
        match value.state {
            ReceiptState::Prepared => "Prepared",
            ReceiptState::Unknown => "Unknown",
            ReceiptState::Answered => "Answered",
            ReceiptState::Refused => "Refused",
        },
        palette,
    ));
    rows.push(text(
        format!("{key}:outcome"),
        value
            .outcome
            .map_or_else(|| "No result yet.".into(), Value::to_string),
        TextRole::Code,
        palette.text,
    ));
    View::new_v3("cloud-request-receipt", 1, stack(key, rows))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_native::Activation;

    fn palette() -> Palette {
        let color = Color::rgb(255, 255, 255);
        Palette {
            text: color,
            heading: color,
            secondary: color,
            border: color,
        }
    }
    fn target() -> Target<'static> {
        Target {
            host: "host-a",
            generation: 7,
            workspace: "workspace-a",
            task: Some("task-a"),
            revision: Some(12),
            attempt: Some(3),
        }
    }
    fn grant_value() -> Grant<'static> {
        Grant {
            key: "grant-a",
            host: "host-a",
            generation: 7,
            workspace: "workspace-a",
            device: "device-a",
            id: "native-grant-a",
            epoch: 4,
            expires_at: 2000,
            rights: &["Observe"],
            scope: "workspace-a only",
            custody: "Explicit server custody",
            consent: "Pending review",
        }
    }

    #[test]
    fn current_gates_keep_queue_send_review_and_publication_distinct() {
        let denied = Gate {
            enabled: false,
            reason: Some("Current grant permits observation only."),
        };
        let allowed = Gate {
            enabled: true,
            reason: None,
        };
        let view = controls(
            &Controls {
                key: "controls",
                target: target(),
                queue: denied,
                send: denied,
                steer: denied,
                cancel: denied,
                review: denied,
                publish: denied,
                detach: allowed,
            },
            palette(),
        )
        .validate()
        .unwrap();
        let activation = |key: &str| Activation {
            instance: view.view().instance.clone(),
            revision: view.view().revision,
            node: key.into(),
        };
        assert!(
            view.activate(&activation("controls:send:activate"))
                .is_err()
        );
        assert!(
            view.activate(&activation("controls:publish:activate"))
                .is_err()
        );
        assert_eq!(
            view.activate(&activation("controls:detach:activate"))
                .unwrap(),
            &ControlIntent::Detach
        );
        let json = serde_json::to_string(view.view()).unwrap();
        for label in [
            "Queue task",
            "Send task",
            "Steer task",
            "Cancel task",
            "Review changes",
            "Publish changes",
            "Task revision: 12",
            "Host generation: 7",
        ] {
            assert!(json.contains(label));
        }
    }

    #[test]
    fn enrollment_review_keeps_exact_grant_and_explicit_custody() {
        let view = enrollment_review(
            &EnrollmentReview {
                key: "enrollment",
                grant: grant_value(),
                account: "account-a",
                account_workspace: "account-workspace-a",
                disclosure: "Use this protected device only for the named account binding.",
                confirm: Gate {
                    enabled: false,
                    reason: Some("Explicit consent is required."),
                },
            },
            palette(),
        )
        .validate()
        .unwrap();
        let json = serde_json::to_string(view.view()).unwrap();
        for label in [
            "native-grant-a",
            "Device: device-a",
            "Access expires (Unix time): 2000",
            "Allowed actions: Observe",
            "Consent: Pending review",
            "Explicit server custody",
        ] {
            assert!(json.contains(label));
        }
        let view = invitation(
            "invitation",
            Gate {
                enabled: false,
                reason: None,
            },
            palette(),
        )
        .validate()
        .unwrap();
        let Element::Stack { children, .. } = &view.view().root.element else {
            panic!("expected invitation field")
        };
        assert!(
            matches!(&children[0].element,Element::Field {secret:true,value,enabled:false,..} if value.is_empty())
        );
    }

    #[test]
    fn missing_capacity_and_executor_are_not_projected_as_ready() {
        let host = computer(
            &Host {
                key: "host",
                host: "host-a",
                generation: 7,
                workspace: "workspace-a",
                route: "Admitted direct route",
                version: None,
                capabilities: &[],
                capacity: Capacity::Unknown,
                providers: &[],
                observed_at: None,
            },
            palette(),
        )
        .validate()
        .unwrap();
        let json = serde_json::to_string(host.view()).unwrap();
        assert!(json.contains("Capacity: Unknown") && json.contains("Provider readiness: Unknown"));
        let view = composer(
            &Composer {
                key: "composer",
                target: target(),
                engine: None,
                engine_readiness: "Unknown",
                max_bytes: 8192,
                enabled: true,
                reason: None,
            },
            palette(),
        )
        .validate()
        .unwrap();
        let json = serde_json::to_string(view.view()).unwrap();
        assert!(json.contains("Requested engine: Choose an engine"));
        let Element::Stack { children, .. } = &view.view().root.element else {
            panic!("expected composer")
        };
        assert!(children.iter().any(|node| matches!(&node.element,Element::Field {value,multiline:true,on_change:ControlIntent::Prompt,..} if value.is_empty())));
    }

    #[test]
    fn unknown_request_and_original_outcome_do_not_invent_completion() {
        let outcome = serde_json::json!({"status":"queued","source":{"task":"task-a","revision":12},"publication":"unknown"});
        let value = Receipt {
            key: "receipt",
            request: "request-a",
            request_digest: "sha256:original",
            principal: "device-a",
            target: target(),
            state: ReceiptState::Unknown,
            operation: "task.create",
            outcome: Some(&outcome),
        };
        let view = receipt(&value, palette()).validate().unwrap();
        let Element::Stack { children, .. } = &view.view().root.element else {
            panic!("expected receipt")
        };
        assert!(children.iter().any(|node| matches!(&node.element,Element::Text {value,role:TextRole::Code} if value==&outcome.to_string())));
        let json = serde_json::to_string(view.view()).unwrap();
        assert!(json.contains("Request state: Unknown"));
        assert!(!json.contains("\"button\"") && !json.contains("Completed"));
    }
}
