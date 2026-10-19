//! Read-only account usage for the computer handling a voice call.
use super::{CoreClient, CoreError, CoreResult, VoiceCall};
use crate::voice_ui::{self, VoiceIntent};
use openagents_client::{Client, ClientError, DeviceView};
use openagents_proto::{AgentAccountsSnapshot, HarnessId};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum VoiceUsagePhase {
    NoHost,
    Loading,
    Ready,
    Offline,
    SignInRequired,
    Unsupported,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct VoiceUsageWindow {
    pub label: String,
    pub used_fraction: f32,
    pub resets_at_ms: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct VoiceUsageSnapshot {
    pub phase: VoiceUsagePhase,
    pub host_id: Option<String>,
    pub host_name: Option<String>,
    pub account: Option<String>,
    pub plan: Option<String>,
    pub windows: Vec<VoiceUsageWindow>,
    pub fetched_at_ms: Option<i64>,
    pub error: Option<String>,
    pub refreshing: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Target {
    id: String,
    name: String,
    online: bool,
    supported: bool,
}

/// A live call always wins over the preferences for the next call.
fn target(devices: &[DeviceView], preferred: Option<&str>, active: Option<&str>) -> Option<Target> {
    let compatible = |d: &&DeviceView| {
        d.is_execution_host
            && d.capabilities
                .iter()
                .any(|c| c == &super::voice_host_capability())
    };
    let selected = if let Some(id) = active {
        // Keep the call's identity even when presence disappears.
        return Some(devices.iter().find(|d| d.id == id).map_or_else(
            || Target {
                id: id.into(),
                name: "Call computer".into(),
                online: false,
                supported: true,
            },
            |d| Target {
                id: d.id.clone(),
                name: d.name.clone(),
                online: d.online,
                supported: true,
            },
        ));
    } else {
        let ready: Vec<_> = devices
            .iter()
            .filter(compatible)
            .filter(|d| d.online)
            .collect();
        preferred
            .and_then(|id| ready.iter().copied().find(|d| d.id == id))
            .or_else(|| (ready.len() == 1).then(|| ready[0]))
            .or_else(|| preferred.and_then(|id| devices.iter().find(|d| d.id == id)))
    };
    selected.map(|d| Target {
        id: d.id.clone(),
        name: d.name.clone(),
        online: d.online,
        supported: compatible(&d),
    })
}

impl VoiceUsageSnapshot {
    fn initial(target: Option<&Target>) -> Self {
        Self {
            phase: match target {
                None => VoiceUsagePhase::NoHost,
                Some(t) if !t.online => VoiceUsagePhase::Offline,
                Some(t) if !t.supported => VoiceUsagePhase::Unsupported,
                _ => VoiceUsagePhase::Loading,
            },
            host_id: target.map(|t| t.id.clone()),
            host_name: target.map(|t| t.name.clone()),
            account: None,
            plan: None,
            windows: vec![],
            fetched_at_ms: None,
            error: None,
            refreshing: false,
        }
    }

    fn apply_accounts(&mut self, accounts: AgentAccountsSnapshot) {
        // Clear the previous account even if the new account has no usage.
        self.account = None;
        self.plan = None;
        self.windows.clear();
        self.fetched_at_ms = None;
        self.error = None;
        let Some(account) = accounts
            .accounts
            .into_iter()
            .find(|a| a.harness == HarnessId::Codex && a.active)
        else {
            self.phase = VoiceUsagePhase::SignInRequired;
            return;
        };
        self.account = account.email.or(account.display_name);
        self.plan = account.plan_label;
        self.fetched_at_ms = account.usage_fetched_at;
        self.error = account.usage_error;
        self.windows = account
            .usage_windows
            .into_iter()
            .filter(|w| w.used_fraction.is_finite())
            .map(|w| VoiceUsageWindow {
                label: w.label,
                used_fraction: w.used_fraction.clamp(0.0, 1.0),
                resets_at_ms: w.resets_at.map(|at| at.timestamp_millis()),
            })
            .collect();
        self.phase = if self.windows.is_empty() {
            VoiceUsagePhase::Unavailable
        } else {
            VoiceUsagePhase::Ready
        };
    }

    fn failed(&mut self, error: ClientError) {
        self.refreshing = false;
        self.phase = match error {
            ClientError::Unsupported(_) => VoiceUsagePhase::Unsupported,
            ClientError::HostUnavailable(_) | ClientError::Network(_) => VoiceUsagePhase::Offline,
            _ => VoiceUsagePhase::Unavailable,
        };
        self.error = Some(
            match self.phase {
                VoiceUsagePhase::Unsupported => {
                    "Update OpenAgents on this computer to see account usage."
                }
                VoiceUsagePhase::Offline => {
                    "Couldn't reach this computer. Try again when it's online."
                }
                _ => "Couldn't refresh Codex usage. Try again.",
            }
            .into(),
        );
    }
}

struct State {
    ticket: u64,
    target: Option<Target>,
    snapshot: VoiceUsageSnapshot,
    view_instance: String,
    view_revision: u64,
    current_view: Option<rust_native::ValidatedView<VoiceIntent>>,
}

static NEXT_USAGE_VIEW: AtomicU64 = AtomicU64::new(1);
impl State {
    fn retarget(&mut self, next: Option<Target>) -> bool {
        if next == self.target {
            return false;
        }
        let same_host = next.as_ref().map(|t| &t.id) == self.target.as_ref().map(|t| &t.id);
        self.ticket += 1;
        let initial = VoiceUsageSnapshot::initial(next.as_ref());
        if same_host {
            self.snapshot.host_name = initial.host_name;
            self.snapshot.phase = initial.phase;
            self.snapshot.refreshing = false;
        } else {
            self.snapshot = initial;
        }
        self.target = next;
        self.current_view = None;
        true
    }

    fn apply(&mut self, ticket: u64, update: impl FnOnce(&mut VoiceUsageSnapshot)) -> bool {
        if self.ticket != ticket {
            return false;
        }
        update(&mut self.snapshot);
        self.current_view = None;
        true
    }
}

#[uniffi::export(with_foreign)]
pub trait VoiceUsageListener: Send + Sync {
    /// Pull the current snapshot on the UI thread; queued callbacks carry no old host data.
    fn on_voice_usage_changed(&self);
}

#[derive(uniffi::Object)]
pub struct CoreVoiceUsage {
    client: Client,
    state: Arc<Mutex<State>>,
    changed: watch::Sender<u64>,
    cancel: CancellationToken,
}
impl Drop for CoreVoiceUsage {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}
fn lock<T>(value: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    value
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[uniffi::export]
impl CoreVoiceUsage {
    pub fn snapshot(&self) -> VoiceUsageSnapshot {
        lock(&self.state).snapshot.clone()
    }

    /// Presence and preferences may change while this screen is open.
    pub fn track(&self, preferred_host: Option<String>, active_call: Option<Arc<VoiceCall>>) {
        let workspace = self.client.workspace();
        let next = target(
            &workspace.devices,
            preferred_host.as_deref(),
            active_call.as_ref().map(|c| c.host_device_id.as_str()),
        );
        let mut state = lock(&self.state);
        if !state.retarget(next) {
            return;
        }
        self.changed.send_replace(state.ticket);
    }

    pub fn refresh(&self) {
        let mut state = lock(&self.state);
        state.current_view = None;
        state.ticket += 1;
        self.changed.send_replace(state.ticket);
    }

    /// Build and retain the current usage view so activations resolve in Rust.
    pub fn view_json(&self) -> CoreResult<String> {
        if self.cancel.is_cancelled() {
            return Err(CoreError::Closed);
        }
        let mut state = lock(&self.state);
        if self.cancel.is_cancelled() {
            return Err(CoreError::Closed);
        }
        state.view_revision =
            state
                .view_revision
                .checked_add(1)
                .ok_or_else(|| CoreError::InvalidArgument {
                    message: "Couldn't refresh voice usage.".into(),
                })?;
        let view =
            voice_ui::voice_usage_view(&state.view_instance, state.view_revision, &state.snapshot)
                .map_err(|_| CoreError::InvalidArgument {
                    message: "Couldn't open voice usage.".into(),
                })?;
        let json = serde_json::to_string(view.view()).map_err(|_| CoreError::InvalidArgument {
            message: "Couldn't open voice usage.".into(),
        })?;
        state.current_view = Some(view);
        Ok(json)
    }

    /// Resolve and run refresh from the current Rust Native tree by identity only.
    pub fn activate_view(&self, instance: String, revision: u64, node: String) -> CoreResult<()> {
        if self.cancel.is_cancelled() {
            return Err(CoreError::Closed);
        }
        let mut state = lock(&self.state);
        if self.cancel.is_cancelled() {
            return Err(CoreError::Closed);
        }
        let activation = rust_native::Activation {
            instance,
            revision,
            node,
        };
        let intent = state
            .current_view
            .as_ref()
            .and_then(|view| view.activate(&activation).ok().cloned())
            .ok_or_else(|| CoreError::InvalidArgument {
                message: "Those voice controls are out of date.".into(),
            })?;
        match intent {
            VoiceIntent::RefreshUsage => {
                state.current_view = None;
                drop(state);
                self.refresh();
                Ok(())
            }
            _ => Err(CoreError::InvalidArgument {
                message: "That control is not available here.".into(),
            }),
        }
    }
}

#[uniffi::export]
impl CoreClient {
    pub fn voice_usage(&self, listener: Arc<dyn VoiceUsageListener>) -> Arc<CoreVoiceUsage> {
        let (changed, mut requests) = watch::channel(0);
        let state = Arc::new(Mutex::new(State {
            ticket: 0,
            target: None,
            snapshot: VoiceUsageSnapshot::initial(None),
            view_instance: format!(
                "voice-usage-{}",
                NEXT_USAGE_VIEW.fetch_add(1, Ordering::Relaxed)
            ),
            view_revision: 0,
            current_view: None,
        }));
        let cancel = self.client.voice_cancellation();
        let usage = Arc::new(CoreVoiceUsage {
            client: self.client.clone(),
            state: state.clone(),
            changed,
            cancel: cancel.clone(),
        });
        let client = self.client.clone();
        // The task owns no CoreVoiceUsage, so dismissing the screen cancels it.
        super::zc::runtime::handle().spawn(async move {
            let mut probes = std::collections::HashMap::<String, Instant>::new();
            loop {
                let (ticket, target) = {
                    let s = lock(&state);
                    (s.ticket, s.target.clone())
                };
                let load = async {
                    if let Some(t) = target.filter(|t| t.online && t.supported) {
                        lock(&state).apply(ticket, |s| s.refreshing = true);
                        listener.on_voice_usage_changed();
                        match client.agent_accounts(&t.id, false).await {
                            Ok(accounts) => {
                                lock(&state).apply(ticket, |s| s.apply_accounts(accounts));
                            }
                            Err(error) => {
                                lock(&state).apply(ticket, |s| s.failed(error));
                                return;
                            }
                        }
                        listener.on_voice_usage_changed();
                        if probes
                            .get(&t.id)
                            .is_none_or(|at| at.elapsed() >= Duration::from_secs(30))
                        {
                            probes.insert(t.id.clone(), Instant::now());
                            match client.agent_accounts(&t.id, true).await {
                                Ok(accounts) => {
                                    lock(&state).apply(ticket, |s| s.apply_accounts(accounts));
                                }
                                Err(error) => {
                                    lock(&state).apply(ticket, |s| s.failed(error));
                                }
                            }
                        }
                        lock(&state).apply(ticket, |s| s.refreshing = false);
                    }
                };
                tokio::select! {
                    biased;
                    _ = cancel.cancelled() => break,
                    r = requests.changed() => { if r.is_err() { break; } continue; },
                    _ = load => {},
                }
                listener.on_voice_usage_changed();
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    r = requests.changed() => { if r.is_err() { break; } },
                    _ = tokio::time::sleep(Duration::from_secs(300)) => {},
                }
            }
        });
        usage
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn device(id: &str, online: bool) -> DeviceView {
        DeviceView {
            id: id.into(),
            name: format!("Computer {id}"),
            platform: "macos".into(),
            origin: openagents_proto::DeviceOrigin::Unknown,
            online,
            last_seen_ms: None,
            version: None,
            capabilities: vec![super::super::voice_host_capability()],
            is_execution_host: true,
            is_self: false,
            session_count: 0,
        }
    }
    fn accounts(
        email: &str,
        windows: serde_json::Value,
        error: Option<&str>,
    ) -> AgentAccountsSnapshot {
        serde_json::from_value(json!({"accounts":[
            {"id":"other", "harness":"codex", "email":"unused@example.com", "planLabel":"Pro", "active":false,
             "usageWindows":[{"label":"Session", "usedFraction":0.99}]},
            {"id":"active", "harness":"codex", "email":email, "planLabel":"Plus", "active":true,
             "usageWindows":windows, "usageFetchedAt":1234, "usageError":error}
        ], "warnings":[]})).unwrap()
    }
    fn windows() -> serde_json::Value {
        json!([
            {"label":"Session", "usedFraction":0.42, "resetsAt":"2026-10-19T12:00:00Z"},
            {"label":"Week", "usedFraction":0.7}
        ])
    }
    #[test]
    fn call_host_wins_over_next_preference_and_missing_presence() {
        let devices = vec![device("a", true), device("b", true)];
        assert_eq!(target(&devices, Some("b"), Some("a")).unwrap().id, "a");
        assert_eq!(target(&devices, Some("b"), None).unwrap().id, "b");
        let lost = target(&[], Some("b"), Some("a")).unwrap();
        assert_eq!(lost.id, "a");
        assert!(!lost.online);
        assert!(target(&devices, None, None).is_none());
        assert_eq!(target(&devices[..1], None, None).unwrap().id, "a");
    }
    #[test]
    fn idle_fallback_offline_and_unsupported_hosts() {
        let devices = vec![device("a", false), device("b", true)];
        assert_eq!(target(&devices, Some("a"), None).unwrap().id, "b");
        let offline = target(&devices[..1], Some("a"), None).unwrap();
        assert_eq!(
            VoiceUsageSnapshot::initial(Some(&offline)).phase,
            VoiceUsagePhase::Offline
        );
        let mut old = device("old", true);
        old.capabilities.clear();
        let unsupported = target(&[old], Some("old"), None).unwrap();
        assert_eq!(
            VoiceUsageSnapshot::initial(Some(&unsupported)).phase,
            VoiceUsagePhase::Unsupported
        );
    }
    #[test]
    fn account_windows_reset_and_fetch_time_come_from_active_codex() {
        let mut s = VoiceUsageSnapshot::initial(None);
        s.apply_accounts(accounts("active@example.com", windows(), None));
        assert_eq!(s.phase, VoiceUsagePhase::Ready);
        assert_eq!(s.account.as_deref(), Some("active@example.com"));
        assert_eq!(s.plan.as_deref(), Some("Plus"));
        assert_eq!(s.windows[0].used_fraction, 0.42);
        assert_eq!(s.windows[0].resets_at_ms, Some(1_792_411_200_000));
        assert_eq!(s.windows[1].resets_at_ms, None);
        assert_eq!(s.fetched_at_ms, Some(1234));
    }
    #[test]
    fn failures_keep_last_good_data_but_account_changes_clear_old_windows() {
        let mut s = VoiceUsageSnapshot::initial(None);
        s.apply_accounts(accounts("a@example.com", windows(), Some("Rate limited")));
        assert_eq!(s.error.as_deref(), Some("Rate limited"));
        s.failed(ClientError::HostUnavailable("offline".into()));
        assert_eq!(s.phase, VoiceUsagePhase::Offline);
        assert_eq!(s.windows.len(), 2);
        assert_eq!(s.fetched_at_ms, Some(1234));
        s.apply_accounts(accounts("b@example.com", json!([]), Some("Sign in again")));
        assert_eq!(s.account.as_deref(), Some("b@example.com"));
        assert!(s.windows.is_empty());
        assert_eq!(s.phase, VoiceUsagePhase::Unavailable);
        assert_eq!(s.error.as_deref(), Some("Sign in again"));
        s.apply_accounts(AgentAccountsSnapshot::default());
        assert_eq!(s.phase, VoiceUsagePhase::SignInRequired);
        assert!(s.account.is_none());
        assert!(s.fetched_at_ms.is_none());
    }
    #[test]
    fn old_host_replies_cannot_replace_new_host() {
        let b = target(&[device("b", true)], None, None).unwrap();
        let mut state = State {
            ticket: 1,
            snapshot: VoiceUsageSnapshot::initial(None),
            target: None,
            view_instance: "test-view".into(),
            view_revision: 0,
            current_view: None,
        };
        assert!(state.retarget(Some(b)));
        assert!(!state.apply(1, |s| s.apply_accounts(accounts(
            "a@example.com",
            windows(),
            None
        ))));
        assert!(!state.apply(1, |s| s.failed(ClientError::Unsupported("old host".into()))));
        assert_eq!(state.snapshot.host_id.as_deref(), Some("b"));
        assert!(state.snapshot.account.is_none());
        assert!(state.apply(2, |s| s.apply_accounts(accounts(
            "b@example.com",
            windows(),
            None
        ))));
        assert_eq!(state.snapshot.account.as_deref(), Some("b@example.com"));
    }
    #[test]
    fn same_host_rename_and_offline_keep_last_known_data() {
        let a = target(&[device("a", true)], None, None).unwrap();
        let mut state = State {
            ticket: 0,
            target: None,
            snapshot: VoiceUsageSnapshot::initial(None),
            view_instance: "test-view".into(),
            view_revision: 0,
            current_view: None,
        };
        assert!(state.retarget(Some(a.clone())));
        state
            .snapshot
            .apply_accounts(accounts("a@example.com", windows(), None));
        let mut renamed = a;
        renamed.name = "Renamed computer".into();
        renamed.online = false;
        assert!(state.retarget(Some(renamed)));
        assert_eq!(
            state.snapshot.host_name.as_deref(),
            Some("Renamed computer")
        );
        assert_eq!(state.snapshot.phase, VoiceUsagePhase::Offline);
        assert_eq!(state.snapshot.windows.len(), 2);
        assert_eq!(state.snapshot.fetched_at_ms, Some(1234));
        assert!(state.retarget(None));
        assert_eq!(state.snapshot.phase, VoiceUsagePhase::NoHost);
        assert!(state.snapshot.windows.is_empty());
    }
}
