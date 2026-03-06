//! Subscription management for receiving filtered events.

use crate::error::{ClientError, Result};
use nostr::Event;
use nostr::nip77::{
    Bound, NegClose, NegOpen, NegentropyMessage, Range, Record, calculate_fingerprint,
    sort_records,
};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Callback type for handling received events.
pub type EventCallback = Arc<dyn Fn(Event) -> Result<()> + Send + Sync>;

/// Gap-recovery transport selected for a sync attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GapRecoveryTransport {
    ReqEoseOnly,
    NegentropyReqFallback,
}

/// Relay capability signal for NIP-77 support.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayGapRecoveryCapability {
    Unknown,
    Negentropy,
    ReqOnly,
}

/// Initial sync plan for a subscription.
#[derive(Debug, Clone, PartialEq)]
pub struct GapRecoveryPlan {
    pub transport: GapRecoveryTransport,
    pub req_payload: Value,
    pub neg_open_payload: Option<Value>,
    pub neg_close_payload: Option<Value>,
}

/// A subscription to filtered events from a relay.
#[derive(Clone)]
pub struct Subscription {
    /// Subscription ID.
    pub id: String,
    /// Filters for this subscription.
    pub filters: Vec<Value>,
    eose_received: Arc<std::sync::atomic::AtomicBool>,
    callback: Option<EventCallback>,
    event_tx: Option<mpsc::Sender<Event>>,
}

impl Subscription {
    /// Create a new subscription with filters.
    pub fn new(id: String, filters: Vec<Value>) -> Self {
        Self {
            id,
            filters,
            eose_received: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            callback: None,
            event_tx: None,
        }
    }

    /// Create a subscription with callback-based event handling.
    pub fn with_callback(id: String, filters: Vec<Value>, callback: EventCallback) -> Self {
        Self {
            id,
            filters,
            eose_received: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            callback: Some(callback),
            event_tx: None,
        }
    }

    /// Create a subscription that receives events on a bounded channel.
    pub fn with_channel(id: String, filters: Vec<Value>) -> (Self, mpsc::Receiver<Event>) {
        let (tx, rx) = mpsc::channel(1000);
        let subscription = Self {
            id,
            filters,
            eose_received: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            callback: None,
            event_tx: Some(tx),
        };
        (subscription, rx)
    }

    /// Handle a received event.
    pub fn handle_event(&self, event: Event) -> Result<()> {
        if let Some(callback) = &self.callback {
            callback(event.clone())?;
        }

        if let Some(tx) = &self.event_tx {
            tx.try_send(event).map_err(|error| match error {
                mpsc::error::TrySendError::Full(_) => {
                    ClientError::Subscription("event channel full - consumer too slow".to_string())
                }
                mpsc::error::TrySendError::Closed(_) => {
                    ClientError::Subscription("event channel closed".to_string())
                }
            })?;
        }

        Ok(())
    }

    /// Mark EOSE as received.
    pub fn mark_eose(&self) {
        self.eose_received
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// Check if EOSE has been received.
    pub fn has_eose(&self) -> bool {
        self.eose_received
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Build the initial gap-recovery plan for this subscription.
    ///
    /// NIP-77 currently works with a single filter. Multi-filter subscriptions
    /// fall back to a normal `REQ` / `EOSE` sync, which remains deterministic.
    pub fn build_gap_recovery_plan(
        &self,
        capability: RelayGapRecoveryCapability,
        local_records: &[Record],
    ) -> nostr::nip77::Result<GapRecoveryPlan> {
        let req_payload = build_req_payload(self);
        let use_negentropy = matches!(capability, RelayGapRecoveryCapability::Negentropy)
            && self.filters.len() == 1;
        if !use_negentropy {
            return Ok(GapRecoveryPlan {
                transport: GapRecoveryTransport::ReqEoseOnly,
                req_payload,
                neg_open_payload: None,
                neg_close_payload: None,
            });
        }

        let mut records = local_records.to_vec();
        sort_records(&mut records);
        let ids = records.iter().map(|record| record.id).collect::<Vec<_>>();
        let fingerprint = calculate_fingerprint(&ids);
        let message = NegentropyMessage::new(vec![Range::fingerprint(
            Bound::infinity(),
            fingerprint,
        )]);
        let neg_subscription_id = format!("neg-{}", self.id);
        let neg_open =
            NegOpen::new(neg_subscription_id.clone(), self.filters[0].clone(), &message)?
                .to_json();
        let neg_close = NegClose::new(neg_subscription_id).to_json();
        Ok(GapRecoveryPlan {
            transport: GapRecoveryTransport::NegentropyReqFallback,
            req_payload,
            neg_open_payload: Some(neg_open),
            neg_close_payload: Some(neg_close),
        })
    }
}

pub(crate) fn build_req_payload(subscription: &Subscription) -> Value {
    let mut frame = Vec::with_capacity(subscription.filters.len().saturating_add(2));
    frame.push(Value::String("REQ".to_string()));
    frame.push(Value::String(subscription.id.clone()));
    frame.extend(subscription.filters.iter().cloned());
    Value::Array(frame)
}

#[cfg(test)]
mod tests {
    use super::{
        GapRecoveryTransport, RelayGapRecoveryCapability, Subscription, build_req_payload,
    };
    use nostr::nip77::{NegOpen, Record};
    use serde_json::json;

    fn record(timestamp: u64, byte: u8) -> Record {
        Record::new(timestamp, [byte; 32])
    }

    #[test]
    fn gap_recovery_plan_prefers_negentropy_for_single_filter_relays() {
        let subscription = Subscription::new(
            "chat-sync".to_string(),
            vec![json!({"kinds":[42], "#h":["oa-main"]})],
        );
        let plan = subscription
            .build_gap_recovery_plan(
                RelayGapRecoveryCapability::Negentropy,
                &[record(10, 0x01), record(20, 0x02)],
            )
            .expect("negentropy plan");

        assert_eq!(plan.transport, GapRecoveryTransport::NegentropyReqFallback);
        assert_eq!(plan.req_payload, build_req_payload(&subscription));
        let neg_open = NegOpen::from_json(plan.neg_open_payload.as_ref().expect("neg-open"))
            .expect("parse neg-open");
        assert_eq!(neg_open.subscription_id, "neg-chat-sync");
        assert_eq!(neg_open.filter, json!({"kinds":[42], "#h":["oa-main"]}));
        assert!(plan.neg_close_payload.is_some());
    }

    #[test]
    fn gap_recovery_plan_falls_back_when_negentropy_is_unavailable_or_ambiguous() {
        let single_filter = Subscription::new(
            "chat-sync".to_string(),
            vec![json!({"kinds":[42], "#h":["oa-main"]})],
        );
        let no_negentropy = single_filter
            .build_gap_recovery_plan(RelayGapRecoveryCapability::ReqOnly, &[record(10, 0x01)])
            .expect("req fallback");
        assert_eq!(no_negentropy.transport, GapRecoveryTransport::ReqEoseOnly);
        assert!(no_negentropy.neg_open_payload.is_none());

        let multi_filter = Subscription::new(
            "chat-sync".to_string(),
            vec![json!({"kinds":[42]}), json!({"kinds":[39000]})],
        );
        let ambiguous = multi_filter
            .build_gap_recovery_plan(
                RelayGapRecoveryCapability::Negentropy,
                &[record(10, 0x01)],
            )
            .expect("multi-filter fallback");
        assert_eq!(ambiguous.transport, GapRecoveryTransport::ReqEoseOnly);
        assert!(ambiguous.neg_open_payload.is_none());
    }
}
