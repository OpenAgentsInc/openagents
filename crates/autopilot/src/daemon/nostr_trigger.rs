//! Nostr event trigger watcher for autonomous agent activation
//!
//! This module monitors Nostr relays for events that should trigger autopilot runs:
//! - Mentions of the agent's pubkey
//! - Direct messages (NIP-04) to the agent
//! - Zaps (NIP-57) received by the agent
//! - Heartbeat timer based on AgentSchedule
//!
//! When a trigger fires, it signals the daemon supervisor to spawn a worker.

use anyhow::{Context, Result};
use nostr_core::nip_sa::schedule::{AgentSchedule, TriggerType, KIND_AGENT_SCHEDULE};
use nostr_core::{Event, Filter, Kind};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::sleep;

/// Trigger event that signals the agent should run
#[derive(Debug, Clone)]
pub enum TriggerEvent {
    /// Heartbeat timer expired
    Heartbeat,
    /// Agent was mentioned in a note
    Mention {
        event_id: String,
        author: String,
    },
    /// Agent received a DM
    DirectMessage {
        event_id: String,
        author: String,
    },
    /// Agent received a zap
    Zap {
        event_id: String,
        amount_msats: u64,
    },
}

/// Nostr trigger watcher that monitors relays for agent activation events
pub struct NostrTrigger {
    /// Agent's public key (npub format)
    agent_pubkey: String,
    /// Relay URLs to monitor
    relay_urls: Vec<String>,
    /// Current schedule configuration
    schedule: Option<AgentSchedule>,
    /// Last heartbeat time
    last_heartbeat: Option<Instant>,
    /// Enabled trigger types
    enabled_triggers: HashSet<TriggerType>,
}

impl NostrTrigger {
    /// Create a new Nostr trigger watcher
    pub fn new(agent_pubkey: String, relay_urls: Vec<String>) -> Self {
        Self {
            agent_pubkey,
            relay_urls,
            schedule: None,
            last_heartbeat: None,
            enabled_triggers: HashSet::new(),
        }
    }

    /// Fetch and update agent schedule from relays
    pub async fn update_schedule(&mut self) -> Result<()> {
        // TODO: Fetch kind:38002 event for this agent from relays
        // For now, use default schedule
        let default_schedule = AgentSchedule {
            heartbeat_seconds: Some(900), // 15 minutes
            triggers: vec![
                TriggerType::Mention,
                TriggerType::Dm,
                TriggerType::Zap,
            ],
        };

        self.enabled_triggers = default_schedule.triggers.iter().cloned().collect();
        self.schedule = Some(default_schedule);

        Ok(())
    }

    /// Start watching for triggers and send to channel
    pub async fn watch(
        mut self,
        tx: mpsc::UnboundedSender<TriggerEvent>,
    ) -> Result<()> {
        // Update schedule on startup
        self.update_schedule().await?;

        eprintln!("NostrTrigger: Watching for agent activation events");
        eprintln!("  Agent pubkey: {}", self.agent_pubkey);
        eprintln!("  Relays: {:?}", self.relay_urls);
        eprintln!("  Schedule: {:?}", self.schedule);

        loop {
            // Check heartbeat
            if let Some(schedule) = &self.schedule {
                if let Some(heartbeat_secs) = schedule.heartbeat_seconds {
                    let should_fire = match self.last_heartbeat {
                        Some(last) => last.elapsed() >= Duration::from_secs(heartbeat_secs),
                        None => true, // First heartbeat
                    };

                    if should_fire {
                        eprintln!("NostrTrigger: Heartbeat fired");
                        tx.send(TriggerEvent::Heartbeat)
                            .context("Failed to send heartbeat trigger")?;
                        self.last_heartbeat = Some(Instant::now());
                    }
                }
            }

            // Check for Nostr events
            if let Err(e) = self.check_nostr_events(&tx).await {
                eprintln!("NostrTrigger: Error checking events: {}", e);
            }

            // Sleep before next check (poll every 10 seconds)
            sleep(Duration::from_secs(10)).await;

            // Periodically refresh schedule (every 5 minutes)
            if self.last_heartbeat.as_ref().map(|t| t.elapsed().as_secs() > 300).unwrap_or(false) {
                if let Err(e) = self.update_schedule().await {
                    eprintln!("NostrTrigger: Error updating schedule: {}", e);
                }
            }
        }
    }

