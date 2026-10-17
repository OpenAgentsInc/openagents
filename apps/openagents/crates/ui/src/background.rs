//! The background engine in the desktop app (#11931,
//! `docs/desktop/move-running-agents.md`).
//!
//! - [`ensure_engine`] runs before the app decides to embed an engine: it
//!   starts this data folder's background service (installing it first when
//!   needed) so the app attaches over IPC instead.
//! - [`init`] adds the menu-bar item on macOS: how many agents are running
//!   here, **Open**, **Move to background** (only while the engine runs in
//!   the app), and **Stop all and quit**.
//!
//! The service logic lives in `openagents-background`. The hooks into
//! upstream files are listed in `docs/downstream-patches.md`.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use gpui::{App, Entity};
use openagents_background::preflight::{self, Checks, HarnessInUse, Problem};
use openagents_background::{EngineMode, Ensure, START_WAIT, SystemManager};
use openagents_proto::{HarnessId, Session};

use crate::state::{AppState, EngineBootConfig, EngineMode as Connection, Indicator};

/// Start this data folder's background engine when the app should attach to
/// it. True when an engine now answers on the IPC port.
pub(crate) async fn ensure_engine(config: &EngineBootConfig) -> bool {
    if cfg!(test) {
        return false;
    }
    let data_dir = config.data_dir.clone();
    let port = config.ipc_port;
    tokio::task::spawn_blocking(move || {
        let (mode, reason) = openagents_background::cli::mode_from_env(&data_dir);
        if mode == EngineMode::Embedded {
            tracing::info!(reason, "engine runs in the app");
            return false;
        }
        let spec = match openagents_background::cli::spec_from_env(&data_dir, port) {
            Ok(spec) => spec,
            Err(error) => {
                tracing::warn!(%error, "no background engine");
                return false;
            }
        };
        let outcome = openagents_background::ensure(Ensure {
            spec: &spec,
            mode,
            data_dir_locked: openagents_engine::InstanceLock::holder(&data_dir).is_some(),
            manager: &SystemManager,
            listening: &|| openagents_background::port_listening(port),
            wait: START_WAIT,
        });
        tracing::info!(?outcome, reason, service = %spec.file_path().display(), "background engine");
        outcome.attach()
    })
    .await
    .unwrap_or(false)
}

/// Agents running on this computer: sessions of the local device that are
/// working or waiting for an answer.
pub(crate) fn running_here(
    sessions: &[Session],
    local: Option<&str>,
    now: DateTime<Utc>,
) -> Vec<String> {
    sessions
        .iter()
        .filter(|session| local.is_none_or(|device| session.device_id == device))
        .filter(|session| {
            matches!(
                crate::state::effective_indicator(Some(session), now),
                Indicator::Working | Indicator::AwaitingInput
            )
        })
        .map(|session| session.chat_id.clone())
        .collect()
}

/// The menu-bar item's header.
pub(crate) fn running_label(count: usize) -> String {
    match count {
        0 => "No agents running".into(),
        1 => "1 agent running".into(),
        n => format!("{n} agents running"),
    }
}

fn harness_key(id: HarnessId) -> Option<&'static str> {
    match id {
        HarnessId::ClaudeCode => Some("claude-code"),
        HarnessId::Codex => Some("codex"),
        _ => None,
    }
}

/// What the background engine couldn't use if this app's runs moved now.
fn handoff_problems(data_dir: &Path, harnesses: &[HarnessId]) -> Vec<Problem> {
    let signed_in = openagents_account::desktop::registered(data_dir).is_some_and(|account| {
        matches!(
            account.snapshot().phase,
            openagents_account::Phase::SignedIn { .. }
        )
    });
    let in_use: Vec<HarnessInUse> = harnesses
        .iter()
        .filter_map(|id| {
            let login = preflight::harness_login(harness_key(*id)?)?;
            Some(HarnessInUse {
                login,
                installed: openagents_harness::install::installed(*id),
            })
        })
        .collect();
    let home = openagents_brand::env_var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    preflight::preflight(&Checks {
        home: &home,
        signed_in,
        sign_in_saved: signed_in && openagents_account::desktop::has_saved_sign_in(data_dir),
        harnesses: &in_use,
        keychain_has: &preflight::keychain_has,
    })
}

