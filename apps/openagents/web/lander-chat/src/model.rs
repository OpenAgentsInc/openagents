//! What the chat shows, apart from drawing it: the page's address, a chat's
//! messages as transcript entries, the words the page says, and where the
//! card sits on a page of a given size.

use std::collections::BTreeMap;

use openagents_doc::{MessagePart, MessageRole, MessageStatus, SessionMessageEntry};
use openagents_web_chat_view::labels::NOTICE_PREFIX;
use openagents_web_client::ThreadMessage;
use serde::Deserialize;

/// The pages the chat is on: `/lander8`, and `/lander10` with the live
/// pylons around it.
pub const BASE: &str = "/lander8";
pub const BASES: [&str; 3] = ["/lander8", "/lander10", "/lander11"];
/// The Grid homepage, `/` (#12413).
pub const HOME_BASE: &str = "/";
/// The Grid homepage's first address, which now redirects to [`HOME_BASE`].
const OLD_HOME_BASE: &str = "/home";
/// `/lander10`: the pylons share the page.
pub const PYLONS_BASE: &str = "/lander10";
/// `/lander11`: Verse's Grid draws behind the chat.
pub const VERSE_BASE: &str = "/lander11";
/// What the chat reads before it sends anything (`pages/lander8.rs`).
pub const SESSION: &str = "/lander8/session";
/// The composer's longest message, as the site takes it.
pub const MAX_CHARS: usize = 4_000;
/// Where a message waits while its sender signs in: this tab's session
/// storage, under this key.
pub const DRAFT_KEY: &str = "openagents.lander8.draft";

/// The Grid terminal's model choice beside the composer (#12421): DeepSeek
/// V4.1 while a ready group of pylons serves it, else any Pylon. The site
/// routes a turn that names [`GROUP_MODEL`] to that group.
pub const GROUP_MODEL: &str = "deepseek-v4.1-flash";
pub const PICK_GROUP: &str = "DeepSeek V4.1";
pub const PICK_GROUP_SHORT: &str = "DeepSeek";
pub const PICK_ANY: &str = "Any Pylon";
pub const PICK_ANY_SHORT: &str = "Any";
/// The page global the Grid's pylons layer sets to `true` while a ready
/// V4.1 group is online.
pub const GROUP_READY_GLOBAL: &str = "openagentsPylonGroupReady";
/// The page global the chat's client sends as `X-OpenAgents-Pylon-Model`.
pub const MODEL_GLOBAL: &str = "openagentsPylonModel";
/// Where a chat's choice is kept in this browser, followed by its id.
pub const PICK_KEY: &str = "openagents.grid.pick.";

/// A chat's kept choice: DeepSeek V4.1 unless it chose any Pylon.
pub fn asks_group(kept: Option<&str>) -> bool {
    kept != Some("any")
}

/// What [`asks_group`] reads back.
pub fn kept_pick(group: bool) -> &'static str {
    if group { "group" } else { "any" }
}

/// The line over the composer before the first message.
pub const WELCOME: &str = "What can we help with?";
pub const PLACEHOLDER: &str = "Message OpenAgents";
pub const NEW_CHAT: &str = "New chat";
pub const TOO_LONG: &str = "Messages can be at most 4,000 characters.";
pub const SIGN_IN: &str = "Sign in";
pub const CONNECTING: &str = "One moment, we're connecting.";
pub const WENT_WRONG: &str = "Something went wrong. Try again.";
pub const LOST: &str = "We couldn't follow this answer. Reload to try again.";

/// `GET /lander8/session`.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Session {
    pub csrf: String,
    pub signed_in: bool,
}

/// The chat an address names, if any: `/lander8/chat/{id}`.
pub fn chat_of(path: &str) -> Option<String> {
    let id = path.strip_prefix(base_of(path))?.strip_prefix("/chat/")?;
    let id = id.trim_end_matches('/');
    (!id.is_empty() && !id.contains('/')).then(|| id.to_owned())
}

