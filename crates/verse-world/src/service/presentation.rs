//! Life-bound presentation extracted from the same authority as combat snapshots.
use super::wire::{ActorBinding, Life};
use crate::{play::Game, utilities::Area};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use verse_engine::{director::Actor, motion::Selection};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pose {
    pub actor: Actor,
    pub life: Life,
    pub teleport_stamp: Option<f32>,
    pub animation: Selection,
    pub animation_time: f32,
    pub visible: bool,
    pub health: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Effects {
    pub life: Life,
    pub position: [f32; 3],
    pub shield: i32,
    pub shield_until: f32,
    pub light: Option<[f32; 3]>,
    pub areas: Vec<Area>,
}
/// Hostile telegraph and flight data; clients cannot apply its damage.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostileCast {
    pub caster: Life,
    pub target_life: Life,
    pub position: Option<[f32; 3]>,
    pub origin: [f32; 3],
    pub target: [f32; 3],
    pub started: f32,
    pub release: f32,
    pub impact: f32,
    pub radius: f32,
    pub boss: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Impact {
    pub position: [f32; 3],
    pub at: f32,
    pub kind: u8,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Presentation {
    pub time: f32,
    pub actors: Vec<Pose>,
    pub effects: Vec<Effects>,
    pub hostile_casts: Vec<HostileCast>,
    pub impacts: Vec<Impact>,
    pub props: Vec<crate::visuals::Prop>,
    pub blockers: Vec<crate::visuals::Blocker>,
}
impl Presentation {
    pub(super) fn extract(game: &Game, bindings: &[ActorBinding]) -> Self {
        let lives: BTreeSet<_> = bindings
            .iter()
            .map(|b| verse_engine::core::LifeId::from(b.life))
            .collect();
        let teleports: std::collections::BTreeMap<_, _> = game
            .controlled_effects()
            .map(|(life, _, c)| (life, c.teleport_stamp()))
            .collect();
        let frame = game.frame();
        let actors = frame
            .actors
            .into_iter()
            .filter_map(|a| {
                let life = a.life.filter(|life| lives.contains(life))?;
                Some(Pose {
                    actor: a.actor,
                    life: life.into(),
                    teleport_stamp: teleports.get(&life).copied().flatten(),
                    animation: a.animation,
                    animation_time: a.animation_time,
                    visible: a.visible,
                    health: a.health,
                })
            })
            .collect();
        let effects = game
            .controlled_effects()
            .filter(|(life, _, _)| lives.contains(life))
            .map(|(life, position, c)| Effects {
                life: life.into(),
                position: position.to_array(),
                shield: c.shield,
                shield_until: c.shield_until,
                light: c.light.map(|p| p.to_array()),
                areas: c.areas.clone(),
            })
            .collect();
        Self {
            props: crate::visuals::prop_poses(game, 1.),
            blockers: crate::visuals::blocker_bounds(game),
            time: frame.time,
            actors,
            effects,
            hostile_casts: game
                .encounter
                .iter()
                .flat_map(|e| &e.casts)
                .filter(|c| lives.contains(&c.life) && lives.contains(&c.target_life))
                .map(|c| HostileCast {
                    caster: c.life.into(),
                    target_life: c.target_life.into(),
                    position: c.position.map(|p| p.to_array()),
                    origin: c.origin.to_array(),
                    target: c.target.to_array(),
                    started: c.started,
                    release: c.release,
                    impact: c.impact,
                    radius: c.radius,
                    boss: c.boss,
                })
                .collect(),
            impacts: game
                .impacts
                .iter()
                .filter(|(_, at, _)| (0.0..0.6).contains(&(frame.time - at)))
                .map(|(p, at, kind)| Impact {
                    position: p.to_array(),
                    at: *at,
                    kind: *kind,
                })
                .collect(),
        }
    }
    pub fn validate(&self, instance: u64, bindings: &[ActorBinding]) -> Result<(), String> {
        let finite = |p: [f32; 3]| p.iter().all(|v| v.is_finite() && v.abs() <= 1_000_000.);
        let known: BTreeSet<_> = bindings
            .iter()
            .map(|b| verse_engine::core::LifeId::from(b.life))
            .collect();
        let mut poses = BTreeSet::new();
        let mut players = BTreeSet::new();
        if !self.time.is_finite()
            || self.time < 0.
            || self.actors.len() > 256
            || self.effects.len() > 64
            || self.hostile_casts.len() > 256
            || self.impacts.len() > 512
        {
            return Err("Chamber presentation budget or clock refused".into());
        }
        for p in &self.actors {
            let life = verse_engine::core::LifeId::from(p.life);
            if p.life.instance != instance
                || p.actor.id != p.life.actor
                || !known.contains(&life)
                || !poses.insert(life)
                || !finite(p.actor.position.to_array())
                || !p.actor.yaw.is_finite()
                || !p.actor.scale.is_finite()
                || !(0.001..=100.).contains(&p.actor.scale)
                || !p.animation_time.is_finite()
                || p.animation_time < 0.
                || p.teleport_stamp
                    .is_some_and(|stamp| !stamp.is_finite() || stamp < 0.)
                || p.actor.name.len() > 256
                || p.actor.model.is_empty()
                || p.actor.model.len() > 128
            {
                return Err("Invalid chamber actor presentation".into());
            }
            if p.actor.model == "adventurer" {
                players.insert(life);
            }
        }
        if poses != known {
            return Err("Chamber presentation is missing actor lives".into());
        }
        let mut effects = BTreeSet::new();
        for e in &self.effects {
            let life = verse_engine::core::LifeId::from(e.life);
            if !players.contains(&life)
                || !effects.insert(life)
                || !finite(e.position)
                || !(0..=18).contains(&e.shield)
                || !e.shield_until.is_finite()
                || e.shield_until < 0.
                || e.light.is_some_and(|p| !finite(p))
                || e.areas.len() > 128
                || e.areas
                    .iter()
                    .any(|a| !finite(a.position.to_array()) || !a.until.is_finite() || a.until < 0.)
            {
                return Err("Invalid chamber player effects".into());
            }
        }
        if effects != players {
            return Err("Chamber presentation is missing player effects".into());
        }
        let mut casters = BTreeSet::new();
        for cast in &self.hostile_casts {
            let caster = cast.caster.into();
            let target = cast.target_life.into();
            if !known.contains(&caster)
                || players.contains(&caster)
                || !players.contains(&target)
                || !casters.insert(caster)
                || !finite(cast.origin)
                || !finite(cast.target)
                || cast.position.is_some_and(|p| !finite(p))
                || !cast.started.is_finite()
                || cast.started < 0.
                || cast.started > self.time
                || !cast.release.is_finite()
                || cast.release <= cast.started
                || !cast.impact.is_finite()
                || cast.impact < cast.release
                || self.time > cast.impact
                || !cast.radius.is_finite()
                || !(0.01..=100.).contains(&cast.radius)
            {
                return Err("Invalid chamber hostile cast presentation".into());
            }
        }
        for impact in &self.impacts {
            if !finite(impact.position)
                || !impact.at.is_finite()
                || !(0.0..0.6).contains(&(self.time - impact.at))
                || impact.kind > 3
            {
                return Err("Invalid chamber impact presentation".into());
            }
        }
        crate::visuals::validate_props(&self.props, instance)?;
        crate::visuals::validate_blockers(&self.blockers, instance)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::EnemyCast;
    use glam::Vec3;

    fn add_cast(game: &mut Game) {
        let frame = game.frame();
        let caster = frame
            .actors
            .iter()
            .find(|p| p.actor.model != "adventurer")
            .unwrap();
        let target = frame
            .actors
            .iter()
            .find(|p| p.actor.model == "adventurer")
            .unwrap();
        game.encounter.as_mut().unwrap().casts.push(EnemyCast {
            actor: caster.actor.id,
            life: caster.life.unwrap(),
            target_life: target.life.unwrap(),
            position: None,
            origin: caster.actor.position + Vec3::Y,
            target: target.actor.position,
            started: game.time,
            release: game.time + 1.,
            impact: game.time + 2.,
            damage: 8,
            radius: 1.6,
            boss: false,
        });
        game.impacts.push((target.actor.position, game.time, 1));
    }
    fn fixture() -> (Presentation, Vec<ActorBinding>) {
        let scene = verse_engine::director::Scene::from_json(include_bytes!(
            "../../../../assets/verse/original/ritual.json"
        ))
        .unwrap();
        let mut game = Game::combat_in(scene, false, 150).unwrap();
        game.time = game.scene.cut_at;
        game.tick(0., [0.; 2]).unwrap();
        add_cast(&mut game);
        let bindings = game
            .frame()
            .actors
            .iter()
            .map(|p| ActorBinding {
                source: p.actor.id as u32,
                life: p.life.unwrap().into(),
            })
            .collect::<Vec<_>>();
        (Presentation::extract(&game, &bindings), bindings)
    }
    #[test]
    fn extraction_retains_life_bound_hostile_visuals_and_transient_impacts() {
        let (p, b) = fixture();
        p.validate(150, &b).unwrap();
        assert_eq!(p.hostile_casts.len(), 1);
        assert_eq!(p.impacts.len(), 1);
        assert!(p.hostile_casts[0].position.is_none());
        let bytes = serde_json::to_vec(&p).unwrap();
        let decoded: Presentation = serde_json::from_slice(&bytes).unwrap();
        decoded.validate(150, &b).unwrap();
        assert_eq!(serde_json::to_vec(&decoded).unwrap(), bytes);
    }
    #[test]
    fn malformed_hostile_visuals_and_impact_budgets_are_refused() {
        let (p, b) = fixture();
        for case in 0..12 {
            let mut bad = p.clone();
            let c = &mut bad.hostile_casts[0];
            match case {
                0 => c.caster.instance += 1,
                1 => c.target_life.generation += 1,
                2 => c.origin[0] = f32::NAN,
                3 => c.position = Some([f32::INFINITY, 0., 0.]),
                4 => c.release = c.started,
                5 => c.impact = c.release - 1.,
                6 => c.radius = -1.,
                7 => bad.hostile_casts.push(p.hostile_casts[0].clone()),
                8 => bad.impacts[0].at = p.time + 1.,
                9 => bad.impacts[0].kind = 4,
                10 => bad.impacts = vec![p.impacts[0].clone(); 513],
                _ => bad.hostile_casts = vec![p.hostile_casts[0].clone(); 257],
            }
            assert!(bad.validate(150, &b).is_err(), "case {case}");
        }
    }
    #[cfg(feature = "service-net")]
    #[tokio::test]
    async fn tls_spectator_receives_hostile_telegraphs_and_impact_flashes() {
        use crate::service::{
            client::Client,
            net::{
                serve,
                tests::{gateway, key, tls},
            },
        };
        use tokio::{net::TcpListener, sync::oneshot};
        let keys = [key(81), key(82), key(83)];
        let mut gateway = gateway(&keys);
        add_cast(&mut gateway.chamber.game);
        gateway
            .chamber
            .game
            .spawn_prop(
                "Crate",
                crate::spells::PropSpec::reference(crate::spells::PropKind::Crate).secured(),
                Vec3::new(0., 1., -8.),
                0.,
            )
            .unwrap();
        let blocker = physics::queries::Life {
            instance: 120,
            entity: 10000,
            generation: 0,
        };
        gateway
            .chamber
            .game
            .set_navigation_blocker(
                blocker,
                glam::DVec3::new(-0.5, 0., -8.5),
                glam::DVec3::new(0.5, 1., -7.5),
            )
            .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (tls, connector) = tls();
        let (stop, stopped) = oneshot::channel();
        let task = tokio::spawn(serve(listener, tls, gateway, async {
            let _ = stopped.await;
        }));
        let mut client = Client::connect(
            address,
            rustls::pki_types::ServerName::try_from("localhost").unwrap(),
            connector.config().clone(),
            120,
            &keys[2],
        )
        .await
        .unwrap();
        let state = client.snapshot().await.unwrap();
        assert_eq!(state.presentation.hostile_casts.len(), 1);
        assert_eq!(state.presentation.impacts.len(), 1);
        assert_eq!(state.presentation.props.len(), 1);
        assert_eq!(state.presentation.props[0].life.instance, 120);
        assert_eq!(state.presentation.props[0].center, Vec3::new(0., 1., -8.));
        assert!(
            state
                .presentation
                .blockers
                .iter()
                .any(|b| b.life == blocker && b.table_proxy)
        );
        assert!(client.control().is_none());
        client.close().await.unwrap();
        stop.send(()).unwrap();
        assert!(task.await.unwrap().failure.is_none());
    }
}
