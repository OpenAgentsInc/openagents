//! **Give feedback** on selected text, shared by the desktop and the phone
//! (#10127; the report itself is [`playtest::feedback`]).
//!
//! The adapters know the selected text and the transcript row it starts
//! in. This module turns that row into where the text came from (the
//! conversation, the message's index, and a reply's route, tier, and
//! model), and seals and sends a report to the triage key over NIP-42, as
//! the phone's Report a problem does.

use std::time::Duration;

use openagents_chat::basic_coder::{Role, Turn};
use playtest::report::{self, ChatRole, Randomness, Report, Sealed, Selection};
use secp256k1::{SecretKey, XOnlyPublicKey};
use serde_json::json;

/// The message index a transcript row key names: `turn-3`, `turn-3-body`,
/// or `talk-m3-md` under the prefix `turn-` or `talk-m`.
#[must_use]
pub fn turn_index(key: &str, prefix: &str) -> Option<usize> {
    let rest = key.strip_prefix(prefix)?;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty()
        || !rest[digits.len()..].is_empty() && !rest[digits.len()..].starts_with('-')
    {
        return None;
    }
    digits.parse().ok()
}

/// The selection a report carries: `text`, in conversation `thread`, in
/// the message at `index` of `turns` (whose first is message `start`).
#[must_use]
pub fn selection(
    text: &str,
    thread: Option<&str>,
    turns: &[Turn],
    start: usize,
    index: Option<usize>,
) -> Selection {
    let turn = index
        .and_then(|index| index.checked_sub(start))
        .and_then(|at| turns.get(at));
    let meta = turn.and_then(|turn| turn.meta.as_ref());
    Selection {
        text: playtest::feedback::clip(text),
        thread: thread.map(str::to_owned),
        turn: turn.and(index).and_then(|index| u32::try_from(index).ok()),
        role: turn.map(|turn| match turn.role {
            Role::User => ChatRole::User,
            Role::Assistant => ChatRole::Assistant,
        }),
        route: meta.and_then(|meta| meta.route.clone()),
        tier: meta.and_then(|meta| meta.tier.clone()),
        answer: meta.and_then(|meta| meta.answer.clone()),
        model: turn.and_then(|turn| turn.model.clone()),
    }
}

fn random() -> Randomness {
    use secp256k1::rand::{Rng, RngCore};
    let mut rng = secp256k1::rand::rng();
    let mut seal_nonce = [0; 32];
    let mut wrap_nonce = [0; 32];
    rng.fill_bytes(&mut seal_nonce);
    rng.fill_bytes(&mut wrap_nonce);
    Randomness {
        wrapper: SecretKey::new(&mut rng),
        seal_nonce,
        wrap_nonce,
        // NIP-59 moves outer timestamps back; an hour keeps relays happy.
        seal_earlier: rng.random_range(0..3_600),
        wrap_earlier: rng.random_range(0..3_600),
    }
}

/// Publishes `event` to [`playtest::RELAY`], authenticating as `auth`.
/// Blocking.
///
/// # Errors
///
/// When the relay can't be reached or refuses it.
pub fn publish(event: &nostr::domain::Event, auth: &SecretKey) -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| "Couldn't start a connection.".to_string())?;
    runtime.block_on(async {
        let mut socket =
            nostr_transport::Connection::connect(playtest::RELAY, auth, Duration::from_secs(10))
                .await
                .map_err(|e| e.to_string())?;
        socket
            .send(json!(["EVENT", event]))
            .await
            .map_err(|e| e.to_string())?;
        for _ in 0..16 {
            let frame = socket.next().await.map_err(|e| e.to_string())?;
            if frame[0] == "OK" && frame[1] == event.id.as_str() {
                let _ = socket.close().await;
                return if frame[2] == true {
                    Ok(())
                } else {
                    Err(format!(
                        "The relay refused it: {}",
                        frame[3].as_str().unwrap_or("no reason")
                    ))
                };
            }
        }
        Err("The relay didn't answer.".to_string())
    })
}

/// Seals `report` from `tester` to `triage` and sends it: the private
/// report authenticated as the wrap's one-time key, then, when `public`,
/// its public, content-free record as the tester. Blocking. Returns the sealed report
/// (its code is what the triage inbox lists).
///
/// # Errors
///
/// When the report can't be sealed or the relay doesn't take the private
/// report. A public record the relay refused is not an error: the report
/// reached the triage key.
pub fn send(
    report: &Report,
    tester: &SecretKey,
    triage: &XOnlyPublicKey,
    public: bool,
) -> Result<Sealed, String> {
    let random = random();
    let sealed = report::wrap(report, tester, triage, &random)?;
    publish(&sealed.wrap, &random.wrapper)?;
    if public {
        let _ = publish(&sealed.public, tester);
    }
    Ok(sealed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use openagents_chat::router::Meta;

    #[test]
    fn a_row_key_names_its_message_and_the_selection_names_the_reply() {
        assert_eq!(turn_index("turn-3", "turn-"), Some(3));
        assert_eq!(turn_index("turn-3-body", "turn-"), Some(3));
        assert_eq!(turn_index("talk-m12-md", "talk-m"), Some(12));
        assert_eq!(turn_index("talk-working", "talk-m"), None);
        assert_eq!(turn_index("turn-3x", "turn-"), None);
        assert_eq!(turn_index("stream", "turn-"), None);

        let mut reply = Turn::user("Coder runs on your phone.");
        reply.role = Role::Assistant;
        reply.model = Some("gpt-5.4".into());
        reply.meta = Some(Meta {
            tier: Some("model".into()),
            route: Some("chat".into()),
            ..Meta::default()
        });
        let turns = vec![Turn::user("where does Coder run?"), reply];
        let picked = selection("Coder runs", Some("c1"), &turns, 4, Some(5));
        assert_eq!(picked.turn, Some(5));
        assert_eq!(picked.role, Some(ChatRole::Assistant));
        assert_eq!(picked.route.as_deref(), Some("chat"));
        assert_eq!(picked.tier.as_deref(), Some("model"));
        assert_eq!(picked.model.as_deref(), Some("gpt-5.4"));
        // A row outside the page names the conversation only.
        let outside = selection("x", Some("c1"), &turns, 4, Some(9));
        assert_eq!((outside.turn, outside.model), (None, None));
        assert_eq!(outside.thread.as_deref(), Some("c1"));
    }
}
