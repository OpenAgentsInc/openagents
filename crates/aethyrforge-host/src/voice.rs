//! Voice input and buttons select the same reviewed operations; speech grants no authority.
use crate::{Action, Capability, Project, SelectedObject, ViewMode};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputContext {
    pub session: String,
    pub project: Project,
    pub selection: Option<SelectedObject>,
}
/// A request is usable only in the world and selection where it was made.
#[derive(Default)]
pub struct InputTurn {
    epoch: u64,
    context: Option<InputContext>,
}
impl InputTurn {
    pub fn begin(&mut self, context: InputContext) -> u64 {
        self.cancel();
        self.context = Some(context);
        self.epoch
    }
    pub fn cancel(&mut self) {
        self.epoch = self.epoch.wrapping_add(1);
        self.context = None;
    }
    pub fn current(&self, epoch: u64, context: &InputContext) -> bool {
        self.epoch == epoch && self.context.as_ref() == Some(context)
    }
    pub fn active(&self) -> bool {
        self.context.is_some()
    }
    pub fn changed(&self, context: &InputContext) -> bool {
        self.context.as_ref().is_some_and(|c| c != context)
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum RequestControl {
    Review(Action),
    Inspect,
    Map,
    Options,
    Stop,
    AskKiln,
    ReviewCandidate,
    ConfirmChange,
    CancelReview,
    Option(TutorialOption),
}
/// Only exact control names are shortcuts. Natural-language editing stays in Kiln's
/// bounded project workspace, rather than guessing targets or granting mutations.
pub fn request_control(
    text: &str,
    capabilities: &[Capability],
) -> Result<RequestControl, &'static str> {
    let text = text.trim();
    if text.is_empty() || text.len() > 4800 || text.contains('\0') {
        return Err("Describe one change in a short request.");
    }
    let words = text.trim_end_matches(['.', '!']).to_ascii_lowercase();
    let control = match words.as_str() {
        "stop" | "mute" => RequestControl::Stop,
        "cancel" | "cancel change" => RequestControl::CancelReview,
        "review change" | "review kiln change" => RequestControl::ReviewCandidate,
        "confirm change" => RequestControl::ConfirmChange,
        "show options" | "show me the options" | "options" => RequestControl::Options,
        "inspect" | "inspect world" => RequestControl::Inspect,
        "map" | "back to planets" | "open the map" => RequestControl::Map,
        "mesh" | "show mesh" => RequestControl::Review(Action::View {
            mode: ViewMode::Mesh,
        }),
        "shaded" | "hide mesh" => RequestControl::Review(Action::View {
            mode: ViewMode::Shaded,
        }),
        "compile" | "compile world effect" => RequestControl::Review(Action::Compile {
            asset: Some(crate::EFFECT_ASSET.into()),
            source_sha256: None,
        }),
        "undo" | "undo last change" => RequestControl::Review(Action::Undo),
        "redo" => RequestControl::Review(Action::Redo),
        // These controls are added to the same registry when their owning features exist.
        // A request must never manufacture a playable link or publish by implication.
        "play" => RequestControl::Option(TutorialOption::Play),
        "share" => RequestControl::Option(TutorialOption::Share),
        "publish" => RequestControl::Option(TutorialOption::Publish),
        "save" => RequestControl::Option(TutorialOption::Save),
        "revert" => RequestControl::Option(TutorialOption::Revert),
        "change art style" => RequestControl::Option(TutorialOption::ArtStyle),
        "set time" => RequestControl::Option(TutorialOption::Time),
        "day night cycle" => RequestControl::Option(TutorialOption::Cycle),
        _ => RequestControl::AskKiln,
    };
    if let RequestControl::Review(action) = &control {
        if action
            .capability()
            .is_some_and(|c| !capabilities.contains(&c))
        {
            return Err("This option isn't available in this world.");
        }
    }
    Ok(control)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn context() -> InputContext {
        serde_json::from_value(serde_json::json!({"session":"s1","project":{"id":"haunted-house","branch":"main","revision":2},"selection":{"id":"wolf","asset":"custom:haunt:werewolf","position":[0,0,0]}})).unwrap()
    }
    #[test]
    fn canceled_and_changed_input_cannot_reuse_a_turn() {
        let mut turn = InputTurn::default();
        let c = context();
        let first = turn.begin(c.clone());
        assert!(turn.current(first, &c));
        let mut changed = c.clone();
        changed.project.revision += 1;
        assert!(turn.changed(&changed));
        assert!(!turn.current(first, &changed));
        changed = c.clone();
        changed.selection = None;
        assert!(!turn.current(first, &changed));
        changed = c.clone();
        changed.session = "s2".into();
        assert!(!turn.current(first, &changed));
        turn.cancel();
        assert!(!turn.current(first, &c));
        let second = turn.begin(c.clone());
        assert_ne!(first, second);
        assert!(!turn.current(first, &c));
        assert!(turn.current(second, &c));
    }
    #[test]
    fn spoken_confirmation_is_explicit_and_downstream_options_have_closed_control_ids() {
        assert_eq!(
            request_control("confirm change", &[]).unwrap(),
            RequestControl::ConfirmChange
        );
        assert_eq!(
            request_control("review change", &[]).unwrap(),
            RequestControl::ReviewCandidate
        );
        assert_eq!(
            request_control("yes", &[]).unwrap(),
            RequestControl::AskKiln
        );
        assert_eq!(
            request_control("publish", &[]).unwrap(),
            RequestControl::Option(TutorialOption::Publish)
        );
        let options = [
            TutorialOption::Play,
            TutorialOption::Share,
            TutorialOption::Publish,
            TutorialOption::Save,
            TutorialOption::Revert,
            TutorialOption::ArtStyle,
            TutorialOption::Time,
            TutorialOption::Cycle,
        ];
        let ids: std::collections::BTreeSet<_> =
            options.map(|o| o.control_id()).into_iter().collect();
        assert_eq!(ids.len(), 8);
    }
    #[test]
    fn controls_are_capability_checked_and_semantic_edits_remain_proposals() {
        assert_eq!(
            request_control("Undo.", &[Capability::Undo]).unwrap(),
            RequestControl::Review(Action::Undo)
        );
        assert!(request_control("undo", &[]).is_err());
        assert_eq!(
            request_control("Do not undo; make the fur rougher", &[]).unwrap(),
            RequestControl::AskKiln
        );
        assert_eq!(
            request_control("Add five huge pumpkins to my left", &[]).unwrap(),
            RequestControl::AskKiln
        );
        assert_eq!(
            request_control("show me the options", &[]).unwrap(),
            RequestControl::Options
        );
        assert_eq!(
            request_control("publish", &[Capability::Publish]).unwrap(),
            RequestControl::Option(TutorialOption::Publish)
        );
        assert!(request_control("", &[]).is_err());
    }
}

/// Closed names shared by tutorial voice and each owning feature's real controls.
/// An option is unavailable until its surface registers a visible, enabled control.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TutorialOption {
    Play,
    Share,
    Publish,
    Save,
    Revert,
    ArtStyle,
    Time,
    Cycle,
}
impl TutorialOption {
    pub fn control_id(self) -> &'static str {
        match self {
            Self::Play => "project-play",
            Self::Share => "project-share",
            Self::Publish => "project-publish",
            Self::Save => "project-save",
            Self::Revert => "project-revert",
            Self::ArtStyle => "planet-art-style",
            Self::Time => "planet-time",
            Self::Cycle => "planet-cycle",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreatorAsset {
    pub id: String,
    pub name: String,
}
impl CreatorAsset {
    pub fn valid(&self) -> bool {
        crate::protocol::id(&self.id) && !self.name.is_empty() && self.name.len() <= 128
    }
}

/// A browser speech-service refusal is distinct from microphone permission denial.
pub(crate) fn speech_problem(code: &str) -> &'static str {
    match code {
        "not-allowed" => {
            "Microphone access was denied. Type your request or allow access in your browser."
        }
        "service-not-allowed" => {
            "Your browser's speech service isn't available. Type your request or try another browser."
        }
        "audio-capture" => "The microphone isn't available. Check it or type your request.",
        "network" => "Voice input couldn't connect. Type your request or try again.",
        _ => "Voice input stopped. Type your request or try again.",
    }
}
#[cfg(test)]
#[test]
fn speech_refusals_distinguish_permission_service_and_network() {
    assert!(speech_problem("not-allowed").contains("access was denied"));
    assert!(speech_problem("service-not-allowed").contains("speech service"));
    assert!(speech_problem("network").contains("couldn't connect"));
}
