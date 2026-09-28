//! The OpenAgents mobile app's Rust library. Rust owns the app's state and
//! builds each screen as a Rust Native view; the thin SwiftUI host decodes
//! and renders it and forwards button activations.
//!
//! The home screen lists the devices on the user's tailnet.

mod tailnet;

use rust_native::style::{Color, Space, Style, TextWeight};
use rust_native::{Activation, Axis, Element, Node, TextRole, ValidatedView, View};
use serde::{Deserialize, Serialize};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::ptr;
use std::time::Duration;
use tailnet::{Client, Outcome, Tailnet};
use url::Url;

const REFRESH_LIMIT: Duration = Duration::from_secs(20);
const SIGN_IN_LIMIT: Duration = Duration::from_secs(300);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Intent {
    SignIn,
    Refresh,
}

#[derive(Deserialize)]
pub struct Config {
    /// A private directory for this app's Tailscale node keys.
    pub state_dir: PathBuf,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Request {
    /// Show the current screen without network activity.
    Show,
    /// Register and read the tailnet again.
    Refresh,
    /// Wait for the user to finish signing in, then read the tailnet.
    WaitForSignIn,
    Activate {
        activation: Activation,
    },
}

#[derive(Serialize)]
pub struct Reply {
    view: serde_json::Value,
    /// Open this URL in the browser.
    open_url: Option<String>,
    /// Send `wait_for_sign_in` next.
    wait_for_sign_in: bool,
}

enum Screen {
    Loading,
    SignIn(Url),
    Devices(Tailnet),
    Failed(String),
}

pub struct App {
    runtime: tokio::runtime::Runtime,
    client: Result<Client, String>,
    screen: Screen,
    revision: u64,
    current: Option<ValidatedView<Intent>>,
}

impl App {
    pub fn new(config: Config) -> Result<Self, String> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            runtime,
            client: Client::open(&config.state_dir),
            screen: Screen::Loading,
            revision: 0,
            current: None,
        })
    }

    pub fn respond(&mut self, request: Request) -> Reply {
        let mut open_url = None;
        let mut wait_for_sign_in = false;
        match request {
            Request::Show => {}
            Request::Refresh => self.load(None, REFRESH_LIMIT),
            Request::WaitForSignIn => {
                if let Screen::SignIn(url) = &self.screen {
                    let url = url.clone();
                    self.load(Some(url), SIGN_IN_LIMIT);
                }
            }
            Request::Activate { activation } => {
                let intent = self
                    .current
                    .as_ref()
                    .and_then(|view| view.activate(&activation).ok())
                    .copied();
                match (intent, &self.screen) {
                    (Some(Intent::SignIn), Screen::SignIn(url)) => {
                        open_url = Some(url.to_string());
                        wait_for_sign_in = true;
                    }
                    (Some(Intent::Refresh), _) => self.load(None, REFRESH_LIMIT),
                    _ => {}
                }
            }
        }
        Reply {
            view: self.render(),
            open_url,
            wait_for_sign_in,
        }
    }

    fn load(&mut self, followup: Option<Url>, limit: Duration) {
        let outcome = match &self.client {
            Ok(client) => self.runtime.block_on(client.devices(followup, limit)),
            Err(error) => Err(error.clone()),
        };
        self.screen = match outcome {
            Ok(Outcome::Devices(tailnet)) => Screen::Devices(tailnet),
            Ok(Outcome::SignIn(url)) => Screen::SignIn(url),
            Err(error) => Screen::Failed(error),
        };
    }

    fn render(&mut self) -> serde_json::Value {
        self.revision += 1;
        let view = View::new("openagents.home", self.revision, screen(&self.screen))
            .validate()
            .or_else(|_| {
                View::new(
                    "openagents.home",
                    self.revision,
                    page(vec![body("This screen could not be shown.")]),
                )
                .validate()
            });
        match view {
            Ok(view) => {
                let value = serde_json::to_value(view.view()).unwrap_or_default();
                self.current = Some(view);
                value
            }
            Err(_) => serde_json::Value::Null,
        }
    }
}

const WHITE: Color = Color::rgb(255, 255, 255);
const GRAY: Color = Color::rgb(153, 153, 153);
const GREEN: Color = Color::rgb(52, 199, 89);

fn screen(screen: &Screen) -> Node<Intent> {
    match screen {
        Screen::Loading => page(vec![
            heading("devices-title", "Devices"),
            status("loading", "Checking your tailnet…", GRAY),
        ]),
        Screen::SignIn(_) => page(vec![
            heading("devices-title", "Connect to a tailnet"),
            body("Sign in with Tailscale to see the devices on your tailnet."),
            button("sign-in", "Sign in with Tailscale", Intent::SignIn),
        ]),
        Screen::Failed(error) => page(vec![
            heading("devices-title", "Devices"),
            body(error),
            button("refresh", "Try again", Intent::Refresh),
        ]),
        Screen::Devices(tailnet) if tailnet.devices.is_empty() => page(vec![
            heading("devices-title", "Connect to a tailnet"),
            body(
                "No other devices are on this tailnet. Connect a device with Tailscale, then refresh.",
            ),
            button("refresh", "Refresh", Intent::Refresh),
        ]),
        Screen::Devices(tailnet) => {
            let mut header = vec![heading("devices-title", "Devices")];
            let count = match tailnet.devices.len() {
                1 => "1 device".to_string(),
                n => format!("{n} devices"),
            };
            let summary = match &tailnet.name {
                Some(name) => format!("{count} on {name}"),
                None => count,
            };
            header.push(status("devices-summary", &summary, GRAY));
            let rows = tailnet
                .devices
                .iter()
                .enumerate()
                .map(|(index, device)| {
                    let (state, color) = match device.online {
                        Some(true) => ("Online", GREEN),
                        Some(false) => ("Offline", GRAY),
                        None => ("Status unknown", GRAY),
                    };
                    stack(
                        &format!("device-{index}"),
                        Axis::Vertical,
                        Space::Xs,
                        vec![
                            text(
                                &format!("device-{index}-name"),
                                &device.name,
                                TextRole::Body,
                                WHITE,
                                true,
                            ),
                            status(
                                &format!("device-{index}-detail"),
                                &format!("{} · {}", device.os, device.address),
                                GRAY,
                            ),
                            status(&format!("device-{index}-state"), state, color),
                        ],
                    )
                })
                .collect();
            let mut children = header;
            children.push(Node {
                key: "devices".into(),
                style: Style::default(),
                element: Element::List {
                    label: "Tailnet devices".into(),
                    children: rows,
                },
            });
            if let Some(this) = &tailnet.this_device {
                children.push(status(
                    "this-device",
                    &format!("This app appears on the tailnet as {this}."),
                    GRAY,
                ));
            }
            children.push(button("refresh", "Refresh", Intent::Refresh));
            page(children)
        }
    }
}

