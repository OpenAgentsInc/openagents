//! Agent Scheduler
//!
//! Manages heartbeat timing and event triggers for agent ticks.

use super::tick::{TickExecutor, TickResult, TickTrigger};
use agent::LifecycleState;
use anyhow::Result;
use nostr::Event;
use nostr_client::RelayConnection;
use std::time::Duration;
use tokio::sync::mpsc;

/// Agent scheduler that fires ticks on heartbeat and events
pub struct Scheduler {
    /// Heartbeat interval in seconds
    heartbeat_seconds: u64,
    /// Event triggers to listen for
    triggers: Vec<String>,
    /// Agent pubkey (for filtering mentions)
    agent_pubkey: String,
    /// Relay connection
    relay: RelayConnection,
}

impl Scheduler {
    /// Create a new scheduler
    pub fn new(
        heartbeat_seconds: u64,
        triggers: Vec<String>,
        agent_pubkey: String,
        relay: RelayConnection,
    ) -> Self {
        Self {
            heartbeat_seconds,
            triggers,
            agent_pubkey,
            relay,
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

        // Run until agent dies or interrupted
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

                    // Check if agent is dead
                    if matches!(result.lifecycle_state, LifecycleState::Dead) {
                        tracing::warn!("Agent is dead. Stopping scheduler.");
                        break;
                    }
                }
                Err(e) => {
                    tracing::error!("Tick execution failed: {}", e);
                    // Continue running - one failed tick shouldn't kill the agent
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

        if filter_kinds.is_empty() {
            // No triggers enabled - return a dummy channel
            let (_, rx) = mpsc::channel(1);
            return Ok(rx);
        }

        let filters = vec![serde_json::json!({
            "kinds": filter_kinds,
            "#p": [self.agent_pubkey]
        })];

        let rx = self
            .relay
            .subscribe_with_channel("agent-triggers", &filters)
            .await?;

        Ok(rx)
    }

    /// Classify an event into a trigger type
    fn classify_trigger(&self, event: &Event) -> TickTrigger {
        match event.kind {
            1 => TickTrigger::Mention(event.clone()),
            4 => TickTrigger::DirectMessage(event.clone()),
            9735 => TickTrigger::Zap(event.clone()),
            _ => TickTrigger::Heartbeat, // Fallback
        }
    }

    /// Log tick result summary
    fn log_tick_result(&self, result: &TickResult) {
        tracing::info!(
            "Tick #{} complete: state={:?}, cost={} sats, actions={}",
            result.tick_number,
            result.lifecycle_state,
            result.compute_cost_sats,
            result.actions.len()
        );

        if result.runway.days_remaining < 7.0 {
            tracing::warn!(
                "Low runway warning: {:.1} days remaining ({} sats)",
                result.runway.days_remaining,
                result.runway.balance_sats
            );
        }
    }
}
