//! Settings → Providers (#12283): each computer's agents, as the desktop's
//! Providers and Accounts pages have them (`openagents-ui`'s
//! `settings::harnesses` and `settings::accounts`): install or cancel, turn
//! an agent on or off, its update policy, check for and apply updates, and
//! its accounts (add with a sign-in, switch, remove, and usage). The values
//! are the engine's own (`ListHarnesses`, `WatchHarnessUpdates`,
//! `ListAgentAccounts`), read by the host from the chosen computer.

use openagents_proto::{
    AgentAccount, HarnessId, HarnessUpdatePhase, HarnessUpdatePolicy, HarnessUpdateStatus,
};
use rust_native::Node;
use serde::{Deserialize, Serialize};

use super::{
    Intent, Load, Named, Paints, button, card, danger_button, enabled_button, header, row, section,
    segmented, text, toggle,
};

/// One agent as the computer lists it (`ListHarnesses`).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Agent {
    pub id: HarnessId,
    pub name: String,
    #[serde(default = "yes")]
    pub installed: bool,
    #[serde(default)]
    pub can_install: bool,
    #[serde(default)]
    pub enabled: Option<bool>,
}

fn yes() -> bool {
    true
}

impl Agent {
    /// On: the computer's setting, or for an engine that predates it,
    /// installed.
    pub fn on(&self) -> bool {
        self.enabled.unwrap_or(self.installed)
    }
}

/// A sign-in under way on the computer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Login {
    pub harness: HarnessId,
    pub login_id: String,
    pub url: String,
    /// The person pastes the sign-in page's code back here.
    pub code: bool,
    pub message: Option<String>,
}

/// What the page shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    pub agents: Load<Vec<Agent>>,
    pub updates: Vec<HarnessUpdateStatus>,
    pub accounts: Load<Vec<AgentAccount>>,
    /// The agent being installed.
    pub installing: Option<HarnessId>,
    pub login: Option<Login>,
    /// The account waiting for Remove to be confirmed: harness and ID.
    pub confirm_forget: Option<(HarnessId, String)>,
    pub notice: Option<String>,
}

/// What a control on the page asks for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Install(HarnessId),
    CancelInstall(HarnessId),
    Enable(HarnessId, bool),
    Policy(HarnessId, HarnessUpdatePolicy),
    /// Check one agent for an update, or all of them.
    Check(Option<HarnessId>),
    Apply(HarnessId),
    CancelUpdate(HarnessId),
    AddAccount(HarnessId),
    /// Open the sign-in page again.
    OpenLogin(String),
    CancelLogin,
    Activate(HarnessId, String),
    ConfirmForget(Option<(HarnessId, String)>),
    Forget(HarnessId, String),
}

fn act(action: Action) -> Intent {
    Intent::Providers(action)
}

