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
pub const FLINT_MODEL: &str = "flint";
pub const PICK_FLINT: &str = "Flint";
pub const FLINT_READY_GLOBAL: &str = "openagentsPylonFlintReady";
pub const PICK_GROUP: &str = "DeepSeek V4.1";
pub const PICK_GROUP_SHORT: &str = "DeepSeek";
pub const PICK_ANY: &str = "Any Pylon";
pub const PICK_ANY_SHORT: &str = "Any";
/// The page global the Grid's pylons layer sets to `true` while a ready
/// V4.1 group is online.
pub const GROUP_READY_GLOBAL: &str = "openagentsPylonGroupReady";
/// The page global the chat's client sends as `X-OpenAgents-Pylon-Model`.
pub const MODEL_GLOBAL: &str = "openagentsPylonModel";
/// The page global that pins the Flint choice to one artifact, as
/// `flint@sha256:` and 64 lowercase hex digits (#12436). A check sets it to
/// reach one Flint Pylon; without it, the choice asks for any Flint.
pub const FLINT_PIN_GLOBAL: &str = "openagentsPylonFlintPin";

/// What the Flint choice names: the pin when `pin` is an exact Flint
/// artifact, else [`FLINT_MODEL`].
pub fn flint_choice(pin: Option<&str>) -> &str {
    pin.filter(|pin| {
        pin.strip_prefix(FLINT_MODEL)
            .and_then(|rest| rest.strip_prefix("@sha256:"))
            .is_some_and(|hex| {
                hex.len() == 64
                    && hex
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            })
    })
    .unwrap_or(FLINT_MODEL)
}
/// Where a chat's choice is kept in this browser, followed by its id.
pub const PICK_KEY: &str = "openagents.grid.pick.";

/// A chat's kept choice: DeepSeek V4.1 unless it chose any Pylon.
pub fn asks_group(kept: Option<&str>) -> bool {
    !matches!(kept, Some("any" | "flint"))
}

/// What [`asks_group`] reads back.
pub fn kept_pick(group: bool, flint: bool) -> &'static str {
    if flint {
        "flint"
    } else if group {
        "group"
    } else {
        "any"
    }
}

pub fn asks_flint(kept: Option<&str>) -> bool {
    kept == Some("flint")
}

/// The word the Grid homepage's visit records a model choice as
/// (#12439): `deepseek`, `flint`, or `any`.
pub fn pick_word(group: bool, flint: bool) -> &'static str {
    if flint {
        "flint"
    } else if group {
        "deepseek"
    } else {
        "any"
    }
}

/// Who answered, from the line the site puts over a pylon's answer
/// (`**DeepSeek-V4.1-Flash-1017** · group ...` or `**NAME** · MODEL`): the source
/// (`group`, `pylon`, or `hosted`) and the model's family, a fixed word.
/// Never the pylon's name.
pub fn answer_source(text: &str) -> (&'static str, &'static str) {
    let first = text.trim_start().lines().next().unwrap_or_default();
    let Some((name, from)) = first
        .strip_prefix("**")
        .and_then(|line| line.split_once("** · "))
    else {
        return ("hosted", "other");
    };
    if from.starts_with("group ") {
        return ("group", family(name));
    }
    ("pylon", family(from))
}

/// The model families the visit records, as the site lists them.
pub const FAMILIES: [&str; 10] = [
    "deepseek", "flint", "gemma", "qwen", "llama", "mistral", "gpt", "claude", "gemini", "other",
];

/// The family of a model name: one of [`FAMILIES`], or `other`.
pub fn family(model: &str) -> &'static str {
    let model = model.to_ascii_lowercase();
    FAMILIES[..FAMILIES.len() - 1]
        .iter()
        .find(|family| model.contains(*family))
        .copied()
        .unwrap_or("other")
}

/// Who answered and the model's family: the family the site sent with
/// the answer when it is one of [`FAMILIES`] (it knows the hosted chat's
/// model), else what the answer's first line says.
pub fn answered(text: &str, sent_family: Option<&str>) -> (&'static str, &'static str) {
    let (source, family) = answer_source(text);
    let sent = sent_family.and_then(|sent| FAMILIES.iter().find(|f| **f == sent).copied());
    (source, sent.unwrap_or(family))
}

