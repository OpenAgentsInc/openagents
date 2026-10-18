//! One owner-scoped CAS record orders saved pins without rewriting chats.
use crate::{
    App,
    chat_store::{Error, Store},
    coder_sync::{answer, refused, stored},
};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Response,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashSet;

const MAX_PINS: usize = 4096;
const KEY: &str = "history/pins.json";
const SCHEMA: &str = "openagents.history.pins.v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Pin {
    id: String,
    /// A repin starts a new placement; an old order cannot revive it.
    stamp: u64,
}
#[derive(Serialize, Deserialize)]
struct Order {
    schema: String,
    revision: u64,
    pins: Vec<Pin>,
}
impl Default for Order {
    fn default() -> Self {
        Self {
            schema: SCHEMA.into(),
            revision: 0,
            pins: vec![],
        }
    }
}
impl Order {
    fn valid(&self) -> bool {
        let mut seen = HashSet::new();
        self.schema == SCHEMA
            && self.pins.len() <= MAX_PINS
            && self
                .pins
                .iter()
                .all(|pin| crate::chat_store::valid_id(&pin.id) && seen.insert(&pin.id))
    }
    /// Keep confirmed placements; new pins lead in the existing timestamp order.
    fn current(&self, active: Vec<Pin>) -> Vec<Pin> {
        let active_ids: HashSet<_> = active
            .iter()
            .map(|pin| (pin.id.clone(), pin.stamp))
            .collect();
        let retained: Vec<_> = self
            .pins
            .iter()
            .filter(|pin| active_ids.contains(&(pin.id.clone(), pin.stamp)))
            .cloned()
            .collect();
        let retained_ids: HashSet<_> = retained
            .iter()
            .map(|pin| (pin.id.clone(), pin.stamp))
            .collect();
        active
            .into_iter()
            .filter(|pin| !retained_ids.contains(&(pin.id.clone(), pin.stamp)))
            .chain(retained)
            .collect()
    }
    fn response(&self) -> serde_json::Value {
        json!({"revision": self.revision, "ids": self.pins.iter().map(|p| &p.id).collect::<Vec<_>>()})
    }
}

async fn read(store: &Store, owner: &str) -> Result<(Order, Option<String>), Error> {
    let key = Store::owner_key(owner, KEY)?;
    let Some((bytes, generation)) = store.read_key(&key).await? else {
        return Ok((Order::default(), None));
    };
    let order: Order = serde_json::from_slice(&bytes)
        .map_err(|_| Error::Invalid("The saved pin order is invalid."))?;
    if order.revision == 0 || !order.valid() {
        return Err(Error::Invalid("The saved pin order is invalid."));
    }
    Ok((order, Some(generation)))
}

async fn active_pins(store: &Store, owner: &str) -> Result<Vec<Pin>, Error> {
    let scan = async {
        let mut after = None;
        let mut pins = Vec::new();
        // Keep only tiny pin records, never whole transcripts or an unbounded scan.
        for _ in 0..50 {
            let (rows, next) = store
                .history_page(owner, after.as_deref(), 200, |chat| {
                    chat.pinned_unix
                        .filter(|_| {
                            chat.archived_unix.is_none() && chat.links.continued_by.is_none()
                        })
                        .map(|stamp| Pin {
                            id: chat.id.clone(),
                            stamp,
                        })
                })
                .await?;
            pins.extend(rows.into_iter().flatten());
            if pins.len() > MAX_PINS {
                return Err(Error::Unavailable("There are too many pinned chats."));
            }
            match next {
                None => {
                    pins.sort_by(|a, b| b.stamp.cmp(&a.stamp).then_with(|| a.id.cmp(&b.id)));
                    return Ok(pins);
                }
                Some(next) => after = Some(next),
            }
        }
        Err(Error::Unavailable(
            "The chat list exceeds its pin-order limit.",
        ))
    };
    tokio::time::timeout(std::time::Duration::from_secs(10), scan)
        .await
        .map_err(|_| Error::Unavailable("The pinned chats could not be loaded. Try again."))?
}

