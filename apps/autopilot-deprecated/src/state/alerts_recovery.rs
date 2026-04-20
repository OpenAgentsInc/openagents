use crate::app_state::{PaneLoadState, PaneStatusAccess};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

impl AlertSeverity {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Critical => "critical",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AlertLifecycle {
    Active,
    Acknowledged,
    Resolved,
}

impl AlertLifecycle {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Acknowledged => "acknowledged",
            Self::Resolved => "resolved",
        }
    }
}

#[allow(dead_code)] // Identity/wallet/relay/sync domains are reserved for non-command alert ingestion paths.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AlertDomain {
    Identity,
    Wallet,
    Relays,
    ProviderRuntime,
    Sync,
    SkillTrust,
    Credit,
}

impl AlertDomain {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Identity => "identity",
            Self::Wallet => "wallet",
            Self::Relays => "relays",
            Self::ProviderRuntime => "provider",
            Self::Sync => "sync",
            Self::SkillTrust => "skill-trust",
            Self::Credit => "credit",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryAlertRow {
    pub alert_id: String,
    pub domain: AlertDomain,
    pub severity: AlertSeverity,
    pub lifecycle: AlertLifecycle,
    pub summary: String,
    pub remediation: String,
    pub last_transition_epoch_seconds: u64,
}

pub struct AlertsRecoveryState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub alerts: Vec<RecoveryAlertRow>,
    pub selected_alert_id: Option<String>,
    next_transition_seq: u64,
}

impl Default for AlertsRecoveryState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for alert lane snapshot".to_string()),
            alerts: Vec::new(),
            selected_alert_id: None,
            next_transition_seq: 1,
        }
    }
}

impl AlertsRecoveryState {
    pub fn select_by_index(&mut self, index: usize) -> bool {
        let Some(alert_id) = self.alerts.get(index).map(|alert| alert.alert_id.clone()) else {
            return false;
        };
        self.selected_alert_id = Some(alert_id);
        self.pane_clear_error();
        true
    }

    pub fn selected(&self) -> Option<&RecoveryAlertRow> {
        let selected = self.selected_alert_id.as_deref()?;
        self.alerts.iter().find(|alert| alert.alert_id == selected)
    }

    pub fn selected_domain(&self) -> Option<AlertDomain> {
        self.selected().map(|alert| alert.domain)
    }

    pub fn acknowledge_selected(&mut self) -> Result<String, String> {
        let selected = self
            .selected_alert_id
            .as_deref()
            .ok_or_else(|| "Select an alert first".to_string())?
            .to_string();
        let transition_epoch = self.next_transition_epoch();
        let Some(alert) = self
            .alerts
            .iter_mut()
            .find(|alert| alert.alert_id == selected)
        else {
            return Err(self.pane_set_error("Selected alert no longer exists"));
        };

        if alert.lifecycle == AlertLifecycle::Resolved {
            return Err(self.pane_set_error("Resolved alert cannot be acknowledged"));
        }

        alert.lifecycle = AlertLifecycle::Acknowledged;
        alert.last_transition_epoch_seconds = transition_epoch;
        let alert_id = alert.alert_id.clone();
        self.pane_set_ready(format!("Acknowledged {alert_id}"));
        Ok(alert_id)
    }

    pub fn resolve_selected(&mut self) -> Result<String, String> {
        let selected = self
            .selected_alert_id
            .as_deref()
            .ok_or_else(|| "Select an alert first".to_string())?
            .to_string();
        let transition_epoch = self.next_transition_epoch();
        let Some(alert) = self
            .alerts
            .iter_mut()
            .find(|alert| alert.alert_id == selected)
        else {
            return Err(self.pane_set_error("Selected alert no longer exists"));
        };

        alert.lifecycle = AlertLifecycle::Resolved;
        alert.last_transition_epoch_seconds = transition_epoch;
        let alert_id = alert.alert_id.clone();
        self.pane_set_ready(format!("Resolved {alert_id}"));
        Ok(alert_id)
    }

    fn next_transition_epoch(&mut self) -> u64 {
        let epoch = 1_761_920_000u64.saturating_add(self.next_transition_seq * 17);
        self.next_transition_seq = self.next_transition_seq.saturating_add(1);
        epoch
    }
}
