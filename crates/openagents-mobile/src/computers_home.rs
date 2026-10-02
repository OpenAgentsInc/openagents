//! The Computers list the Account tab shows as a native list: one row per
//! computer with its name, a short status, and the choices its menu offers.
//!
//! Coder's shared Computers screens still draw everything past the list
//! (a computer's status, order work, terminal, access, activity, and adding
//! a computer). Every choice here becomes a typed `coder_computers::Intent`
//! and runs through `Computers::perform`, the same authority check the
//! shared screens use; a row offers only the choices that check allows.

use coder_computers::authority::check;
use coder_computers::{
    Action, Capabilities, Computers, DataState, DirectoryState, HostRecord, HostStatus, Intent,
    OfflineCause, Outcome, Refusal, Screen, Snapshot,
};
use coder_reach::hints::Class;
use serde::{Deserialize, Serialize};

/// How a row's status reads at a glance. The word always says the same.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    Online,
    Pending,
    Offline,
    Alert,
}

/// A choice a row's menu or the list's menu offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Choice {
    SwitchOn,
    SwitchOff,
    TryNow,
    Access,
    AddToDirectory,
    Rename,
    ChangeWeight,
    RemoveFromDirectory,
    /// Removes the computer from this device's list. The host asks the
    /// person to confirm before it sends this.
    Forget,
}

/// Where the list's own controls lead.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Destination {
    /// The list itself.
    Home,
    Add,
    Activity,
    /// Enter the owner key to read the owner directory.
    OwnerKey,
    /// Keep this device's version of a conflicting directory.
    KeepDirectory,
    Refresh,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Item {
    pub choice: Choice,
    pub label: &'static str,
    /// For a destructive choice, the question the host asks before it sends
    /// the choice.
    pub confirm: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Row {
    pub host: String,
    pub name: String,
    pub status: String,
    pub tone: Tone,
    /// The background watchers an online computer runs, the line the
    /// terminal and desktop show: "1 background watcher · disk cleanup".
    /// `None` when it runs none, is not online, or has not said.
    pub watchers: Option<String>,
    pub menu: Vec<Item>,
}

/// The Computers list, present while the shared screens are on their list.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Home {
    pub rows: Vec<Row>,
    /// What an empty list says.
    pub empty: Option<&'static str>,
    /// The latest outcome or refusal, such as "Switched off Studio.".
    pub notice: Option<String>,
    /// Owner-directory controls for the list's menu, when they apply.
    pub owner_key: bool,
    pub keep_directory: bool,
    /// The row that opens **Connect a computer** (`SCR-22`), the scanner.
    pub connect: &'static str,
    /// The list menu's entry for Coder's other ways to add a computer: a
    /// pasted invitation, SSH, or an approval code.
    pub add_other: &'static str,
}

pub fn home(computers: &Computers, caps: Capabilities) -> Option<Home> {
    if computers.screen() != &Screen::Computers {
        return None;
    }
    let snapshot = computers.snapshot();
    let notice = computers
        .notice()
        .map(|notice| notice.text.clone())
        .or_else(|| match &snapshot.service {
            coder_computers::ServiceState::Unavailable { reason } => Some(reason.clone()),
            _ => None,
        });
    Some(Home {
        rows: snapshot
            .hosts
            .iter()
            .map(|host| row(snapshot, caps, host))
            .collect(),
        empty: snapshot
            .hosts
            .is_empty()
            .then_some("No computers yet. Connect one to reach it from this device."),
        notice,
        owner_key: snapshot.directory == DirectoryState::NoOwnerKey
            && check(snapshot, caps, Action::ImportOwnerKey).is_ok(),
        keep_directory: matches!(snapshot.directory, DirectoryState::Conflict { .. }),
        connect: "Connect a computer",
        add_other: "Add another way",
    })
}