/// The Grid homepage keeps the conversation in the query so it stays on the Grid page.
pub fn home_chat(search: &str) -> Option<String> {
    search.trim_start_matches('?').split('&').find_map(|part| {
        let id = part.strip_prefix("chat=")?;
        (!id.is_empty()
            && id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'))
        .then(|| id.to_owned())
    })
}

/// The address for `chat`, or the page's own for a new one.
pub fn path_for(base: &str, chat: Option<&str>) -> String {
    match chat {
        Some(id) if base == HOME_BASE => format!("{base}?chat={id}"),
        Some(id) => format!("{base}/chat/{id}"),
        None => base.to_owned(),
    }
}

/// How opaque the card is: on `/lander11` it is glass, so the Grid shows
/// through it faintly.
pub fn card_alpha(base: &str) -> f32 {
    if matches!(base, VERSE_BASE | HOME_BASE) {
        0.72
    } else {
        0.92
    }
}

/// Which of [`BASES`] the address `path` is under (`/lander8` when none).
pub fn base_of(path: &str) -> &'static str {
    if path.is_empty() || path == HOME_BASE || path.trim_end_matches('/') == OLD_HOME_BASE {
        return HOME_BASE;
    }
    BASES
        .into_iter()
        .find(|base| {
            path.strip_prefix(base)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
        })
        .unwrap_or(BASE)
}

/// Whether the page says something over the composer before the first
/// message: `/lander10` and `/lander11` show only the composer.
pub fn welcomes(base: &str) -> bool {
    base == BASE
}

/// Whether a refusal means the person has to sign in to keep going.
pub fn asks_sign_in(code: &str) -> bool {
    matches!(code, "sign_in_required" | "signed_out" | "http_401")
}

/// The parts of an answer from its messages, in order: thinking folds into
/// a group, the site's own notes ("Stopped.") draw as quiet lines, and the
/// rest is markdown.
pub fn answer_parts<'a>(
    messages: impl IntoIterator<Item = (usize, &'a str, &'a str)>,
) -> Vec<MessagePart> {
    messages
        .into_iter()
        .filter(|(_, _, text)| !text.trim().is_empty())
        .map(|(index, role, text)| match role {
            "thinking" => MessagePart::Reasoning {
                id: format!("r{index}"),
                text: text.to_owned(),
            },
            "tool" => MessagePart::Text {
                id: format!("{NOTICE_PREFIX}-{index}"),
                text: text.to_owned(),
            },
            _ => MessagePart::Text {
                id: format!("m{index}"),
                text: text.to_owned(),
            },
        })
        .collect()
}

pub fn entry(
    id: String,
    role: MessageRole,
    parts: Vec<MessagePart>,
    status: MessageStatus,
) -> SessionMessageEntry {
    SessionMessageEntry {
        id,
        role,
        parts,
        created_at: 0,
        device_id: String::new(),
        status: Some(status),
        continuation_of: None,
        duration_ms: None,
    }
}

/// A person's message as an entry.
pub fn user_entry(id: String, text: &str) -> SessionMessageEntry {
    entry(
        id,
        MessageRole::User,
        vec![MessagePart::Text {
            id: "t".into(),
            text: text.to_owned(),
        }],
        MessageStatus::Complete,
    )
}

/// The transcript's entries for a chat's messages (`GET /v1/threads/{id}`):
/// each person's message is one entry, and the answer after it one more.
pub fn entries(messages: &[ThreadMessage]) -> Vec<SessionMessageEntry> {
    let mut out = Vec::new();
    let mut ix = 0;
    while ix < messages.len() {
        if messages[ix].role == "user" {
            if !messages[ix].text.trim().is_empty() {
                out.push(user_entry(format!("u{ix}"), &messages[ix].text));
            }
            ix += 1;
            continue;
        }
        let start = ix;
        while ix < messages.len() && messages[ix].role != "user" {
            ix += 1;
        }
        let parts = answer_parts(
            messages[start..ix]
                .iter()
                .enumerate()
                .map(|(at, m)| (start + at, m.role.as_str(), m.text.as_str())),
        );
        if !parts.is_empty() {
            out.push(entry(
                format!("a{start}"),
                MessageRole::Assistant,
                parts,
                MessageStatus::Complete,
            ));
        }
    }
    out
}

