//! Chat-to-chat messages: the header `send_message` puts on them, and the
//! guard that ends an exchange of pleasantries (#12124).
//!
//! Two agent chats used to thank each other through `send_message` for
//! turns on end, because every message asked for a reply. The header now
//! asks for a reply only when one is needed, and [`closing_refusal`] stops
//! a send that would only acknowledge an acknowledgement.

use openagents_doc::{MessagePart, MessageRole, SessionMessageEntry};

/// The header's fixed tail before the sender's chat id. The desktop
/// transcript parses it to show a short attribution, so keep it stable.
const REPLY_TAIL: &str = ". Reply only if it asks you something or you need something from it, with the OpenAgents `send_message` tool, chat ";

/// Longest message, in words, that can count as an acknowledgement.
const ACK_MAX_WORDS: usize = 25;

/// Words that make up thanks, acknowledgements, and sign-offs.
const COURTESY: &[&str] = &[
    "a",
    "ack",
    "acknowledged",
    "acknowledge",
    "again",
    "all",
    "also",
    "am",
    "and",
    "any",
    "anything",
    "anytime",
    "appreciate",
    "appreciated",
    "awesome",
    "back",
    "bye",
    "by",
    "care",
    "cheers",
    "clear",
    "confirmed",
    "confirm",
    "cool",
    "copy",
    "day",
    "do",
    "else",
    "end",
    "excellent",
    "fine",
    "for",
    "from",
    "further",
    "glad",
    "good",
    "got",
    "great",
    "happy",
    "have",
    "help",
    "here",
    "i",
    "i'll",
    "i'm",
    "if",
    "it",
    "know",
    "let",
    "likewise",
    "luck",
    "me",
    "message",
    "more",
    "much",
    "my",
    "need",
    "needed",
    "nice",
    "no",
    "noted",
    "nothing",
    "now",
    "of",
    "ok",
    "okay",
    "on",
    "perfect",
    "pleasure",
    "problem",
    "received",
    "roger",
    "same",
    "see",
    "set",
    "side",
    "so",
    "sounds",
    "standing",
    "sure",
    "thank",
    "thanks",
    "that",
    "the",
    "this",
    "thx",
    "to",
    "too",
    "ty",
    "understood",
    "update",
    "very",
    "we",
    "we're",
    "welcome",
    "well",
    "will",
    "with",
    "wonderful",
    "you",
    "you're",
    "your",
    "yours",
];

/// Words one of which an acknowledgement must carry.
const ANCHORS: &[&str] = &[
    "ack",
    "acknowledged",
    "appreciate",
    "appreciated",
    "bye",
    "cheers",
    "confirmed",
    "copy",
    "got",
    "great",
    "likewise",
    "noted",
    "ok",
    "okay",
    "perfect",
    "pleasure",
    "received",
    "roger",
    "thank",
    "thanks",
    "thx",
    "ty",
    "understood",
    "welcome",
];

/// The text a chat sends to another, attributed to the chat `origin_id`
/// labelled `label`.
pub fn attributed(label: &str, origin_id: &str, text: &str) -> String {
    format!("[Message from OpenAgents chat {label}{REPLY_TAIL}{origin_id}.]\n\n{text}")
}

/// The sending chat's id and the body of an attributed message, for this
/// header and the earlier ones (which ended `` `send_message` tool, chat ID.] ``).
pub fn parse_attributed(text: &str) -> Option<(&str, &str)> {
    let rest = text.strip_prefix("[Message from ")?;
    let (header, body) = rest.split_once("]\n\n")?;
    let (_, id) = header.rsplit_once("`send_message` tool, chat ")?;
    Some((id.strip_suffix('.')?, body))
}

