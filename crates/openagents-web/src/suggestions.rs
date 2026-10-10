//! The chat's suggestion chips on the website, chosen as on the phone
//! (`openagents_chat::suggestions`, the one source for every surface):
//!
//! - on the homepage, under the composer: the first four starter questions
//!   the visitor has not used yet, then used ones ([`starters`]);
//! - under a settled answer in `/chat/{id}`: its routed chips and its
//!   follow-ups not used yet ([`reply_chips`]).
//!
//! A chip that asks posts its words exactly as the composer would (`q`,
//! `request_id`, `csrf`, `selection`), so the worker's router picks the
//! same prepared answer it picks for the phone's tap: the phone sends the
//! words too, and the suggestion's id is the bank id the answer comes back
//! named with (`Meta::answer`).
//!
//! "Used" is read from what the visitor's chats already keep: the words of
//! every message they sent and the prepared answers they were shown
//! (`Request::reply`), the same marks the phone keeps as it goes.
//!
//! Routed chips come from the reply's typed `open_screen` and `run_coder`
//! offers, through the phone's own selection
//! (`openagents_chat::suggestions::screen_offers`); a screen with no
//! working page on this site is left out.
//!
//! An answer that came with plugin cards (the reply's typed `plugins`,
//! `docs/web/plugin-card.md`) shows them inside its message
//! ([`plugin_cards`]), drawn from Coder's built-in plugins
//! (`coder::builtin_plugins`).

use maud::{Markup, html};
use openagents_chat::router::{Meta, Screen};
use openagents_chat::suggestions;
use openagents_ui::content::{PluginCard, PluginCards};
use openagents_ui::icons::Icon;
use openagents_ui::shell::{SuggestionChip, SuggestionChips};

use crate::App;
use crate::chat_store::{Conversation, Message, Outcome, Role};

/// The used marks of `owner`'s chats. An unavailable store reads as
/// nothing used, so the starters still show.
pub(crate) async fn used(app: &App, owner: &str) -> Vec<String> {
    match app.config.chat_store.list(owner).await {
        Ok(chats) => markers(&chats),
        Err(_) => Vec::new(),
    }
}

/// The used marks of `chats`: every message sent, every prepared answer
/// shown.
pub(crate) fn markers(chats: &[Conversation]) -> Vec<String> {
    let sent = chats.iter().flat_map(|chat| {
        chat.messages
            .iter()
            .filter(|message| message.role == Role::User)
            .map(|message| message.text.as_str())
    });
    let answers = chats.iter().flat_map(|chat| {
        chat.requests
            .iter()
            .filter_map(|request| request.reply.as_ref()?.answer.as_deref())
    });
    suggestions::markers(sent, answers)
}

/// The fields a chip posts: what the composer posts, with `q` its words.
fn fields(app: &App, owner: &str, words: &str, selection: String) -> Vec<(String, String)> {
    vec![
        ("q".to_owned(), words.to_owned()),
        ("request_id".to_owned(), crate::pages::chat::new_id()),
        ("csrf".to_owned(), crate::pages::chat::csrf(app, owner)),
        ("selection".to_owned(), selection),
    ]
}

/// The homepage's starter chips: each starts a new chat at `/chat` with
/// its message, as the composer there does (a plain post; the server
/// redirects to the chat).
pub(crate) fn starters(app: &App, owner: &str, used: &[String]) -> Markup {
    let chips = suggestions::suggestions(used).map(|suggestion| {
        SuggestionChip::send(
            suggestion.label,
            "/chat",
            fields(app, owner, suggestion.message, String::new()),
        )
    });
    html! { (SuggestionChips::new("Suggestions").id("chat-suggestions").chips(chips)) }
}

/// Where a routed chip for `screen` goes on this site, and what it reads.
/// Only screens with a working page here; the rest are left out.
fn route(screen: Screen, connecting: bool) -> Option<(&'static str, &'static str)> {
    match screen {
        // A visitor here has no computer: the guide to connecting one.
        Screen::Computers if connecting => Some((
            suggestions::screen_label(screen, connecting),
            "/docs/connect-a-computer",
        )),
        // The Wallet, identity keys, Playtest, problem reports, the Gym's
        // screens and the Map live in the apps.
        _ => None,
    }
}

