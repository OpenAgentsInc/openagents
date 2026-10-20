//! A computer chat's live transcript for the GPUI web chat (#12224).
//!
//! The chat page loads this module the first time a computer chat opens
//! and calls [`open`] with the chat's id; each change to the chat's chat2
//! room ([`openagents_web_client::chat_room::ChatRoom`]) reaches the page as
//! an `oa-room` event whose detail is JSON: `{chat, entries, caught_up,
//! unavailable}`, the entries being the desktop's `SessionMessageEntry`
//! values. [`close`] leaves the room.

#[cfg(target_family = "wasm")]
mod web {
    use std::cell::RefCell;

    use futures::StreamExt as _;
    use openagents_web_client::chat_room::ChatRoom;
    use openagents_web_client::web::Client;
    use wasm_bindgen::prelude::*;

    thread_local! {
        static ROOM: RefCell<Option<(String, ChatRoom)>> = const { RefCell::new(None) };
    }

    /// Follows chat `chat`'s room, leaving the one open before.
    #[wasm_bindgen]
    pub fn open(chat: String) {
        console_error_panic_hook::set_once();
        if ROOM.with(|room| {
            room.borrow()
                .as_ref()
                .is_some_and(|(open, _)| *open == chat)
        }) {
            return;
        }
        let (room, mut changed) = ChatRoom::open(Client::from_page(), &chat);
        ROOM.with(|slot| *slot.borrow_mut() = Some((chat.clone(), room)));
        wasm_bindgen_futures::spawn_local(async move {
            while changed.next().await.is_some() {
                let Some(detail) = ROOM.with(|slot| {
                    let slot = slot.borrow();
                    let (open, room) = slot.as_ref()?;
                    (*open == chat).then(|| {
                        let (entries, caught_up) = room.read(|r| (r.entries(), r.caught_up()));
                        serde_json::json!({
                            "chat": chat,
                            "entries": entries,
                            "caught_up": caught_up,
                            "unavailable": room.unavailable(),
                        })
                        .to_string()
                    })
                }) else {
                    break;
                };
                send("oa-room", &detail);
            }
        });
    }

    /// Leaves the open room.
    #[wasm_bindgen]
    pub fn close() {
        ROOM.with(|slot| slot.borrow_mut().take());
    }

    fn send(name: &str, detail: &str) {
        let init = web_sys::CustomEventInit::new();
        init.set_detail(&JsValue::from_str(detail));
        if let Some(window) = web_sys::window()
            && let Ok(event) = web_sys::CustomEvent::new_with_event_init_dict(name, &init)
        {
            let _ = window.dispatch_event(&event);
        }
    }
}