/// A chat's messages without the answer still being written, which the
/// answer's event stream sends again from its start.
pub fn without_pending(mut messages: Vec<ThreadMessage>, pending: bool) -> Vec<ThreadMessage> {
    if pending {
        let cut = messages
            .iter()
            .rposition(|m| m.role == "user")
            .map_or(0, |at| at + 1);
        messages.truncate(cut);
    }
    messages
}

/// The answer being written as an entry, from its messages by position.
pub fn live_entry(
    request: &str,
    messages: &BTreeMap<usize, (String, String)>,
) -> SessionMessageEntry {
    entry(
        format!("live-{request}"),
        MessageRole::Assistant,
        answer_parts(
            messages
                .iter()
                .map(|(index, (role, text))| (*index, role.as_str(), text.as_str())),
        ),
        MessageStatus::Streaming,
    )
}

/// A rectangle on the page, in pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
}

/// Where things sit on a page `w` by `h`: the composer at the bottom, and
/// above it the room the card may grow into, under the page's name.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub narrow: bool,
    pub composer: Rect,
    /// The card's column: its full height is the most it grows to.
    pub room: Rect,
}

/// Under the mark and name in the page's corner.
const TOP: f32 = 60.;

/// How far down the page the card may grow on `/lander10`, as a share of
/// its height: the pylons stand above it. A wide page has room for them
/// beside the card too; a phone keeps the band under the horizon for them.
pub const PYLONS_SHARE: f32 = 0.4;
pub const PYLONS_SHARE_NARROW: f32 = 0.58;

/// The highest the card grows on a `w` by `h` page under `base`.
pub fn card_top(base: &str, w: f32, h: f32) -> f32 {
    let share = if w < 640. {
        PYLONS_SHARE_NARROW
    } else {
        PYLONS_SHARE
    };
    if base != PYLONS_BASE {
        TOP
    } else {
        (h * share).max(TOP)
    }
}

impl Layout {
    pub fn new(w: f32, h: f32) -> Self {
        Self::under(w, h, TOP)
    }

    /// [`Layout::new`] with the card growing no higher than `top`
    /// (`/lander10` keeps the upper part of the page for the pylons).
    pub fn under(w: f32, h: f32, top: f32) -> Self {
        let top = top.max(TOP);
        let narrow = w < 640.;
        let gutter = if narrow { 12. } else { 24. };
        let col = (w - 2. * gutter).clamp(0., 760.);
        let x = (w - col) / 2.;
        let composer_h = if narrow { 52. } else { 56. };
        let bottom = if narrow { 12. } else { 28. };
        let composer = Rect {
            x,
            y: (h - bottom - composer_h).max(top),
            w: col,
            h: composer_h,
        };
        let gap = if narrow { 10. } else { 14. };
        let room = Rect {
            x,
            y: top,
            w: col,
            h: (composer.y - gap - top).max(0.),
        };
        Self {
            narrow,
            composer,
            room,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(role: &str, text: &str) -> ThreadMessage {
        serde_json::from_value(serde_json::json!({"role": role, "text": text})).unwrap()
    }

    #[test]
    fn addresses_name_chats() {
        assert_eq!(chat_of("/lander8"), None);
        assert_eq!(chat_of("/lander8/chat/abc"), Some("abc".into()));
        assert_eq!(chat_of("/lander8/chat/abc/"), Some("abc".into()));
        assert_eq!(chat_of("/lander8/chat/a/b"), None);
        assert_eq!(chat_of("/gpui/chat/abc"), None);
        assert_eq!(path_for(BASE, Some("x")), "/lander8/chat/x");
        assert_eq!(path_for(BASE, None), "/lander8");
        assert_eq!(chat_of("/lander10/chat/abc"), Some("abc".into()));
        assert_eq!(chat_of("/lander100/chat/abc"), None);
        assert_eq!(base_of("/lander10"), "/lander10");
        assert_eq!(base_of("/lander10/chat/x"), "/lander10");
        assert_eq!(base_of("/lander8/chat/x"), "/lander8");
        assert_eq!(path_for("/lander10", Some("x")), "/lander10/chat/x");
        assert!(welcomes("/lander8") && !welcomes("/lander10") && !welcomes("/lander11"));
        assert_eq!(base_of("/lander11/chat/x"), "/lander11");
        assert_eq!(chat_of("/lander11/chat/x"), Some("x".into()));
        assert!(card_alpha("/lander11") < card_alpha("/lander8"));
        assert_eq!(card_top("/lander11", 1440., 900.), 60.);
    }

    #[test]
    fn home_chat_stays_on_the_grid_and_restores_its_query() {
        assert_eq!(base_of("/"), HOME_BASE);
        assert_eq!(base_of("/home"), HOME_BASE);
        assert_eq!(base_of("/lander8"), BASE);
        assert_eq!(base_of("/chat/abc"), BASE);
        assert_eq!(path_for(HOME_BASE, Some("abc-123")), "/?chat=abc-123");
        assert_eq!(path_for(HOME_BASE, None), "/");
        assert_eq!(home_chat("?gl&chat=abc-123"), Some("abc-123".into()));
        assert_eq!(home_chat("?chat="), None);
        assert_eq!(home_chat("?chat=x/y"), None);
        assert!(!welcomes(HOME_BASE));
    }

    #[test]
    fn an_answer_folds_into_one_entry() {
        let messages = [
            message("user", "Hi"),
            message("thinking", "Hmm"),
            message("assistant", "Hello"),
            message("tool", "Stopped."),
            message("user", "Again"),
        ];
        let entries = entries(&messages);
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].role, MessageRole::User);
        let answer = &entries[1].parts;
        assert!(matches!(&answer[0], MessagePart::Reasoning { text, .. } if text == "Hmm"));
        assert!(matches!(&answer[1], MessagePart::Text { id, .. } if id == "m2"));
        assert!(
            matches!(&answer[2], MessagePart::Text { id, .. } if id.starts_with(NOTICE_PREFIX))
        );
        assert_eq!(entries[2].role, MessageRole::User);
    }

