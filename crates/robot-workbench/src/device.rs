//! Shared native preview controls for devices without an inline 3D renderer.
use crate::view::State;
use robot_lab::{Reference, remote::Control};
use rust_native::{Axis, Element, Node, TextRole, ValidatedView, View};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Intent {
    Joint(String),
    MoveJoint { increase: bool },
    More(u32),
    Close,
    Refresh,
    Open(Reference),
    Previous,
    Next,
    Play,
    Pause,
    Run { experiment: Reference, seed: u64 },
    Control(Control),
    OpenSaved(Reference),
    NewRun,
    Watch(String),
}
pub struct Card<'a> {
    pub next: Option<u32>,
    pub previous: Option<u32>,
    pub sensor_resource: Option<&'a str>,
    pub state: Option<&'a State>,
    pub resource: Option<&'a str>,
    pub jobs: &'a [String],
    pub documents: &'a [(Reference, String)],
    pub experiments: &'a [(Reference, u64)],
    pub operate: bool,
    pub controllable: bool,
    pub execution: bool,
    pub playing: bool,
    pub busy: bool,
    pub job: Option<&'a str>,
    pub status: &'a str,
    pub saved: Option<&'a Reference>,
    pub error: Option<&'a str>,
}
pub fn card(
    card: Card<'_>,
    instance: &str,
    revision: u64,
) -> Result<ValidatedView<Intent>, String> {
    let node = |key: &str, element| Node {
        key: key.into(),
        style: Default::default(),
        element,
    };
    let text = |key: &str, value: String, role| node(key, Element::Text { value, role });
    let button = |key: &str, label: &str, intent| {
        node(
            key,
            Element::Button {
                label: label.into(),
                enabled: !card.busy,
                icon: None,
                shortcut: None,
                intent,
            },
        )
    };
    let mut children = vec![
        text("title", "Robot Lab".into(), TextRole::Heading),
        node(
            "close",
            Element::Button {
                label: "Close".into(),
                enabled: true,
                icon: None,
                shortcut: None,
                intent: Intent::Close,
            },
        ),
        button("refresh", "Refresh", Intent::Refresh),
    ];
    if let Some(error) = card.error {
        children.push(text("error", error.into(), TextRole::Status));
    }
    if card.busy {
        children.push(text("loading", "Opening…".into(), TextRole::Status));
    }
    if let Some(state) = card.state {
        let bundle = state.bundle();
        children.push(text("name", bundle.scene.name.clone(), TextRole::Heading));
        children.push(text(
            "mode",
            format!("{} · Version {}", bundle.mode(), bundle.number),
            TextRole::Status,
        ));
        if let Some(resource) = card.resource {
            children.push(node(
                "preview",
                Element::Surface {
                    resource: resource.into(),
                    label: format!("{} at frame {}", bundle.scene.name, state.frame),
                },
            ));
        }
        children.push(text("positions", state.readout(), TextRole::Body));
        let positions = state
            .step()
            .map_or(&bundle.scene.robots[0].joints, |step| &step.joints);
        for (index, joint) in bundle
            .robot
            .joints
            .iter()
            .filter(|joint| joint.kind != robot_lab::document::JointKind::Fixed)
            .enumerate()
        {
            let unit = if joint.kind == robot_lab::document::JointKind::Prismatic {
                "m"
            } else {
                "rad"
            };
            let value = positions.get(&joint.name).copied().unwrap_or_default();
            children.push(button(
                &format!("joint-{index}"),
                &format!("{}: {value:.3} {unit}", joint.name),
                Intent::Joint(joint.name.clone()),
            ));
        }
        if card.operate && bundle.episode.is_none() {
            children.push(button(
                "less",
                "Move joint −",
                Intent::MoveJoint { increase: false },
            ));
            children.push(button(
                "more",
                "Move joint +",
                Intent::MoveJoint { increase: true },
            ));
        }
        if let Some(episode) = &bundle.episode {
            children.push(text(
                "frame",
                format!(
                    "Frame {} of {} · {:.3} s",
                    state.frame + 1,
                    episode.steps.len(),
                    episode.steps[state.frame].time_seconds
                ),
                TextRole::Body,
            ));
            children.push(button("previous", "Previous frame", Intent::Previous));
            children.push(button("next", "Next frame", Intent::Next));
            children.push(button(
                "play",
                if card.playing { "Pause" } else { "Play" },
                if card.playing {
                    Intent::Pause
                } else {
                    Intent::Play
                },
            ));
        }
        // Camera names and joint positions remain available to screen readers.
        for (i, camera) in bundle.scene.cameras.iter().enumerate() {
            children.push(text(
                &format!("camera-{i}"),
                format!("Camera: {}", camera.id),
                TextRole::Body,
            ));
        }
    }
    if let Some(job) = card.job {
        if let Some(resource) = card.sensor_resource {
            children.push(node(
                "live-camera",
                Element::Surface {
                    resource: resource.into(),
                    label: "Latest robot camera image".into(),
                },
            ));
        }
        children.push(text(
            "job",
            format!("Run {job} · {}", card.status),
            TextRole::Status,
        ));
        if card.operate && card.controllable && card.saved.is_none() && card.execution {
            for (key, label, command) in [
                ("resume", "Resume", Control::Resume),
                ("pause-run", "Pause run", Control::Pause),
                ("step", "Step", Control::Step),
                ("stop", "Stop", Control::Stop),
            ] {
                children.push(button(key, label, Intent::Control(command)));
            }
        }
        if let Some(saved) = card.saved {
            children.push(button(
                "saved",
                "Open saved attempt",
                Intent::OpenSaved(saved.clone()),
            ));
        }
    }
    if card.job.is_some() {
        children.push(button("another", "Show experiments", Intent::NewRun));
    }
    if card.operate && card.execution && card.job.is_none() {
        for (i, (experiment, seed)) in card.experiments.iter().enumerate() {
            children.push(button(
                &format!("run-{i}"),
                &format!("Run batch {} · seed {seed}", experiment.document),
                Intent::Run {
                    experiment: experiment.clone(),
                    seed: *seed,
                },
            ));
        }
    }
    for (i, job) in card.jobs.iter().enumerate() {
        children.push(button(
            &format!("watch-{i}"),
            &format!("Open run {job}"),
            Intent::Watch(job.clone()),
        ));
    }
    for (i, (reference, title)) in card.documents.iter().enumerate() {
        children.push(button(
            &format!("document-{i}"),
            title,
            Intent::Open(reference.clone()),
        ));
    }
    if let Some(offset) = card.previous {
        children.push(button(
            "previous-page",
            "Previous scenes and runs",
            Intent::More(offset),
        ));
    }
    if let Some(offset) = card.next {
        children.push(button(
            "more-scenes",
            "More scenes and runs",
            Intent::More(offset),
        ));
    }
    View::new(
        instance,
        revision,
        node(
            "robot-card",
            Element::Stack {
                axis: Axis::Vertical,
                children,
            },
        ),
    )
    .validate()
    .map_err(|e| e.to_string())
}
