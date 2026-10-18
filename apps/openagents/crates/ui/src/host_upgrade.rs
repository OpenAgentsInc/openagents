//! OpenAgents #12056: the app's half of a host upgrade.
//!
//! `openagents host-upgrade` builds `main` beside this development bundle and
//! hands it over in `<data>/host-upgrade`. This watches for it, waits until
//! the owner isn't typing, on a call, or holding unsaved files, puts up a
//! notice, then swaps the bundle in and relaunches onto the open chat.
//! Quitting ends the engine in the app but not the agents under keepers; the
//! relaunched app picks them up. At start, an app that an upgrade reopened
//! says so and logs it.
#![cfg_attr(not(target_os = "macos"), allow(dead_code, unused_imports))]

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use gpui::{App, Entity};
use openagents_host_upgrade::app::{self as upgrade_app, Action, Watcher};
use openagents_host_upgrade::quiet::Activity;

use crate::state::AppState;

/// The last keystroke in any text field.
static LAST_INPUT: Mutex<Option<Instant>> = Mutex::new(None);

const POLL: Duration = Duration::from_secs(2);

/// Mark a keystroke: an upgrade won't restart the app for a while after one.
pub(crate) fn note_input() {
    if let Ok(mut last) = LAST_INPUT.lock() {
        *last = Some(Instant::now());
    }
}

/// Watch for upgrades handed to this app, and report the one that reopened
/// it. Does nothing outside a development bundle.
pub(crate) fn init(state: Entity<AppState>, data_dir: PathBuf, ipc_port: u16, cx: &mut App) {
    #[cfg(target_os = "macos")]
    {
        let Some(bundle) = std::env::current_exe()
            .ok()
            .and_then(|exe| openagents_host_upgrade::layout::bundle_of(&exe))
        else {
            return;
        };
        if let Some((entry, _chat)) =
            upgrade_app::reopened(&data_dir, &bundle, openagents_host_upgrade::now_ms())
        {
            tracing::info!(upgrade = %entry.id, summary = %entry.summary(), "reopened after a host upgrade");
            crate::toast::success(cx, upgrade_app::reopened_line(&entry));
        }
        let mut watcher = Watcher::new(&data_dir, bundle.clone());
        // Upgraders hand this app their bundle instead of quitting it.
        let _ =
            openagents_host_upgrade::record::write_watcher(watcher.state_dir(), std::process::id());
        cx.spawn(async move |cx| {
            loop {
                cx.background_executor().timer(POLL).await;
                let restarting =
                    cx.update(|cx| tick(&mut watcher, &state, &data_dir, &bundle, ipc_port, cx));
                if restarting {
                    break;
                }
            }
        })
        .detach();
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (state, data_dir, ipc_port, cx);
}

/// One look; true once the app is restarting.
#[cfg(target_os = "macos")]
fn tick(
    watcher: &mut Watcher,
    state: &Entity<AppState>,
    data_dir: &Path,
    bundle: &Path,
    ipc_port: u16,
    cx: &mut App,
) -> bool {
    match watcher.poll(Instant::now(), &activity(cx)) {
        Action::None => false,
        Action::Notice(text) | Action::Cancel(text) => {
            crate::toast::info(cx, text);
            false
        }
        Action::Restart(pending) => restart(&pending, state, data_dir, bundle, ipc_port, cx),
    }
}

/// What the owner is doing: typing, a call, or a file with unsaved changes
/// (which counts as typing now, so the quit never stops on it).
fn activity(cx: &App) -> Activity {
    let mut last_input = LAST_INPUT.lock().ok().and_then(|last| *last);
    let mut voice_live = false;
    for window in cx.windows() {
        let Some(shell) = window.downcast::<crate::shell::Shell>() else {
            continue;
        };
        if let Ok(shell) = shell.read(cx) {
            voice_live |= shell.voice_live(cx);
            if shell.has_unsaved_files(cx) {
                last_input = Some(Instant::now());
            }
        }
    }
    Activity {
        last_input,
        voice_live,
    }
}

#[cfg(target_os = "macos")]
fn restart(
    pending: &openagents_host_upgrade::Pending,
    state: &Entity<AppState>,
    data_dir: &Path,
    bundle: &Path,
    ipc_port: u16,
    cx: &mut App,
) -> bool {
    use openagents_host_upgrade::{Layout, layout, now_ms, swap, system};

    let (chat, running) = {
        let state = state.read(cx);
        let chat = state.selected_chat.as_deref().and_then(|chat| {
            crate::links::workspace_locator(
                state.workspace_scope,
                state.auth.as_ref(),
                state.local_device_id.as_deref(),
            )
            .map(|workspace| crate::links::openagents_conversation_link(chat, &workspace))
        });
        let running = crate::background::running_here(
            &state.sessions,
            state.local_device_id.as_deref(),
            crate::state::clock_now(),
        );
        (chat, running)
    };
    let state_dir = layout::state_dir(data_dir);
    if let Err(e) =
        upgrade_app::begin_restart(&state_dir, pending, chat.clone(), &running, now_ms())
    {
        crate::toast::error(cx, format!("The upgrade couldn't be installed: {e}"));
        return false;
    }
    let launch = Layout {
        checkout: PathBuf::new(),
        target_dir: PathBuf::new(),
        bundle: bundle.to_path_buf(),
        data_dir: data_dir.to_path_buf(),
        ipc_port,
    };
    let (program, args) = system::launch_command(&launch, bundle, chat.as_deref(), true);
    if let Err(e) = system::relaunch_after_exit(std::process::id(), &program, &args) {
        // Without a relaunch, quitting would leave no app: put the old
        // bundle back and stay.
        let _ = swap::roll_back(bundle, &pending.previous);
        upgrade_app::abandon(&state_dir, pending, &e, now_ms());
        crate::toast::error(cx, format!("The upgrade couldn't restart the app: {e}"));
        return false;
    }
    tracing::info!(upgrade = %pending.id, to = %pending.to, "restarting into a host upgrade");
    crate::app_menus::request_quit(cx);
    true
}
