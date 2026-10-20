//! Settings → Earn for a paired computer (#12294), in the desktop Earn
//! page's words (`openagents-ui`'s `settings::earn`). The computer runs its
//! own `openagents pylon` command for each read and change; the host sends
//! the `Pylon*` calls and fills [`State`]. Where earnings go is shown, and
//! changed only at the computer.

use rust_native::Node;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    Intent, Load, Paints, Remote, card, enabled_button, header, row, section, segmented, text,
    toggle, wide_row,
};
use crate::earn::{Earnings, pauses, status_line};

/// What the desktop says when a command isn't in the computer's build.
pub const NOT_AVAILABLE: &str = "Not available on this computer yet";

/// What the page read from the chosen computer.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    /// `pylon status`, `earnings`, and `settings`'s data.
    pub status: Load<Value>,
    pub earnings: Load<Value>,
    pub settings: Load<Value>,
    /// The computer's command line doesn't have Earn.
    pub unavailable: bool,
    /// Turning on still sets up on the computer.
    pub setting_up: bool,
    /// A change is on its way.
    pub busy: bool,
}

impl State {
    /// Whether sharing is on, as the desktop's switch reads it.
    pub fn sharing(&self) -> bool {
        self.setting_up
            || self
                .status
                .ready()
                .and_then(|d| d["state"].as_str())
                .is_some_and(|s| s != "off")
    }
}

/// What a control on the page asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    TurnOn,
    TurnOff,
    PauseOnBattery(bool),
    PauseWhenHot(bool),
}

fn act(action: Action) -> Intent {
    Intent::Earn(action)
}

pub(super) fn page(remote: &Remote, paints: &Paints) -> Vec<Node<Intent>> {
    let mut out = vec![header(
        "earn-title",
        "Earn",
        Some("Let a computer do paid work for others while you aren't using it."),
        paints,
    )];
    if remote.computers.is_empty() {
        out.push(card(
            "earn-none",
            vec![row(
                "earn-none-row",
                "Share a computer and earn",
                Some("Add a computer from the OpenAgents app to earn with it here."),
                Vec::new(),
                paints,
            )],
            paints,
        ));
        return out;
    }
    let chosen = remote
        .computer
        .as_deref()
        .and_then(|id| remote.computers.iter().position(|c| c.id == id));
    if remote.computers.len() > 1 || chosen.is_none() {
        let options: Vec<(&str, Intent)> = remote
            .computers
            .iter()
            .map(|c| (c.name.as_str(), Intent::Computer(c.id.clone())))
            .collect();
        out.push(card(
            "earn-computer-card",
            vec![wide_row(
                "earn-computer",
                "Computer",
                None,
                segmented("earn-computer", &options, chosen.unwrap_or(usize::MAX)),
                paints,
            )],
            paints,
        ));
    }
    if chosen.is_none() {
        return out;
    }
    let state = &remote.earn;
    if state.unavailable {
        out.push(section(
            "earn-share",
            "This computer",
            vec![row(
                "earn-share-row",
                "Share this computer and earn",
                Some(NOT_AVAILABLE),
                Vec::new(),
                paints,
            )],
            paints,
        ));
        return out;
    }
    let status = match &state.status {
        Load::Ready(data) => status_line(data, state.setting_up),
        Load::Failed(_) => "Couldn't read this computer's state".into(),
        _ if state.setting_up => "Setting up".into(),
        _ => "Checking this computer…".into(),
    };
    let on = state.sharing();
    out.push(section(
        "earn-share",
        "This computer",
        vec![row(
            "earn-share-row",
            "Share this computer and earn",
            Some(&status),
            vec![enabled_button(
                "earn-switch",
                if on { "Turn off" } else { "Turn on" },
                !state.busy && !matches!(state.status, Load::Idle | Load::Loading),
                act(if on { Action::TurnOff } else { Action::TurnOn }),
            )],
            paints,
        )],
        paints,
    ));
    let earned = match &state.earnings {
        Load::Ready(data) => {
            let earned = Earnings::from(data);
            let (title, line) = earned.lines();
            let goes = match &earned.elsewhere {
                Some(address) => format!("Earnings go to {address}"),
                None => "Earnings go to this computer's wallet".into(),
            };
            vec![
                row("earn-earned", &title, Some(&line), Vec::new(), paints),
                row("earn-goes", &goes, None, Vec::new(), paints),
            ]
        }
        Load::Failed(_) => vec![row(
            "earn-earned",
            "Earned so far",
            Some("Couldn't read your earnings right now"),
            Vec::new(),
            paints,
        )],
        _ => vec![row(
            "earn-earned",
            "Earned so far",
            None,
            Vec::new(),
            paints,
        )],
    };
    out.push(section("earn-earnings", "Earnings", earned, paints));
    let pause_rows = match state.settings.ready().map(pauses) {
        Some((battery, hot)) => vec![
            row(
                "earn-battery",
                "Pause on battery",
                None,
                vec![toggle(
                    "earn-battery-toggle",
                    battery,
                    act(Action::PauseOnBattery(!battery)),
                )],
                paints,
            ),
            row(
                "earn-hot",
                "Pause when hot",
                None,
                vec![toggle(
                    "earn-hot-toggle",
                    hot,
                    act(Action::PauseWhenHot(!hot)),
                )],
                paints,
            ),
        ],
        None => vec![text(
            "earn-pauses-state",
            if matches!(state.settings, Load::Failed(_)) {
                "Couldn't read the pauses right now"
            } else {
                "Loading…"
            },
            paints.muted,
        )],
    };
    out.push(section("earn-pauses", "Pauses", pause_rows, paints));
    out
}