fn page(children: Vec<Node<Intent>>) -> Node<Intent> {
    let mut node = stack("home", Axis::Vertical, Space::Md, children);
    node.style.padding_top = Some(Space::Lg);
    node.style.padding_end = Some(Space::Md);
    node.style.padding_bottom = Some(Space::Md);
    node.style.padding_start = Some(Space::Md);
    node
}

fn stack(key: &str, axis: Axis, gap: Space, children: Vec<Node<Intent>>) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Style {
            gap: Some(gap),
            ..Style::default()
        },
        element: Element::Stack { axis, children },
    }
}

fn text(key: &str, value: &str, role: TextRole, foreground: Color, bold: bool) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Style {
            foreground: Some(foreground),
            weight: bold.then_some(TextWeight::Bold),
            ..Style::default()
        },
        element: Element::Text {
            value: value.into(),
            role,
        },
    }
}

fn heading(key: &str, value: &str) -> Node<Intent> {
    text(key, value, TextRole::Heading, WHITE, true)
}

fn body(value: &str) -> Node<Intent> {
    text("message", value, TextRole::Body, WHITE, false)
}

fn status(key: &str, value: &str, color: Color) -> Node<Intent> {
    text(key, value, TextRole::Status, color, false)
}

fn button(key: &str, label: &str, intent: Intent) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Style {
            foreground: Some(WHITE),
            ..Style::default()
        },
        element: Element::Button {
            label: label.into(),
            enabled: true,
            intent,
        },
    }
}

#[repr(C)]
pub struct OpenAgentsMobileBuffer {
    pub data: *mut u8,
    pub len: usize,
}

fn buffer(bytes: Vec<u8>) -> OpenAgentsMobileBuffer {
    let mut bytes = bytes.into_boxed_slice();
    let result = OpenAgentsMobileBuffer {
        data: if bytes.is_empty() {
            ptr::null_mut()
        } else {
            bytes.as_mut_ptr()
        },
        len: bytes.len(),
    };
    std::mem::forget(bytes);
    result
}

/// # Safety
/// `bytes` must point to `len` readable bytes for this call. Destroy the
/// returned handle once, and call it from one serial queue.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn openagents_mobile_create(bytes: *const u8, len: usize) -> *mut App {
    if bytes.is_null() || len == 0 || len > 16 * 1024 {
        return ptr::null_mut();
    }
    catch_unwind(AssertUnwindSafe(|| {
        let bytes = unsafe { std::slice::from_raw_parts(bytes, len) };
        let config: Config = serde_json::from_slice(bytes).ok()?;
        App::new(config)
            .ok()
            .map(|app| Box::into_raw(Box::new(app)))
    }))
    .ok()
    .flatten()
    .unwrap_or(ptr::null_mut())
}

/// # Safety
/// `handle` must be a live handle from `openagents_mobile_create` with
/// exclusive access, and `bytes` must point to `len` readable bytes. Free the
/// result once with `openagents_mobile_buffer_free`. An empty result means
/// the request failed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn openagents_mobile_call(
    handle: *mut App,
    bytes: *const u8,
    len: usize,
) -> OpenAgentsMobileBuffer {
    if handle.is_null() || bytes.is_null() || len == 0 || len > 64 * 1024 {
        return buffer(vec![]);
    }
    catch_unwind(AssertUnwindSafe(|| {
        let bytes = unsafe { std::slice::from_raw_parts(bytes, len) };
        let Ok(request) = serde_json::from_slice::<Request>(bytes) else {
            return vec![];
        };
        let reply = unsafe { &mut *handle }.respond(request);
        serde_json::to_vec(&reply).unwrap_or_default()
    }))
    .map(buffer)
    .unwrap_or_else(|_| buffer(vec![]))
}

/// # Safety
/// The buffer must be an unmodified, not-yet-freed result from this library.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn openagents_mobile_buffer_free(value: OpenAgentsMobileBuffer) {
    if !value.data.is_null() {
        unsafe {
            drop(Box::from_raw(ptr::slice_from_raw_parts_mut(
                value.data, value.len,
            )))
        }
    }
}

/// # Safety
/// The handle must come from `openagents_mobile_create`, have no call in
/// progress, and not be destroyed already.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn openagents_mobile_destroy(handle: *mut App) {
    if !handle.is_null() {
        unsafe { drop(Box::from_raw(handle)) }
    }
}

#[cfg(test)]
mod tests;
