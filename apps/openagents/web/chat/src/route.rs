//! The page's address: under the base the page names (`/gpui` by default,
//! in its `openagents-chat-base` meta tag), the base is a new chat and
//! `{base}/chat/{id}` an open one. Opening a chat pushes its address, and
//! Back and Forward come back through [`watch`].

#[cfg(target_family = "wasm")]
use futures::channel::mpsc;
#[cfg(target_family = "wasm")]
use wasm_bindgen::JsCast;
#[cfg(target_family = "wasm")]
use wasm_bindgen::prelude::*;

/// The page's `<meta>` that says whether the person is signed in (`1`) or
/// chatting as the browser's visitor (`0`).
pub const SIGNED_IN_META: &str = "openagents-signed-in";

/// The HTML chat's address for `chat` (a new chat when `None`), which the
/// `ui=html` switch shows even where the GPUI chat is turned on.
pub fn regular(chat: Option<&str>) -> String {
    match chat {
        Some(id) => format!("/chat/{id}?ui=html"),
        None => "/new?ui=html".to_owned(),
    }
}

/// The base when the page names none.
pub const BASE: &str = "/gpui";
/// The page's `<meta>` that names the base.
pub const BASE_META: &str = "openagents-chat-base";

/// The chat an address under `base` names, if any.
pub fn chat_of(base: &str, path: &str) -> Option<String> {
    let id = path.strip_prefix(base)?.strip_prefix("/chat/")?;
    let id = id.trim_end_matches('/');
    (!id.is_empty() && !id.contains('/')).then(|| id.to_owned())
}

pub fn path_for(base: &str, chat: Option<&str>) -> String {
    match chat {
        Some(id) => format!("{base}/chat/{id}"),
        None if base.is_empty() => "/".to_owned(),
        None => base.to_owned(),
    }
}

/// The base the page names.
#[cfg(target_family = "wasm")]
pub fn base() -> String {
    web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| {
            d.query_selector(&format!("meta[name=\"{BASE_META}\"]"))
                .ok()
                .flatten()
        })
        .and_then(|e| e.get_attribute("content"))
        .map(|base| base.trim_end_matches('/').to_owned())
        .unwrap_or_else(|| BASE.to_owned())
}

/// Whether the page says the person is signed in. A page that doesn't say
/// is the `/gpui` page of a signed-in account.
#[cfg(target_family = "wasm")]
pub fn signed_in() -> bool {
    web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| {
            d.query_selector(&format!("meta[name=\"{SIGNED_IN_META}\"]"))
                .ok()
                .flatten()
        })
        .and_then(|e| e.get_attribute("content"))
        .is_none_or(|value| value != "0")
}

#[cfg(target_family = "wasm")]
pub fn current() -> Option<String> {
    let path = web_sys::window()?.location().pathname().ok()?;
    chat_of(&base(), &path)
}

#[cfg(target_family = "wasm")]
/// Shows `chat`'s address, adding a history entry when it changed.
pub fn push(chat: Option<&str>) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let path = path_for(&base(), chat);
    if window.location().pathname().ok().as_deref() == Some(path.as_str()) {
        return;
    }
    if let Ok(history) = window.history() {
        let _ = history.push_state_with_url(&JsValue::NULL, "", Some(&path));
    }
}

#[cfg(target_family = "wasm")]
pub fn set_title(title: &str) {
    if let Some(document) = web_sys::window().and_then(|w| w.document()) {
        document.set_title(&format!("{title} \u{b7} OpenAgents"));
    }
}

#[cfg(target_family = "wasm")]
/// Leaves the canvas for another page of the site.
pub fn go(url: &str) {
    if let Some(window) = web_sys::window() {
        let _ = window.location().assign(url);
    }
}

#[cfg(target_family = "wasm")]
/// Signs in again, then comes back here.
pub fn sign_in() {
    let Some(window) = web_sys::window() else {
        return;
    };
    let here = window.location().pathname().unwrap_or_else(|_| base());
    let back = String::from(js_sys::encode_uri_component(&here));
    go(&format!("/login?return_to={back}"));
}

#[cfg(target_family = "wasm")]
/// The chat each Back or Forward lands on.
pub fn watch() -> mpsc::UnboundedReceiver<Option<String>> {
    let (sender, receiver) = mpsc::unbounded();
    if let Some(window) = web_sys::window() {
        let listener = Closure::<dyn FnMut(web_sys::PopStateEvent)>::new(move |_| {
            let _ = sender.unbounded_send(current());
        });
        let _ =
            window.add_event_listener_with_callback("popstate", listener.as_ref().unchecked_ref());
        // The page keeps one listener for its whole life.
        listener.forget();
    }
    receiver
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_name_chats() {
        assert_eq!(chat_of(BASE, "/gpui"), None);
        assert_eq!(chat_of(BASE, "/gpui/chat/abc"), Some("abc".into()));
        assert_eq!(chat_of(BASE, "/gpui/chat/abc/"), Some("abc".into()));
        assert_eq!(chat_of(BASE, "/gpui/chat/a/b"), None);
        assert_eq!(path_for(BASE, Some("x")), "/gpui/chat/x");
        assert_eq!(path_for(BASE, None), "/gpui");
        // At the regular addresses, once swapped in.
        assert_eq!(chat_of("", "/chat/abc"), Some("abc".into()));
        assert_eq!(path_for("", None), "/");
    }

    #[test]
    fn the_regular_chat_is_asked_for_by_name() {
        assert_eq!(regular(Some("abc")), "/chat/abc?ui=html");
        assert_eq!(regular(None), "/new?ui=html");
    }
}
