//! Explicit configuration for an authenticated chamber host.
use super::{Chamber, auth::Gateway};
use crate::play::Game;
use glam::Vec3;
use serde::Deserialize;
use std::{collections::BTreeSet, net::SocketAddr, path::PathBuf};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub listen: SocketAddr,
    pub instance: u64,
    pub scene: PathBuf,
    pub pack: PathBuf,
    pub certificate_der: PathBuf,
    pub private_key_der: PathBuf,
    pub enrollments: Vec<Enrollment>,
    #[serde(default)]
    pub state_dir: Option<PathBuf>,
    #[serde(default)]
    pub rewards: Vec<super::rewards::Policy>,
    #[serde(default)]
    pub progression: super::progression::Config,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Enrollment {
    pub public_key: String,
    pub role: Role,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Role {
    Primary {},
    Player { spawn: [f32; 3] },
    Spectator {},
}
impl Config {
    pub fn from_json(bytes: &[u8]) -> Result<Self, String> {
        if bytes.is_empty() || bytes.len() > 64 * 1024 {
            return Err("Host configuration exceeds its byte budget".into());
        }
        let config: Self =
            serde_json::from_slice(bytes).map_err(|_| "Invalid chamber host configuration")?;
        config.validate()?;
        Ok(config)
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.instance == 0 || self.enrollments.is_empty() || self.enrollments.len() > 128 {
            return Err("Invalid chamber instance or enrollment budget".into());
        }
        for path in [
            &self.scene,
            &self.pack,
            &self.certificate_der,
            &self.private_key_der,
        ] {
            if path.as_os_str().is_empty() {
                return Err("Host configuration requires explicit file paths".into());
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
        if primary > 1 || players > 63 {
            return Err("Configured controlled player capacity exceeded".into());
        }
        Ok(())
    }
    /// Enrolls a prepared authority; callers load scene assets and collision first.
    pub fn gateway(&self, game: Game) -> Result<Gateway, String> {
        self.validate()?;
        if game.player_life().instance != self.instance {
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
            .with_rewards(self.rewards.clone())?
            .with_progression(self.progression.clone())
    }
    /// Refuses changed startup rights instead of silently replacing saved character ownership.
    pub fn validate_recovered(&self, gateway: &Gateway) -> Result<(), String> {
        self.validate()?;
        if gateway.game().player_life().instance != self.instance
            || gateway.chamber.grants.len() != self.enrollments.len()
            || gateway.reward_policy() != self.rewards
            || gateway.progression() != &self.progression
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
fn public_key(text: &str) -> Result<[u8; 32], String> {
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
            certificate_der: "cert.der".into(),
            private_key_der: "key.der".into(),
            enrollments: vec![Enrollment {
                public_key: keys,
                role: Role::Primary {},
            }],
            state_dir: None,
            rewards: Vec::new(),
            progression: Default::default(),
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
