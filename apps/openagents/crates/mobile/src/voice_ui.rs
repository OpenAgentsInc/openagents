//! Rust Native projections for the active voice call and voice preferences.
//!
//! These trees carry presentation and typed actions only. `VoiceCall` and the
//! voice-agent picker remain the authority for call and selection changes.

use crate::client_ffi::{
    CoreError, CoreResult, VoiceCallPhase, VoiceCallState, VoiceCallWork, VoiceUsagePhase,
    VoiceUsageSnapshot,
};
use rust_native::{Axis, Element, Node, TextRole, View};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VoiceIntent {
    ToggleMute,
    EndCall,
    OpenCall,
    RefreshUsage,
}

fn text(key: &str, value: impl Into<String>, role: TextRole) -> Node<VoiceIntent> {
    Node {
        key: key.into(),
        style: Default::default(),
        element: Element::Text {
            value: value.into(),
            role,
        },
    }
}

fn button(key: &str, label: &str, intent: VoiceIntent, enabled: bool) -> Node<VoiceIntent> {
    Node {
        key: key.into(),
        style: Default::default(),
        element: Element::Button {
            label: label.into(),
            enabled,
            icon: None,
            shortcut: None,
            intent,
        },
    }
}

fn stack(key: &str, axis: Axis, children: Vec<Node<VoiceIntent>>) -> Node<VoiceIntent> {
    Node {
        key: key.into(),
        style: Default::default(),
        element: Element::Stack { axis, children },
    }
}

fn call_status(state: &VoiceCallState) -> String {
    match state.phase {
        VoiceCallPhase::Connecting => "Connecting".into(),
        VoiceCallPhase::Ending => "Ending call".into(),
        VoiceCallPhase::Active => match state.work {
            VoiceCallWork::Working => "Working".into(),
            VoiceCallWork::AwaitingInput => "Waiting for your answer".into(),
            VoiceCallWork::Idle if state.speaking => "Speaking".into(),
            VoiceCallWork::Idle if state.muted => "Microphone off".into(),
            VoiceCallWork::Idle => "Listening".into(),
        },
    }
}

/// Compact, persistent call control for a host shell to mount above its screen.
pub fn call_capsule(
    instance: &str,
    revision: u64,
    state: &VoiceCallState,
) -> Result<rust_native::ValidatedView<VoiceIntent>, rust_native::ViewError> {
    View::new_v3(
        instance,
        revision,
        stack(
            "voice-capsule",
            Axis::Horizontal,
            vec![
                text("voice-status", call_status(state), TextRole::Status),
                button("open-call", "Open call", VoiceIntent::OpenCall, true),
                button(
                    "toggle-mute",
                    if state.muted {
                        "Turn microphone on"
                    } else {
                        "Mute"
                    },
                    VoiceIntent::ToggleMute,
                    state.phase == VoiceCallPhase::Active,
                ),
                button(
                    "end-call",
                    "End call",
                    VoiceIntent::EndCall,
                    state.phase != VoiceCallPhase::Ending,
                ),
            ],
        ),
    )
    .validate()
}

/// Full call screen. Caption text is rendered from the current ephemeral Rust snapshot.
pub fn call_screen(
    instance: &str,
    revision: u64,
    state: &VoiceCallState,
) -> Result<rust_native::ValidatedView<VoiceIntent>, rust_native::ViewError> {
    let caption = if state.caption.is_empty() {
        "Your conversation will appear here.".to_owned()
    } else {
        state.caption.clone()
    };
    let caption_role = match state.caption_speaker {
        Some(crate::client_ffi::VoiceSpeaker::User) => "You",
        Some(crate::client_ffi::VoiceSpeaker::Assistant) => "Codex",
        None => "",
    };
    let mut children = vec![
        text("voice-title", "Voice call", TextRole::Heading),
        Node {
            key: "voice-orb".into(),
            style: Default::default(),
            element: Element::Surface {
                resource: "voice-orb".into(),
                label: "Voice activity".into(),
            },
        },
        text("voice-status", call_status(state), TextRole::Status),
    ];
    if !caption_role.is_empty() {
        children.push(text(
            "voice-caption-speaker",
            caption_role,
            TextRole::Status,
        ));
    }
    children.extend([
        text("voice-caption", caption, TextRole::Body),
        button(
            "toggle-mute",
            if state.muted {
                "Turn microphone on"
            } else {
                "Mute"
            },
            VoiceIntent::ToggleMute,
            state.phase == VoiceCallPhase::Active,
        ),
        button(
            "end-call",
            "End call",
            VoiceIntent::EndCall,
            state.phase != VoiceCallPhase::Ending,
        ),
    ]);
    View::new_v3(
        instance,
        revision,
        stack("voice-call", Axis::Vertical, children),
    )
    .validate()
}

