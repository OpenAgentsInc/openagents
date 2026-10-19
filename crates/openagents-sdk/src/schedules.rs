//! The account's scheduled prompts (`/v1/schedules`, #11177). The site keeps
//! one list per account; each computer runs its own entries, so no server
//! clock starts work. The app host uses this for Foreman's check-ins
//! (#11965); Coder keeps its own copy of the same records.
use crate::{CancellationToken, Client, Error, Session};
use reqwest::Method;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

/// Every id starts with this (a Coder background rule's id).
pub const PREFIX: &str = "prompt-";
/// The longest prompt the site keeps.
pub const MAX_PROMPT_BYTES: usize = 4000;
/// The shortest and longest interval the site keeps.
pub const MIN_EVERY_SECS: u64 = 60;
/// The longest interval the site keeps.
pub const MAX_EVERY_SECS: u64 = 30 * 24 * 3600;

/// One scheduled prompt, or the record that it was deleted. Either a time
/// of day on chosen days or an interval. `chat` names the conversation it
/// posts into; unset means a new run.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Schedule {
    pub id: String,
    pub computer: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub days: Vec<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub every_secs: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat_title: Option<String>,
    /// The saved agent this schedule wakes, by definition ID (#12044), so
    /// it is known by its agent rather than its prompt text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    #[serde(default)]
    pub paused: bool,
    /// Unix seconds of the last change; the newer record wins a merge.
    #[serde(default)]
    pub updated: u64,
    #[serde(default)]
    pub deleted: bool,
    /// Unix seconds of the last run, written by the host on each tick
    /// (#12051). The site keeps the later one whichever record wins.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_ran_at: Option<i64>,
}

/// Whether a schedule ran when it should have (`GET /v1/schedules/{id}/health`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Health {
    /// It ran within its last interval.
    OnTime,
    /// It missed this many runs in a row. At two, the site tells the owner
    /// once, through the agent the schedule wakes.
    Late { missed: u32 },
    /// No host has recorded a run yet.
    NeverRan,
}

#[derive(Deserialize)]
struct HealthAnswer {
    health: Health,
}

impl Schedule {
    /// Whether the site would keep this record as sent.
    pub fn valid(&self) -> bool {
        let id = self.id.strip_prefix(PREFIX).is_some()
            && self.id.len() <= 40
            && self
                .id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        if !id || self.computer.trim().is_empty() || self.computer.len() > 64 || self.updated == 0 {
            return false;
        }
        if self.deleted {
            return true;
        }
        let prompt = self.prompt.as_deref().unwrap_or_default().trim();
        let when = match (&self.time, self.every_secs) {
            (Some(_), None) => self.days.iter().all(|d| *d <= 6),
            (None, Some(s)) => (MIN_EVERY_SECS..=MAX_EVERY_SECS).contains(&s),
            _ => false,
        };
        let chat = self.chat.as_deref().is_none_or(|c| {
            !c.is_empty()
                && c.len() <= 128
                && c.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
        });
        !prompt.is_empty() && prompt.len() <= MAX_PROMPT_BYTES && when && chat
    }
}

/// A fresh schedule id.
pub fn new_id() -> Result<String, Error> {
    let mut bytes = [0_u8; 6];
    getrandom::fill(&mut bytes).map_err(|_| Error::InvalidReference)?;
    Ok(format!(
        "{PREFIX}{}",
        bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
    ))
}

#[derive(Deserialize)]
struct Listing {
    #[serde(default)]
    schedules: Vec<Schedule>,
}

/// A client for the scheduled prompts on one account.
pub struct Schedules<'a> {
    client: &'a Client,
    session: &'a Session,
}

impl Client {
    /// Returns a client for the scheduled prompts on the session's account.
    pub fn schedules<'a>(&'a self, session: &'a Session) -> Result<Schedules<'a>, Error> {
        self.check_session(session)?;
        Ok(Schedules {
            client: self,
            session,
        })
    }
}

