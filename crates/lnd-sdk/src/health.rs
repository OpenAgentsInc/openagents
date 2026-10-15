//! Node health policy shared by the timer and its forced-condition checks.
use crate::service::Health;
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Policy {
    pub minimum_inbound_msat: u64,
    pub hot_cap_sats: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Alert {
    NotSynced,
    NoActiveChannels,
    PendingForceClose,
    LowInbound,
    HotBalanceCap,
}
impl Policy {
    pub fn alerts(&self, node: &Health, hot_sats: u128) -> Vec<Alert> {
        let mut alerts = Vec::new();
        if !node.synced_to_chain || !node.synced_to_graph {
            alerts.push(Alert::NotSynced);
        }
        if node.active_channels == 0 {
            alerts.push(Alert::NoActiveChannels);
        }
        if node.pending_force_closes > 0 {
            alerts.push(Alert::PendingForceClose);
        }
        if node.inbound_msat < self.minimum_inbound_msat {
            alerts.push(Alert::LowInbound);
        }
        if hot_sats > u128::from(self.hot_cap_sats) {
            alerts.push(Alert::HotBalanceCap);
        }
        alerts
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_condition_fires_its_own_alert() {
        let policy = Policy {
            minimum_inbound_msat: 100,
            hot_cap_sats: 100,
        };
        let baseline = Health {
            synced_to_chain: true,
            synced_to_graph: true,
            active_channels: 2,
            inbound_msat: 100,
            ..Default::default()
        };
        assert!(policy.alerts(&baseline, 100).is_empty());
        let mut node = baseline.clone();
        node.synced_to_chain = false;
        assert_eq!(policy.alerts(&node, 0), vec![Alert::NotSynced]);
        let mut node = baseline.clone();
        node.active_channels = 0;
        assert_eq!(policy.alerts(&node, 0), vec![Alert::NoActiveChannels]);
        let mut node = baseline.clone();
        node.pending_force_closes = 1;
        assert_eq!(policy.alerts(&node, 0), vec![Alert::PendingForceClose]);
        let mut node = baseline.clone();
        node.inbound_msat = 99;
        assert_eq!(policy.alerts(&node, 0), vec![Alert::LowInbound]);
        assert_eq!(policy.alerts(&baseline, 101), vec![Alert::HotBalanceCap]);
    }
}