pub(super) async fn get(State(app): State<App>, headers: HeaderMap) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    match current(&app.config.chat_store, &owner).await {
        Ok((order, _)) => answer(StatusCode::OK, order.response()),
        Err(error) => stored(&error),
    }
}
async fn current(store: &Store, owner: &str) -> Result<(Order, Option<String>), Error> {
    let (mut order, generation) = read(store, owner).await?;
    order.pins = order.current(active_pins(store, owner).await?);
    Ok((order, generation))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Move {
    revision: u64,
    id: String,
    after: Option<String>,
    before: Option<String>,
}
/// Neighbours describe one gap after removing the moved pin. Stale gaps fail.
fn move_in(pins: &mut Vec<Pin>, change: &Move) -> bool {
    let Some(index) = pins.iter().position(|pin| pin.id == change.id) else {
        return false;
    };
    let pin = pins.remove(index);
    let at = match (&change.after, &change.before) {
        (None, None) if pins.is_empty() => Some(0),
        (None, Some(before)) if pins.first().is_some_and(|pin| pin.id == *before) => Some(0),
        (Some(after), None) if pins.last().is_some_and(|pin| pin.id == *after) => Some(pins.len()),
        (Some(after), Some(before)) => pins
            .windows(2)
            .position(|pair| pair[0].id == *after && pair[1].id == *before)
            .map(|at| at + 1),
        _ => None,
    };
    let Some(at) = at else {
        pins.insert(index, pin);
        return false;
    };
    pins.insert(at, pin);
    true
}

pub(super) async fn patch(
    State(app): State<App>,
    headers: HeaderMap,
    body: Result<axum::Json<Move>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    let Ok(axum::Json(change)) = body else {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Choose where to move the pinned chat.",
        );
    };
    if !crate::chat_store::valid_id(&change.id)
        || change
            .after
            .iter()
            .chain(change.before.iter())
            .any(|id| !crate::chat_store::valid_id(id))
    {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Choose a pinned chat.",
        );
    }
    let store = &app.config.chat_store;
    let (mut order, generation) = match current(store, &owner).await {
        Ok(found) => found,
        Err(error) => return stored(&error),
    };
    if order.revision != change.revision || !move_in(&mut order.pins, &change) {
        return refused(
            StatusCode::CONFLICT,
            "pin_order_changed",
            "Your pinned chats changed. Refresh and try again.",
        );
    }
    let Some(revision) = order.revision.checked_add(1) else {
        return refused(
            StatusCode::CONFLICT,
            "pin_order_changed",
            "Refresh your pinned chats.",
        );
    };
    order.revision = revision;
    let key = match Store::owner_key(&owner, KEY) {
        Ok(key) => key,
        Err(error) => return stored(&error),
    };
    let bytes = match serde_json::to_vec(&order) {
        Ok(bytes) => bytes,
        Err(_) => return stored(&Error::Invalid("The saved pin order is invalid.")),
    };
    match store.write_key(&key, bytes, generation.as_deref()).await {
        Ok(_) => answer(StatusCode::OK, order.response()),
        Err(Error::Conflict) => refused(
            StatusCode::CONFLICT,
            "pin_order_changed",
            "Your pinned chats changed. Refresh and try again.",
        ),
        Err(error) => stored(&error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pin(n: u8) -> Pin {
        Pin {
            id: format!("{n:08x}-1234-4234-8234-123456789abc"),
            stamp: n.into(),
        }
    }
    #[test]
    fn gaps_are_exact_and_a_failed_move_leaves_the_order_unchanged() {
        let mut pins = vec![pin(1), pin(2), pin(3)];
        assert!(move_in(
            &mut pins,
            &Move {
                revision: 0,
                id: pin(3).id,
                after: None,
                before: Some(pin(1).id)
            }
        ));
        assert_eq!(pins, [pin(3), pin(1), pin(2)]);
        let before = pins.clone();
        assert!(!move_in(
            &mut pins,
            &Move {
                revision: 0,
                id: pin(3).id,
                after: Some(pin(2).id),
                before: Some(pin(1).id)
            }
        ));
        assert_eq!(pins, before);
    }
    #[test]
    fn deleted_archived_unpinned_and_repinned_rows_do_not_reuse_a_placement() {
        let order = Order {
            pins: vec![pin(2), pin(1)],
            ..Default::default()
        };
        let repin = Pin {
            stamp: 10,
            ..pin(1)
        };
        assert_eq!(
            order.current(vec![repin.clone(), pin(3), pin(2)]),
            [repin, pin(3), pin(2)]
        );
        assert!(order.current(vec![]).is_empty());
    }
}