/// A status word, with the route when online. `status_line` in
/// `coder-computers` keeps the full sentence for the computer's own screen.
pub fn short_status(host: &HostRecord, now: u64) -> (String, Tone) {
    match HostStatus::derive(host, now) {
        HostStatus::Online { data } => {
            let route = if host.tunnel.is_some_and(|tunnel| tunnel.in_use) {
                Some("SSH tunnel")
            } else {
                host.route.map(|class| match class {
                    Class::Loopback => "This computer",
                    Class::Lan => "Local network",
                    Class::Tailnet => "Tailnet",
                    Class::Public => "Internet",
                    Class::Relay => "Relay",
                })
            };
            let word = match data {
                DataState::Current => "Online",
                DataState::CatchingUp => "Catching up",
                DataState::UpdatesFailed => "Updates failed",
            };
            let tone = if data == DataState::UpdatesFailed {
                Tone::Alert
            } else {
                Tone::Online
            };
            (
                route.map_or_else(|| word.to_owned(), |route| format!("{word} · {route}")),
                tone,
            )
        }
        HostStatus::Connecting { .. } => ("Connecting".into(), Tone::Pending),
        HostStatus::Offline { cause } => match cause {
            OfflineCause::SwitchedOff => ("Switched off".into(), Tone::Offline),
            OfflineCause::Refused | OfflineCause::Misconfigured => ("Blocked".into(), Tone::Alert),
            OfflineCause::NotConnected
            | OfflineCause::NoNetwork
            | OfflineCause::Retrying { .. } => ("Offline".into(), Tone::Offline),
        },
        HostStatus::OutOfDate { .. } => ("Needs an update".into(), Tone::Alert),
        HostStatus::NotEnrolled { .. } => ("No access".into(), Tone::Offline),
        HostStatus::Revoked => ("Revoked".into(), Tone::Alert),
    }
}

fn row(snapshot: &Snapshot, caps: Capabilities, host: &HostRecord) -> Row {
    let (status, tone) = short_status(host, snapshot.now);
    let watchers = matches!(
        HostStatus::derive(host, snapshot.now),
        HostStatus::Online { .. }
    )
    .then(|| {
        host.watchers
            .as_deref()
            .and_then(openagents_chat_app::watchers::line)
    })
    .flatten();
    let key = host.key.as_str();
    let mut menu = vec![];
    let offer = |menu: &mut Vec<Item>, choice, label, confirm, action: Option<Action>| {
        if action.is_none_or(|action| check(snapshot, caps, action).is_ok()) {
            menu.push(Item {
                choice,
                label,
                confirm,
            });
        }
    };
    let name = &host.label;
    if !host.directory_only() {
        let enabled = host.link.is_none_or(|link| link.enabled);
        let (choice, label) = if enabled {
            (Choice::SwitchOff, "Switch off")
        } else {
            (Choice::SwitchOn, "Switch on")
        };
        offer(
            &mut menu,
            choice,
            label,
            None,
            Some(Action::SetEnabled { host: key }),
        );
        if enabled
            && matches!(
                HostStatus::derive(host, snapshot.now),
                HostStatus::Offline { .. }
            )
        {
            offer(
                &mut menu,
                Choice::TryNow,
                "Try now",
                None,
                Some(Action::RetryNow { host: key }),
            );
        }
    }
    offer(&mut menu, Choice::Access, "Access", None, None);
    if !host.directory_only()
        && host.listing.is_none()
        && snapshot.directory != DirectoryState::NoOwnerKey
    {
        offer(
            &mut menu,
            Choice::AddToDirectory,
            "Add to directory",
            None,
            Some(Action::ListInDirectory { host: key }),
        );
    }
    if host.listing.is_some() && snapshot.directory != DirectoryState::NoOwnerKey {
        let revision = snapshot.directory.revision().unwrap_or(0);
        // The same questions as the shared screens' confirmations.
        let delist = if host.enrollment.rights(snapshot.now).is_some() {
            format!(
                "Remove {name} from your directory? It gets no new work. This device keeps its access, and the computer keeps running."
            )
        } else {
            format!(
                "Remove {name} from your directory? This device has no access to it, so it leaves this list."
            )
        };
        for (choice, label, confirm) in [
            (Choice::Rename, "Rename", None),
            (Choice::ChangeWeight, "Change weight", None),
            (
                Choice::RemoveFromDirectory,
                "Remove from directory",
                Some(delist),
            ),
        ] {
            offer(
                &mut menu,
                choice,
                label,
                confirm,
                Some(Action::EditListing {
                    host: key,
                    revision,
                }),
            );
        }
    }
    if !host.directory_only() {
        offer(
            &mut menu,
            Choice::Forget,
            "Forget",
            Some(format!(
                "Forget {name}? This device stops connecting and removes it from this list. The computer keeps this device's access until you revoke it."
            )),
            Some(Action::Forget { host: key }),
        );
    }
    Row {
        host: host.key.clone(),
        name: host.label.clone(),
        status,
        tone,
        watchers,
        menu,
    }
}