/// Hand this app's runs to the background engine: check the logins, save
/// the choice, write the service file, and relaunch. Quitting ends the
/// engine in the app but not the agents under keepers; the relaunched app
/// starts the service, which picks them up.
fn move_to_background(state: &Entity<AppState>, data_dir: PathBuf, cx: &mut App) {
    let now = crate::state::clock_now();
    let state = state.read(cx);
    let running = running_here(&state.sessions, state.local_device_id.as_deref(), now);
    let mut harnesses: Vec<HarnessId> = state
        .chats
        .iter()
        .filter(|chat| running.contains(&chat.id))
        .filter_map(|chat| chat.config.as_ref().map(|config| config.harness))
        .collect();
    harnesses.sort_by_key(|id| format!("{id:?}"));
    harnesses.dedup();
    let ipc_port = openagents_brand::env_var("OPENAGENTS_IPC_PORT")
        .ok()
        .and_then(|port| port.parse().ok())
        .unwrap_or(openagents_brand::DEFAULT_IPC_PORT);
    cx.spawn(async move |cx| {
        let checked_dir = data_dir.clone();
        let result: Result<(), String> = cx
            .background_executor()
            .spawn(async move {
                let problems = handoff_problems(&checked_dir, &harnesses);
                if !problems.is_empty() {
                    return Err(problems
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("\n\n"));
                }
                let spec = openagents_background::cli::spec_from_env(&checked_dir, ipc_port)?;
                spec.write()
                    .map_err(|error| format!("We couldn't set up background agents: {error}"))?;
                openagents_background::write_preference(&checked_dir, EngineMode::Background)
                    .map_err(|error| format!("We couldn't save your choice: {error}"))?;
                Ok(())
            })
            .await;
        let _ = cx.update(|cx| match result {
            Ok(()) => cx.restart(),
            Err(message) => {
                #[cfg(target_os = "macos")]
                status_item::alert("Agents can't move to the background yet", &message);
                #[cfg(not(target_os = "macos"))]
                tracing::warn!(%message, "move to background refused");
            }
        });
    })
    .detach();
}

/// Ask every agent running here to stop, then quit the app.
fn stop_all_and_quit(state: &Entity<AppState>, cx: &mut App) {
    let now = crate::state::clock_now();
    let state = state.read(cx);
    let running = running_here(&state.sessions, state.local_device_id.as_deref(), now);
    let engine = state.engine().cloned();
    cx.spawn(async move |cx| {
        if let Some(engine) = engine {
            let stops = running.into_iter().map(|chat_id| {
                let client = engine.client();
                async move {
                    let params = serde_json::json!({
                        "chatId": chat_id,
                        "command": { "kind": "interrupt" },
                    });
                    let _ = client
                        .call(openagents_rpc::methods::QUEUE_COMMAND, params)
                        .await;
                }
            });
            let all = futures::future::join_all(stops);
            let timeout = cx
                .background_executor()
                .timer(std::time::Duration::from_secs(5));
            futures::pin_mut!(all);
            futures::future::select(all, timeout).await;
        }
        let _ = cx.update(crate::app_menus::request_quit);
    })
    .detach();
}

/// The menu-bar item's actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MenuAction {
    Open,
    MoveToBackground,
    StopAllAndQuit,
}

/// Add the menu-bar item (macOS) and keep its count current.
pub(crate) fn init(state: Entity<AppState>, data_dir: PathBuf, cx: &mut App) {
    #[cfg(target_os = "macos")]
    {
        use futures::StreamExt;
        let (tx, mut rx) = futures::channel::mpsc::unbounded::<MenuAction>();
        if !status_item::install(tx) {
            return;
        }
        let mut shown: Option<(usize, bool)> = None;
        cx.observe(&state, move |state, cx| {
            let now = crate::state::clock_now();
            let state = state.read(cx);
            let count = running_here(&state.sessions, state.local_device_id.as_deref(), now).len();
            let embedded = state
                .engine()
                .is_some_and(|engine| engine.mode() == Connection::InProcess);
            if shown != Some((count, embedded)) {
                shown = Some((count, embedded));
                status_item::update(count, embedded);
            }
        })
        .detach();
        cx.spawn(async move |cx| {
            while let Some(action) = rx.next().await {
                let state = state.clone();
                let data_dir = data_dir.clone();
                let _ = cx.update(|cx| match action {
                    MenuAction::Open => crate::activate_main_window(cx),
                    MenuAction::MoveToBackground => move_to_background(&state, data_dir, cx),
                    MenuAction::StopAllAndQuit => stop_all_and_quit(&state, cx),
                });
            }
        })
        .detach();
    }
    #[cfg(not(target_os = "macos"))]
    {
        // GPUI has no tray on Linux; agents keep running in the service.
        let _ = (state, data_dir, cx);
    }
}

