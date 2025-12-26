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
use nostr::{AgentSchedule, Event, TriggerType, KIND_AGENT_SCHEDULE, npub_to_public_key};
use nostr::nip_sa::{BusinessHours, BusinessTime, Weekday};
use nostr_client::{PoolConfig, RelayPool};
use serde_json::json;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;
use tokio::time::{sleep, timeout};

/// Nostr event kinds for triggers
const KIND_TEXT_NOTE: u16 = 1;
const KIND_ENCRYPTED_DM: u16 = 4;
const KIND_ZAP_RECEIPT: u16 = 9735;

/// Trigger event that signals the agent should run
#[derive(Debug, Clone)]
pub enum TriggerEvent {
    /// Heartbeat timer expired
    Heartbeat,
    /// Agent was mentioned in a note
    Mention { event_id: String, author: String },
    /// Agent received a DM
    DirectMessage { event_id: String, author: String },
    /// Agent received a zap
    Zap { event_id: String, amount_msats: u64 },
}

/// Nostr trigger watcher that monitors relays for agent activation events
pub struct NostrTrigger {
    /// Agent's public key (hex format, 64 chars)
    agent_pubkey: String,
    /// Relay URLs to monitor
    relay_urls: Vec<String>,
    /// Current schedule configuration
    schedule: Option<AgentSchedule>,
    /// Last heartbeat time
    last_heartbeat: Option<Instant>,
    /// Enabled trigger types
    enabled_triggers: HashSet<TriggerType>,
    /// Relay pool for subscriptions
    relay_pool: Option<Arc<RelayPool>>,
    /// Last event check timestamp (unix seconds)
    last_check_time: u64,
}

