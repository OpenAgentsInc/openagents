//! This computer's sessions in the account's chat list on openagents.com
//! (#11897). Each pass publishes summary rows only: title, folder, status,
//! and last activity. A read-only transcript copy goes up only for the
//! sessions whose transcript the person turned on, and only while it stays
//! on; the service says which ones in its answer to the publish.
use super::*;
use openagents_account::{
    Lease,
    openagents::sessions::{
        self, CopiedMessage, CopiedRole, PublishedSession, SessionStatus as Wire,
    },
};
use openagents_doc::MessagePart;
use openagents_proto::{AuthState, Chat, Session, SessionStatus, Space, WorkspaceScope};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Mutex as StdMutex, OnceLock};

/// Publishing also tells the account this computer is online.
const REPUBLISH_MS: i64 = 30_000;
/// A Working row older than this is shown idle: its engine may be gone.
const STALE_WORKING_MS: i64 = 10 * 60_000;
const MAX_COPIED: usize = 400;

#[derive(Default)]
struct Last {
    at_ms: i64,
    body: String,
    copies: HashMap<String, String>,
}

fn last() -> &'static StdMutex<HashMap<String, Last>> {
    static LAST: OnceLock<StdMutex<HashMap<String, Last>>> = OnceLock::new();
    LAST.get_or_init(Default::default)
}

/// The synced workspace's locator, as `ui::links::workspace_locator`
/// computes it, so the web can open the chat in this app.
pub(crate) fn locator(auth: &AuthState) -> Option<String> {
    let AuthState::SignedIn { user, org_id } = auth else {
        return None;
    };
    let identity = format!(
        "user:{}:org:{}",
        user.id,
        org_id.as_deref().unwrap_or("personal")
    );
    let mut hash = Sha256::new();
    hash.update(format!("{:?}\0{identity}", WorkspaceScope::Synced));
    Some(format!("{:x}", hash.finalize())[..16].to_string())
}

fn status(session: Option<&Session>, now_ms: i64) -> Wire {
    let Some(session) = session else {
        return Wire::Idle;
    };
    match session.status {
        SessionStatus::Working
            if now_ms - session.updated_at.timestamp_millis() > STALE_WORKING_MS =>
        {
            Wire::Idle
        }
        SessionStatus::Working => Wire::Working,
        SessionStatus::AwaitingInput => Wire::AwaitingInput,
        SessionStatus::Errored => Wire::Errored,
        SessionStatus::Idle => Wire::Idle,
    }
}

/// This device's top-level chats as summary rows, newest first. No
/// message text: the recap stays empty until there is a summary that
/// isn't the transcript.
pub(crate) fn rows(
    device: &str,
    chats: &[Chat],
    states: &[Session],
    spaces: &[Space],
    workspace: Option<&str>,
    now_ms: i64,
) -> Vec<PublishedSession> {
    let mut mine: Vec<&Chat> = chats
        .iter()
        .filter(|c| {
            c.device_id == device && !c.archived && c.is_top_level() && chat_home::valid_id(&c.id)
        })
        .collect();
    let active = |c: &Chat| c.last_message_at.unwrap_or(c.created_at).timestamp();
    mine.sort_by(|a, b| active(b).cmp(&active(a)).then_with(|| a.id.cmp(&b.id)));
    mine.into_iter()
        .take(sessions::MAX_PER_COMPUTER)
        .map(|chat| {
            let folder = chat
                .space_id
                .as_ref()
                .and_then(|id| spaces.iter().find(|s| &s.id == id))
                .map(|s| {
                    s.name.clone().unwrap_or_else(|| {
                        s.path
                            .rsplit('/')
                            .find(|p| !p.is_empty())
                            .unwrap_or(&s.path)
                            .to_owned()
                    })
                });
            PublishedSession {
                id: chat.id.clone(),
                title: chat.title.clone().unwrap_or_default(),
                project: None,
                folder,
                status: status(states.iter().find(|s| s.chat_id == chat.id), now_ms),
                last_activity_unix: active(chat).max(0) as u64,
                recap: String::new(),
                workspace: workspace.map(str::to_owned),
            }
        })
        .collect()
}