/// Usage details beside the existing Rust-owned voice-agent picker.
pub fn voice_usage_view(
    instance: &str,
    revision: u64,
    usage: &VoiceUsageSnapshot,
) -> Result<rust_native::ValidatedView<VoiceIntent>, rust_native::ViewError> {
    let mut children = vec![text("voice-usage-title", "Voice usage", TextRole::Heading)];
    match usage.phase {
        VoiceUsagePhase::NoHost => children.push(text(
            "voice-usage-status",
            "Choose a computer for voice calls.",
            TextRole::Status,
        )),
        VoiceUsagePhase::Loading => children.push(text(
            "voice-usage-status",
            "Loading usage",
            TextRole::Status,
        )),
        VoiceUsagePhase::Ready => {}
        VoiceUsagePhase::Offline => children.push(text(
            "voice-usage-status",
            "Computer is offline",
            TextRole::Status,
        )),
        VoiceUsagePhase::SignInRequired => children.push(text(
            "voice-usage-status",
            "Sign in to Codex to see usage.",
            TextRole::Status,
        )),
        VoiceUsagePhase::Unsupported => children.push(text(
            "voice-usage-status",
            "This computer does not support voice calls.",
            TextRole::Status,
        )),
        VoiceUsagePhase::Unavailable => children.push(text(
            "voice-usage-status",
            "Usage is unavailable right now.",
            TextRole::Status,
        )),
    }
    if let Some(host) = &usage.host_name {
        children.push(text("voice-usage-host", host, TextRole::Body));
    }
    if let Some(account) = &usage.account {
        children.push(text("voice-account", account, TextRole::Body));
    }
    if let Some(plan) = &usage.plan {
        children.push(text("voice-plan", plan, TextRole::Status));
    }
    for (index, window) in usage.windows.iter().enumerate() {
        children.push(text(
            &format!("voice-usage-{index}"),
            format!(
                "{}: {}% used",
                window.label,
                (window.used_fraction.clamp(0.0, 1.0) * 100.0).round() as u8
            ),
            TextRole::Body,
        ));
    }
    if usage.refreshing {
        children.push(text(
            "voice-usage-refreshing",
            "Updating usage",
            TextRole::Status,
        ));
    }
    children.push(button(
        "refresh-voice-usage",
        "Refresh usage",
        VoiceIntent::RefreshUsage,
        !usage.refreshing,
    ));
    View::new_v3(
        instance,
        revision,
        stack("voice-settings", Axis::Vertical, children),
    )
    .validate()
}

fn encode_view(
    view: Result<rust_native::ValidatedView<VoiceIntent>, rust_native::ViewError>,
) -> CoreResult<String> {
    let view = view.map_err(|_| CoreError::InvalidArgument {
        message: "Couldn't open voice settings.".into(),
    })?;
    serde_json::to_string(view.view()).map_err(|_| CoreError::InvalidArgument {
        message: "Couldn't open voice settings.".into(),
    })
}

/// Build the compact native tree from the current Rust-owned call snapshot.
#[uniffi::export]
pub fn voice_call_capsule_view(
    instance: String,
    revision: u64,
    state: VoiceCallState,
) -> CoreResult<String> {
    encode_view(call_capsule(&instance, revision, &state))
}

/// Build the full call tree from the current Rust-owned call snapshot.
#[uniffi::export]
pub fn voice_call_screen_view(
    instance: String,
    revision: u64,
    state: VoiceCallState,
) -> CoreResult<String> {
    encode_view(call_screen(&instance, revision, &state))
}

/// Build settings from the existing voice-agent choice and usage snapshots.
#[uniffi::export]
pub fn voice_usage_view_json(
    instance: String,
    revision: u64,
    usage: VoiceUsageSnapshot,
) -> CoreResult<String> {
    encode_view(voice_usage_view(&instance, revision, &usage))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orb::VoiceOrb;

    fn active_state() -> VoiceCallState {
        VoiceCallState {
            phase: VoiceCallPhase::Active,
            orb: VoiceOrb::Listening,
            chat_id: Some("chat-1".into()),
            work: VoiceCallWork::Idle,
            muted: false,
            speaking: false,
            caption: "Hello".into(),
            caption_speaker: Some(crate::client_ffi::VoiceSpeaker::Assistant),
            caption_item: Some("item-1".into()),
            microphone: 0.2,
            speaker: 0.0,
            voices: vec!["Breeze".into(), "Cove".into()],
        }
    }

    #[test]
    fn call_views_are_valid_and_keep_actions_typed() {
        let state = active_state();
        assert!(call_capsule("call-capsule", 1, &state).is_ok());
        assert!(call_screen("call-screen", 1, &state).is_ok());
    }

    #[test]
    fn usage_view_keeps_saved_voice_selection_in_existing_picker() {
        let usage = VoiceUsageSnapshot {
            phase: VoiceUsagePhase::Ready,
            host_id: Some("host-1".into()),
            host_name: Some("Office computer".into()),
            account: Some("test@example.com".into()),
            plan: Some("Plus".into()),
            windows: vec![],
            fetched_at_ms: Some(1),
            error: None,
            refreshing: false,
        };
        assert!(voice_usage_view("voice-settings", 1, &usage).is_ok());
    }
}
