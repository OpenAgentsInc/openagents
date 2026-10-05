//! Explicit configuration for an authenticated chamber host.
use super::{Chamber, auth::Gateway};
use crate::play::Game;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, net::SocketAddr, path::PathBuf};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub listen: SocketAddr,
    pub instance: u64,
    pub scene: PathBuf,
    pub pack: PathBuf,
    /// How clients reach the chamber. TLS remains for offline test hosts.
    #[serde(default)]
    pub transport: Transport,
    /// TLS only: the certificate and key a client trusts out of band.
    #[serde(default)]
    pub certificate_der: PathBuf,
    #[serde(default)]
    pub private_key_der: PathBuf,
    /// The role table. Over TLS it is also the admission list; over a REACH
    /// channel a NIP-HOST `world` grant admits, and a granted key that is not
    /// listed joins as a spectator.
    #[serde(default)]
    pub enrollments: Vec<Enrollment>,
    /// Public admission: any key that proves itself joins as a player on a
    /// spawn ring, up to the cap. Spectators stay enrollment-only.
    #[serde(default)]
    pub guests: Option<super::auth::Guests>,
    #[serde(default)]
    pub authored_combat_health: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authored: Option<crate::content::Authored>,
    /// Explicit hosted social rules. Omission retains the combat profile.
    #[serde(default)]
    pub social_profile: Option<crate::play::social::Profile>,
    /// A hosted social profile the host builds itself, instead of spelling
    /// one out in `social_profile`: `"profile": "everglade"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<Named>,
    #[serde(default)]
    pub state_dir: Option<PathBuf>,
    #[serde(default)]
    pub rewards: Vec<super::rewards::Policy>,
    #[serde(default)]
    pub progression: super::progression::Config,
    #[serde(default)]
    pub items: super::items::Catalog,
    #[serde(default)]
    pub outfits: super::outfits::Catalog,
    #[serde(default)]
    pub equipment: super::equipment::Catalog,
}
/// The transport that carries the chamber's frames.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Transport {
    /// TLS with a configured certificate and a static enrollment list.
    Tls {},
    /// A NIP-REACH direct channel admitted by NIP-HOST `world` grants, over
    /// TCP or, with `websocket`, a WebSocket upgrade browsers can open.
    Reach {
        #[serde(default)]
        websocket: bool,
    },
}
/// A hosted social profile named in the configuration.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Named {
    /// Everglade's heightfield and studio seats
    /// ([`crate::social::hosted::everglade_profile`]).
    Everglade,
}
impl Default for Transport {
    fn default() -> Self {
        Self::Tls {}
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Enrollment {
    pub public_key: String,
    pub role: Role,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Role {
    Primary {},
    Player { spawn: [f32; 3] },
    Spectator {},
}
impl Config {
    /// The channel carrier when the chamber runs over a REACH channel.
    #[cfg(feature = "service-reach")]
    pub fn reach(&self) -> Option<super::reach::Carrier> {
        match self.transport {
            Transport::Tls {} => None,
            Transport::Reach { websocket: false } => Some(super::reach::Carrier::Tcp),
            Transport::Reach { websocket: true } => Some(super::reach::Carrier::WebSocket),
        }
    }
    /// Prepares combat using only the operator's configured health policy.
    pub fn prepare_game(&self, scene: verse_engine::director::Scene) -> Result<Game, String> {
        self.validate()?;
        let mut game = if let Some(profile) = &self.social_profile {
            Game::social_in(scene, self.instance, profile.clone())
        } else if self.authored.is_some() {
            Game::combat_content_in(scene, self.instance)
        } else if self.authored_combat_health {
            Game::combat_authored_in(scene, false, self.instance)
        } else {
            Game::combat_in(scene, false, self.instance)
        }?;
        if let Some(authored) = &self.authored {
            authored.apply(&mut game)?;
        }
        Ok(game)
    }
    /// Includes social geometry and rules in the scene/asset content identity.
    pub fn bind_content(&self, content: [u8; 32]) -> Result<[u8; 32], String> {
        let content = self
            .authored
            .as_ref()
            .map_or(Ok(content), |a| a.bind_content(content))?;
        let content = if self.authored.is_some() {
            crate::content::bind_gameplay(
                content,
                &crate::content::Gameplay {
                    authored_combat_health: self.authored_combat_health,
                    rewards: &self.rewards,
                    progression: &self.progression,
                    items: &self.items,
                    outfits: &self.outfits,
                    equipment: &self.equipment,
                },
            )?
        } else {
            content
        };
        let Some(profile) = &self.social_profile else {
            return Ok(content);
        };
        use sha2::{Digest, Sha256};
        let mut hash = Sha256::new();
        hash.update(b"verse.hosted.social.content.v1\0");
        hash.update(content);
        hash.update(profile.digest()?);
        Ok(hash.finalize().into())
    }
    pub fn from_json(bytes: &[u8]) -> Result<Self, String> {
        if bytes.is_empty() || bytes.len() > 64 * 1024 {
            return Err("Host configuration exceeds its byte budget".into());
        }
        let mut config: Self =
            serde_json::from_slice(bytes).map_err(|_| "Invalid chamber host configuration")?;
        if let Some(named) = config.profile {
            if config.social_profile.is_some() {
                return Err("profile and social_profile do not go together".into());
            }
            config.social_profile = Some(match named {
                Named::Everglade => crate::social::hosted::everglade_profile()?,
            });
        }
        config.validate()?;
        Ok(config)
    }
    pub fn validate(&self) -> Result<(), String> {
        if let Some(authored) = &self.authored {
            authored.validate()?;
        }
        if let Some(profile) = &self.social_profile {
            if self
                .authored
                .as_ref()
                .is_some_and(|a| a.character.is_some() || !a.blockers.is_empty())
            {
                return Err(
                    "Author social collision in social_profile; character tuning requires combat"
                        .into(),
                );
            }
            profile.validate()?;
            if self.authored_combat_health || !self.rewards.is_empty() {
                return Err("Social profiles cannot enable combat health or kill rewards".into());
            }
        }
        let tls = self.transport == Transport::Tls {};
        if let Some(guests) = &self.guests {
            guests.validate()?;
        }
        if self.instance == 0
            || (tls && self.enrollments.is_empty() && self.guests.is_none())
            || self.enrollments.len() > 128
        {
            return Err("Invalid chamber instance or enrollment budget".into());
        }
        for path in [&self.scene, &self.pack] {
            if path.as_os_str().is_empty() {
                return Err("Host configuration requires explicit file paths".into());
            }
        }
        for path in [&self.certificate_der, &self.private_key_der] {
            if path.as_os_str().is_empty() == tls {
                return Err(if tls {
                    "Host configuration requires explicit file paths"
                } else {
                    "A REACH chamber proves the host key and takes no TLS certificate"
                }
                .into());
            }
        }
        if self
            .state_dir
            .as_ref()
            .is_some_and(|p| p.as_os_str().is_empty())
        {
            return Err("Host state directory must be explicit".into());
        }
        super::rewards::Policy::validate(&self.rewards)?;
        self.progression.validate()?;
        self.equipment
            .validate_catalogs(&self.items, &self.outfits)?;
        let mut keys = BTreeSet::new();
        let mut primary = 0;
        let mut players = 0;
        for enrollment in &self.enrollments {
            let key = public_key(&enrollment.public_key)?;
            if !keys.insert(key) {
                return Err("Duplicate chamber enrollment key".into());
            }
            match enrollment.role {
                Role::Primary {} => {
                    primary += 1;
                }
                Role::Player { spawn } => {
                    players += 1;
                    let spawn = Vec3::from(spawn);
                    if !spawn.is_finite() || spawn.abs().max_element() > 1_000_000. {
                        return Err("Invalid configured player spawn".into());
                    }
                }
                Role::Spectator {} => {}
            }
        }
        let guests = self.guests.as_ref().map_or(0, |g| g.cap as usize);
        if primary > 1 || players + guests > 63 {
            return Err("Configured controlled player capacity exceeded".into());
        }
        Ok(())
    }
    /// Enrolls a prepared authority; callers load scene assets and collision first.
    pub fn gateway(&self, game: Game) -> Result<Gateway, String> {
        self.validate()?;
        if game.player_life().instance != self.instance
            || game.social_state().map(|s| &s.profile) != self.social_profile.as_ref()
        {
            return Err("Host game instance mismatch".into());
        }
        let mut gateway = Gateway::new(Chamber::new(game)?)?;
        for enrollment in &self.enrollments {
            let key = public_key(&enrollment.public_key)?;
            match enrollment.role {
                Role::Primary {} => gateway.enroll_primary(key)?,
                Role::Player { spawn } => {
                    gateway.enroll_player(key, spawn.into())?;
                }
                Role::Spectator {} => gateway.enroll_spectator(key)?,
            }
        }
        gateway
            .with_guests(self.guests.clone(), self.configured_players())?
            .with_rewards(self.rewards.clone())?
            .with_progression(self.progression.clone())?
            .with_items(self.items.clone())?
            .with_outfits(self.outfits.clone())?
            .with_equipment(self.equipment.clone())
    }
    fn configured_players(&self) -> usize {
        self.enrollments
            .iter()
            .filter(|e| matches!(e.role, Role::Player { .. }))
            .count()
    }
    /// Refuses changed startup rights instead of silently replacing saved character ownership.
    pub fn validate_recovered(&self, gateway: &Gateway) -> Result<(), String> {
        self.validate()?;
        // Over a REACH channel, granted keys outside the role table were
        // enrolled as spectators; with guests, as players; nothing else may differ.
        let primary = gateway.game().player_life().actor;
        let granted = gateway.chamber.grants.iter().filter(|(principal, rights)| {
            !self.enrollments.iter().any(|enrollment| {
                public_key(&enrollment.public_key).is_ok_and(|key| key == principal.0)
            }) && !(self.transport != Transport::Tls {}
                && matches!(rights, super::Rights::Spectator))
                && !(self.guests.is_some()
                    && matches!(rights, super::Rights::Player(actor) if *actor != primary))
        });
        if gateway.game().social_state().map(|s| &s.profile) != self.social_profile.as_ref()
            || gateway.game().player_life().instance != self.instance
            || granted.count() != 0
            || gateway.reward_policy() != self.rewards
            || gateway.progression() != &self.progression
            || gateway.items() != &self.items
            || gateway.outfits() != &self.outfits
            || gateway.equipment() != &self.equipment
        {
            return Err("Recovered host enrollment context is incompatible".into());
        }
        for enrollment in &self.enrollments {
            let principal = super::Principal(public_key(&enrollment.public_key)?);
            let rights = gateway
                .chamber
                .grants
                .get(&principal)
                .ok_or("Recovered principal is missing")?;
            let compatible = match (&enrollment.role, rights) {
                (Role::Primary {}, super::Rights::Player(actor)) => {
                    *actor == gateway.game().player_life().actor
                }
                (Role::Player { spawn }, super::Rights::Player(actor)) => {
                    *actor != gateway.game().player_life().actor
                        && gateway.game().player_spawn(*actor) == Some(Vec3::from(*spawn))
                }
                (Role::Spectator {}, super::Rights::Spectator) => true,
                _ => false,
            };
            if !compatible {
                return Err("Recovered principal rights or spawn are incompatible".into());
            }
        }
        Ok(())
    }
    /// Additional adventurer appearances belong to the save, not the authored scene.
    pub fn validate_recovered_scene(
        &self,
        gateway: &Gateway,
        prepared: &Game,
    ) -> Result<(), String> {
        self.validate_recovered(gateway)?;
        if prepared.player_life().instance != self.instance {
            return Err("Prepared host instance is incompatible".into());
        }
        let mut scene = gateway.game().scene.clone();
        if scene.actors.iter().any(|a| {
            !prepared.scene.actors.iter().any(|b| a.id == b.id)
                && (a.model != "adventurer" || gateway.game().player_admission(a.id).is_none())
        }) {
            return Err("Recovered scene has an unauthored actor".into());
        }
        scene
            .actors
            .retain(|a| prepared.scene.actors.iter().any(|b| a.id == b.id));
        if serde_json::to_vec(&scene).map_err(|_| "Cannot validate recovered scene")?
            != serde_json::to_vec(&prepared.scene)
                .map_err(|_| "Cannot validate configured scene")?
        {
            return Err("Recovered scene is incompatible with configured content".into());
        }
        Ok(())
    }
}
pub(super) fn public_key(text: &str) -> Result<[u8; 32], String> {
    if text.len() != 64 {
        return Err("Configured enrollment key must be a 32-byte public key".into());
    }
    text.parse::<secp256k1::XOnlyPublicKey>()
        .map(|k| k.serialize())
        .map_err(|_| "Invalid configured enrollment public key".into())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn key(n: u8) -> String {
        super::super::net::tests::key(n)
            .x_only_public_key()
            .0
            .to_string()
    }
    fn config() -> Config {
        let keys = super::super::net::tests::key(1)
            .x_only_public_key()
            .0
            .to_string();
        Config {
            listen: "127.0.0.1:0".parse().unwrap(),
            instance: 170,
            scene: "scene.json".into(),
            pack: "pack.json".into(),
            transport: Transport::default(),
            certificate_der: "cert.der".into(),
            private_key_der: "key.der".into(),
            enrollments: vec![Enrollment {
                public_key: keys,
                role: Role::Primary {},
            }],
            guests: None,
            authored_combat_health: false,
            authored: None,
            social_profile: None,
            profile: None,
            state_dir: None,
            rewards: Vec::new(),
            progression: Default::default(),
            items: Default::default(),
            outfits: Default::default(),
            equipment: Default::default(),
        }
    }
    fn game(instance: u64) -> Game {
        let scene = verse_engine::director::Scene::from_json(include_bytes!(
            "../../../../assets/verse/original/ritual.json"
        ))
        .unwrap();
        Game::combat_in(scene, false, instance).unwrap()
    }
    #[test]
    fn a_named_everglade_profile_resolves_to_the_shared_social_profile() {
        let mut value = serde_json::to_value(config()).unwrap();
        value["profile"] = serde_json::json!("everglade");
        let resolved = Config::from_json(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(
            resolved.social_profile,
            Some(crate::social::hosted::everglade_profile().unwrap())
        );
        value["social_profile"] =
            serde_json::to_value(crate::social::hosted::everglade_profile().unwrap()).unwrap();
        assert!(Config::from_json(&serde_json::to_vec(&value).unwrap()).is_err());
    }
    #[test]
    fn social_host_configuration_binds_rules_and_refuses_changed_recovery() {
        use crate::play::social::{Zone, tests::game};
        let mut config = config();
        let authored = game(config.instance, Zone::Plaza);
        config.social_profile = Some(authored.social_state().unwrap().profile.clone());
        let scene = authored.scene.clone();
        let prepared = config.prepare_game(scene).unwrap();
        let bound = config.bind_content([7; 32]).unwrap();
        assert_ne!(bound, [7; 32]);
        let gateway = config
            .gateway(prepared)
            .unwrap()
            .with_content(bound)
            .unwrap();
        config.validate_recovered(&gateway).unwrap();
        let recovered =
            Gateway::restore(&gateway.checkpoint().unwrap(), bound, config.instance).unwrap();
        config.validate_recovered(&recovered).unwrap();
        config.social_profile.as_mut().unwrap().zone = Zone::Everglade;
        assert_ne!(config.bind_content([7; 32]).unwrap(), bound);
        assert!(config.validate_recovered(&recovered).is_err());
        config.authored_combat_health = true;
        assert!(config.validate().is_err());
    }
    #[test]
    fn authored_health_survives_reset_and_recovery_fences() {
        let mut config = config();
        let mut scene = verse_engine::director::Scene::from_json(include_bytes!(
            "../../../../assets/verse/original/ritual.json"
        ))
        .unwrap();
        scene
            .actors
            .iter_mut()
            .find(|actor| actor.id == 2)
            .unwrap()
            .health = 20_000;
        let default = config.prepare_game(scene.clone()).unwrap();
        assert_eq!(
            default
                .frame()
                .actors
                .iter()
                .find(|actor| actor.actor.id == 2)
                .unwrap()
                .health,
            15
        );
        config.authored_combat_health = true;
        let mut authored = config.prepare_game(scene.clone()).unwrap();
        assert_eq!(
            authored
                .frame()
                .actors
                .iter()
                .find(|actor| actor.actor.id == 2)
                .unwrap()
                .health,
            20_000
        );
        authored.restart_combat(false).unwrap();
        assert_eq!(
            authored
                .frame()
                .actors
                .iter()
                .find(|actor| actor.actor.id == 2)
                .unwrap()
                .health,
            20_000
        );
        let restored = Game::restore(&authored.checkpoint().unwrap()).unwrap();
        assert_eq!(
            restored
                .frame()
                .actors
                .iter()
                .find(|actor| actor.actor.id == 2)
                .unwrap()
                .health,
            20_000
        );
        let prepared = config.prepare_game(scene).unwrap();
        let gateway = config.gateway(restored).unwrap();
        config
            .validate_recovered_scene(&gateway, &prepared)
            .unwrap();
        config.authored_combat_health = false;
        let other = config.prepare_game(prepared.scene.clone()).unwrap();
        assert!(config.validate_recovered_scene(&gateway, &other).is_err());
    }
    #[test]
    fn recovery_refuses_changed_identity_roles_and_spawns() {
        let mut config = config();
        config.enrollments.push(Enrollment {
            public_key: key(52),
            role: Role::Player {
                spawn: [3., 0., -22.],
            },
        });
        config.enrollments.push(Enrollment {
            public_key: key(53),
            role: Role::Spectator {},
        });
        let gateway = config
            .gateway(game(config.instance))
            .unwrap()
            .with_content([8; 32])
            .unwrap();
        let mut recovered =
            Gateway::restore(&gateway.checkpoint().unwrap(), [8; 32], config.instance).unwrap();
        config.validate_recovered(&recovered).unwrap();
        config
            .validate_recovered_scene(&recovered, &game(config.instance))
            .unwrap();
        for case in 0..6 {
            let mut changed = config.clone();
            match case {
                0 => changed.enrollments[1].public_key = key(54),
                1 => changed.enrollments[1].role = Role::Spectator {},
                2 => {
                    changed.enrollments[1].role = Role::Player {
                        spawn: [4., 0., -22.],
                    }
                }
                3 => {
                    changed.enrollments.pop();
                }
                4 => changed.state_dir = Some(PathBuf::new()),
                _ => changed.progression.levels = vec![0, 100],
            }
            assert!(changed.validate_recovered(&recovered).is_err());
        }
        recovered.chamber.game.scene.actors[0].scale += 1.;
        assert!(
            config
                .validate_recovered_scene(&recovered, &game(config.instance))
                .is_err()
        );
    }
    #[test]
    fn reward_configuration_and_recovery_refuse_changed_or_foreign_policies() {
        use super::super::rewards::Policy;
        let mut config = config();
        config.rewards = vec![Policy {
            participation: Default::default(),
            target: 2,
            experience: 45,
            items: vec![],
            quests: vec![],
        }];
        let g = config
            .gateway(game(config.instance))
            .unwrap()
            .with_content([8; 32])
            .unwrap();
        let recovered =
            Gateway::restore(&g.checkpoint().unwrap(), [8; 32], config.instance).unwrap();
        config.validate_recovered(&recovered).unwrap();
        let mut changed = config.clone();
        changed.rewards[0].experience = 90;
        assert!(changed.validate_recovered(&recovered).is_err());
        for target in [14, 99999] {
            let mut changed = config.clone();
            changed.rewards[0].target = target;
            assert!(changed.gateway(game(config.instance)).is_err());
        }
        config.rewards.push(config.rewards[0].clone());
        assert!(config.validate().is_err());
    }
    #[test]
    fn reach_configuration_drops_the_certificate_and_keeps_granted_spectators() {
        let json = br#"{"listen":"127.0.0.1:0","instance":170,"scene":"s","pack":"p","transport":{"type":"reach","websocket":true}}"#;
        let parsed = Config::from_json(json).unwrap();
        assert_eq!(parsed.transport, Transport::Reach { websocket: true });
        assert!(parsed.enrollments.is_empty());
        let reach = Config {
            transport: Transport::Reach { websocket: false },
            certificate_der: PathBuf::new(),
            private_key_der: PathBuf::new(),
            ..config()
        };
        reach.validate().unwrap();
        assert!(
            Config {
                certificate_der: "cert.der".into(),
                ..reach.clone()
            }
            .validate()
            .is_err()
        );
        assert!(
            Config {
                enrollments: Vec::new(),
                ..config()
            }
            .validate()
            .is_err()
        );
        let mut gateway = reach.gateway(game(170)).unwrap();
        let granted = super::super::net::tests::key(9)
            .x_only_public_key()
            .0
            .serialize();
        assert!(gateway.admit_spectator(granted).unwrap());
        assert!(!gateway.admit_spectator(granted).unwrap());
        reach.validate_recovered(&gateway).unwrap();
        // Over TLS the role table is the whole admission list.
        assert!(config().validate_recovered(&gateway).is_err());
    }
    #[test]
    fn guests_join_as_players_on_the_ring_up_to_the_cap_and_recover() {
        use secp256k1::Secp256k1;
        let secp = Secp256k1::new();
        let mut config = config();
        config.enrollments.clear();
        assert!(config.validate().is_err());
        config.guests = Some(super::super::auth::Guests {
            cap: 2,
            ring: [0., 0., -22.],
            radius: 3.,
        });
        config.validate().unwrap();
        let mut over = config.clone();
        over.guests.as_mut().unwrap().cap = 64;
        assert!(over.validate().is_err());
        let mut gateway = config.gateway(game(170)).unwrap();
        let join = |gateway: &mut Gateway, n: u8| {
            let keypair = super::super::net::tests::key(n);
            let key = keypair.x_only_public_key().0.serialize();
            let (id, challenge) = gateway.open(0).unwrap();
            let signature = secp
                .sign_schnorr_no_aux_rand(&challenge.signing_digest(key), &keypair)
                .to_byte_array();
            gateway.authenticate(id, 0, key, signature).map(|()| id)
        };
        let first = join(&mut gateway, 21).unwrap();
        let second = join(&mut gateway, 22).unwrap();
        assert_eq!(gateway.guest_count(), 2);
        let a = gateway.admission(first).unwrap().actor();
        let b = gateway.admission(second).unwrap().actor();
        assert_ne!(a, b);
        assert_ne!(a.actor, gateway.game().player_life().actor);
        let error = join(&mut gateway, 23).unwrap_err();
        assert!(error.contains("guest capacity"), "{error}");
        // A returning guest keeps its adventurer instead of taking a slot.
        let again = join(&mut gateway, 21).unwrap();
        assert_eq!(gateway.admission(again).unwrap().actor(), a);
        assert_eq!(gateway.guest_count(), 2);
        config.validate_recovered(&gateway).unwrap();
        let mut closed = config.clone();
        closed.guests = None;
        closed.enrollments.push(Enrollment {
            public_key: key(1),
            role: Role::Primary {},
        });
        assert!(closed.validate_recovered(&gateway).is_err());
        // Spectators stay enrollment-only: nothing admits an unknown key as one.
        assert!(
            gateway
                .chamber
                .grants
                .values()
                .all(|r| matches!(r, super::super::Rights::Player(_)))
        );
    }
    #[test]
    fn strict_configuration_refuses_duplicate_keys_roles_and_spawn_budgets() {
        let c = config();
        c.validate().unwrap();
        let mut bad = c.clone();
        bad.enrollments.push(bad.enrollments[0].clone());
        assert!(bad.validate().is_err());
        let mut bad = c.clone();
        bad.enrollments[0].public_key = "0".repeat(64);
        assert!(bad.validate().is_err());
        let mut bad = c.clone();
        bad.enrollments[0].role = Role::Player {
            spawn: [f32::NAN, 0., 0.],
        };
        assert!(bad.validate().is_err());
        let mut bad = c.clone();
        bad.instance = 0;
        assert!(bad.validate().is_err());
        let mut bad = c.clone();
        bad.enrollments = vec![];
        assert!(bad.validate().is_err());
        let mut bad = c.clone();
        bad.enrollments = vec![c.enrollments[0].clone(); 129];
        assert!(bad.validate().is_err());
        let mut bad = c.clone();
        bad.enrollments.push(Enrollment {
            public_key: super::super::net::tests::key(2)
                .x_only_public_key()
                .0
                .to_string(),
            role: Role::Primary {},
        });
        assert!(bad.validate().is_err());
        assert!(Config::from_json(&vec![b' '; 65537]).is_err());
        assert!(Config::from_json(br#"{"listen":"127.0.0.1:0","instance":170,"scene":"s","pack":"p","certificate_der":"c","private_key_der":"k","enrollments":[],"allow_plaintext":true}"#).is_err());
        assert!(c.gateway(game(171)).is_err());
    }
    #[tokio::test]
    async fn configured_host_enrolls_players_and_spectators_over_real_tls() {
        use super::super::{client::Client, net};
        use rustls::pki_types::ServerName;
        let keys = [net::tests::key(1), net::tests::key(2), net::tests::key(3)];
        let mut c = config();
        c.enrollments.push(Enrollment {
            public_key: keys[1].x_only_public_key().0.to_string(),
            role: Role::Player {
                spawn: [1., 0., -20.],
            },
        });
        c.enrollments.push(Enrollment {
            public_key: keys[2].x_only_public_key().0.to_string(),
            role: Role::Spectator {},
        });
        let gateway = c.gateway(game(c.instance)).unwrap();
        let (tls, connector) = net::tests::tls();
        let listener = tokio::net::TcpListener::bind(c.listen).await.unwrap();
        let address = listener.local_addr().unwrap();
        let (stop, stopping) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(net::serve(listener, tls, gateway, async {
            let _ = stopping.await;
        }));
        let mut first = Client::connect(
            address,
            ServerName::try_from("localhost").unwrap(),
            connector.config().clone(),
            c.instance,
            &keys[0],
        )
        .await
        .unwrap();
        let second = Client::connect(
            address,
            ServerName::try_from("localhost").unwrap(),
            connector.config().clone(),
            c.instance,
            &keys[1],
        )
        .await
        .unwrap();
        let spectator = Client::connect(
            address,
            ServerName::try_from("localhost").unwrap(),
            connector.config().clone(),
            c.instance,
            &keys[2],
        )
        .await
        .unwrap();
        assert!(first.control().is_some());
        assert!(second.control().is_some());
        assert!(spectator.control().is_none());
        assert_ne!(
            first.control().unwrap().life,
            second.control().unwrap().life
        );
        first
            .request(super::super::wire::Body::Snapshot {})
            .await
            .unwrap();
        stop.send(()).unwrap();
        let exit = server.await.unwrap();
        assert!(exit.failure.is_none());
        assert!(exit.stats.requests >= 4);
    }
}