/// Whether `text` only thanks, acknowledges, or signs off.
pub fn is_acknowledgement(text: &str) -> bool {
    if text.contains('?') {
        return false;
    }
    let words: Vec<String> = text
        .split_whitespace()
        .map(|w| {
            w.trim_matches(|c: char| !c.is_alphanumeric() && c != '\'')
                .to_lowercase()
                .replace('\u{2019}', "'")
        })
        .filter(|w| !w.is_empty())
        .collect();
    if words.is_empty() || words.len() > ACK_MAX_WORDS {
        return false;
    }
    let other = words
        .iter()
        .filter(|w| !COURTESY.contains(&w.as_str()))
        .count();
    // A name or one stray word ("Thanks, Foreman!") still counts.
    other * 2 <= words.len() && other <= 2 && words.iter().any(|w| ANCHORS.contains(&w.as_str()))
}

fn normalized(text: &str) -> String {
    text.split_whitespace()
        .map(|w| {
            w.trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase()
        })
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// The bodies, oldest first, of the attributed messages in `transcript`
/// that the chat `from_id` sent.
pub fn messages_from(transcript: &[SessionMessageEntry], from_id: &str) -> Vec<String> {
    transcript
        .iter()
        .filter(|e| e.role == MessageRole::User)
        .filter_map(|e| {
            let text: String = e
                .parts
                .iter()
                .filter_map(|p| match p {
                    MessagePart::Text { text, .. } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            let (id, body) = parse_attributed(&text)?;
            (id == from_id).then(|| body.to_owned())
        })
        .collect()
}

/// Why `next` shouldn't go to a chat: the last message from that chat
/// (`received`) only acknowledged, and `next` would only acknowledge it
/// back or repeat the last message sent there (`sent`). `None` sends.
pub fn closing_refusal(
    sent: &[String],
    received: &[String],
    next: &str,
    target: &str,
) -> Option<String> {
    let last_received = received.last()?;
    if !is_acknowledgement(last_received) {
        return None;
    }
    let repeat = sent
        .last()
        .is_some_and(|last| normalized(last) == normalized(next));
    (is_acknowledgement(next) || repeat).then(|| {
        format!(
            "Not sent: chat {target} only acknowledged your last message, and this one doesn't ask or tell it anything new. The conversation is finished, so don't reply to it. Send a message only when you need something from that chat."
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pleasantries_count_as_acknowledgements() {
        for text in [
            "Thanks!",
            "You're welcome.",
            "Acknowledged.",
            "Understood, thanks.",
            "Thanks, Foreman!",
            "Got it, thank you very much.",
            "Noted. Let me know if you need anything else.",
            "You’re welcome — glad to help.",
        ] {
            assert!(is_acknowledgement(text), "{text}");
        }
        for text in [
            "Done.",
            "Here is the report.",
            "Can you rerun the tests?",
            "Thanks. The build on #12081 failed in crates/mcp; please look.",
            "Please review the diff in apps/openagents/crates/mcp.",
            "Thanks — anything else?",
        ] {
            assert!(!is_acknowledgement(text), "{text}");
        }
    }

    #[test]
    fn headers_round_trip_old_and_new() {
        let new = attributed("Helper (chat-1)", "chat-1-full", "Done.");
        assert_eq!(parse_attributed(&new), Some(("chat-1-full", "Done.")));
        let old = "[Message from OpenAgents chat Helper (chat-1). Reply to it with the OpenAgents `send_message` tool, chat chat-1.]\n\nDone.";
        assert_eq!(parse_attributed(old), Some(("chat-1", "Done.")));
        assert_eq!(parse_attributed("Done."), None);
    }

    #[test]
    fn a_real_reply_to_thanks_still_goes() {
        let received = vec!["Thanks!".to_owned()];
        assert!(closing_refusal(&[], &received, "Thanks, you too.", "b").is_some());
        assert!(closing_refusal(&[], &received, "One more thing: rebase first.", "b").is_none());
        let received = vec!["Here is the report.".to_owned()];
        assert!(closing_refusal(&[], &received, "Thanks!", "b").is_none());
        assert!(closing_refusal(&[], &[], "Thanks!", "b").is_none());
    }

    #[test]
    fn a_repeat_after_an_acknowledgement_is_refused() {
        let sent = vec!["Status: merged.".to_owned()];
        let received = vec!["Noted.".to_owned()];
        assert!(closing_refusal(&sent, &received, "Status: merged", "b").is_some());
    }
}
