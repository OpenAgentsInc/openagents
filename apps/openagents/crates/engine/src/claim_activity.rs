//! Claim planning from working-chat activity, for any coordinating agent.

use std::collections::BTreeMap;

use openagents_proto::{Chat, Session, SessionStatus};

#[derive(Clone, Debug)]
pub(crate) struct Claim {
    pub number: u64,
    pub holder: String,
    pub claimed_at: i64,
    pub last_progress_at: i64,
}

#[derive(Clone, Debug)]
struct Activity {
    chat: String,
    harness_session: Option<String>,
    last_output: i64,
    updated_at: i64,
    working: bool,
}

/// Session-keyed observations. No coordinator identity or durable state.
#[derive(Default)]
pub(crate) struct Claims {
    activity: BTreeMap<String, Activity>,
    claims: BTreeMap<String, Vec<Claim>>,
}

impl Claims {
    pub fn observe_chats(&mut self, chats: &[Chat], rows: &[Session], now: i64) {
        for activity in self.activity.values_mut() {
            activity.working = false;
        }
        for row in rows {
            let chat = chats.iter().find(|chat| chat.id == row.chat_id);
            let activity = Activity {
                chat: row.chat_id.clone(),
                harness_session: chat
                    .map_or_else(
                        || {
                            self.activity
                                .get(&row.chat_id)
                                .and_then(|a| a.harness_session.clone())
                        },
                        |c| c.harness_session_id.clone(),
                    )
                    .filter(|s| !s.is_empty()),
                last_output: row
                    .last_output_at
                    .or(row.started_at)
                    .map_or(0, |t| t.timestamp_millis())
                    .max(self.activity.get(&row.chat_id).map_or(0, |a| a.last_output)),
                updated_at: row.updated_at.timestamp_millis(),
                working: row.status == SessionStatus::Working
                    && now.saturating_sub(row.updated_at.timestamp_millis()) < 90_000,
            };
            self.activity.insert(row.chat_id.clone(), activity);
        }
        while self.activity.len() > 1000 {
            let oldest = self
                .activity
                .iter()
                .min_by_key(|(_, a)| a.updated_at)
                .map(|(id, _)| id.clone());
            if let Some(id) = oldest {
                self.activity.remove(&id);
            }
        }
    }

    pub fn replace(&mut self, claims: Vec<Claim>) {
        self.claims.clear();
        for claim in claims {
            self.claims
                .entry(claim.holder.clone())
                .or_default()
                .push(claim);
        }
    }

    pub fn remove(&mut self, number: u64) {
        self.claims.retain(|_, claims| {
            claims.retain(|claim| claim.number != number);
            !claims.is_empty()
        });
    }

    fn matching<'a>(&'a self, holder: &'a str) -> impl Iterator<Item = &'a Activity> {
        self.activity.values().filter(move |a| {
            holder == a.chat
                || holder.strip_prefix("chat:") == Some(a.chat.as_str())
                || a.harness_session.as_deref().is_some_and(|id| {
                    holder == id
                        || holder
                            .split_once(':')
                            .is_some_and(|(_, native)| native == id)
                })
        })
    }

    fn activity<'a>(&'a self, holder: &'a str, now: i64) -> Option<&'a Activity> {
        self.matching(holder)
            .filter(|a| a.working && now.saturating_sub(a.updated_at) < 90_000)
            .max_by_key(|a| a.last_output)
    }

    pub fn progress(&self, holder: &str) -> Option<i64> {
        self.matching(holder).map(|a| a.last_output).max()
    }

    pub fn stalled(&self, now: i64, window: i64) -> Vec<Claim> {
        self.claims
            .values()
            .flatten()
            .filter_map(|claim| {
                let mut observed = claim.clone();
                observed.last_progress_at = observed
                    .last_progress_at
                    .max(observed.claimed_at)
                    .max(self.progress(&claim.holder).unwrap_or(0));
                (now.saturating_sub(observed.last_progress_at) >= window).then_some(observed)
            })
            .collect()
    }

