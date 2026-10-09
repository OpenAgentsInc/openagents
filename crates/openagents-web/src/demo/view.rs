//! The demo's markup, built from `openagents-ui` only: the chat list in the
//! left panel, the thread, and the docked composer.

use maud::{Markup, html};
use openagents_ui::actions::{Badge, Color, Variant};
use openagents_ui::content::MarkdownRoot;
use openagents_ui::shell::{Composer, HxGet, Message, NavItem, SidebarSection};

use super::chats::{CHATS, DemoChat};

/// The longest message the composer accepts.
pub(crate) const MAX_PROMPT_CHARS: usize = 4_000;

/// The element the chat list swaps the thread into.
pub(crate) const CONTENT_ID: &str = "demo-content";

/// The chat list in the left panel, the current chat highlighted. Each row
/// is a plain link (no JavaScript) that HTMX upgrades to swap only the
/// thread; the response pushes the chat's URL.
///
/// A local builder with the shape of the shared chat-list sidebar that
/// `openagents-ui` is adding (`shell::ChatList`): a titled section, one
/// row per chat, the current one marked, replaceable out of band. Switch to
/// that builder once it lands.
pub(crate) fn chat_list(current: &DemoChat, oob: bool) -> SidebarSection {
    SidebarSection::new("Demo chats")
        .id("demo-chats")
        .swap_oob(oob)
        .items(CHATS.iter().map(|chat| {
            NavItem::new(chat.title, format!("/demo/{}", chat.slug))
                .current(chat.slug == current.slug)
                .hx(HxGet::new(format!("/demo/{}/thread", chat.slug))
                    .target(format!("#{CONTENT_ID}"))
                    .swap("innerHTML")
                    .sync(format!("#{CONTENT_ID}:replace")))
        }))
}

/// The thread: a header with the title and a demo badge, then the turns
/// in the scrolling column. `extra` turns follow the script (a message sent
/// from the composer without JavaScript).
pub(crate) fn thread(chat: &DemoChat, extra: &[Message]) -> Markup {
    html! {
        header.oa-thread-header {
            h1.oa-thread-title { (chat.title) }
            (Badge::new("Demo").color(Color::Secondary).variant(Variant::Outline).pill(true))
        }
        section #demo-thread.oa-thread aria-label="Conversation" {
            div.oa-thread-column {
                p.oa-thread-notice { "A scripted conversation. Nothing here ran on a real machine." }
                div #demo-transcript {
                    @for (index, turn) in (chat.turns)().into_iter().chain(extra.iter().cloned()).enumerate() {
                        (turn.id(format!("demo-{}-{index}", chat.slug)))
                    }
                }
            }
        }
    }
}

/// The reply to a message sent from the composer: the person's text and an
/// honest answer. Nothing is stored or sent anywhere.
pub(crate) fn exchange(text: &str) -> [Message; 2] {
    [
        Message::user(text),
        Message::assistant(MarkdownRoot::new(html! {
            p { "This demo is scripted, so your message wasn't sent to an agent. "
                a href="/" { "Start a real chat" } " to run it." }
        }))
        .author("OpenAgents"),
    ]
}

/// The docked composer for `chat`, inside `#demo-dock` so a reply can
/// replace it (out of band) with an empty one.
pub(crate) fn dock(chat: &DemoChat, oob: bool) -> Markup {
    let composer = Composer::new("demo-composer", format!("/demo/{}/message", chat.slug))
        .label("Message the demo")
        .input_label("Message")
        .placeholder("Message OpenAgents")
        .max_chars(MAX_PROMPT_CHARS)
        .rows(1);
    // The model picker, repository/branch selectors, and "add context"
    // action are left out: the demo has no models, repositories, or
    // attachments, and a control that does nothing must not render.
    // .model_picker(ModelPickerTrigger::new("Auto"))
    // .dropdown(ComposerDropdown::new("Repository", "openagents"))
    // .leading(ComposerAction::new(Icon::Plus, "Add context"))
    html! {
        div #demo-dock hx-swap-oob=[oob.then_some("outerHTML")] { (composer) }
    }
}