/// Open one computer's own screen.
pub fn open(computers: &mut Computers, host: &str) -> Result<Outcome, Refusal> {
    computers.perform(Intent::Show {
        screen: Screen::Host { host: host.into() },
    })
}

/// Run a row's choice. A destructive choice arrives already confirmed by
/// the host, so its confirmation step runs here too.
pub fn choose(computers: &mut Computers, host: &str, choice: Choice) -> Result<Outcome, Refusal> {
    let host = host.to_owned();
    let revision = computers.snapshot().directory.revision().unwrap_or(0);
    match choice {
        Choice::SwitchOn | Choice::SwitchOff => computers.perform(Intent::SetEnabled {
            host,
            enabled: choice == Choice::SwitchOn,
        }),
        Choice::TryNow => computers.perform(Intent::RetryNow { host }),
        Choice::Access => computers.perform(Intent::Show {
            screen: Screen::Access { host },
        }),
        Choice::AddToDirectory => computers.perform(Intent::ListInDirectory { host }),
        Choice::Rename => computers.perform(Intent::EditLabel { host, revision }),
        Choice::ChangeWeight => computers.perform(Intent::EditWeight { host, revision }),
        Choice::RemoveFromDirectory => {
            computers.perform(Intent::RemoveFromDirectory {
                host: host.clone(),
                revision,
            })?;
            computers.perform(Intent::ConfirmRemoveFromDirectory { host, revision })
        }
        Choice::Forget => {
            computers.perform(Intent::Forget { host: host.clone() })?;
            computers.perform(Intent::ConfirmForget { host })
        }
    }
}

