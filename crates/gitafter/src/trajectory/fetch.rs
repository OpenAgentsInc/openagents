//! Fetch trajectory events from Nostr relays
//!
//! Retrieves TrajectorySession (kind:38030) and TrajectoryEvent (kind:38031) from relays.

use anyhow::{Context, Result};
use nostr::{
    Event, KIND_TRAJECTORY_EVENT, KIND_TRAJECTORY_SESSION, StepType, TrajectoryEventContent,
    TrajectorySessionContent,
};
use nostr_client::{RelayConnection, RelayMessage};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use tokio::time::timeout;

/// Fetched trajectory data from relays
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchedTrajectory {
    /// Session metadata
    pub session: TrajectorySessionData,
    /// All events in sequence order
    pub events: Vec<TrajectoryEventData>,
    /// Relay URLs that provided this data
    pub sources: Vec<String>,
}

/// Trajectory session data from kind:38030 event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrajectorySessionData {
    pub session_id: String,
    pub started_at: u64,
    pub ended_at: Option<u64>,
    pub model: String,
    pub total_events: u32,
    pub trajectory_hash: Option<String>,
    pub tick_id: String,
}

/// Trajectory event data from kind:38031 event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrajectoryEventData {
    pub session_id: String,
    pub tick_id: String,
    pub sequence: u32,
    pub step_type: String,
    pub data: HashMap<String, serde_json::Value>,
    pub timestamp: u64,
}

const FETCH_TIMEOUT_SECS: u64 = 6;
const MESSAGE_TIMEOUT_SECS: u64 = 1;
const FETCH_LIMIT: u32 = 2000;

/// Fetch trajectory events from relays
///
/// # Arguments
/// * `session_id` - Trajectory session identifier
/// * `relay_urls` - List of relay URLs to query
///
/// # Returns
/// Complete trajectory with all events in sequence order
pub async fn fetch_trajectory(
    session_id: &str,
    relay_urls: &[String],
) -> Result<FetchedTrajectory> {
    let (session_event, events, sources) =
        fetch_events_for_session(session_id, relay_urls, true).await?;

    let session_event = session_event.ok_or_else(|| {
        anyhow::anyhow!("No trajectory session found for session '{}'", session_id)
    })?;
    let session = parse_session_event(&session_event)?;

    let mut parsed_events = Vec::new();
    for event in events {
        if event.kind != KIND_TRAJECTORY_EVENT {
            continue;
        }
        let parsed = parse_trajectory_event(&event)?;
        if parsed.session_id != session.session_id {
            continue;
        }
        parsed_events.push(parsed);
    }

    parsed_events.sort_by_key(|event| event.sequence);

    if session.total_events > 0 && parsed_events.len() < session.total_events as usize {
        anyhow::bail!(
            "Trajectory incomplete: expected {} events, fetched {}",
            session.total_events,
            parsed_events.len()
        );
    }

    Ok(FetchedTrajectory {
        session,
        events: parsed_events,
        sources,
    })
}

/// Fetch trajectory session metadata only
pub async fn fetch_trajectory_session(
    session_id: &str,
    relay_urls: &[String],
) -> Result<TrajectorySessionData> {
    let (session_event, _events, _sources) =
        fetch_events_for_session(session_id, relay_urls, false).await?;
    let session_event = session_event.ok_or_else(|| {
        anyhow::anyhow!("No trajectory session found for session '{}'", session_id)
    })?;
    parse_session_event(&session_event)
}

