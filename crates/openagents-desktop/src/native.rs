//! The window's native surroundings (#10023), gathered so the shell only
//! calls in: the Mac's menu bar ([`crate::appmenu`]), clicks on Coder's
//! notifications, and whether the window should come forward.
//!
//! A notification click arrives on the platform's listener thread
//! ([`crate::platform::listen_notifications`]) as a chat ID; [`open_chat`]
//! queues it and wakes the window. On the next tick [`tick`] turns menu
//! choices and clicks into the window's intents: each runs a shared registry
//! command (`chat_action::Action::Command`), a click the chat's
//! "Switch to" command ([`openagents_desktop::notices::open_command`]), with
//! any open palette or menu dismissed first so its filter cannot hide the
//! command. A click also asks for the window to come forward
//! ([`focus_request`]).

use openagents_chat_app::commands::Entry;
use openagents_desktop::chat_action::Action as ChatAction;
use openagents_desktop::model::Intent;
use rust_native_desktop::Waker;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

static OPENED: Mutex<Vec<String>> = Mutex::new(Vec::new());
static FOCUS: AtomicBool = AtomicBool::new(false);
static WAKER: OnceLock<Waker> = OnceLock::new();

/// Starts the menu bar and the notification listener. Call once, on the
/// main thread, when a live window's event loop starts.
pub fn start(waker: Waker) {
    let _ = WAKER.set(waker.clone());
    crate::appmenu::start(waker);
    crate::platform::listen_notifications();
}

/// The person clicked the notification for `chat`: open it. Any thread.
pub fn open_chat(chat: String) {
    if let Ok(mut opened) = OPENED.lock() {
        opened.push(chat);
    }
    if let Some(waker) = WAKER.get() {
        waker.wake();
    }
}

/// The intents for menu `commands` (registry keys) and `opened` chats, in
/// that order.
pub fn intents(commands: Vec<String>, opened: Vec<String>) -> Vec<Intent> {
    commands
        .into_iter()
        .chain(
            opened
                .iter()
                .map(|chat| openagents_desktop::notices::open_command(chat)),
        )
        .flat_map(|key| {
            [
                Intent::Chat {
                    action: ChatAction::DismissOverlay,
                },
                Intent::Chat {
                    action: ChatAction::Command { key },
                },
            ]
        })
        .collect()
}

/// Redraws the menu bar from `registry` and returns what to activate now.
pub fn tick(registry: &[Entry]) -> Vec<Intent> {
    let commands = crate::appmenu::sync(registry);
    let opened = OPENED
        .lock()
        .map(|mut opened| std::mem::take(&mut *opened))
        .unwrap_or_default();
    if !opened.is_empty() {
        FOCUS.store(true, Ordering::Relaxed);
    }
    intents(commands, opened)
}

/// Whether the window should come forward, once per click.
pub fn focus_request() -> bool {
    FOCUS.swap(false, Ordering::Relaxed)
}