    pub fn summary(&self, now: i64, window: i64) -> String {
        let chatless: Vec<_> = self
            .claims
            .values()
            .flatten()
            .filter(|c| self.activity(&c.holder, now).is_none())
            .map(|c| format!("#{} ({})", c.number, c.holder))
            .collect();
        let stalled: Vec<_> = self
            .stalled(now, window)
            .iter()
            .map(|c| format!("#{}", c.number))
            .collect();
        format!(
            "\nClaims with no live chat: {}.\nClaims with no progress through the stall window; plan as unclaimed and use the stall ladder: {}.\n",
            if chatless.is_empty() {
                "none".into()
            } else {
                chatless.join(", ")
            },
            if stalled.is_empty() {
                "none".into()
            } else {
                stalled.join(", ")
            }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sixteen_claims_without_a_progressing_chat_are_unclaimed_at_the_stall_window() {
        let mut claims = Claims::default();
        claims.replace(
            (1..=16)
                .map(|number| Claim {
                    number,
                    holder: "claude-code:coordinator".into(),
                    claimed_at: 0,
                    last_progress_at: 0,
                })
                .collect(),
        );
        assert!(claims.stalled(19 * 60_000, 20 * 60_000).is_empty());
        assert_eq!(claims.stalled(20 * 60_000, 20 * 60_000).len(), 16);
        let summary = claims.summary(20 * 60_000, 20 * 60_000);
        assert!(summary.contains("Claims with no live chat: #1"));
        assert!(summary.contains("plan as unclaimed and use the stall ladder: #1"));
    }

    #[test]
    fn recent_chat_output_counts_but_a_heartbeat_alone_does_not() {
        let mut claims = Claims::default();
        claims.replace(vec![Claim {
            number: 7,
            holder: "codex:native".into(),
            claimed_at: 0,
            last_progress_at: 0,
        }]);
        claims.activity.insert(
            "chat".into(),
            Activity {
                chat: "chat".into(),
                harness_session: Some("native".into()),
                last_output: 15 * 60_000,
                updated_at: 20 * 60_000,
                working: true,
            },
        );
        assert!(claims.stalled(20 * 60_000, 20 * 60_000).is_empty());
        assert!(
            !claims
                .summary(20 * 60_000, 20 * 60_000)
                .contains("Claims with no live chat: #7")
        );
        assert_eq!(claims.stalled(35 * 60_000, 20 * 60_000).len(), 1);
        claims.activity.clear();
        assert_eq!(claims.stalled(20 * 60_000, 20 * 60_000).len(), 1);
    }

    #[test]
    fn fresh_heartbeats_do_not_hide_a_stalled_turn_and_idle_chats_are_listed_separately() {
        let now = 20 * 60_000;
        let mut claims = Claims::default();
        claims.replace(vec![Claim {
            number: 7,
            holder: "chat:worker".into(),
            claimed_at: 0,
            last_progress_at: 0,
        }]);
        let mut row = Session {
            last_completed_turn: None,
            chat_id: "worker".into(),
            device_id: "computer".into(),
            status: SessionStatus::Working,
            started_at: chrono::DateTime::from_timestamp_millis(0),
            updated_at: chrono::DateTime::from_timestamp_millis(now).unwrap(),
            running_subagents: 0,
            background_tasks: 0,
            last_output_at: chrono::DateTime::from_timestamp_millis(60_000),
        };
        claims.observe_chats(&[], &[row.clone()], now);
        assert_eq!(claims.stalled(now, 10 * 60_000).len(), 1);
        row.last_output_at = chrono::DateTime::from_timestamp_millis(now);
        claims.observe_chats(&[], &[row.clone()], now);
        assert!(claims.stalled(now, 10 * 60_000).is_empty());
        claims.activity.get_mut("worker").unwrap().harness_session = Some("native".into());
        claims.observe_chats(&[], &[row.clone()], now);
        assert_eq!(
            claims.progress("codex:native"),
            Some(now),
            "a missing chat snapshot must not lose a known native session"
        );
        row.status = SessionStatus::Idle;
        claims.observe_chats(&[], &[row], now);
        assert!(
            claims
                .summary(now, 10 * 60_000)
                .contains("Claims with no live chat: #7")
        );
        assert!(claims.stalled(now, 10 * 60_000).is_empty());
        claims.observe_chats(&[], &[], now + 5 * 60_000);
        assert!(claims.stalled(now + 5 * 60_000, 10 * 60_000).is_empty());
        assert_eq!(claims.stalled(now + 10 * 60_000, 10 * 60_000).len(), 1);
    }
}
