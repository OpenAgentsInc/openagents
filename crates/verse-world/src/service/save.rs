//! Content-bound world and enrolled-character recovery without saved sessions.
use super::{Chamber, Principal, Rights, auth::Gateway};
use crate::play::Game;
use secp256k1::XOnlyPublicKey;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_BYTES: usize = 8 * 1024 * 1024;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Grant {
    key: [u8; 32],
    actor: Option<u64>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Saved {
    version: u32,
    content: [u8; 32],
    world: String,
    grants: Vec<Grant>,
    #[serde(default)]
    rewards: Option<Vec<super::rewards::Transaction>>,
    #[serde(default)]
    reward_policy: Vec<super::rewards::Policy>,
    #[serde(default)]
    reward_cursor: u64,
    #[serde(default)]
    progression: Option<super::progression::Config>,
    #[serde(default)]
    items: Option<super::items::Catalog>,
    #[serde(default)]
    outfits: Option<super::outfits::Catalog>,
}
fn grants(saved: &[Grant], game: &Game) -> Result<BTreeMap<Principal, Rights>, String> {
    if saved.len() > 128 {
        return Err("Saved chamber grant budget exceeded".into());
    }
    let mut result = BTreeMap::new();
    let mut owned = BTreeSet::new();
    for grant in saved {
        XOnlyPublicKey::from_byte_array(grant.key)
            .map_err(|_| "Saved chamber public key is invalid")?;
        let right = match grant.actor {
            Some(actor) => {
                if game.player_admission(actor).is_none() || !owned.insert(actor) {
                    return Err("Saved adventurer ownership is missing or duplicated".into());
                }
                Rights::Player(actor)
            }
            None => Rights::Spectator,
        };
        if result.insert(Principal(grant.key), right).is_some() {
            return Err("Saved chamber principal is duplicated".into());
        }
    }
    Ok(result)
}
pub(super) fn encode(gateway: &Gateway) -> Result<Vec<u8>, String> {
    let saved = Saved {
        version: 5,
        content: gateway
            .content()
            .ok_or("Saved chamber requires bound content")?,
        world: String::from_utf8(gateway.game().checkpoint()?)
            .map_err(|_| "Cannot encode saved world")?,
        rewards: Some(gateway.chamber.rewards.transactions()),
        reward_policy: gateway.chamber.reward_policy.clone(),
        reward_cursor: gateway.chamber.reward_cursor,
        progression: Some(gateway.chamber.progression.clone()),
        items: Some(gateway.chamber.items.clone()),
        outfits: Some(gateway.chamber.outfits.clone()),
        grants: gateway
            .chamber
            .grants
            .iter()
            .map(|(principal, rights)| Grant {
                key: principal.0,
                actor: match rights {
                    Rights::Player(actor) => Some(*actor),
                    Rights::Spectator => None,
                },
            })
            .collect(),
    };
    grants(&saved.grants, gateway.game())?;
    let bytes = serde_json::to_vec(&saved).map_err(|_| "Cannot encode saved chamber")?;
    if bytes.len() > MAX_BYTES {
        return Err("Saved chamber byte budget exceeded".into());
    }
    Ok(bytes)
}
pub(super) fn decode(bytes: &[u8], content: [u8; 32], instance: u64) -> Result<Gateway, String> {
    if bytes.is_empty() || bytes.len() > MAX_BYTES {
        return Err("Saved chamber byte budget exceeded".into());
    }
    let saved: Saved = serde_json::from_slice(bytes).map_err(|_| "Invalid saved chamber")?;
    if !matches!(saved.version, 1 | 2 | 3 | 4 | 5)
        || (saved.version == 1 && saved.rewards.is_some())
        || (saved.version >= 2 && saved.rewards.is_none())
        || (saved.version < 3 && saved.progression.is_some())
        || (saved.version >= 3 && saved.progression.is_none())
        || (saved.version < 4 && saved.items.is_some())
        || (saved.version >= 4 && saved.items.is_none())
        || (saved.version < 5 && saved.outfits.is_some())
        || (saved.version == 5 && saved.outfits.is_none())
        || saved.content != content
        || instance == 0
    {
        return Err("Saved chamber version or content is incompatible".into());
    }
    let game = Game::restore(saved.world.as_bytes())?;
    if game.player_life().instance != instance {
        return Err("Saved chamber instance is incompatible".into());
    }
    let grants = grants(&saved.grants, &game)?;
    let mut chamber = Chamber::new(game)?;
    chamber.grants = grants;
    chamber.progression = saved.progression.unwrap_or_default();
    chamber.progression.validate()?;
    chamber.items = saved.items.unwrap_or_default();
    chamber.outfits = saved.outfits.unwrap_or_default();
    chamber.outfits.validate_items(&chamber.items)?;
    for (index, transaction) in saved.rewards.unwrap_or_default().into_iter().enumerate() {
        if saved.version < 5 && transaction.outfit.is_some() {
            return Err("Legacy save cannot contain outfit changes".into());
        }
        if saved.version < 4 && !transaction.spent.is_empty() {
            return Err("Legacy save cannot contain item debits".into());
        }
        if chamber.restore_reward(transaction)?.revision != index as u64 + 1 {
            return Err("Saved reward transaction is duplicated".into());
        }
    }
    if saved.version == 1 && (!saved.reward_policy.is_empty() || saved.reward_cursor != 0) {
        return Err("Legacy chamber cannot contain combat reward policy".into());
    }
    chamber.configure_rewards(saved.reward_policy)?;
    if (chamber.reward_policy.is_empty() && saved.reward_cursor != 0)
        || saved.reward_cursor > chamber.reward_cursor
        || (!chamber.reward_policy.is_empty()
            && chamber
                .game
                .events
                .first()
                .is_some_and(|e| saved.reward_cursor.saturating_add(1) < e.serial))
    {
        return Err("Saved combat reward cursor is incompatible".into());
    }
    chamber.reward_cursor = saved.reward_cursor;
    Gateway::new(chamber)?.with_content(content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Controller, Intent, play::Ability};
    use glam::Vec3;
    use secp256k1::{Keypair, Secp256k1, SecretKey};
    use verse_engine::director::Scene;
    fn key(n: u8) -> Keypair {
        Keypair::from_secret_key(
            &Secp256k1::new(),
            &SecretKey::from_byte_array([n; 32]).unwrap(),
        )
    }
    fn public(k: &Keypair) -> [u8; 32] {
        k.x_only_public_key().0.serialize()
    }
    fn join(g: &mut Gateway, k: &Keypair) -> super::super::auth::ConnectionId {
        let (id, challenge) = g.open(0).unwrap();
        let sig =
            Secp256k1::new().sign_schnorr_no_aux_rand(&challenge.signing_digest(public(k)), k);
        g.authenticate(id, 0, public(k), sig.to_byte_array())
            .unwrap();
        id
    }
    fn fixture() -> (Gateway, [Keypair; 3]) {
        let scene = Scene::from_json(include_bytes!(
            "../../../../assets/verse/original/ritual.json"
        ))
        .unwrap();
        let mut game = Game::combat_in(scene, false, 240).unwrap();
        game.time = game.scene.cut_at;
        game.tick(0., [0.; 2]).unwrap();
        game.encounter
            .as_mut()
            .unwrap()
            .postpone_casts_until(600.)
            .unwrap();
        let mut g = Gateway::new(Chamber::new(game).unwrap())
            .unwrap()
            .with_content([6; 32])
            .unwrap();
        let keys = [key(91), key(92), key(93)];
        g.enroll_primary(public(&keys[0])).unwrap();
        g.enroll_player(public(&keys[1]), Vec3::new(3., 0., -22.))
            .unwrap();
        g.enroll_spectator(public(&keys[2])).unwrap();
        (g, keys)
    }
    #[test]
    fn recovery_preserves_characters_combat_and_grants_but_fences_connections() {
        let (mut original, keys) = fixture();
        let a = join(&mut original, &keys[0]);
        let b = join(&mut original, &keys[1]);
        let spectator = join(&mut original, &keys[2]);
        let old_a = original.admission(a).unwrap();
        let old_b = original.admission(b).unwrap();
        original
            .submit(
                a,
                old_a
                    .command(
                        original.game().authority_tick,
                        Intent::Cast {
                            ability: Ability::Shield,
                            target: None,
                            aim: [0., 0., 1.],
                        },
                    )
                    .unwrap(),
            )
            .unwrap();
        let corpse = original.game().actor_life(2).unwrap();
        let source = original.chamber.game.ids[&2];
        original
            .chamber
            .game
            .simulation
            .bow_impact(source, 1000)
            .unwrap();
        for _ in 0..90 {
            original.tick(1. / 30.).unwrap();
        }
        let cast = original
            .admission(b)
            .unwrap()
            .command(
                original.game().authority_tick,
                Intent::Cast {
                    ability: Ability::Fireball,
                    target: original.game().actor_life(1),
                    aim: [0., 0., 1.],
                },
            )
            .unwrap();
        original.submit(b, cast.clone()).unwrap();
        let before_a = serde_json::to_value(original.snapshot(a).unwrap()).unwrap();
        let before_b = serde_json::to_value(original.snapshot(b).unwrap()).unwrap();
        let (pending, challenge) = original.open(0).unwrap();
        let signature = Secp256k1::new()
            .sign_schnorr_no_aux_rand(&challenge.signing_digest(public(&keys[0])), &keys[0]);
        let bytes = original.checkpoint().unwrap();
        let mut recovered = Gateway::restore(&bytes, [6; 32], 240).unwrap();
        assert_eq!(recovered.game().controlled_effects().count(), 2);
        assert!(
            recovered
                .authenticate(pending, 0, public(&keys[0]), signature.to_byte_array())
                .is_err()
        );
        assert!(recovered.snapshot(a).is_err());
        assert!(recovered.snapshot(b).is_err());
        assert!(recovered.snapshot(spectator).is_err());
        for (_, rights) in &recovered.chamber.grants {
            if let Rights::Player(actor) = rights {
                let admission = recovered.game().player_admission(*actor).unwrap();
                assert_eq!(admission.controller(), Controller(0));
            }
        }
        let new_a = join(&mut recovered, &keys[0]);
        let new_b = join(&mut recovered, &keys[1]);
        let new_s = join(&mut recovered, &keys[2]);
        assert_eq!(recovered.admission(new_a).unwrap().actor(), old_a.actor());
        assert_eq!(recovered.admission(new_b).unwrap().actor(), old_b.actor());
        assert!(recovered.admission(new_a).unwrap().epoch() > old_a.epoch());
        assert!(recovered.admission(new_b).unwrap().epoch() > old_b.epoch());
        assert_eq!(
            serde_json::to_value(recovered.snapshot(new_a).unwrap()).unwrap(),
            before_a
        );
        assert_eq!(
            serde_json::to_value(recovered.snapshot(new_b).unwrap()).unwrap(),
            before_b
        );
        assert!(recovered.admission(new_s).is_err());
        assert!(recovered.submit(new_b, cast).is_err());
        assert!(
            recovered
                .game()
                .physics_bodies()
                .get(physics::queries::Life {
                    instance: corpse.instance,
                    entity: corpse.actor,
                    generation: corpse.generation,
                })
                .is_some()
        );
        let left = original.game().player_hud(old_b.actor()).unwrap();
        let right = recovered.game().player_hud(old_b.actor()).unwrap();
        assert_eq!(
            serde_json::to_value(left.casting).unwrap(),
            serde_json::to_value(right.casting).unwrap()
        );
        for _ in 0..60 {
            original.tick(1. / 30.).unwrap();
            recovered.tick(1. / 30.).unwrap();
        }
        assert_eq!(
            serde_json::to_value(original.snapshot(a).unwrap()).unwrap(),
            serde_json::to_value(recovered.snapshot(new_a).unwrap()).unwrap()
        );
    }
    #[test]
    fn campaign_claims_are_owned_once_and_validated_on_recovery() {
        use super::super::{
            progression::{Config, Quest},
            rewards::{Entry, Transaction},
        };
        let (g, keys) = fixture();
        let config = Config {
            version: 1,
            levels: vec![0, 100, 300],
            quests: vec![Quest {
                id: 1,
                name: "Disrupt the summoning".into(),
                objective: 1,
                goal: 2,
                experience: 75,
                items: vec![Entry { id: 1, count: 2 }],
            }],
        };
        let mut g = g.with_progression(config.clone()).unwrap();
        let a = join(&mut g, &keys[0]);
        let b = join(&mut g, &keys[1]);
        let spectator = join(&mut g, &keys[2]);
        let own = g.admission(a).unwrap();
        let other = g.admission(b).unwrap();
        assert!(g.claim_quest(a, own.actor(), own.epoch(), 1).is_err());
        for actor in [own.actor().actor, other.actor().actor] {
            g.grant_reward(Transaction {
                outfit: None,
                spent: vec![],
                instance: 240,
                actor,
                source: [5; 32],
                experience: 45,
                items: vec![Entry { id: 1, count: 1 }],
                quests: vec![Entry { id: 1, count: 2 }],
            })
            .unwrap();
        }
        assert!(g.claim_quest(b, own.actor(), own.epoch(), 1).is_err());
        assert!(
            g.claim_quest(spectator, own.actor(), own.epoch(), 1)
                .is_err()
        );
        assert!(g.claim_quest(a, own.actor(), own.epoch() + 1, 1).is_err());
        let receipt = g.claim_quest(a, own.actor(), own.epoch(), 1).unwrap();
        assert_eq!(
            g.claim_quest(a, own.actor(), own.epoch(), 1).unwrap(),
            receipt
        );
        assert_eq!(
            g.character_rewards(own.actor().actor).unwrap().experience,
            120
        );
        assert_eq!(g.progression().level(120).unwrap().level, 2);
        assert!(g.quest_log(own.actor().actor)[0].claimed);
        assert!(
            g.grant_reward(config.quests[0].transaction(240, own.actor().actor))
                .is_err()
        );
        g.grant_reward(Transaction {
            outfit: None,
            spent: vec![],
            instance: 240,
            actor: other.actor().actor,
            source: [9; 32],
            experience: u64::MAX - 45,
            items: vec![],
            quests: vec![],
        })
        .unwrap();
        let before = g.checkpoint().unwrap();
        assert!(g.claim_quest(b, other.actor(), other.epoch(), 1).is_err());
        assert_eq!(g.checkpoint().unwrap(), before);
        let saved = g.checkpoint().unwrap();
        let mut recovered = Gateway::restore(&saved, [6; 32], 240).unwrap();
        let connection = join(&mut recovered, &keys[0]);
        let admission = recovered.admission(connection).unwrap();
        assert_eq!(
            recovered
                .claim_quest(connection, admission.actor(), admission.epoch(), 1)
                .unwrap(),
            receipt
        );
        recovered.reset().unwrap();
        let admission = recovered.admission(connection).unwrap();
        assert_eq!(
            recovered
                .claim_quest(connection, admission.actor(), admission.epoch(), 1)
                .unwrap(),
            receipt
        );
        assert_eq!(
            recovered
                .character_rewards(admission.actor().actor)
                .unwrap()
                .items[&1],
            3
        );
        for case in 0..4 {
            let mut value: serde_json::Value = serde_json::from_slice(&saved).unwrap();
            match case {
                0 => value["progression"]["quests"][0]["experience"] = 76.into(),
                1 => value["rewards"][2]["experience"] = 1000.into(),
                2 => {
                    let claim = value["rewards"].as_array_mut().unwrap().remove(2);
                    value["rewards"].as_array_mut().unwrap().insert(0, claim);
                }
                _ => {
                    value.as_object_mut().unwrap().remove("progression");
                }
            }
            assert!(Gateway::restore(&serde_json::to_vec(&value).unwrap(), [6; 32], 240).is_err());
        }
        let mut legacy: serde_json::Value = serde_json::from_slice(&saved).unwrap();
        legacy["version"] = 2.into();
        legacy.as_object_mut().unwrap().remove("items");
        legacy.as_object_mut().unwrap().remove("outfits");
        legacy.as_object_mut().unwrap().remove("progression");
        legacy["rewards"].as_array_mut().unwrap().truncate(2);
        let legacy = Gateway::restore(&serde_json::to_vec(&legacy).unwrap(), [6; 32], 240).unwrap();
        assert_eq!(
            legacy
                .character_rewards(own.actor().actor)
                .unwrap()
                .experience,
            45
        );
        assert!(legacy.quest_log(own.actor().actor).is_empty());
    }
    #[test]
    fn rewards_survive_recovery_and_legacy_saves_upgrade_without_grants() {
        use super::super::rewards::{Entry, Transaction};
        let (mut g, _) = fixture();
        let actor = g.game().player_life().actor;
        let tx = Transaction {
            outfit: None,
            spent: vec![],
            instance: 240,
            actor,
            source: [8; 32],
            experience: 90,
            items: vec![Entry { id: 1, count: 2 }],
            quests: vec![Entry { id: 3, count: 1 }],
        };
        let receipt = g.grant_reward(tx.clone()).unwrap();
        let bytes = g.checkpoint().unwrap();
        let mut recovered = Gateway::restore(&bytes, [6; 32], 240).unwrap();
        assert_eq!(recovered.grant_reward(tx.clone()).unwrap(), receipt);
        assert_eq!(
            recovered.character_rewards(actor),
            g.character_rewards(actor)
        );
        let mut foreign = tx.clone();
        foreign.actor = 9999;
        assert!(recovered.grant_reward(foreign).is_err());
        let mut foreign = tx.clone();
        foreign.instance = 241;
        assert!(recovered.grant_reward(foreign).is_err());
        recovered.reset().unwrap();
        assert_eq!(recovered.grant_reward(tx).unwrap(), receipt);
        for case in 0..4 {
            let mut saved: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            match case {
                0 => {
                    let tx = saved["rewards"][0].clone();
                    saved["rewards"].as_array_mut().unwrap().push(tx);
                }
                1 => saved["rewards"][0]["actor"] = 9999.into(),
                2 => saved["rewards"][0]["instance"] = 241.into(),
                _ => {
                    saved.as_object_mut().unwrap().remove("rewards");
                }
            }
            assert!(Gateway::restore(&serde_json::to_vec(&saved).unwrap(), [6; 32], 240).is_err());
        }
        let mut legacy: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        legacy["version"] = 1.into();
        legacy.as_object_mut().unwrap().remove("items");
        legacy.as_object_mut().unwrap().remove("outfits");
        legacy.as_object_mut().unwrap().remove("rewards");
        legacy.as_object_mut().unwrap().remove("progression");
        let upgraded =
            Gateway::restore(&serde_json::to_vec(&legacy).unwrap(), [6; 32], 240).unwrap();
        assert!(upgraded.character_rewards(actor).is_none());
        assert_eq!(upgraded.game().player_life(), g.game().player_life());
        let saved: serde_json::Value =
            serde_json::from_slice(&upgraded.checkpoint().unwrap()).unwrap();
        assert_eq!(saved["version"], 5);
    }
    #[test]
    fn recovery_items_spend_once_restore_only_owned_resources_and_validate_saved_debits() {
        use super::super::{
            items::{Catalog, Item},
            rewards::{Entry, Transaction},
        };
        let (g, keys) = fixture();
        let mut g = g
            .with_items(Catalog {
                version: 1,
                items: vec![
                    Item {
                        id: 1,
                        name: "Recovery ember".into(),
                        health: 45,
                        mana: 5,
                    },
                    Item {
                        id: 2,
                        name: "Mana draught".into(),
                        health: 0,
                        mana: 10,
                    },
                ],
            })
            .unwrap();
        let actor = g.game().player_life().actor;
        g.grant_reward(Transaction {
            outfit: None,
            instance: 240,
            actor,
            source: [1; 32],
            experience: 1,
            items: vec![Entry { id: 1, count: 2 }, Entry { id: 2, count: 1 }],
            quests: vec![],
            spent: vec![],
        })
        .unwrap();
        let a = join(&mut g, &keys[0]);
        let b = join(&mut g, &keys[1]);
        let spectator = join(&mut g, &keys[2]);
        let own = g.admission(a).unwrap();
        let life = own.actor();
        let epoch = own.epoch();
        let before = g.checkpoint().unwrap();
        for (connection, context, control, item, operation) in [
            (a, life, epoch, 1, [0; 16]),
            (a, life, epoch, 3, [1; 16]),
            (a, life, epoch + 1, 1, [1; 16]),
            (b, life, epoch, 1, [1; 16]),
            (spectator, life, epoch, 1, [1; 16]),
            (a, life, epoch, 1, [1; 16]),
        ] {
            assert!(
                g.use_item(connection, context, control, item, operation)
                    .is_err()
            );
            assert_eq!(g.checkpoint().unwrap(), before);
        }
        g.chamber.game.simulation.player_damage_for(0, 100).unwrap();
        g.chamber.game.simulation.spend_mana_for(0, 10).unwrap();
        let secondary = g
            .game()
            .player_snapshot(g.admission(b).unwrap().actor())
            .unwrap();
        let first = g.use_item(a, life, epoch, 1, [1; 16]).unwrap();
        assert_eq!(first.revision, 2);
        assert_eq!(g.game().snapshot().player.hp, 145);
        assert_eq!(g.game().snapshot().player.mana, 15);
        assert_eq!(
            serde_json::to_value(
                g.game()
                    .player_snapshot(g.admission(b).unwrap().actor())
                    .unwrap()
                    .player
            )
            .unwrap(),
            serde_json::to_value(secondary.player).unwrap()
        );
        assert_eq!(g.character_rewards(actor).unwrap().items[&1], 1);
        let after = g.checkpoint().unwrap();
        assert_eq!(g.use_item(a, life, epoch, 1, [1; 16]).unwrap(), first);
        assert!(g.use_item(a, life, epoch, 2, [1; 16]).is_err());
        assert_eq!(g.checkpoint().unwrap(), after);
        let mut recovered = Gateway::restore(&after, [6; 32], 240).unwrap();
        let session = join(&mut recovered, &keys[0]);
        let own = recovered.admission(session).unwrap();
        assert_eq!(
            recovered
                .use_item(session, own.actor(), own.epoch(), 1, [1; 16])
                .unwrap(),
            first
        );
        assert_eq!(recovered.game().snapshot().player.hp, 145);
        recovered
            .use_item(session, own.actor(), own.epoch(), 1, [2; 16])
            .unwrap();
        assert_eq!(recovered.game().snapshot().player.hp, 190);
        assert_eq!(recovered.game().snapshot().player.mana, 20);
        let after = recovered.checkpoint().unwrap();
        assert!(
            recovered
                .use_item(session, own.actor(), own.epoch(), 1, [3; 16])
                .is_err()
        );
        assert!(
            recovered
                .use_item(session, own.actor(), own.epoch(), 2, [3; 16])
                .is_err()
        );
        assert_eq!(recovered.checkpoint().unwrap(), after);
        recovered
            .chamber
            .game
            .simulation
            .player_damage_for(0, 1000)
            .unwrap();
        assert!(
            recovered
                .use_item(session, own.actor(), own.epoch(), 2, [3; 16])
                .is_err()
        );
        // Saved world already includes restoration; replay validates only the debit.
        for case in 0..5 {
            let mut bad: serde_json::Value = serde_json::from_slice(&after).unwrap();
            match case {
                0 => bad["rewards"][1]["spent"][0]["count"] = 2.into(),
                1 => bad["rewards"][1]["experience"] = 9.into(),
                2 => bad["items"]["items"] = serde_json::json!([]),
                3 => {
                    bad["rewards"].as_array_mut().unwrap().swap(0, 1);
                }
                _ => {
                    bad["version"] = 3.into();
                    bad.as_object_mut().unwrap().remove("items");
                    bad.as_object_mut().unwrap().remove("outfits");
                }
            }
            assert!(Gateway::restore(&serde_json::to_vec(&bad).unwrap(), [6; 32], 240).is_err());
        }
    }
    #[test]
    fn outfit_ownership_retries_and_recovery_preserve_character_identity() {
        use super::super::{
            outfits::{Catalog, Outfit},
            rewards::{Entry, Transaction},
        };
        let (g, keys) = fixture();
        let mut g = g
            .with_outfits(Catalog {
                version: 1,
                outfits: vec![
                    Outfit {
                        id: 2,
                        name: "Ranger outfit".into(),
                        model: "universal-male-ranger".into(),
                    },
                    Outfit {
                        id: 3,
                        name: "Peasant outfit".into(),
                        model: "universal-male-peasant".into(),
                    },
                ],
            })
            .unwrap();
        let actor = g.game().player_life().actor;
        g.grant_reward(Transaction {
            instance: 240,
            actor,
            source: [4; 32],
            experience: 1,
            items: vec![Entry { id: 2, count: 1 }],
            quests: vec![],
            spent: vec![],
            outfit: None,
        })
        .unwrap();
        let a = join(&mut g, &keys[0]);
        let b = join(&mut g, &keys[1]);
        let spectator = join(&mut g, &keys[2]);
        let own = g.admission(a).unwrap();
        let life = own.actor();
        let epoch = own.epoch();
        let before = g.checkpoint().unwrap();
        for (connection, control, item, operation) in [
            (a, epoch, 2, [0; 16]),
            (a, epoch, 3, [1; 16]),
            (a, epoch, 4, [1; 16]),
            (a, epoch + 1, 2, [1; 16]),
            (b, epoch, 2, [1; 16]),
            (spectator, epoch, 2, [1; 16]),
        ] {
            assert!(
                g.equip_outfit(connection, life, control, item, operation)
                    .is_err()
            );
            assert_eq!(g.checkpoint().unwrap(), before);
        }
        let world = g.game().checkpoint().unwrap();
        let first = g.equip_outfit(a, life, epoch, 2, [1; 16]).unwrap();
        assert_eq!(first.revision, 2);
        assert_eq!(g.character_rewards(actor).unwrap().outfit, 2);
        assert_eq!(g.character_rewards(actor).unwrap().items[&2], 1);
        assert_eq!(g.game().checkpoint().unwrap(), world);
        assert_eq!(g.equip_outfit(a, life, epoch, 2, [1; 16]).unwrap(), first);
        assert!(g.equip_outfit(a, life, epoch, 0, [1; 16]).is_err());
        let after = g.checkpoint().unwrap();
        let mut recovered = Gateway::restore(&after, [6; 32], 240).unwrap();
        let session = join(&mut recovered, &keys[0]);
        let own = recovered.admission(session).unwrap();
        assert_eq!(
            recovered
                .equip_outfit(session, own.actor(), own.epoch(), 2, [1; 16])
                .unwrap(),
            first
        );
        recovered
            .equip_outfit(session, own.actor(), own.epoch(), 0, [2; 16])
            .unwrap();
        assert_eq!(recovered.character_rewards(actor).unwrap().outfit, 0);
        assert_eq!(
            recovered
                .equip_outfit(session, own.actor(), own.epoch(), 2, [1; 16])
                .unwrap(),
            first
        );
        assert_eq!(recovered.character_rewards(actor).unwrap().outfit, 0);
        for case in 0..4 {
            let mut bad: serde_json::Value = serde_json::from_slice(&after).unwrap();
            match case {
                0 => bad["rewards"][1]["outfit"] = 3.into(),
                1 => bad["outfits"]["outfits"] = serde_json::json!([]),
                2 => bad["rewards"][1]["experience"] = 9.into(),
                _ => {
                    bad["rewards"].as_array_mut().unwrap().swap(0, 1);
                }
            }
            assert!(Gateway::restore(&serde_json::to_vec(&bad).unwrap(), [6; 32], 240).is_err());
        }
        let mut legacy: serde_json::Value = serde_json::from_slice(&before).unwrap();
        legacy["version"] = 4.into();
        legacy.as_object_mut().unwrap().remove("outfits");
        let upgraded =
            Gateway::restore(&serde_json::to_vec(&legacy).unwrap(), [6; 32], 240).unwrap();
        assert!(upgraded.outfits().outfits.is_empty());
        assert_eq!(upgraded.character_rewards(actor).unwrap().outfit, 0);
    }
    #[test]
    fn malformed_or_incompatible_saves_never_admit_ownership() {
        let (g, _) = fixture();
        let bytes = g.checkpoint().unwrap();
        assert!(Gateway::restore(&bytes, [7; 32], 240).is_err());
        assert!(Gateway::restore(&bytes, [6; 32], 241).is_err());
        assert!(Gateway::restore(&[], [6; 32], 240).is_err());
        assert!(Gateway::restore(&vec![0; MAX_BYTES + 1], [6; 32], 240).is_err());
        for case in 0..8 {
            let mut saved: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            match case {
                0 => saved["version"] = 6.into(),
                1 => saved["grants"][0]["key"] = serde_json::to_value([0u8; 32]).unwrap(),
                2 => {
                    let grant = saved["grants"][0].clone();
                    saved["grants"].as_array_mut().unwrap().push(grant);
                }
                3 => {
                    let players: Vec<_> = saved["grants"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .enumerate()
                        .filter_map(|(i, grant)| grant["actor"].is_number().then_some(i))
                        .collect();
                    saved["grants"][players[1]]["actor"] =
                        saved["grants"][players[0]]["actor"].clone();
                }
                4 => saved["grants"][0]["actor"] = 99999.into(),
                5 => saved["world"] = "{}".into(),
                6 => saved["unexpected"] = true.into(),
                _ => {
                    let grant = saved["grants"][0].clone();
                    saved["grants"] = serde_json::to_value(vec![grant; 129]).unwrap();
                }
            }
            assert!(
                Gateway::restore(&serde_json::to_vec(&saved).unwrap(), [6; 32], 240).is_err(),
                "case {case}"
            );
        }
    }
}
