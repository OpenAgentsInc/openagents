//! This browser's storage for Settings (#12288, #12289): its choices in
//! `localStorage`, and pictures too large for it, such as the new-thread
//! background image, in IndexedDB (`openagents-settings`, `files`). The
//! chat and the Settings module read and write the same keys.

use anyhow::{Result, anyhow};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};

use crate::computer::done;

const DB: &str = "openagents-settings";
const STORE: &str = "files";

/// The new-thread background image's IndexedDB key.
pub const BACKGROUND: &str = "new-thread-background";

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

/// `localStorage[key]`, if this browser keeps it.
pub fn get(key: &str) -> Option<String> {
    storage()?.get_item(key).ok().flatten()
}

pub fn set(key: &str, value: &str) {
    if let Some(storage) = storage() {
        let _ = storage.set_item(key, value);
    }
}

async fn store(mode: web_sys::IdbTransactionMode) -> Result<web_sys::IdbObjectStore> {
    let js = |value: JsValue| anyhow!("{value:?}");
    let factory = web_sys::window()
        .ok_or_else(|| anyhow!("no window"))?
        .indexed_db()
        .map_err(js)?
        .ok_or_else(|| anyhow!("This browser has no IndexedDB."))?;
    let open = factory.open_with_u32(DB, 1).map_err(js)?;
    let upgrade = {
        let open = open.clone();
        Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
            if let Ok(db) = open
                .result()
                .and_then(|db| db.dyn_into::<web_sys::IdbDatabase>())
                && !db.object_store_names().contains(STORE)
            {
                let _ = db.create_object_store(STORE);
            }
        })
    };
    open.set_onupgradeneeded(Some(upgrade.as_ref().unchecked_ref()));
    let db: web_sys::IdbDatabase = done(&open).await?.dyn_into().map_err(js)?;
    drop(upgrade);
    db.transaction_with_str_and_mode(STORE, mode)
        .map_err(js)?
        .object_store(STORE)
        .map_err(js)
}

/// The bytes kept under `key`, if any.
pub async fn read(key: &str) -> Result<Option<Vec<u8>>> {
    let store = store(web_sys::IdbTransactionMode::Readonly).await?;
    let request = store
        .get(&JsValue::from_str(key))
        .map_err(|e| anyhow!("{e:?}"))?;
    let found = done(&request).await?;
    Ok(found
        .dyn_ref::<js_sys::Uint8Array>()
        .map(js_sys::Uint8Array::to_vec))
}

pub async fn write(key: &str, bytes: &[u8]) -> Result<()> {
    let store = store(web_sys::IdbTransactionMode::Readwrite).await?;
    let value = js_sys::Uint8Array::from(bytes);
    let request = store
        .put_with_key(&value, &JsValue::from_str(key))
        .map_err(|e| anyhow!("{e:?}"))?;
    done(&request).await?;
    Ok(())
}

/// Tell the page `name` happened, with `detail` (a `CustomEvent` on the
/// window), as the chat and its lazily loaded modules talk.
pub fn signal(name: &str, detail: &str) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let init = web_sys::CustomEventInit::new();
    init.set_detail(&JsValue::from_str(detail));
    if let Ok(event) = web_sys::CustomEvent::new_with_event_init_dict(name, &init) {
        let _ = window.dispatch_event(&event);
    }
}

/// Every `name` event's detail on the window, for the life of the page.
pub fn listen(name: &str) -> futures::channel::mpsc::UnboundedReceiver<String> {
    let (tx, rx) = futures::channel::mpsc::unbounded();
    if let Some(window) = web_sys::window() {
        let listener =
            Closure::<dyn FnMut(web_sys::CustomEvent)>::new(move |event: web_sys::CustomEvent| {
                let _ = tx.unbounded_send(event.detail().as_string().unwrap_or_default());
            });
        let _ = window.add_event_listener_with_callback(name, listener.as_ref().unchecked_ref());
        listener.forget();
    }
    rx
}