/// Check if trajectory exists on relays
pub async fn trajectory_exists(session_id: &str, relay_urls: &[String]) -> Result<bool> {
    match fetch_trajectory_session(session_id, relay_urls).await {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

async fn fetch_events_for_session(
    session_id: &str,
    relay_urls: &[String],
    include_events: bool,
) -> Result<(Option<Event>, Vec<Event>, Vec<String>)> {
    if relay_urls.is_empty() {
        anyhow::bail!("At least one relay URL is required.");
    }

    let mut session_candidates = Vec::new();
    let mut events = Vec::new();
    let mut sources = HashSet::new();
    let mut seen_sessions = HashSet::new();
    let mut seen_events = HashSet::new();
    let mut last_error: Option<anyhow::Error> = None;

    let mut filters = vec![json!({
        "kinds": [KIND_TRAJECTORY_SESSION],
        "#d": [session_id],
        "limit": 1
    })];
    if include_events {
        filters.push(json!({
            "kinds": [KIND_TRAJECTORY_EVENT],
            "#session": [session_id],
            "limit": FETCH_LIMIT
        }));
    }

    for url in relay_urls {
        match fetch_from_relay(url, &filters).await {
            Ok(fetched) => {
                if !fetched.is_empty() {
                    sources.insert(url.to_string());
                }
                for event in fetched {
                    if event.kind == KIND_TRAJECTORY_SESSION {
                        if seen_sessions.insert(event.id.clone()) {
                            session_candidates.push(event);
                        }
                    } else if include_events && event.kind == KIND_TRAJECTORY_EVENT {
                        if seen_events.insert(event.id.clone()) {
                            events.push(event);
                        }
                    }
                }
            }
            Err(err) => {
                last_error = Some(err);
            }
        }
    }

    let session_event = session_candidates
        .into_iter()
        .max_by_key(|event| event.created_at);

    if session_event.is_none() && last_error.is_some() {
        return Err(last_error.unwrap());
    }

    let mut sources = sources.into_iter().collect::<Vec<_>>();
    sources.sort();
    Ok((session_event, events, sources))
}

async fn fetch_from_relay(url: &str, filters: &[serde_json::Value]) -> Result<Vec<Event>> {
    let relay = RelayConnection::new(url)
        .with_context(|| format!("Failed to create relay connection to {}", url))?;
    relay
        .connect()
        .await
        .with_context(|| format!("Failed to connect to {}", url))?;

    relay
        .subscribe("trajectory-fetch", filters)
        .await
        .with_context(|| format!("Failed to subscribe on {}", url))?;

    let mut events = Vec::new();
    let timeout_duration = Duration::from_secs(FETCH_TIMEOUT_SECS);
    let start = Instant::now();

    while start.elapsed() < timeout_duration {
        match timeout(Duration::from_secs(MESSAGE_TIMEOUT_SECS), relay.recv()).await {
            Ok(Ok(Some(message))) => match message {
                RelayMessage::Event(_, event) => events.push(event),
                RelayMessage::Eose(_) => break,
                _ => {}
            },
            Ok(Ok(None)) => break,
            Ok(Err(_)) => break,
            Err(_) => continue,
        }
    }

    if let Err(err) = relay.disconnect().await {
        tracing::debug!("Failed to disconnect from relay after fetch: {}", err);
    }

    Ok(events)
}

fn parse_session_event(event: &Event) -> Result<TrajectorySessionData> {
    if event.kind != KIND_TRAJECTORY_SESSION {
        anyhow::bail!("Expected trajectory session event, got kind {}", event.kind);
    }

    let content = TrajectorySessionContent::from_json(&event.content)
        .context("Failed to parse trajectory session content")?;
    let session_id = if !content.session_id.is_empty() {
        content.session_id.clone()
    } else {
        tag_value(&event.tags, "d")
            .ok_or_else(|| anyhow::anyhow!("Missing session id tag"))?
            .to_string()
    };
    let tick_id = tag_value(&event.tags, "tick")
        .ok_or_else(|| anyhow::anyhow!("Missing tick tag"))?
        .to_string();

    Ok(TrajectorySessionData {
        session_id,
        started_at: content.started_at,
        ended_at: content.ended_at,
        model: content.model,
        total_events: content.total_events,
        trajectory_hash: content.trajectory_hash,
        tick_id,
    })
}

fn parse_trajectory_event(event: &Event) -> Result<TrajectoryEventData> {
    if event.kind != KIND_TRAJECTORY_EVENT {
        anyhow::bail!("Expected trajectory event, got kind {}", event.kind);
    }

    let content = TrajectoryEventContent::from_json(&event.content)
        .context("Failed to parse trajectory event content")?;
    let session_id = tag_value(&event.tags, "session")
        .ok_or_else(|| anyhow::anyhow!("Missing session tag"))?
        .to_string();
    let tick_id = tag_value(&event.tags, "tick")
        .ok_or_else(|| anyhow::anyhow!("Missing tick tag"))?
        .to_string();
    let sequence = tag_value(&event.tags, "seq")
        .ok_or_else(|| anyhow::anyhow!("Missing seq tag"))?
        .parse::<u32>()
        .context("Invalid seq tag")?;

    let step_type = step_type_label(&content.step_type).to_string();
    let data = content.data.into_iter().collect::<HashMap<_, _>>();

    Ok(TrajectoryEventData {
        session_id,
        tick_id,
        sequence,
        step_type,
        data,
        timestamp: event.created_at,
    })
}

fn step_type_label(step_type: &StepType) -> &'static str {
    match step_type {
        StepType::ToolUse => "ToolUse",
        StepType::ToolResult => "ToolResult",
        StepType::Message => "Message",
        StepType::Thinking => "Thinking",
    }
}

fn tag_value<'a>(tags: &'a [Vec<String>], key: &str) -> Option<&'a str> {
    tags.iter().find_map(|tag| {
        if tag.get(0).map(String::as_str) == Some(key) {
            tag.get(1).map(String::as_str)
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_session_event() -> Event {
        let content = TrajectorySessionContent::new("session-123", 1_700_000_000, "sonnet")
            .with_total_events(2)
            .with_hash("hash-123")
            .to_json()
            .unwrap();

        Event {
            id: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            pubkey: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
            created_at: 1_700_000_000,
            kind: KIND_TRAJECTORY_SESSION,
            tags: vec![
                vec!["d".to_string(), "session-123".to_string()],
                vec!["tick".to_string(), "tick-1".to_string()],
            ],
            content,
            sig: "sig".to_string(),
        }
    }

    fn sample_event() -> Event {
        let content = TrajectoryEventContent::new(StepType::ToolUse)
            .with_data("tool", json!("rg"))
            .with_data("input", json!({"pattern": "foo"}))
            .to_json()
            .unwrap();

        Event {
            id: "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".to_string(),
            pubkey: "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd".to_string(),
            created_at: 1_700_000_100,
            kind: KIND_TRAJECTORY_EVENT,
            tags: vec![
                vec!["session".to_string(), "session-123".to_string()],
                vec!["tick".to_string(), "tick-1".to_string()],
                vec!["seq".to_string(), "3".to_string()],
            ],
            content,
            sig: "sig".to_string(),
        }
    }

    #[test]
    fn test_parse_session_event() {
        let event = sample_session_event();
        let parsed = parse_session_event(&event).unwrap();
        assert_eq!(parsed.session_id, "session-123");
        assert_eq!(parsed.tick_id, "tick-1");
        assert_eq!(parsed.total_events, 2);
        assert_eq!(parsed.trajectory_hash, Some("hash-123".to_string()));
    }

    #[test]
    fn test_parse_trajectory_event() {
        let event = sample_event();
        let parsed = parse_trajectory_event(&event).unwrap();
        assert_eq!(parsed.session_id, "session-123");
        assert_eq!(parsed.tick_id, "tick-1");
        assert_eq!(parsed.sequence, 3);
        assert_eq!(parsed.step_type, "ToolUse");
        assert_eq!(
            parsed.data.get("tool").and_then(|value| value.as_str()),
            Some("rg")
        );
    }

    #[tokio::test]
    async fn test_fetch_trajectory_requires_relays() {
        let result = fetch_trajectory("session-123", &[]).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_fetch_trajectory_session_requires_relays() {
        let result = fetch_trajectory_session("session-123", &[]).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_trajectory_exists_empty_relays() {
        let exists = trajectory_exists("session-123", &[]).await.unwrap();
        assert!(!exists);
    }
}