/// The answer whose chips show: the chat's last message, a settled answer
/// whose request was answered, and what the router said about it.
fn settled(chat: &Conversation) -> Option<&Meta> {
    if chat.pending.is_some() {
        return None;
    }
    let last = chat.messages.last()?;
    if last.role != Role::Assistant || last.text.is_empty() {
        return None;
    }
    let id = last.request_id.as_deref()?;
    let request = chat.requests.iter().find(|request| request.id == id)?;
    if request.outcome != Outcome::Answered || request.cloud.is_some() {
        return None;
    }
    request.reply.as_ref()
}

/// The chips under a settled answer in `chat`: its routed chips, then its
/// follow-ups not used yet. Each follow-up posts its words to the chat
/// with HTMX, as the chat's composer does. Empty while an answer streams.
pub(crate) async fn reply_chips(app: &App, chat: &Conversation) -> Markup {
    let Some(meta) = settled(chat) else {
        return html! {};
    };
    let mut chips = Vec::new();
    // A visitor here has no computer, so an offer to run Coder reads as
    // connecting one, as on a phone with none.
    let run_offered = openagents_chat::delegation::offered(Some(meta), false);
    if run_offered {
        chips.push(SuggestionChip::link(
            suggestions::CONNECT_LABEL,
            "/docs/connect-a-computer",
        ));
    }
    for (_, screen, connecting) in suggestions::screen_offers(meta, false, true, run_offered) {
        if let Some((label, href)) = route(screen, connecting) {
            chips.push(SuggestionChip::link(label, href));
        }
    }
    if !meta.followups.is_empty() {
        let used = used(app, &chat.owner).await;
        let action = format!("/chat/{}", chat.id);
        let selection = crate::composer::seal(
            app,
            &chat.owner,
            &chat.selection.clone().unwrap_or_default(),
        );
        for (_, followup) in suggestions::followups(meta, &used) {
            chips.push(SuggestionChip::send(
                followup.label.clone(),
                action.clone(),
                fields(app, &chat.owner, &followup.label, selection.clone()),
            ));
        }
    }
    html! {
        (SuggestionChips::new("Suggestions")
            .id("chat-followups")
            .enhanced(true)
            .chips(chips))
    }
}

/// Where every built-in plugin runs, as its card says.
const PLUGIN_RUNS_ON: &str = "With Coder on your computer";

/// The card action that works on this site: nothing here runs a plugin,
/// so each card leads to getting Coder, which does.
const PLUGIN_ACTION: (&str, &str) = ("Get Coder", "/download");

/// The icon a built-in plugin's card shows, by slug.
fn plugin_icon(slug: &str) -> Icon {
    match slug {
        "claude-code" => Icon::Assistant,
        "codex" => Icon::Code,
        "cursor" => Icon::Cursor,
        "grok-build" => Icon::Agent,
        "openrouter" => Icon::ApiKey,
        _ => Icon::PluginPuzzle,
    }
}

/// The plugin cards an answer comes with (`docs/web/plugin-card.md`): one
/// per slug the reply carried that Coder's built-in plugins know, in the
/// reply's order, named and described by `coder::builtin_plugins`. A slug
/// it doesn't know draws nothing. Empty without slugs.
pub(crate) fn plugin_cards(slugs: &[String]) -> Markup {
    if slugs.is_empty() {
        return html! {};
    }
    let cards = slugs.iter().filter_map(|slug| {
        let plugin = coder::builtin_plugins::builtin_plugin(slug)?;
        Some(
            PluginCard::new(plugin.name, plugin_icon(plugin.slug), plugin.summary)
                .runs_on(PLUGIN_RUNS_ON)
                .action(PLUGIN_ACTION.0, PLUGIN_ACTION.1),
        )
    });
    html! { (PluginCards::new("Plugins").cards(cards)) }
}

/// The plugins a stored assistant `message` shows as cards: those its
/// answered request's reply carried. None while it streams or for any
/// other message.
pub(crate) fn message_plugins<'a>(chat: &'a Conversation, message: &Message) -> &'a [String] {
    message_reply(chat, message)
        .map(|reply| reply.plugins.as_slice())
        .unwrap_or_default()
}