    #[test]
    fn a_pending_answer_is_left_to_its_stream() {
        let messages = vec![message("user", "Hi"), message("assistant", "Hel")];
        assert_eq!(without_pending(messages.clone(), true).len(), 1);
        assert_eq!(without_pending(messages, false).len(), 2);
        let mut live = BTreeMap::new();
        live.insert(1, ("assistant".to_owned(), "Hel".to_owned()));
        let entry = live_entry("r1", &live);
        assert_eq!(entry.status, Some(MessageStatus::Streaming));
        assert_eq!(entry.parts.len(), 1);
    }

    #[test]
    fn signing_in_is_asked_for_by_these_refusals() {
        assert!(asks_sign_in("sign_in_required"));
        assert!(asks_sign_in("signed_out"));
        assert!(!asks_sign_in("busy"));
    }

    #[test]
    fn the_card_grows_between_the_name_and_the_composer() {
        let desktop = Layout::new(1440., 900.);
        assert!(!desktop.narrow);
        assert_eq!(desktop.composer.w, 760.);
        assert_eq!(desktop.composer.bottom(), 900. - 28.);
        assert!(desktop.room.bottom() < desktop.composer.y);
        assert_eq!(desktop.room.y, 60.);
        let phone = Layout::new(390., 844.);
        assert!(phone.narrow);
        assert_eq!(phone.composer.w, 390. - 24.);
        assert_eq!(phone.composer.x, 12.);
        assert!(phone.room.h > 600.);
        let shared = Layout::under(1440., 900., card_top("/lander10", 1440., 900.));
        assert_eq!(shared.room.y, 360.);
        assert_eq!(shared.composer, desktop.composer);
    }

    #[test]
    fn the_page_s_words_are_plain() {
        for text in [
            WELCOME,
            PLACEHOLDER,
            NEW_CHAT,
            TOO_LONG,
            SIGN_IN,
            CONNECTING,
            WENT_WRONG,
            LOST,
            PICK_GROUP,
            PICK_GROUP_SHORT,
            PICK_ANY,
            PICK_ANY_SHORT,
        ] {
            assert!(oa_copy::violations(text, &[]).is_empty(), "{text}");
        }
    }

    #[test]
    fn a_chat_asks_deepseek_unless_it_chose_any_pylon() {
        assert!(asks_group(None));
        assert!(asks_group(Some(kept_pick(true))));
        assert!(!asks_group(Some(kept_pick(false))));
    }
}