/// The word a refused message is recorded as: `busy`, `sign_in`,
/// `unavailable` (no pylon could answer), or `refused`; `error` when the
/// site gave no reason.
pub fn failure_word(code: Option<&str>) -> &'static str {
    match code {
        Some(code) if asks_sign_in(code) => "sign_in",
        Some("busy" | "deepseek_busy" | "rate_limited" | "http_429" | "http_409") => "busy",
        Some("pylon_unavailable") => "unavailable",
        Some(_) => "refused",
        None => "error",
    }
}

/// The Kiln control beside the model choice (#12440): it opens Kiln, the
/// in-browser coding agent, in a panel over the Grid.
pub const KILN: &str = "Kiln";
/// The page global that is `true` when the site offers this visitor Kiln
/// at the terminal: an admin, or a signed-in player (#12488).
pub const KILN_GLOBAL: &str = "openagentsKiln";
/// The page global that is `true` while the chat is open at the Grid's
/// help desk (#12443): the chat then asks the site's free hosted agent, in
/// a chat of its own, and offers no model choice.
pub const DESK_GLOBAL: &str = "openagentsChatDesk";
/// The page global that is `true` while the chat is open with the Grid
/// Foreman (#12446): the chat then names the `foreman` surface, so the site
/// answers as the Foreman, in a chat of its own, with no model choice.
pub const FOREMAN_GLOBAL: &str = "openagentsChatForeman";
/// The page function that opens Kiln's panel.
pub const OPEN_KILN: &str = "homeKiln";

/// Whether the terminal offers Kiln: on the Grid homepage, outside Game
/// Lab, and only when the site offers it to this visitor (`offered`: an
/// admin, or a signed-in player, #12488). The server refuses Kiln's files
/// to everyone else too; this only keeps the control from showing.
pub fn offers_kiln(base: &str, game: bool, offered: bool) -> bool {
    offered && !game && base == HOME_BASE
}

/// The Grid homepage's overlay (#12459): the page globals and functions
/// the chat shares with the page's glue and with Verse.
///
/// `true` while the chat's composer is open: at the terminal, the help
/// desk, or in Game Lab.
pub const CHAT_OPEN_GLOBAL: &str = "openagentsChatOpen";
/// `true` while Game Lab's panel is open or opening: Verse sets it through
/// `homeGameLab(open)` when the visitor presses E under the arch.
pub const GAME_LAB_GLOBAL: &str = "openagentsGameLabOpen";
/// The page function that opens and closes Game Lab.
pub const GAME_LAB_CALL: &str = "homeGameLab";
/// The page function that shows a game full screen, or hides it with
/// `null`.
pub const GAME_PLAY_CALL: &str = "homeGamePlay";
/// `true` while a game is shown full screen; the page's own Back to Game
/// Lab, for keyboards, sets it back.
pub const GAME_PLAYING_GLOBAL: &str = "openagentsGamePlaying";
/// `true` while Kiln's panel is open (`homeKiln(open)`).
pub const KILN_OPEN_GLOBAL: &str = "openagentsKilnOpen";
/// The page function that opens and closes the shared terminal's chat.
pub const TERMINAL_CALL: &str = "homeTerminal";
/// The page function that clips the chat's canvas to what it draws and
/// places the frames it draws around (JSON: [`Overlay`]).
pub const OVERLAY_CALL: &str = "homeOverlay";
/// The page's account links the top bar repeats: they stay in the page,
/// for keyboards and screen readers, and a click on the bar clicks them.
pub const ACCOUNT_LINKS: &str = "#home-account a";
/// Game Lab's sign-in check and its card, for the panel.
pub const GAME_SESSION: &str = "/labs/game/session";
pub const GAME_CARD: &str = "/labs/game/card.json";
/// Where a signed-out visitor signs in from Game Lab, coming back to it,
/// when the page can't keep them in the world.
pub const GAME_SIGN_IN: &str = "/login?return_to=%2F%3Flab%3Dgame";
/// The page function that signs in in a popup, keeping the Grid loaded
/// (#12480). It takes the full-page sign-in to fall back to.
pub const SIGN_IN_CALL: &str = "homeSignIn";
/// Counts the popup sign-ins that finished on this page.
pub const SIGNED_IN_GLOBAL: &str = "openagentsSignIns";
/// Kiln's own page, opened in a new tab from its panel.
pub const KILN_PAGE: &str = "/admin/kiln/";

