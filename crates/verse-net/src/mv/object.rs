//! A world-tree object's dynamic state as a NIP-MV entity state (`33301`)
//! with role `object` (`nips/openagents/NIP-MV.md`, "Object state"). Every
//! device derives the same world tree from the pinned layout, so only
//! what changes is shared: a hosted world authority publishes a lamp, a
//! door, a workstation, the Task Wall, a pylon, or the Wellspring when its
//! state changes (`nips/openagents/NIP-PYLON.md`, World projection). The
//! content is an ordinary entity state, which any NIP-MV client reads,
//! plus the node's ID, the tree's digest, and the state.

use nostr::domain::{Event, RelaySigner};
use serde::{Deserialize, Serialize};
use world_tree::State as ObjectState;
use world_tree::state::entity_id;

use super::{Received, STATE_KIND, cell_tags, decode, state_address, tag};
use glam::Vec3;

/// The role of a world-tree object.
pub const OBJECT_ROLE: &str = "object";

/// Content of an object's entity state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Object {
    /// Content version, 1.
    pub v: u32,
    /// The entity ID: [`entity_id`] of `node`.
    pub id: String,
    /// Always [`OBJECT_ROLE`].
    pub role: String,
    /// The node's standing point, with the ground's height.
    pub p: [f32; 3],
    pub q: [f32; 4],
    /// Publisher time in milliseconds.
    pub t: u64,
    pub online: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The world-tree node's ID.
    pub node: String,
    /// The digest of the tree the node is in.
    pub tree: String,
    pub state: ObjectState,
}

impl Object {
    /// Node `node`'s `state` at `p`, at `t` ms.
    #[must_use]
    pub fn new(node: &str, tree: &str, state: ObjectState, p: [f32; 3], t: u64) -> Self {
        Self {
            v: 1,
            id: entity_id(node),
            role: OBJECT_ROLE.into(),
            p,
            q: [0.0, 0.0, 0.0, 1.0],
            t,
            online: true,
            name: None,
            node: node.into(),
            tree: tree.into(),
            state,
        }
    }
}

/// Signs an object's state.
///
/// # Panics
///
/// Never in practice: serializing owned plain data cannot fail.
#[must_use]
pub fn object_event(signer: &RelaySigner, world: &str, object: &Object, now: u64) -> Event {
    let mut tags = vec![
        tag(&["d", &state_address(world, &object.id)]),
        tag(&["w", world]),
        tag(&["role", OBJECT_ROLE]),
    ];
    tags.extend(cell_tags(std::iter::once(Vec3::from(object.p))));
    let content = serde_json::to_string(object).expect("an object serializes");
    signer.sign(now, STATE_KIND, tags, content)
}

/// Reads an object's state from `event`, after the checks every entity
/// state passes ([`decode`]): its publisher and the object.
///
/// # Errors
///
/// When the event isn't a valid entity state with role `object`, its
/// content isn't an object's, or its entity ID isn't its node's.
pub fn decode_object(event: &Event, world: &str) -> Result<(String, Object), String> {
    let pubkey = match decode(event, world)? {
        Received::State { pubkey, state } if state.role == OBJECT_ROLE => pubkey,
        _ => return Err("not an object's entity state".into()),
    };
    let object: Object = serde_json::from_str(&event.content).map_err(|e| e.to_string())?;
    if object.id != entity_id(&object.node) {
        return Err("the entity ID isn't its node's".into());
    }
    if object.node.is_empty() || object.node.len() > 512 || object.tree.len() > 80 {
        return Err("the node ID or the tree digest is malformed".into());
    }
    Ok((pubkey, object))
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORLD: &str = "everglade";
    const NODE: &str = "everglade/knowledge-district/owners-house/great-room/workstation";

    fn signer() -> RelaySigner {
        RelaySigner::from_secret_hex(&"22".repeat(32)).expect("a valid key")
    }

    #[test]
    fn an_object_state_round_trips_and_reads_as_an_entity_state() {
        let state = ObjectState::Workstation {
            busy: true,
            by: Some("alice".into()),
        };
        let object = Object::new(NODE, "sha256:ab", state, [100.0, 1.6, -24.4], 5);
        assert!(object.id.starts_with("obj-") && object.id.len() == 24);
        let event = object_event(&signer(), WORLD, &object, 1_790_000_000);
        assert_eq!(event.kind, STATE_KIND);
        assert_eq!(event.tag_values("role").collect::<Vec<_>>(), ["object"]);
        assert_eq!(
            event.tag_values("d").collect::<Vec<_>>(),
            [format!("everglade/{}", object.id)]
        );
        let (pubkey, got) = decode_object(&event, WORLD).unwrap();
        assert_eq!(pubkey, signer().pubkey());
        assert_eq!(got, object);
        // A plain NIP-MV client reads it as an ordinary entity state.
        match decode(&event, WORLD) {
            Ok(Received::State { state, .. }) => {
                assert_eq!(state.role, "object");
                assert_eq!(state.p, [100.0, 1.6, -24.4]);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn an_object_whose_id_is_not_its_nodes_is_refused() {
        let mut object = Object::new(
            NODE,
            "sha256:ab",
            ObjectState::Lamp { lit: true },
            [0.0; 3],
            1,
        );
        object.node = "everglade/elsewhere".into();
        let event = object_event(&signer(), WORLD, &object, 1_790_000_000);
        assert!(decode_object(&event, WORLD).unwrap_err().contains("node"));
        let other = object_event(
            &signer(),
            WORLD,
            &Object::new(
                NODE,
                "sha256:ab",
                ObjectState::Door { open: false },
                [0.0; 3],
                1,
            ),
            1_790_000_000,
        );
        assert!(decode_object(&other, "another-world").is_err());
    }

    #[test]
    fn a_pylon_and_the_wellspring_round_trip_as_object_states() {
        use world_tree::{Family, PylonStatus, Tier};
        let field = "everglade/wilds/pylon-field";
        for (node, state) in [
            (
                format!("{field}/pylon-1"),
                ObjectState::Pylon {
                    pylon: "local:this-computer".into(),
                    status: PylonStatus::Unknown,
                    family: Family::Cpu,
                    tier: Tier::Small,
                    busy: 0,
                    total: 1,
                    jobs: 0,
                    paid_msat: std::collections::BTreeMap::new(),
                    uptime: None,
                },
            ),
            (
                format!("{field}/wellspring"),
                ObjectState::Wellspring {
                    pool: "local".into(),
                    online: 0,
                    busy: 0,
                    total: 0,
                    rate: 0,
                    verified: false,
                },
            ),
        ] {
            let object = Object::new(&node, "sha256:ab", state, [0.0, 1.0, 140.0], 9);
            let event = object_event(&signer(), WORLD, &object, 1_790_000_000);
            let (_, got) = decode_object(&event, WORLD).unwrap();
            assert_eq!(got, object);
        }
    }
}