    /// Check for new Nostr events that match triggers
    async fn check_nostr_events(&self, tx: &mpsc::UnboundedSender<TriggerEvent>) -> Result<()> {
        // TODO: Connect to relays and subscribe to filters
        // For now, this is a stub that will be implemented when we have relay client

        // Filter for mentions (kind:1 with agent pubkey in tags or content)
        if self.enabled_triggers.contains(&TriggerType::Mention) {
            // let mention_filter = Filter::new()
            //     .kind(Kind::Text)
            //     .pubkey(&self.agent_pubkey)
            //     .since(last_check_time);
        }

        // Filter for DMs (kind:4 to agent)
        if self.enabled_triggers.contains(&TriggerType::Dm) {
            // let dm_filter = Filter::new()
            //     .kind(Kind::EncryptedDirectMessage)
            //     .pubkey(&self.agent_pubkey)
            //     .since(last_check_time);
        }

        // Filter for zaps (kind:9735 zap receipts with agent pubkey)
        if self.enabled_triggers.contains(&TriggerType::Zap) {
            // let zap_filter = Filter::new()
            //     .kind(Kind::Zap)
            //     .pubkey(&self.agent_pubkey)
            //     .since(last_check_time);
        }

        Ok(())
    }

    /// Parse a mention event and send trigger
    fn handle_mention(&self, event: &Event, tx: &mpsc::UnboundedSender<TriggerEvent>) -> Result<()> {
        eprintln!("NostrTrigger: Mention detected from {}", event.pubkey);
        tx.send(TriggerEvent::Mention {
            event_id: event.id.clone(),
            author: event.pubkey.clone(),
        })?;
        Ok(())
    }

    /// Parse a DM event and send trigger
    fn handle_dm(&self, event: &Event, tx: &mpsc::UnboundedSender<TriggerEvent>) -> Result<()> {
        eprintln!("NostrTrigger: DM received from {}", event.pubkey);
        tx.send(TriggerEvent::DirectMessage {
            event_id: event.id.clone(),
            author: event.pubkey.clone(),
        })?;
        Ok(())
    }

    /// Parse a zap event and send trigger
    fn handle_zap(&self, event: &Event, tx: &mpsc::UnboundedSender<TriggerEvent>) -> Result<()> {
        // Extract amount from zap receipt (bolt11 invoice)
        let amount_msats = extract_zap_amount(event).unwrap_or(0);

        eprintln!("NostrTrigger: Zap received: {} msats", amount_msats);
        tx.send(TriggerEvent::Zap {
            event_id: event.id.clone(),
            amount_msats,
        })?;
        Ok(())
    }
}

/// Extract zap amount from a zap receipt event
fn extract_zap_amount(event: &Event) -> Option<u64> {
    // Look for "bolt11" tag
    for tag in &event.tags {
        if tag.len() >= 2 && tag[0] == "bolt11" {
            // Parse bolt11 invoice to extract amount
            // For now, return None - will implement when we have lightning parser
            return None;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trigger_type_serialization() {
        assert_eq!(TriggerType::Mention.to_tag_value(), "mention");
        assert_eq!(TriggerType::Dm.to_tag_value(), "dm");
        assert_eq!(TriggerType::Zap.to_tag_value(), "zap");
        assert_eq!(TriggerType::Custom(42).to_tag_value(), "custom:42");

        assert_eq!(
            TriggerType::from_tag_value("mention"),
            Some(TriggerType::Mention)
        );
        assert_eq!(
            TriggerType::from_tag_value("custom:42"),
            Some(TriggerType::Custom(42))
        );
        assert_eq!(TriggerType::from_tag_value("invalid"), None);
    }

    #[tokio::test]
    async fn test_nostr_trigger_creation() {
        let trigger = NostrTrigger::new(
            "npub1test".to_string(),
            vec!["wss://relay.damus.io".to_string()],
        );

        assert_eq!(trigger.agent_pubkey, "npub1test");
        assert_eq!(trigger.relay_urls.len(), 1);
        assert!(trigger.schedule.is_none());
    }

    #[tokio::test]
    async fn test_default_schedule() {
        let mut trigger = NostrTrigger::new(
            "npub1test".to_string(),
            vec!["wss://relay.damus.io".to_string()],
        );

        trigger.update_schedule().await.unwrap();

        assert!(trigger.schedule.is_some());
        let schedule = trigger.schedule.unwrap();
        assert_eq!(schedule.heartbeat_seconds, Some(900));
        assert_eq!(schedule.triggers.len(), 3);
    }
}