pub const BRAND: &str = "OpenAgents";
pub const GAME_LAB: &str = "Game Lab";
pub const BACK_TO_GRID: &str = "Back to the Grid";
pub const BACK_TO_LAB: &str = "Back to Game Lab";
pub const GAME_OPENING: &str = "Opening Game Lab\u{2026}";
pub const GAME_SIGN_IN_LINE: &str = "Sign in to make your own version.";
pub const GAME_ASK: &str = "Tell us what you want to change.";
pub const GAME_CHECKING: &str = "Checking\u{2026}";
pub const GAME_FAILED: &str = "Game Lab couldn\u{2019}t open. Close it and try again.";
pub const PLAY: &str = "Play";
pub const COMPARE: &str = "Compare with the original";
pub const OWN_WINDOW: &str = "Play in its own window";
pub const BOT_NOTE: &str =
    "The bot plays both on the same house. Measured over the same seeded runs:";
pub const NEW_TAB: &str = "Open in a new tab";

/// A Game Lab card as `GET /labs/game/card.json` gives it (#12459).
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct GameCard {
    pub title: String,
    pub version: u64,
    pub summary: String,
    /// The bot runs' line, ready to show; empty before any run.
    pub stats: String,
    pub versions: usize,
    /// The game in a frame, full screen, and in a window of its own.
    pub preview: String,
    pub play: String,
    pub player: Option<String>,
    pub can_compare: bool,
    pub compare: Option<GameCompare>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct GameCompare {
    pub a: GameSide,
    pub b: GameSide,
    pub differences: Vec<String>,
    pub changes: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct GameSide {
    pub version: u64,
    pub summary: String,
    pub stats: String,
    pub preview: String,
}

impl GameCard {
    /// The card's heading: its title and version.
    pub fn heading(&self) -> String {
        format!("{} \u{b7} v{}", self.title, self.version)
    }

    /// How many versions there are, in words.
    pub fn versions_words(&self) -> String {
        match self.versions {
            1 => "1 version".to_owned(),
            n => format!("{n} versions"),
        }
    }
}

/// What a Game Lab card from the chat's stream asks for: the version to
/// show, whether it compares that version with the original, and whether
/// it plays at once. `None` for any other card.
pub fn stream_card(cards: &serde_json::Value) -> Option<(u64, bool, bool)> {
    let list = cards
        .as_array()
        .or_else(|| cards.get("cards").and_then(serde_json::Value::as_array))?;
    let card = list
        .iter()
        .find(|card| card["card"] == "game_lab" && card["_hosted"] == true)?;
    Some((
        card["version"].as_u64()?,
        card["compare"].is_object(),
        card["autoplay"] == true,
    ))
}

/// The card's address: `version` (the newest when `None`), compared with
/// the original when `compare`.
pub fn game_card_path(version: Option<u64>, compare: bool) -> String {
    match (version, compare) {
        (Some(v), true) => format!("{GAME_CARD}?v={v}&a=1"),
        (Some(v), false) => format!("{GAME_CARD}?v={v}"),
        (None, _) => GAME_CARD.to_owned(),
    }
}

/// What the chat hands the page's `homeOverlay`: the boxes it draws in,
/// which the page clips its canvas to so the world under the rest still
/// takes clicks, and the frames it draws around.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
pub struct Overlay {
    pub regions: Vec<[f32; 4]>,
    pub frames: Vec<Frame>,
}

/// A page shown in a box the chat draws around: Kiln, or a game.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Frame {
    pub id: &'static str,
    pub src: String,
    pub title: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// Kept loaded, hidden, while its box is gone, so it opens again at
    /// once.
    pub keep: bool,
}

/// Where Game Lab's panel goes on a `w` by `h` page: right of the
/// composer's column on a wide page, over it on a phone.
pub fn game_panel(w: f32, h: f32) -> Rect {
    if w < 760. {
        Rect {
            x: 12.,
            y: 60.,
            w: w - 24.,
            h: (h * 0.48).max(160.),
        }
    } else {
        let width = 560f32.min(w * 0.45);
        Rect {
            x: w - 20. - width,
            y: 70.,
            w: width,
            h: (h - 230.).max(200.),
        }
    }
}

/// Game Lab's sign-in prompt: centered over the Grid, so the action it
/// asks for sits where the eye is (#12480). `h` is the most it may take.
pub fn sign_in_panel(w: f32, h: f32) -> Rect {
    let width = 380f32.min(w - 24.);
    let height = 160f32.min(h - 24.);
    Rect {
        x: ((w - width) / 2.).round(),
        y: ((h - height) / 2.).round(),
        w: width,
        h: height,
    }
}

/// `layout` moved left of Game Lab's panel on a page wide enough for
/// both; unchanged on a phone, where the panel sits over the chat.
pub fn beside_panel(layout: Layout, w: f32, h: f32) -> Layout {
    let panel = game_panel(w, h);
    if w < 760. {
        return layout;
    }
    let gutter = 24.;
    let col = (panel.x - 2. * gutter).clamp(0., 760.);
    let x = (panel.x - col) / 2.;
    Layout {
        composer: Rect {
            x,
            w: col,
            ..layout.composer
        },
        room: Rect {
            x,
            w: col,
            ..layout.room
        },
        ..layout
    }
}

/// Where Kiln's panel goes: nearly the whole page, under the top bar.
pub fn kiln_panel(w: f32, h: f32) -> Rect {
    Rect {
        x: 16.,
        y: 60.,
        w: (w - 32.).max(0.),
        h: (h - 76.).max(0.),
    }
}

/// The line over the composer before the first message./// The line over the composer before the first message.
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
    fn kiln_shows_only_on_the_grid_and_only_when_offered() {
        assert!(offers_kiln(HOME_BASE, false, true));
        assert!(!offers_kiln(HOME_BASE, false, false));
        assert!(!offers_kiln(HOME_BASE, true, true));
        for base in BASES {
            assert!(!offers_kiln(base, false, true), "{base}");
        }
        assert!(oa_copy::violations(KILN, &[]).is_empty());
    }

    #[test]
    fn the_flint_choice_takes_only_an_exact_artifact_pin() {
        let pin = format!("flint@sha256:{}", "0f".repeat(32));
        assert_eq!(flint_choice(Some(&pin)), pin);
        assert_eq!(flint_choice(None), FLINT_MODEL);
        for other in [
            "flint@sha256:short",
            &format!("flint@sha256:{}", "0F".repeat(32)),
            &format!("other@sha256:{}", "0f".repeat(32)),
            "deepseek-v4.1-flash",
        ] {
            assert_eq!(flint_choice(Some(other)), FLINT_MODEL, "{other}");
        }
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
    fn game_lab_reads_its_card_and_the_stream_s_cards() {
        let card: GameCard = serde_json::from_value(serde_json::json!({
            "title": "The Ice Cream Man", "version": 2, "summary": "Slower chase",
            "stats": "Bot runs: 3 of 4 escaped", "versions": 2,
            "preview": "/games/the-ice-cream-man#a", "play": "/games/the-ice-cream-man#a&skip",
            "player": null, "can_compare": true,
            "compare": {"a": {"version": 1, "preview": "/games/the-ice-cream-man#o&bot&speed=2"},
                "b": {"version": 2}, "differences": ["Faster"], "changes": ["man.man_chase: 5 \u{2192} 4"]}
        }))
        .unwrap();
        assert_eq!(card.heading(), "The Ice Cream Man \u{b7} v2");
        assert_eq!(card.versions_words(), "2 versions");
        assert_eq!(card.compare.as_ref().unwrap().a.version, 1);
        // A card the site adds fields to still reads.
        assert!(serde_json::from_str::<GameCard>(r#"{"version":1,"later":true}"#).is_ok());
        let stream = serde_json::json!({"cards": [
            {"card": "other"},
            {"card": "game_lab", "_hosted": true, "version": 3, "autoplay": true}
        ]});
        assert_eq!(stream_card(&stream), Some((3, false, true)));
        assert_eq!(
            stream_card(&serde_json::json!([{"card": "game_lab"}])),
            None
        );
        assert_eq!(
            game_card_path(Some(3), true),
            "/labs/game/card.json?v=3&a=1"
        );
        assert_eq!(game_card_path(None, true), "/labs/game/card.json");
    }

    #[test]
    fn the_panels_fit_a_phone_and_a_wide_page() {
        let wide = game_panel(1440., 900.);
        assert_eq!((wide.w, wide.x + wide.w), (560., 1420.));
        let phone = game_panel(390., 844.);
        assert_eq!((phone.x, phone.w), (12., 366.));
        assert!(phone.bottom() < Layout::new(390., 844.).composer.y);
        let beside = beside_panel(Layout::new(1440., 900.), 1440., 900.);
        assert!(beside.composer.x + beside.composer.w <= wide.x - 24.);
        assert_eq!(
            beside_panel(Layout::new(390., 844.), 390., 844.),
            Layout::new(390., 844.)
        );
        for (w, h) in [(1440., 900.), (390., 844.)] {
            let prompt = sign_in_panel(w, h);
            assert!(prompt.x >= 12. && prompt.x + prompt.w <= w - 12.);
            assert!(((prompt.x + prompt.w / 2.) - w / 2.).abs() <= 1.);
            assert!(((prompt.y + prompt.h / 2.) - h / 2.).abs() <= 1.);
        }
        let kiln = kiln_panel(390., 844.);
        assert!(kiln.w > 0. && kiln.bottom() <= 844.);
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
            BRAND,
            GAME_LAB,
            BACK_TO_GRID,
            BACK_TO_LAB,
            GAME_OPENING,
            GAME_SIGN_IN_LINE,
            GAME_ASK,
            GAME_CHECKING,
            GAME_FAILED,
            PLAY,
            COMPARE,
            OWN_WINDOW,
            BOT_NOTE,
            NEW_TAB,
        ] {
            assert!(oa_copy::violations(text, &[]).is_empty(), "{text}");
        }
    }

    #[test]
    fn the_visit_records_who_answered_by_fixed_words_only() {
        assert_eq!(
            answer_source("**DeepSeek-V4.1-Flash-1017** · group w16-g5 (4 parts)\n\nHello"),
            ("group", "deepseek")
        );
        assert_eq!(
            answer_source("**alice-mac** · gemma-3-4b-it\n\nHi"),
            ("pylon", "gemma")
        );
        assert_eq!(answer_source("**Box** · Flint\n\nHi"), ("pylon", "flint"));
        assert_eq!(
            answer_source("**Box** · my-model\n\nHi"),
            ("pylon", "other")
        );
        assert_eq!(answer_source("Hello there"), ("hosted", "other"));
        assert_eq!(
            answered("Hello there", Some("gemini")),
            ("hosted", "gemini")
        );
        assert_eq!(
            answered("Hello there", Some("gemini-2.5-pro")),
            ("hosted", "other")
        );
        assert_eq!(answered("Hello there", None), ("hosted", "other"));
        assert_eq!(
            answered("**alice-mac** · gemma-3-4b-it\n\nHi", Some("gemma")),
            ("pylon", "gemma")
        );
        assert_eq!(pick_word(true, false), "deepseek");
        assert_eq!(pick_word(true, true), "flint");
        assert_eq!(pick_word(false, false), "any");
        assert_eq!(failure_word(Some("busy")), "busy");
        assert_eq!(failure_word(Some("deepseek_busy")), "busy");
        assert_eq!(failure_word(Some("pylon_unavailable")), "unavailable");
        assert_eq!(failure_word(Some("signed_out")), "sign_in");
        assert_eq!(failure_word(Some("too_long")), "refused");
        assert_eq!(failure_word(None), "error");
    }

    #[test]
    fn a_chat_asks_deepseek_unless_it_chose_any_pylon() {
        assert!(asks_group(None));
        assert!(asks_flint(Some(kept_pick(false, true))));
        assert!(!asks_group(Some("flint")));
        assert!(!asks_flint(Some("group")));
        assert!(asks_group(Some(kept_pick(true, false))));
        assert!(!asks_group(Some(kept_pick(false, false))));
    }
}
