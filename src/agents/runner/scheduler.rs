//! Agent Scheduler
//!
//! Manages heartbeat timing and event triggers for agent ticks.

use super::tick::{TickExecutor, TickResult, TickTrigger};
use crate::agents::SharedRelay;
use agent::LifecycleState;
use anyhow::Result;
use nostr::{Event, KIND_CHANNEL_MESSAGE};
use std::time::Duration;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Agent scheduler that fires ticks on heartbeat and events
pub struct Scheduler {
    /// Heartbeat interval in seconds
    heartbeat_seconds: u64,
    /// Event triggers to listen for
    triggers: Vec<String>,
    /// Agent pubkey (for filtering mentions)
    agent_pubkey: String,
    /// Relay connection
    relay: SharedRelay,
    /// Optional channel ID to listen for NIP-28 messages
    channel_id: Option<String>,
}

impl Scheduler {
    /// Create a new scheduler
    pub fn new(
        heartbeat_seconds: u64,
        triggers: Vec<String>,
        agent_pubkey: String,
        relay: SharedRelay,
        channel_id: Option<String>,
    ) -> Self {
        Self {
            heartbeat_seconds,
            triggers,
            agent_pubkey,
            relay,
            channel_id,
        }
    }

    /// Run the scheduler loop
    pub async fn run(&self, executor: &mut TickExecutor) -> Result<()> {
        let heartbeat = Duration::from_secs(self.heartbeat_seconds);

        // Subscribe to triggers
        let trigger_rx = self.subscribe_triggers().await?;
        let mut trigger_rx = trigger_rx;

        tracing::info!(
            "Scheduler started: heartbeat every {} seconds, triggers: {:?}",
            self.heartbeat_seconds,
            self.triggers
        );

        // Run until agent goes dormant or interrupted
        loop {
            let trigger = tokio::select! {
                // Heartbeat timer
                _ = tokio::time::sleep(heartbeat) => {
                    TickTrigger::Heartbeat
                }

                // Event triggers
                Some(event) = trigger_rx.recv() => {
                    self.classify_trigger(&event)
                }
            };

            // Execute tick
            match executor.execute_tick(trigger).await {
                Ok(result) => {
                    self.log_tick_result(&result);

                    // Check if agent is dormant (zero balance)
                    // Dormant agents stop ticking but can be revived by funding
                    if matches!(result.lifecycle_state, LifecycleState::Dormant) {
                        tracing::warn!("Agent is dormant (zero balance). Stopping scheduler.");
                        tracing::info!("Fund the agent to revive it.");
                        break;
                    }
                }
                Err(e) => {
                    tracing::error!("Tick execution failed: {}", e);
                    // Continue running - one failed tick shouldn't stop the agent
                }
            }
        }

        Ok(())
    }

    /// Run a single tick (for testing or CLI --single-tick mode)
    pub async fn run_single_tick(&self, executor: &mut TickExecutor) -> Result<TickResult> {
        executor.execute_tick(TickTrigger::Heartbeat).await
    }

    /// Subscribe to trigger events (mentions, DMs, zaps)
    async fn subscribe_triggers(&self) -> Result<mpsc::Receiver<Event>> {
        let mut filter_kinds = vec![];
        let mut filters = Vec::new();

        // Build filter based on enabled triggers
        if self.triggers.contains(&"mention".to_string()) {
            filter_kinds.push(1u64); // kind:1 notes
        }
        if self.triggers.contains(&"dm".to_string()) {
            filter_kinds.push(4u64); // kind:4 encrypted DM
        }
        if self.triggers.contains(&"zap".to_string()) {
            filter_kinds.push(9735u64); // kind:9735 zap receipt
        }

        if !filter_kinds.is_empty() {
            filters.push(serde_json::json!({
                "kinds": filter_kinds,
                "#p": [self.agent_pubkey]
            }));
        }

        if self.triggers.contains(&"channel".to_string()) {
            if let Some(channel_id) = &self.channel_id {
                filters.push(serde_json::json!({
                    "kinds": [KIND_CHANNEL_MESSAGE as u64],
                    "#e": [channel_id]
                }));
            }
        }

        if filters.is_empty() {
            let (_, rx) = mpsc::channel(1);
            return Ok(rx);
        }

        let subscription_id = format!("agent-triggers-{}", Uuid::new_v4());
        let rx = self
            .relay
            .subscribe_with_channel(&subscription_id, &filters)
            .await?;

        Ok(rx)
    }

    /// Classify an event into a trigger type
    fn classify_trigger(&self, event: &Event) -> TickTrigger {
        match event.kind {
            1 => TickTrigger::Mention(event.clone()),
            4 => TickTrigger::DirectMessage(event.clone()),
            9735 => TickTrigger::Zap(event.clone()),
            KIND_CHANNEL_MESSAGE => TickTrigger::ChannelMessage(event.clone()),
            _ => TickTrigger::Heartbeat, // Fallback
        }
    }

    /// Log tick result summary
    fn log_tick_result(&self, result: &TickResult) {
        if let Some(hash) = &result.trajectory_hash {
            tracing::info!(
                "Tick #{} complete: state={:?}, cost={} sats, actions={}, trajectory={}...",
                result.tick_number,
                result.lifecycle_state,
                result.compute_cost_sats,
                result.actions.len(),
                &hash[..16]
            );
        } else {
            tracing::info!(
                "Tick #{} complete: state={:?}, cost={} sats, actions={}",
                result.tick_number,
                result.lifecycle_state,
                result.compute_cost_sats,
                result.actions.len()
            );
        }

        if result.runway.days_remaining < 7.0 {
            tracing::warn!(
                "Low runway warning: {:.1} days remaining ({} sats)",
                result.runway.days_remaining,
                result.runway.balance_sats
            );
        }
    }
}