impl NostrTrigger {
    /// Create a new Nostr trigger watcher
    ///
    /// # Arguments
    /// * `agent_pubkey` - Agent's public key in hex format (64 chars) or npub format
    /// * `relay_urls` - List of relay WebSocket URLs to monitor
    pub fn new(agent_pubkey: String, relay_urls: Vec<String>) -> Self {
        // Convert npub to hex if needed
        let pubkey_hex = if agent_pubkey.starts_with("npub1") {
            // Decode bech32 npub to hex
            match npub_to_public_key(&agent_pubkey) {
                Ok(bytes) => hex::encode(bytes),
                Err(_) => agent_pubkey.clone(),
            }
        } else {
            agent_pubkey.clone()
        };

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            agent_pubkey: pubkey_hex,
            relay_urls,
            schedule: None,
            last_heartbeat: None,
            enabled_triggers: HashSet::new(),
            relay_pool: None,
            last_check_time: now,
        }
    }

    /// Initialize the relay pool and connect to relays
    async fn init_relay_pool(&mut self) -> Result<()> {
        if self.relay_pool.is_some() {
            return Ok(());
        }

        let mut config = PoolConfig::default();
        config.max_relays = self.relay_urls.len().max(10);
        config.connection_timeout = Duration::from_secs(10);
        config.auto_reconnect = true;

        let pool = RelayPool::new(config);

        for url in &self.relay_urls {
            if let Err(e) = pool.add_relay(url).await {
                eprintln!("NostrTrigger: Failed to add relay {}: {}", url, e);
            }
        }

        pool.connect_all()
            .await
            .context("Failed to connect to relays")?;

        self.relay_pool = Some(Arc::new(pool));
        Ok(())
    }

    /// Fetch and update agent schedule from relays
    pub async fn update_schedule(&mut self) -> Result<()> {
        let schedule = match self.fetch_schedule_from_relays().await {
            Ok(Some(schedule)) => schedule,
            Ok(None) => Self::default_schedule(),
            Err(err) => {
                eprintln!("NostrTrigger: Failed to fetch schedule: {}", err);
                Self::default_schedule()
            }
        };

        self.enabled_triggers = schedule.triggers.iter().cloned().collect();
        self.schedule = Some(schedule);

        Ok(())
    }

    /// Start watching for triggers and send to channel
    pub async fn watch(mut self, tx: mpsc::UnboundedSender<TriggerEvent>) -> Result<()> {
        // Initialize relay pool
        self.init_relay_pool().await?;

        // Update schedule on startup
        self.update_schedule().await?;

        eprintln!("NostrTrigger: Watching for agent activation events");
        eprintln!("  Agent pubkey: {}", self.agent_pubkey);
        eprintln!("  Relays: {:?}", self.relay_urls);
        eprintln!("  Schedule: {:?}", self.schedule);

        // Start event subscription in background
        let event_rx = self.start_subscription().await?;
        let tx_clone = tx.clone();
        let agent_pubkey = self.agent_pubkey.clone();
        let enabled_triggers = self.enabled_triggers.clone();

        // Spawn event handler task
        tokio::spawn(async move {
            Self::handle_events(event_rx, tx_clone, agent_pubkey, enabled_triggers).await;
        });

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

            // Sleep before next heartbeat check
            sleep(Duration::from_secs(10)).await;

            // Periodically refresh schedule (every 5 minutes)
            if self
                .last_heartbeat
                .as_ref()
                .map(|t| t.elapsed().as_secs() > 300)
                .unwrap_or(false)
            {
                if let Err(e) = self.update_schedule().await {
                    eprintln!("NostrTrigger: Error updating schedule: {}", e);
                }
            }
        }
    }

    /// Start subscription to relay events
    async fn start_subscription(&self) -> Result<mpsc::Receiver<Event>> {
        let pool = self
            .relay_pool
            .as_ref()
            .context("Relay pool not initialized")?;

        // Build filters based on enabled triggers
        let mut filters = Vec::new();

        // Filter for mentions (kind:1 notes that tag our pubkey)
        if self.enabled_triggers.contains(&TriggerType::Mention) {
            filters.push(json!({
                "kinds": [KIND_TEXT_NOTE],
                "#p": [&self.agent_pubkey],
                "since": self.last_check_time
            }));
        }

        // Filter for DMs (kind:4 encrypted messages to our pubkey)
        if self.enabled_triggers.contains(&TriggerType::Dm) {
            filters.push(json!({
                "kinds": [KIND_ENCRYPTED_DM],
                "#p": [&self.agent_pubkey],
                "since": self.last_check_time
            }));
        }

        // Filter for zaps (kind:9735 zap receipts tagging our pubkey)
        if self.enabled_triggers.contains(&TriggerType::Zap) {
            filters.push(json!({
                "kinds": [KIND_ZAP_RECEIPT],
                "#p": [&self.agent_pubkey],
                "since": self.last_check_time
            }));
        }

        if filters.is_empty() {
            // No triggers enabled, create a dummy filter that won't match anything
            filters.push(json!({"kinds": [999999], "limit": 0}));
        }

        let subscription_id = format!("autopilot-{}", &self.agent_pubkey[..8]);
        let rx = pool
            .subscribe(&subscription_id, &filters)
            .await
            .context("Failed to subscribe to relays")?;

        eprintln!("NostrTrigger: Subscribed with {} filters", filters.len());

        Ok(rx)
    }

    fn default_schedule() -> AgentSchedule {
        AgentSchedule {
            heartbeat_seconds: Some(900),
            triggers: vec![TriggerType::Mention, TriggerType::Dm, TriggerType::Zap],
            active: true,
            business_hours: None,
        }
    }

    async fn fetch_schedule_from_relays(&self) -> Result<Option<AgentSchedule>> {
        let pool = match self.relay_pool.as_ref() {
            Some(pool) => pool,
            None => return Ok(None),
        };

        let subscription_id = format!("agent-schedule-{}", uuid::Uuid::new_v4());
        let filter = json!({
            "kinds": [KIND_AGENT_SCHEDULE],
            "authors": [self.agent_pubkey],
            "limit": 1
        });

        let mut rx = pool.subscribe(&subscription_id, &[filter]).await?;
        let event = match timeout(Duration::from_secs(3), rx.recv()).await {
            Ok(Some(event)) => Some(event),
            _ => None,
        };

        if let Err(err) = pool.unsubscribe(&subscription_id).await {
            eprintln!(
                "NostrTrigger: Failed to unsubscribe from schedule fetch: {}",
                err
            );
        }

        Ok(event.and_then(|event| Self::schedule_from_event(&event)))
    }

    fn schedule_from_event(event: &Event) -> Option<AgentSchedule> {
        if !event.content.trim().is_empty() {
            if let Ok(schedule) = serde_json::from_str::<AgentSchedule>(&event.content) {
                return Some(schedule);
            }
        }

        Self::schedule_from_tags(&event.tags)
    }

    fn schedule_from_tags(tags: &[Vec<String>]) -> Option<AgentSchedule> {
        let mut schedule = AgentSchedule::new();
        let mut found = false;

        for tag in tags {
            let tag_name = match tag.get(0) {
                Some(name) => name.as_str(),
                None => continue,
            };

            match tag_name {
                "heartbeat" => {
                    if let Some(value) = tag.get(1) {
                        if let Ok(seconds) = value.parse::<u64>() {
                            if seconds > 0 {
                                schedule.heartbeat_seconds = Some(seconds);
                                found = true;
                            }
                        }
                    }
                }
                "trigger" => {
                    if let Some(value) = tag.get(1) {
                        if let Some(trigger) = TriggerType::from_tag_value(value) {
                            schedule = schedule.add_trigger(trigger);
                            found = true;
                        }
                    }
                }
                "active" => {
                    if let Some(value) = tag.get(1) {
                        let active = matches!(
                            value.as_str(),
                            "true" | "1" | "yes" | "enabled"
                        );
                        schedule = schedule.with_active(active);
                        found = true;
                    }
                }
                "business_hours" => {
                    if let Some(value) = tag.get(1) {
                        if let Some(hours) = Self::parse_business_hours(value) {
                            schedule = schedule.with_business_hours(hours);
                            found = true;
                        }
                    }
                }
                _ => {}
            }
        }

        if found { Some(schedule) } else { None }
    }

    fn parse_business_hours(value: &str) -> Option<BusinessHours> {
        let (days_part, hours_part) = value.split_once(' ')?;
        let (start_str, end_str) = hours_part.split_once('-')?;

        let days = days_part
            .split(',')
            .filter_map(Self::parse_weekday)
            .collect::<Vec<_>>();
        if days.is_empty() {
            return None;
        }

        let start = Self::parse_time(start_str)?;
        let end = Self::parse_time(end_str)?;

        BusinessHours::new(days, start, end).ok()
    }

    fn parse_weekday(value: &str) -> Option<Weekday> {
        match value {
            "mon" => Some(Weekday::Mon),
            "tue" => Some(Weekday::Tue),
            "wed" => Some(Weekday::Wed),
            "thu" => Some(Weekday::Thu),
            "fri" => Some(Weekday::Fri),
            "sat" => Some(Weekday::Sat),
            "sun" => Some(Weekday::Sun),
            _ => None,
        }
    }

    fn parse_time(value: &str) -> Option<BusinessTime> {
        let (hour_str, minute_str) = value.split_once(':')?;
        let hour = hour_str.parse::<u8>().ok()?;
        let minute = minute_str.parse::<u8>().ok()?;
        BusinessTime::new(hour, minute).ok()
    }

    /// Handle incoming events from subscription
    async fn handle_events(
        mut rx: mpsc::Receiver<Event>,
        tx: mpsc::UnboundedSender<TriggerEvent>,
        agent_pubkey: String,
        enabled_triggers: HashSet<TriggerType>,
    ) {
        while let Some(event) = rx.recv().await {
            let kind = event.kind;

            // Match event kind to trigger type
            let trigger = match kind {
                k if k == KIND_TEXT_NOTE && enabled_triggers.contains(&TriggerType::Mention) => {
                    // Check if we're actually mentioned (in tags or content)
                    let mentioned_in_tags = event
                        .tags
                        .iter()
                        .any(|tag| tag.len() >= 2 && tag[0] == "p" && tag[1] == agent_pubkey);
                    let mentioned_in_content = event.content.contains(&agent_pubkey);

                    if mentioned_in_tags || mentioned_in_content {
                        Some(TriggerEvent::Mention {
                            event_id: event.id.clone(),
                            author: event.pubkey.clone(),
                        })
                    } else {
                        None
                    }
                }
                k if k == KIND_ENCRYPTED_DM && enabled_triggers.contains(&TriggerType::Dm) => {
                    Some(TriggerEvent::DirectMessage {
                        event_id: event.id.clone(),
                        author: event.pubkey.clone(),
                    })
                }
                k if k == KIND_ZAP_RECEIPT && enabled_triggers.contains(&TriggerType::Zap) => {
                    let amount_msats = extract_zap_amount(&event).unwrap_or(0);
                    Some(TriggerEvent::Zap {
                        event_id: event.id.clone(),
                        amount_msats,
                    })
                }
                _ => None,
            };

            if let Some(trigger) = trigger {
                eprintln!("NostrTrigger: Event received - {:?}", trigger);
                if tx.send(trigger).is_err() {
                    eprintln!("NostrTrigger: Trigger channel closed, stopping event handler");
                    break;
                }
            }
        }
    }
}