/// A harness's ID as the engine writes it (`claude-code`, `codex`, ...),
/// for keys.
fn slug(id: HarnessId) -> String {
    serde_json::to_value(id)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// Plain words for where an agent's update is.
pub fn update_line(status: &HarnessUpdateStatus) -> String {
    let version = status.installed_version.as_deref();
    let latest = status.latest_version.as_deref().unwrap_or("a new version");
    match status.phase {
        HarnessUpdatePhase::Dormant => version.map_or_else(String::new, |v| format!("Version {v}")),
        HarnessUpdatePhase::Checking => "Checking for updates…".into(),
        HarnessUpdatePhase::Current => match version {
            Some(v) => format!("Version {v}, up to date"),
            None => "Up to date".into(),
        },
        HarnessUpdatePhase::Available => format!("Update available: {latest}"),
        HarnessUpdatePhase::WaitingForIdle => {
            format!("Updating to {latest} once the agent is idle")
        }
        HarnessUpdatePhase::Preparing
        | HarnessUpdatePhase::Downloading
        | HarnessUpdatePhase::Installing
        | HarnessUpdatePhase::Verifying => format!("Updating to {latest}…"),
        HarnessUpdatePhase::Updated => format!("Updated to {latest}"),
        HarnessUpdatePhase::ManualActionRequired => match &status.manual_command {
            Some(command) => format!("Update by running: {command}"),
            None => "Update it on the computer".into(),
        },
        HarnessUpdatePhase::Failed => status
            .error
            .as_ref()
            .map_or_else(|| "The update failed".into(), |e| e.message.clone()),
    }
}

/// An account's usage, such as "5-hour 40% · Weekly 12%".
pub fn usage_line(account: &AgentAccount) -> String {
    let windows: Vec<String> = account
        .usage_windows
        .iter()
        .map(|w| {
            format!(
                "{} {:.0}%",
                w.label,
                (w.used_fraction * 100.).clamp(0., 100.)
            )
        })
        .collect();
    if windows.is_empty() {
        account.usage_error.clone().unwrap_or_default()
    } else {
        windows.join(" · ")
    }
}

pub(super) fn page(
    state: &State,
    computers: &[Named],
    computer: Option<&str>,
    paints: &Paints,
) -> Vec<Node<Intent>> {
    let mut out = vec![header(
        "providers-title",
        "Providers",
        Some("The agents each computer runs, their updates, and their accounts."),
        paints,
    )];
    if computers.is_empty() {
        out.push(card(
            "providers-none",
            vec![row(
                "providers-none-row",
                "No computers yet",
                Some("Add a computer from the OpenAgents app to manage its agents here."),
                Vec::new(),
                paints,
            )],
            paints,
        ));
        return out;
    }
    let chosen = computer.and_then(|id| computers.iter().position(|c| c.id == id));
    if computers.len() > 1 || chosen.is_none() {
        let options: Vec<(&str, Intent)> = computers
            .iter()
            .map(|c| (c.name.as_str(), Intent::Computer(c.id.clone())))
            .collect();
        out.push(card(
            "providers-computer",
            vec![super::wide_row(
                "providers-computer-pick",
                "Computer",
                None,
                segmented("providers-computer", &options, chosen.unwrap_or(usize::MAX)),
                paints,
            )],
            paints,
        ));
    }
    if chosen.is_none() {
        return out;
    }
    if let Some(notice) = &state.notice {
        out.push(text("providers-notice", notice, paints.danger));
    }
    if let Some(login) = &state.login {
        let words = if login.code {
            "Sign in on the page that opened, then paste its code below."
        } else {
            "Finish signing in on the page that opened."
        };
        let mut controls = Vec::new();
        if !login.url.is_empty() {
            controls.push(button(
                "providers-login-open",
                "Open the sign-in page",
                act(Action::OpenLogin(login.url.clone())),
            ));
        }
        controls.push(button(
            "providers-login-cancel",
            "Cancel",
            act(Action::CancelLogin),
        ));
        out.push(card(
            "providers-login",
            vec![row(
                "providers-login-row",
                "Adding an account",
                Some(login.message.as_deref().unwrap_or(words)),
                controls,
                paints,
            )],
            paints,
        ));
    }
    out.push(agents(state, paints));
    out.push(accounts(state, paints));
    out
}

fn agents(state: &State, paints: &Paints) -> Node<Intent> {
    let list = match &state.agents {
        Load::Ready(list) => list,
        Load::Failed(error) => {
            return section(
                "providers-agents",
                "Agents",
                vec![text("providers-agents-state", error, paints.muted)],
                paints,
            );
        }
        _ => {
            return section(
                "providers-agents",
                "Agents",
                vec![text("providers-agents-state", "Loading…", paints.muted)],
                paints,
            );
        }
    };
    let mut rows = vec![row(
        "providers-check-all",
        "Updates",
        Some("Check every agent on this computer for a new version."),
        vec![button(
            "providers-check-all-button",
            "Check for updates",
            act(Action::Check(None)),
        )],
        paints,
    )];
    for agent in list {
        let key = format!("providers-agent-{}", slug(agent.id));
        let status = state.updates.iter().find(|s| s.harness == agent.id);
        let mut controls = Vec::new();
        let detail = if state.installing == Some(agent.id) {
            controls.push(button(
                &format!("{key}-cancel"),
                "Cancel",
                act(Action::CancelInstall(agent.id)),
            ));
            "Installing…".to_owned()
        } else if !agent.installed {
            if agent.can_install {
                controls.push(button(
                    &format!("{key}-install"),
                    "Install",
                    act(Action::Install(agent.id)),
                ));
            }
            "Not installed".to_owned()
        } else {
            controls.push(toggle(
                &format!("{key}-on"),
                agent.on(),
                act(Action::Enable(agent.id, !agent.on())),
            ));
            status.map(update_line).unwrap_or_default()
        };
        rows.push(row(&key, &agent.name, Some(&detail), controls, paints));
        let Some(status) = status.filter(|_| agent.installed && agent.on()) else {
            continue;
        };
        let policies = [
            ("Ask first", HarnessUpdatePolicy::Notify),
            ("When idle", HarnessUpdatePolicy::AutoWhenIdle),
            ("Off", HarnessUpdatePolicy::Off),
        ];
        let options: Vec<(&str, Intent)> = policies
            .iter()
            .map(|(label, policy)| (*label, act(Action::Policy(agent.id, *policy))))
            .collect();
        let chosen = policies
            .iter()
            .position(|(_, policy)| *policy == status.policy)
            .unwrap_or(usize::MAX);
        let mut actions = segmented(&format!("{key}-policy"), &options, chosen);
        let busy = matches!(
            status.phase,
            HarnessUpdatePhase::Checking
                | HarnessUpdatePhase::Preparing
                | HarnessUpdatePhase::Downloading
                | HarnessUpdatePhase::Installing
                | HarnessUpdatePhase::Verifying
                | HarnessUpdatePhase::WaitingForIdle
        );
        actions.push(enabled_button(
            &format!("{key}-check"),
            "Check",
            !busy,
            act(Action::Check(Some(agent.id))),
        ));
        if status.phase == HarnessUpdatePhase::Available && status.can_apply {
            actions.push(button(
                &format!("{key}-apply"),
                "Update",
                act(Action::Apply(agent.id)),
            ));
        }
        if busy && status.phase != HarnessUpdatePhase::Checking {
            actions.push(button(
                &format!("{key}-cancel-update"),
                "Cancel update",
                act(Action::CancelUpdate(agent.id)),
            ));
        }
        rows.push(super::wide_row(
            &format!("{key}-updates"),
            "Updates",
            None,
            actions,
            paints,
        ));
    }
    card("providers-agents", rows, paints)
}

fn accounts(state: &State, paints: &Paints) -> Node<Intent> {
    let mut rows = Vec::new();
    match &state.accounts {
        Load::Ready(accounts) => {
            for (ix, account) in accounts.iter().enumerate() {
                let key = format!("providers-account-{ix}");
                let name = account
                    .email
                    .clone()
                    .or_else(|| account.plan_label.clone())
                    .unwrap_or_else(|| "Account".into());
                let mut detail = account.plan_label.clone().unwrap_or_default();
                let usage = usage_line(account);
                if !usage.is_empty() {
                    detail = if detail.is_empty() {
                        usage
                    } else {
                        format!("{detail} · {usage}")
                    };
                }
                let pair = (account.harness, account.id.clone());
                let mut controls = Vec::new();
                if state.confirm_forget.as_ref() == Some(&pair) {
                    controls.push(danger_button(
                        &format!("{key}-forget-yes"),
                        "Remove account",
                        act(Action::Forget(account.harness, account.id.clone())),
                        paints,
                    ));
                    controls.push(button(
                        &format!("{key}-forget-no"),
                        "Keep",
                        act(Action::ConfirmForget(None)),
                    ));
                } else {
                    if account.active {
                        controls.push(text(&format!("{key}-active"), "In use", paints.muted));
                    } else {
                        controls.push(button(
                            &format!("{key}-use"),
                            "Use",
                            act(Action::Activate(account.harness, account.id.clone())),
                        ));
                    }
                    controls.push(button(
                        &format!("{key}-forget"),
                        "Remove",
                        act(Action::ConfirmForget(Some(pair))),
                    ));
                }
                rows.push(row(&key, &name, Some(&detail), controls, paints));
            }
            if accounts.is_empty() {
                rows.push(text(
                    "providers-accounts-none",
                    "No accounts on this computer yet.",
                    paints.muted,
                ));
            }
        }
        Load::Failed(error) => rows.push(text("providers-accounts-state", error, paints.muted)),
        _ => rows.push(text("providers-accounts-state", "Loading…", paints.muted)),
    }
    if let Load::Ready(agents) = &state.agents {
        let addable: Vec<&Agent> = agents.iter().filter(|a| a.installed && a.on()).collect();
        if !addable.is_empty() && state.login.is_none() {
            rows.push(super::wide_row(
                "providers-add",
                "Add an account",
                Some("Sign in to another account for an agent on this computer."),
                addable
                    .into_iter()
                    .map(|agent| {
                        button(
                            &format!("providers-add-{}", slug(agent.id)),
                            &agent.name,
                            act(Action::AddAccount(agent.id)),
                        )
                    })
                    .collect(),
                paints,
            ));
        }
    }
    section("providers-accounts", "Accounts", rows, paints)
}