/// What the worker said about the reply a stored assistant `message`
/// shows: the parts the chips and cards read, and how it was served (its
/// tier, route, and prepared answer). None while it streams or for any
/// other message.
pub(crate) fn message_reply<'a>(chat: &'a Conversation, message: &Message) -> Option<&'a Meta> {
    if message.role != Role::Assistant {
        return None;
    }
    message
        .request_id
        .as_deref()
        .and_then(|id| chat.requests.iter().find(|request| request.id == id))
        .filter(|request| request.outcome == Outcome::Answered)
        .and_then(|request| request.reply.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat_store::Request;
    use openagents_chat::router::{Followup, Offer};

    fn chat(reply: Option<Meta>) -> Conversation {
        Conversation {
            id: "00000000-0000-4000-8000-000000000000".into(),
            owner: "owner".into(),
            revision: 2,
            title: "What is OpenAgents?".into(),
            messages: vec![
                Message {
                    role: Role::User,
                    text: "What is OpenAgents?".into(),
                    request_id: Some("r".into()),
                },
                Message {
                    role: Role::Assistant,
                    text: "We are OpenAgents.".into(),
                    request_id: Some("r".into()),
                },
            ],
            pending: None,
            requests: vec![Request {
                id: "r".into(),
                digest: String::new(),
                outcome: Outcome::Answered,
                selection: None,
                cloud: None,
                files: Vec::new(),
                reply,
            }],
            selection: None,
            updated_unix: 1,
            pinned_unix: None,
            archived_unix: None,
            project: None,
            terminal: None,
            environment: None,
            tasks: Vec::new(),
            opened_unix: None,
            branch: None,
        }
    }

    #[test]
    fn sent_words_and_shown_answers_are_used() {
        let meta = Meta {
            answer: Some("meta.model@v2".into()),
            ..Meta::default()
        };
        let used = markers(&[chat(Some(meta))]);
        let shown: Vec<&str> = suggestions::suggestions(&used).map(|s| s.id).collect();
        // "What is OpenAgents?" was sent and the model answer shown: both
        // last, in order.
        assert_eq!(
            shown,
            ["meta.codebase", "meta.plugins", "meta.who", "meta.model"]
        );
    }

    #[test]
    fn only_a_settled_answer_offers_chips_and_routes_need_a_page_here() {
        let meta = Meta {
            offers: vec![
                Offer::OpenScreen {
                    screen: Screen::Computers,
                },
                Offer::OpenScreen {
                    screen: Screen::Wallet,
                },
            ],
            followups: vec![Followup {
                answer: None,
                label: "What can you do?".into(),
            }],
            ..Meta::default()
        };
        assert!(settled(&chat(Some(meta.clone()))).is_some());
        let mut pending = chat(Some(meta.clone()));
        pending.pending = Some(crate::chat_store::Pending {
            request_id: "r".into(),
            started_unix: 1,
            job_id: None,
        });
        assert!(settled(&pending).is_none());
        assert!(settled(&chat(None)).is_none());
        let routed: Vec<_> = suggestions::screen_offers(&meta, false, true, false)
            .filter_map(|(_, screen, connecting)| route(screen, connecting))
            .collect();
        assert_eq!(routed, [("Connect a computer", "/docs/connect-a-computer")]);
    }

    /// An answered reply's plugin slugs draw the built-in plugins' cards, each
    /// with the one action that works here; unknown slugs draw nothing,
    /// and a user message or a reply without slugs shows none.
    #[test]
    fn an_answer_with_plugin_slugs_shows_the_catalogs_cards() {
        let meta = Meta {
            answer: Some("plugins.web@2".into()),
            plugins: vec!["claude-code".into(), "project-map".into(), "codex".into()],
            ..Meta::default()
        };
        let answered = chat(Some(meta));
        assert!(message_plugins(&answered, &answered.messages[0]).is_empty());
        let slugs = message_plugins(&answered, &answered.messages[1]);
        assert_eq!(slugs, ["claude-code", "project-map", "codex"]);
        let html = plugin_cards(slugs).into_string();
        assert_eq!(
            html.matches(r#"<article class="oa-plugin-card""#).count(),
            2
        );
        let claude = coder::builtin_plugins::builtin_plugin("claude-code").expect("built in");
        assert!(html.contains(&maud::html! { (claude.name) }.into_string()));
        assert!(html.contains(&maud::html! { (claude.summary) }.into_string()));
        // A sample plugin's slug draws nothing.
        assert!(!html.contains("Project map"));
        assert!(html.contains(PLUGIN_RUNS_ON));
        assert!(html.contains(r#"href="/download""#));
        assert!(!html.contains("<button"));
        crate::copy_guard::assert_plain("/chat", &html);
        assert!(plugin_cards(&[]).into_string().is_empty());
        let plain = chat(None);
        assert!(message_plugins(&plain, &plain.messages[1]).is_empty());
    }
}