/// NSStatusItem through the ObjC runtime the app already links.
#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs)] // objc 0.2 macros
mod status_item {
    use std::cell::RefCell;
    use std::ffi::CString;

    use objc::declare::ClassDecl;
    use objc::runtime::{NO, Object, Sel, YES};
    use objc::{class, msg_send, sel, sel_impl};

    use super::MenuAction;

    struct Item {
        status: *mut Object,
        header: *mut Object,
        move_to_background: *mut Object,
    }

    thread_local! {
        static SENDER: RefCell<Option<futures::channel::mpsc::UnboundedSender<MenuAction>>> =
            const { RefCell::new(None) };
        static ITEM: RefCell<Option<Item>> = const { RefCell::new(None) };
    }

    fn send(action: MenuAction) {
        SENDER.with_borrow(|sender| {
            if let Some(sender) = sender {
                let _ = sender.unbounded_send(action);
            }
        });
    }

    extern "C" fn open(_this: &Object, _sel: Sel, _sender: *mut Object) {
        send(MenuAction::Open);
    }
    extern "C" fn move_to_background(_this: &Object, _sel: Sel, _sender: *mut Object) {
        send(MenuAction::MoveToBackground);
    }
    extern "C" fn stop_all(_this: &Object, _sel: Sel, _sender: *mut Object) {
        send(MenuAction::StopAllAndQuit);
    }

    fn ns_string(text: &str) -> *mut Object {
        let text = CString::new(text).unwrap_or_default();
        unsafe { msg_send![class!(NSString), stringWithUTF8String: text.as_ptr()] }
    }

    unsafe fn menu_item(title: &str, action: Option<Sel>, target: *mut Object) -> *mut Object {
        unsafe {
            let item: *mut Object = msg_send![class!(NSMenuItem), alloc];
            let selector = action.unwrap_or(sel!(self));
            let item: *mut Object = msg_send![item,
                initWithTitle: ns_string(title)
                action: selector
                keyEquivalent: ns_string("")];
            if action.is_some() {
                let _: () = msg_send![item, setTarget: target];
            } else {
                let _: () = msg_send![item, setAction: std::ptr::null::<Object>()];
                let _: () = msg_send![item, setEnabled: NO];
            }
            item
        }
    }