/// Extract zap amount from a zap receipt event
#[allow(dead_code)]
fn extract_zap_amount(event: &Event) -> Option<u64> {
    // Look for "bolt11" tag
    for tag in event.tags.iter() {
        let tag: &Vec<String> = tag;
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

    #[test]
    fn test_schedule_from_tags() {
        let tags = vec![
            vec!["d".to_string(), "schedule".to_string()],
            vec!["heartbeat".to_string(), "120".to_string()],
            vec!["trigger".to_string(), "mention".to_string()],
            vec!["trigger".to_string(), "dm".to_string()],
            vec!["active".to_string(), "false".to_string()],
            vec![
                "business_hours".to_string(),
                "mon,tue 09:00-17:00".to_string(),
            ],
        ];

        let schedule = NostrTrigger::schedule_from_tags(&tags).expect("schedule");
        assert_eq!(schedule.heartbeat_seconds, Some(120));
        assert_eq!(schedule.triggers.len(), 2);
        assert!(!schedule.active);

        let hours = schedule.business_hours.expect("hours");
        assert_eq!(hours.days.len(), 2);
        assert_eq!(hours.start.hour, 9);
        assert_eq!(hours.end.hour, 17);
    }

    #[test]
    fn test_schedule_from_event_content() {
        let schedule = AgentSchedule::new()
            .with_heartbeat(600)
            .unwrap()
            .add_trigger(TriggerType::Zap);
        let content = serde_json::to_string(&schedule).expect("serialize schedule");

        let event = Event {
            id: "event-id".to_string(),
            pubkey: "pubkey".to_string(),
            created_at: 0,
            kind: KIND_AGENT_SCHEDULE,
            tags: vec![vec!["heartbeat".to_string(), "120".to_string()]],
            content,
            sig: "sig".to_string(),
        };

        let parsed = NostrTrigger::schedule_from_event(&event).expect("parse schedule");
        assert_eq!(parsed.heartbeat_seconds, Some(600));
        assert_eq!(parsed.triggers.len(), 1);
    }
}
