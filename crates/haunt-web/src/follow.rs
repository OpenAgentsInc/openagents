//! Following a house document at an address (the website's Game Lab copy,
//! `/labs/game/haunt/house`): `{"version": N, "house": {...}}`. The page
//! asks again every second and applies a new version between two frames,
//! so a change made in the chat shows in the open house without a reload.

use std::cell::RefCell;
use std::rc::Rc;

use haunt_rules::House;
use live_web::{fetch_text, now};
use wasm_bindgen_futures::spawn_local;

/// How often the page asks for a new version, ms.
const EVERY_MS: f64 = 1000.0;

/// A version of the house, as the address gave it.
pub struct Fetched {
    pub version: u64,
    pub house: House,
    /// When the request that found it started, ms.
    pub asked_at: f64,
}

async fn fetch(url: &str) -> Result<Fetched, String> {
    let asked_at = now();
    let text = fetch_text(url).await?;
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("the house doesn't read: {e}"))?;
    let version = value["version"]
        .as_u64()
        .ok_or("the house has no version")?;
    let house = House::parse(&value["house"].to_string())?;
    Ok(Fetched {
        version,
        house,
        asked_at,
    })
}

/// The house at `url` when it answers, else none (signed out, or no
/// server).
pub async fn first(url: &str) -> Option<Fetched> {
    fetch(url).await.ok()
}

/// The page's follower.
pub struct Follow {
    url: String,
    pub version: u64,
    /// Whether to keep asking (no `?v=` pinning a version).
    pub follows: bool,
    next: f64,
    inbox: Rc<RefCell<Option<Fetched>>>,
    busy: Rc<RefCell<bool>>,
}

impl Follow {
    #[must_use]
    pub fn new(url: String, version: u64) -> Self {
        let follows = !url.contains("v=");
        Self {
            url,
            version,
            follows,
            next: now() + EVERY_MS,
            inbox: Rc::default(),
            busy: Rc::default(),
        }
    }

    /// Asks again when it's time; a newer version waiting, if one came.
    pub fn pump(&mut self) -> Option<Fetched> {
        if let Some(found) = self.inbox.borrow_mut().take()
            && found.version != self.version
        {
            self.version = found.version;
            return Some(found);
        }
        let t = now();
        if self.follows && t >= self.next && !*self.busy.borrow() {
            self.next = t + EVERY_MS;
            *self.busy.borrow_mut() = true;
            let (url, inbox, busy) = (self.url.clone(), self.inbox.clone(), self.busy.clone());
            spawn_local(async move {
                if let Ok(found) = fetch(&url).await {
                    *inbox.borrow_mut() = Some(found);
                }
                *busy.borrow_mut() = false;
            });
        }
        None
    }
}