/// A chat's read-only copy: what the person wrote and the replies' text.
/// Tool calls, reasoning, and files stay on this computer.
fn copy_of(entries: Vec<openagents_doc::SessionMessageEntry>) -> Vec<CopiedMessage> {
    let mut copied: Vec<CopiedMessage> = openagents_doc::join_continuation_entries(entries)
        .into_iter()
        .filter_map(|entry| {
            let role = match entry.role {
                openagents_doc::MessageRole::User => CopiedRole::User,
                openagents_doc::MessageRole::Assistant => CopiedRole::Assistant,
                openagents_doc::MessageRole::System => return None,
            };
            let text = entry
                .parts
                .iter()
                .filter_map(|p| match p {
                    MessagePart::Text { text, .. } if !text.trim().is_empty() => {
                        Some(text.as_str())
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n\n");
            (!text.is_empty()).then_some(CopiedMessage { role, text })
        })
        .collect();
    let skip = copied.len().saturating_sub(MAX_COPIED);
    copied.drain(..skip);
    copied
}

impl DocHost {
    /// One pass: publish when the rows changed or the last publish is
    /// getting old, then upload the copies the service asked for.
    pub(super) async fn publish_session_index(
        &self,
        lease: &Lease,
        auth: &AuthState,
    ) -> Result<(), EngineError> {
        let Some(workspace) = self.workspace() else {
            return Ok(());
        };
        let device = self.inner.config.device_id.clone();
        if chat_home::Home::computer(&device).is_none() {
            return Ok(());
        }
        let name = workspace
            .read_devices()?
            .into_iter()
            .find(|d| d.id == device)
            .map(|d| d.name)
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| "This computer".into());
        let now = now_ms();
        let rows = rows(
            &device,
            &workspace.read_chats()?,
            &workspace.read_sessions()?,
            &workspace.read_spaces()?,
            locator(auth).as_deref(),
            now,
        );
        let body = serde_json::to_string(&(&name, &rows)).unwrap_or_default();
        if let Ok(last) = last().lock()
            && let Some(was) = last.get(&device)
            && was.body == body
            && now - was.at_ms < REPUBLISH_MS
        {
            return Ok(());
        }
        let on = sessions::publish(
            lease.client(),
            lease.session(),
            &device,
            &name,
            &rows,
            &lease.cancel,
        )
        .await?;
        let mut uploads = Vec::new();
        for id in &on {
            let messages = copy_of(self.open_local(id)?.doc().read_entries()?);
            let digest = format!(
                "{:x}",
                Sha256::digest(serde_json::to_vec(&messages).unwrap_or_default())
            );
            let fresh = last()
                .lock()
                .ok()
                .and_then(|l| l.get(&device).and_then(|w| w.copies.get(id).cloned()))
                .is_none_or(|d| d != digest);
            if fresh {
                sessions::upload_transcript(
                    lease.client(),
                    lease.session(),
                    id,
                    &messages,
                    &lease.cancel,
                )
                .await?;
            }
            uploads.push((id.clone(), digest));
        }
        if let Ok(mut last) = last().lock() {
            let entry = last.entry(device).or_default();
            entry.at_ms = now;
            entry.body = body;
            entry.copies = uploads.into_iter().collect();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn chat(id: &str, device: &str, at: i64) -> Chat {
        serde_json::from_value(serde_json::json!({
            "id": id, "deviceId": device, "title": "Fix the badge", "archived": false,
            "cwd": null, "branch": null, "checkoutId": null, "config": null,
            "lastMessagePreview": "secret text from the transcript",
            "lastMessageAt": Utc.timestamp_opt(at, 0).unwrap(),
            "createdAt": Utc.timestamp_opt(1, 0).unwrap(),
        }))
        .unwrap()
    }

    #[test]
    fn rows_are_this_devices_summaries_without_message_text() {
        let one = "a1000000-0000-4000-8000-000000000001";
        let two = "a1000000-0000-4000-8000-000000000002";
        let chats = vec![
            chat(one, "mac", 10),
            chat(two, "mac", 20),
            chat("a1000000-0000-4000-8000-000000000003", "phone", 30),
        ];
        let rows = rows("mac", &chats, &[], &[], Some("0123456789abcdef"), 0);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].id, two);
        assert_eq!(rows[0].status, Wire::Idle);
        assert_eq!(rows[0].last_activity_unix, 20);
        assert!(rows.iter().all(|r| r.recap.is_empty()));
        assert!(
            !serde_json::to_string(&rows)
                .unwrap()
                .contains("secret text")
        );
    }

    #[test]
    fn the_locator_matches_the_apps_link_format() {
        let auth = AuthState::SignedIn {
            user: openagents_proto::UserProfile {
                id: "user_1".into(),
                email: "test@example.com".into(),
                name: None,
            },
            org_id: None,
        };
        // sha256("Synced\0user:user_1:org:personal"), as ui::links computes it.
        assert_eq!(locator(&auth).unwrap(), "14644b2abb3703cc");
        assert!(locator(&AuthState::SignedOut).is_none());
    }
}
