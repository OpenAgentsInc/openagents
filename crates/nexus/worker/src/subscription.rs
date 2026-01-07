//! Subscription management and filter matching

use serde::{Deserialize, Serialize};
use worker::*;

use crate::relay_do::ConnectionMeta;

/// NIP-01 filter for querying events
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Filter {
    /// Event IDs
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ids: Vec<String>,

    /// Author pubkeys
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,

    /// Event kinds
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub kinds: Vec<u16>,

    /// #e tag values
    #[serde(default, rename = "#e", skip_serializing_if = "Vec::is_empty")]
    pub e_tags: Vec<String>,

    /// #p tag values
    #[serde(default, rename = "#p", skip_serializing_if = "Vec::is_empty")]
    pub p_tags: Vec<String>,

    /// #t tag values (hashtags)
    #[serde(default, rename = "#t", skip_serializing_if = "Vec::is_empty")]
    pub t_tags: Vec<String>,

    /// #d tag values (for replaceable events)
    #[serde(default, rename = "#d", skip_serializing_if = "Vec::is_empty")]
    pub d_tags: Vec<String>,

    /// Events created after this timestamp
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<u64>,

    /// Events created before this timestamp
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until: Option<u64>,

    /// Maximum number of events to return
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

impl Filter {
    /// Check if an event matches this filter
    pub fn matches(&self, event: &nostr::Event) -> bool {
        // Check IDs
        if !self.ids.is_empty() && !self.ids.iter().any(|id| event.id.starts_with(id)) {
            return false;
        }

        // Check authors
        if !self.authors.is_empty() && !self.authors.iter().any(|a| event.pubkey.starts_with(a)) {
            return false;
        }

        // Check kinds
        if !self.kinds.is_empty() && !self.kinds.contains(&event.kind) {
            return false;
        }

        // Check since
        if let Some(since) = self.since {
            if event.created_at < since {
                return false;
            }
        }

        // Check until
        if let Some(until) = self.until {
            if event.created_at > until {
                return false;
            }
        }

        // Check #e tags
        if !self.e_tags.is_empty() {
            let event_e_tags: Vec<&str> = event
                .tags
                .iter()
                .filter(|t| t.len() >= 2 && t[0] == "e")
                .map(|t| t[1].as_str())
                .collect();
            if !self.e_tags.iter().any(|e| event_e_tags.contains(&e.as_str())) {
                return false;
            }
        }

        // Check #p tags
        if !self.p_tags.is_empty() {
            let event_p_tags: Vec<&str> = event
                .tags
                .iter()
                .filter(|t| t.len() >= 2 && t[0] == "p")
                .map(|t| t[1].as_str())
                .collect();
            if !self.p_tags.iter().any(|p| event_p_tags.contains(&p.as_str())) {
                return false;
            }
        }

        // Check #t tags
        if !self.t_tags.is_empty() {
            let event_t_tags: Vec<&str> = event
                .tags
                .iter()
                .filter(|t| t.len() >= 2 && t[0] == "t")
                .map(|t| t[1].as_str())
                .collect();
            if !self.t_tags.iter().any(|t| event_t_tags.contains(&t.as_str())) {
                return false;
            }
        }

        // Check #d tags
        if !self.d_tags.is_empty() {
            let event_d_tags: Vec<&str> = event
                .tags
                .iter()
                .filter(|t| t.len() >= 2 && t[0] == "d")
                .map(|t| t[1].as_str())
                .collect();
            if !self.d_tags.iter().any(|d| event_d_tags.contains(&d.as_str())) {
                return false;
            }
        }

        true
    }

    /// Convert to SQL WHERE clause components
    pub fn to_sql_conditions(&self) -> (String, Vec<String>) {
        let mut conditions = Vec::new();
        let mut params = Vec::new();

        // IDs (prefix matching)
        if !self.ids.is_empty() {
            let placeholders: Vec<_> = self
                .ids
                .iter()
                .enumerate()
                .map(|(i, id)| {
                    params.push(format!("{}%", id));
                    format!("id LIKE ?{}", params.len())
                })
                .collect();
            conditions.push(format!("({})", placeholders.join(" OR ")));
        }

        // Authors (prefix matching)
        if !self.authors.is_empty() {
            let placeholders: Vec<_> = self
                .authors
                .iter()
                .map(|author| {
                    params.push(format!("{}%", author));
                    format!("pubkey LIKE ?{}", params.len())
                })
                .collect();
            conditions.push(format!("({})", placeholders.join(" OR ")));
        }

        // Kinds
        if !self.kinds.is_empty() {
            let placeholders: Vec<_> = self
                .kinds
                .iter()
                .map(|kind| {
                    params.push(kind.to_string());
                    format!("kind = ?{}", params.len())
                })
                .collect();
            conditions.push(format!("({})", placeholders.join(" OR ")));
        }

        // Since
        if let Some(since) = self.since {
            params.push(since.to_string());
            conditions.push(format!("created_at >= ?{}", params.len()));
        }

        // Until
        if let Some(until) = self.until {
            params.push(until.to_string());
            conditions.push(format!("created_at <= ?{}", params.len()));
        }

        let where_clause = if conditions.is_empty() {
            "1=1".to_string()
        } else {
            conditions.join(" AND ")
        };

        (where_clause, params)
    }
}

/// Subscription manager for tracking active subscriptions
pub struct SubscriptionManager<'a> {
    state: &'a State,
}

impl<'a> SubscriptionManager<'a> {
    pub fn new(state: &'a State) -> Self {
        Self { state }
    }

    /// Get subscription IDs that match an event for a given WebSocket
    pub async fn get_matching_subscriptions(
        &self,
        ws: &WebSocket,
        event: &nostr::Event,
    ) -> Result<Vec<String>> {
        // Use stable conn_id from attachment (not pointer address which changes)
        let meta: ConnectionMeta = ws
            .deserialize_attachment()?
            .unwrap_or_else(|| ConnectionMeta {
                conn_id: String::new(),
                pubkey: None,
                challenge: String::new(),
                authenticated: false,
            });
        let prefix = format!("sub:{}:", meta.conn_id);

        // List all subscriptions for this WebSocket
        let list_result = self.state.storage().list().await?;
        let mut matching = Vec::new();

        for key in list_result.keys() {
            let key_str: String = key?.as_string().unwrap_or_default();
            if key_str.starts_with(&prefix) {
                let sub_id = key_str.strip_prefix(&prefix).unwrap_or("").to_string();
                if let Ok(Some(filters)) = self.state.storage().get::<Vec<Filter>>(&key_str).await {
                    if filters.iter().any(|f| f.matches(event)) {
                        matching.push(sub_id);
                    }
                }
            }
        }

        Ok(matching)
    }
}