    /// Create the item once, on the main thread. False when AppKit has no
    /// status bar.
    pub(super) fn install(tx: futures::channel::mpsc::UnboundedSender<MenuAction>) -> bool {
        SENDER.with_borrow_mut(|sender| *sender = Some(tx));
        if ITEM.with_borrow(Option::is_some) {
            return true;
        }
        unsafe {
            let bar: *mut Object = msg_send![class!(NSStatusBar), systemStatusBar];
            if bar.is_null() {
                return false;
            }
            // NSVariableStatusItemLength; the bar doesn't retain its items.
            let status: *mut Object = msg_send![bar, statusItemWithLength: -1.0f64];
            if status.is_null() {
                return false;
            }
            let _: *mut Object = msg_send![status, retain];

            let Some(class) = ClassDecl::new("OpenAgentsBackgroundMenuTarget", class!(NSObject))
                .map(|mut decl| {
                    decl.add_method(
                        sel!(openApp:),
                        open as extern "C" fn(&Object, Sel, *mut Object),
                    );
                    decl.add_method(
                        sel!(moveToBackground:),
                        move_to_background as extern "C" fn(&Object, Sel, *mut Object),
                    );
                    decl.add_method(
                        sel!(stopAllAndQuit:),
                        stop_all as extern "C" fn(&Object, Sel, *mut Object),
                    );
                    decl.register()
                })
            else {
                return false;
            };
            // Leaked: menu items hold their targets weakly.
            let target: *mut Object = msg_send![class, new];

            let menu: *mut Object = msg_send![class!(NSMenu), alloc];
            let menu: *mut Object = msg_send![menu, initWithTitle: ns_string("OpenAgents")];
            let _: () = msg_send![menu, setAutoenablesItems: NO];
            let header = menu_item(&super::running_label(0), None, target);
            let open_item = menu_item("Open", Some(sel!(openApp:)), target);
            let move_item = menu_item("Move to background", Some(sel!(moveToBackground:)), target);
            let _: () = msg_send![move_item, setHidden: YES];
            // "Move all to a cloud computer" waits for #11933 (move plans).
            // let cloud_item = menu_item("Move all to a cloud computer", Some(sel!(moveAllToCloud:)), target);
            let stop_item = menu_item("Stop all and quit", Some(sel!(stopAllAndQuit:)), target);
            let separator: *mut Object = msg_send![class!(NSMenuItem), separatorItem];
            let separator2: *mut Object = msg_send![class!(NSMenuItem), separatorItem];
            for item in [
                header, separator, open_item, move_item, separator2, stop_item,
            ] {
                let _: () = msg_send![menu, addItem: item];
            }
            let _: () = msg_send![status, setMenu: menu];

            let button: *mut Object = msg_send![status, button];
            if !button.is_null() {
                let image: *mut Object = msg_send![class!(NSImage),
                    imageWithSystemSymbolName: ns_string("cpu")
                    accessibilityDescription: ns_string("OpenAgents")];
                if image.is_null() {
                    let _: () = msg_send![button, setTitle: ns_string("OpenAgents")];
                } else {
                    let _: () = msg_send![image, setTemplate: YES];
                    let _: () = msg_send![button, setImage: image];
                }
            }
            ITEM.with_borrow_mut(|item| {
                *item = Some(Item {
                    status,
                    header,
                    move_to_background: move_item,
                })
            });
        }
        true
    }

    /// Show `count` running agents; offer Move to background while the
    /// engine runs in the app.
    pub(super) fn update(count: usize, embedded: bool) {
        ITEM.with_borrow(|item| {
            let Some(item) = item else { return };
            unsafe {
                let _: () =
                    msg_send![item.header, setTitle: ns_string(&super::running_label(count))];
                let _: () =
                    msg_send![item.move_to_background, setHidden: if embedded { NO } else { YES }];
                let button: *mut Object = msg_send![item.status, button];
                if !button.is_null() {
                    let title = if count == 0 {
                        String::new()
                    } else {
                        format!(" {count}")
                    };
                    let _: () = msg_send![button, setTitle: ns_string(&title)];
                }
            }
        });
    }

    /// A plain modal message.
    pub(super) fn alert(title: &str, body: &str) {
        unsafe {
            let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
            let _: () = msg_send![app, activateIgnoringOtherApps: YES];
            let alert: *mut Object = msg_send![class!(NSAlert), new];
            let _: () = msg_send![alert, setMessageText: ns_string(title)];
            let _: () = msg_send![alert, setInformativeText: ns_string(body)];
            let _: *mut Object = msg_send![alert, addButtonWithTitle: ns_string("OK")];
            let _: isize = msg_send![alert, runModal];
            let _: () = msg_send![alert, release];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openagents_proto::SessionStatus;

    fn session(chat: &str, device: &str, status: SessionStatus, now: DateTime<Utc>) -> Session {
        Session {
            last_completed_turn: None,
            chat_id: chat.into(),
            device_id: device.into(),
            status,
            started_at: Some(now),
            updated_at: now,
            running_subagents: 0,
        }
    }

    #[test]
    fn counts_only_live_agents_on_this_computer() {
        let now = Utc::now();
        let stale = now - chrono::Duration::minutes(10);
        let sessions = vec![
            session("a", "here", SessionStatus::Working, now),
            session("b", "here", SessionStatus::Idle, now),
            session("c", "phone", SessionStatus::Working, now),
            session("d", "here", SessionStatus::Working, stale),
        ];
        assert_eq!(running_here(&sessions, Some("here"), now), vec!["a"]);
        assert_eq!(running_here(&sessions, None, now), vec!["a", "c"]);
        assert_eq!(running_label(0), "No agents running");
        assert_eq!(running_label(1), "1 agent running");
        assert_eq!(running_label(3), "3 agents running");
    }
}