impl Schedules<'_> {
    async fn send<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &[&str],
        body: Option<serde_json::Value>,
        cancel: &CancellationToken,
    ) -> Result<T, Error> {
        self.client.check_session(self.session)?;
        tokio::select! { biased;
            _ = cancel.cancelled() => Err(Error::Cancelled),
            result = self.client.request(method, path, &[], body, Some(self.session.token())) => result,
        }
    }

    /// Every live scheduled prompt on the account, newest first.
    pub async fn list(&self, cancel: &CancellationToken) -> Result<Vec<Schedule>, Error> {
        let listing: Listing = self.send(Method::GET, &["schedules"], None, cancel).await?;
        Ok(listing.schedules)
    }

    /// Send `computer`'s changed records (a newer `updated` wins, a deletion
    /// wins a tie) and receive that computer's whole list, deletions included.
    pub async fn sync(
        &self,
        computer: &str,
        changed: &[Schedule],
        cancel: &CancellationToken,
    ) -> Result<Vec<Schedule>, Error> {
        if changed.iter().any(|s| !s.valid() || s.computer != computer) {
            return Err(Error::InvalidReference);
        }
        let body = serde_json::json!({ "computer": computer, "schedules": changed });
        let listing: Listing = self
            .send(Method::POST, &["schedules", "sync"], Some(body), cancel)
            .await?;
        Ok(listing.schedules)
    }

    /// Whether the schedule ran on time, from its last recorded run.
    pub async fn health(
        &self,
        schedule: &str,
        cancel: &CancellationToken,
    ) -> Result<Health, Error> {
        if !schedule.starts_with(PREFIX) || schedule.len() > 40 {
            return Err(Error::InvalidReference);
        }
        let answer: HealthAnswer = self
            .send(
                Method::GET,
                &["schedules", schedule, "health"],
                None,
                cancel,
            )
            .await?;
        Ok(answer.health)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn every(secs: u64) -> Schedule {
        Schedule {
            id: "prompt-0a1b2c3d4e5f".into(),
            computer: "device-1".into(),
            prompt: Some("Check in.".into()),
            every_secs: Some(secs),
            chat: Some("0123abcd-0000-0000-0000-000000000000".into()),
            updated: 1,
            ..Schedule::default()
        }
    }

    #[test]
    fn records_match_what_the_site_keeps() {
        assert!(every(1200).valid());
        assert!(!every(59).valid());
        assert!(!every(MAX_EVERY_SECS + 1).valid());
        let mut both = every(1200);
        both.time = Some("09:00".into());
        assert!(!both.valid());
        let mut bad_chat = every(1200);
        bad_chat.chat = Some("../x".into());
        assert!(!bad_chat.valid());
        let mut deleted = Schedule {
            deleted: true,
            ..every(1200)
        };
        deleted.prompt = None;
        assert!(deleted.valid());
        let id = new_id().unwrap();
        assert!(Schedule { id, ..every(1200) }.valid());
        let wire = serde_json::to_value(every(1200)).unwrap();
        assert_eq!(wire["every_secs"], 1200);
        assert!(wire.get("time").is_none());
        assert!(wire.get("last_ran_at").is_none());
    }

    #[test]
    fn the_run_time_and_health_read_from_the_wire() {
        let ran = Schedule {
            last_ran_at: Some(1_700_000_000),
            agent: Some("a".repeat(64)),
            ..every(1200)
        };
        let wire = serde_json::to_value(&ran).unwrap();
        assert_eq!(wire["last_ran_at"], 1_700_000_000);
        assert_eq!(serde_json::from_value::<Schedule>(wire).unwrap(), ran);
        // An old record without it still reads.
        let old: Schedule =
            serde_json::from_value(serde_json::json!({"id": "prompt-x", "computer": "c"})).unwrap();
        assert_eq!(old.last_ran_at, None);
        for (text, health) in [
            (r#"{"state":"on_time"}"#, Health::OnTime),
            (r#"{"state":"late","missed":2}"#, Health::Late { missed: 2 }),
            (r#"{"state":"never_ran"}"#, Health::NeverRan),
        ] {
            assert_eq!(serde_json::from_str::<Health>(text).unwrap(), health);
            assert_eq!(serde_json::to_string(&health).unwrap(), text);
        }
    }
}