pub fn go(computers: &mut Computers, to: Destination) -> Result<Outcome, Refusal> {
    match to {
        Destination::Home => computers.perform(Intent::Show {
            screen: Screen::Computers,
        }),
        Destination::Add => computers.perform(Intent::Show {
            screen: Screen::Add,
        }),
        Destination::Activity => computers.perform(Intent::Show {
            screen: Screen::Activity,
        }),
        Destination::OwnerKey => computers.perform(Intent::ImportOwnerKey),
        Destination::KeepDirectory => match computers.snapshot().directory {
            DirectoryState::Conflict { revision } => {
                computers.perform(Intent::KeepDirectory { revision })
            }
            _ => Err(Refusal::Stale),
        },
        Destination::Refresh => computers.perform(Intent::Refresh),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{App, Config, Launch, Request};

    fn app() -> (App, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("temp dir");
        let app = App::open(
            Config {
                state_dir: dir.path().to_path_buf(),
                secret_hex: "11".repeat(32),
            },
            Launch {
                computers_fixture: true,
                native_computers: true,
                ..Launch::default()
            },
        )
        .expect("app");
        (app, dir)
    }

    fn packet(app: &mut App, request: Request) -> serde_json::Value {
        serde_json::from_slice(&app.respond(request)).expect("packet")
    }

    fn row<'a>(packet: &'a serde_json::Value, name: &str) -> &'a serde_json::Value {
        packet["computers_home"]["rows"]
            .as_array()
            .expect("rows")
            .iter()
            .find(|row| row["name"] == name)
            .expect("row")
    }

    fn menu(row: &serde_json::Value) -> Vec<&str> {
        row["menu"]
            .as_array()
            .expect("menu")
            .iter()
            .map(|item| item["choice"].as_str().expect("choice"))
            .collect()
    }

    #[test]
    fn each_computer_is_one_row_with_a_short_status() {
        let (mut app, _dir) = app();
        let home = packet(&mut app, Request::Snapshot);
        assert!(
            home["computers"].is_null(),
            "the list replaces the shared screen"
        );
        let rows = home["computers_home"]["rows"].as_array().expect("rows");
        assert_eq!(rows.len(), 7);
        for (name, status, tone) in [
            ("Studio Mac", "Online · Local network", "online"),
            ("Build server", "Connecting", "pending"),
            ("Home NAS", "Offline", "offline"),
            ("Old laptop", "Needs an update", "alert"),
            ("Lab box", "No access", "offline"),
            ("Former work PC", "Revoked", "alert"),
            ("Travel mini", "Switched off", "offline"),
        ] {
            let row = row(&home, name);
            assert_eq!(
                (row["status"].as_str(), row["tone"].as_str()),
                (Some(status), Some(tone))
            );
            assert!(menu(row).contains(&"access"), "{name}");
        }
        assert_eq!(
            menu(row(&home, "Studio Mac")),
            ["switch_off", "access", "forget"]
        );
        assert_eq!(
            menu(row(&home, "Home NAS")),
            ["switch_off", "try_now", "access", "forget"]
        );
        assert_eq!(menu(row(&home, "Travel mini"))[0], "switch_on");
    }

    #[test]
    fn an_online_computer_names_its_background_watchers_and_others_say_nothing() {
        let (mut app, _dir) = app();
        let home = packet(&mut app, Request::Snapshot);
        // The same words the terminal and desktop show.
        assert_eq!(
            row(&home, "Studio Mac")["watchers"],
            "1 background watcher · disk cleanup"
        );
        assert_eq!(
            row(&home, "Studio Mac")["watchers"].as_str(),
            openagents_chat_app::watchers::line(&["disk cleanup".into()]).as_deref()
        );
        // Home NAS said it runs one, but it is offline now: nothing.
        assert!(row(&home, "Home NAS")["watchers"].is_null());
        // Hosts that never said, and every other row: nothing.
        for name in ["Build server", "Old laptop", "Lab box", "Travel mini"] {
            assert!(row(&home, name)["watchers"].is_null(), "{name}");
        }
    }

    #[test]
    fn a_host_that_draws_the_shared_screens_whole_gets_them_unchanged() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut app = App::new(Config {
            state_dir: dir.path().to_path_buf(),
            secret_hex: "11".repeat(32),
        })
        .expect("app");
        let packet = packet(&mut app, Request::Snapshot);
        assert!(packet["computers_home"].is_null());
        assert!(packet["computers"].to_string().contains("\"tabs\""));
    }

    #[test]
    fn a_row_opens_the_shared_screens_and_back_returns_to_the_list() {
        let (mut app, _dir) = app();
        let home = packet(&mut app, Request::Snapshot);
        let host = row(&home, "Studio Mac")["host"]
            .as_str()
            .expect("host")
            .to_owned();
        let open = packet(&mut app, Request::ComputersOpen { host });
        assert!(open["computers_home"].is_null());
        let view = open["computers"].to_string();
        assert!(view.contains("Studio Mac") && view.contains("host-order"));
        assert!(!view.contains("\"host-end\""));
        assert!(
            !view.contains("\"tabs\""),
            "the host's navigation replaces the tab row"
        );
        let back = packet(
            &mut app,
            Request::ComputersGo {
                to: Destination::Home,
            },
        );
        assert!(back["computers_home"]["rows"].is_array());
        let add = packet(
            &mut app,
            Request::ComputersGo {
                to: Destination::Add,
            },
        );
        let add = add["computers"].to_string();
        assert!(add.contains("Add a computer") && !add.contains("\"local\""));
    }

    #[test]
    fn menu_choices_run_through_the_shared_check() {
        let (mut app, _dir) = app();
        let home = packet(&mut app, Request::Snapshot);
        let studio = row(&home, "Studio Mac")["host"]
            .as_str()
            .expect("host")
            .to_owned();
        let off = packet(
            &mut app,
            Request::ComputersChoose {
                host: studio.clone(),
                choice: Choice::SwitchOff,
            },
        );
        assert_eq!(row(&off, "Studio Mac")["status"], "Switched off");
        assert!(
            off["computers_home"]["notice"]
                .as_str()
                .is_some_and(|notice| notice.starts_with("Switched off Studio Mac"))
        );
        // A revoked computer cannot be switched on; the refusal is shown.
        let revoked = row(&home, "Former work PC")["host"]
            .as_str()
            .expect("host")
            .to_owned();
        let refused = packet(
            &mut app,
            Request::ComputersChoose {
                host: revoked,
                choice: Choice::SwitchOff,
            },
        );
        assert!(refused["computers_home"]["notice"].is_string());
        // Forget arrives confirmed by the host and removes the row.
        let forgot = packet(
            &mut app,
            Request::ComputersChoose {
                host: studio,
                choice: Choice::Forget,
            },
        );
        let names: Vec<&str> = forgot["computers_home"]["rows"]
            .as_array()
            .expect("rows")
            .iter()
            .map(|row| row["name"].as_str().expect("name"))
            .collect();
        assert!(!names.contains(&"Studio Mac"));
    }
}
