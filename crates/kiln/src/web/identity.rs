//! The page's agent identity (#12451): the credential `kiln-git own`
//! printed for this browser's key, kept in local storage and added to every
//! patch and runner job the page signs.

use std::cell::RefCell;

use nostr::domain::Tag;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{Document, HtmlTextAreaElement};

use crate::identity::{Credential, short_npub, verify_credential};

const STORAGE: &str = "kiln-credential";

thread_local! {
    static HELD: RefCell<Option<Credential>> = const { RefCell::new(None) };
}

/// The tags to add to what this page signs: the credential's, when valid.
pub(super) fn tags() -> Vec<Tag> {
    HELD.with(|held| {
        held.borrow()
            .as_ref()
            .map(Credential::tags)
            .unwrap_or_default()
    })
}

fn storage() -> Option<web_sys::Storage> {
    web_sys::window().and_then(|w| w.local_storage().ok().flatten())
}

/// Read `text` as a credential for `agent`, show what it says, and hold it.
fn take(doc: &Document, agent: &str, text: &str) {
    let say = |line: &str| {
        if let Some(node) = doc.get_element_by_id("identity") {
            node.set_text_content(Some(line));
        }
    };
    let text = text.trim();
    if text.is_empty() {
        HELD.with(|held| *held.borrow_mut() = None);
        say("Patches show as from an unverified author until you add an identity.");
        return;
    }
    let read = serde_json::from_str::<Credential>(text)
        .map_err(|_| "This isn't an identity from kiln-git own.".to_string())
        .and_then(|c| verify_credential(&c, agent, super::util::unix()).map(|m| (c, m)));
    match read {
        Ok((credential, mandate)) => {
            let until = js_sys::Date::new(&JsValue::from_f64(mandate.expires as f64 * 1000.0));
            say(&format!(
                "Patches show as an agent authorized by {}, until {}.",
                short_npub(&mandate.owner),
                String::from(until.to_locale_string("en-US", &JsValue::UNDEFINED))
            ));
            if let Some(s) = storage() {
                let _ = s.set_item(STORAGE, text);
            }
            HELD.with(|held| *held.borrow_mut() = Some(credential));
        }
        Err(e) => {
            HELD.with(|held| *held.borrow_mut() = None);
            say(&format!("{e} Patches show as from an unverified author."));
        }
    }
}

/// Load the stored credential for `agent` and follow the paste field.
pub(super) fn wire(doc: &Document, agent: &str) {
    let stored = storage()
        .and_then(|s| s.get_item(STORAGE).ok().flatten())
        .unwrap_or_default();
    // A host page older than this module may lack the field.
    let Some(field) = doc.get_element_by_id("credential") else {
        take(doc, agent, &stored);
        return;
    };
    let area: HtmlTextAreaElement = field.unchecked_into();
    if area.value().is_empty() {
        area.set_value(&stored);
    }
    take(doc, agent, &area.value());
    let (doc2, agent2, area2) = (doc.clone(), agent.to_string(), area.clone());
    let closure = Closure::<dyn Fn(web_sys::Event)>::new(move |_| {
        take(&doc2, &agent2, &area2.value());
    });
    let _ = area.add_event_listener_with_callback("change", closure.as_ref().unchecked_ref());
    closure.forget();
}
