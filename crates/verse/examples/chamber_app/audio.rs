//! Presentation routing over committed world results and sampled animation cues.
use glam::Vec3;
use std::collections::{BTreeMap, BTreeSet};
use verse::{audio_native::Output, imported::MarkerEvent, render::View};
use verse_engine::{
    audio::{Clip, Emitter},
    audio_cues::{Cue, synthesize},
    core::LifeId,
};
use verse_world::{events::Kind, play::Game, rules::ProjectileKind};

pub struct Audio {
    output: Output,
    clips: [Clip; 5],
    serial: Option<(u64, u64)>,
    flights: BTreeSet<u32>,
    lives: BTreeMap<(u64, u64), LifeId>,
    shields: BTreeMap<LifeId, f32>,
    time: f32,
    queued: [u64; 5],
}
impl Audio {
    pub fn open(game: &Game) -> Result<Self, String> {
        let output = Output::open()?;
        let rate = output.rate();
        let cues = [
            Cue::Footstep,
            Cue::FireLaunch,
            Cue::Impact,
            Cue::Shield,
            Cue::RitualAmbience,
        ];
        let clips: Vec<_> = cues
            .into_iter()
            .enumerate()
            .map(|(i, cue)| synthesize(cue, rate, i as u64 + 10491))
            .collect::<Result<_, _>>()?;
        let mut audio = Self {
            output,
            clips: clips.try_into().map_err(|_| "Invalid audio cue bank")?,
            serial: game
                .events
                .last()
                .map(|event| (event.instance, event.serial)),
            flights: game
                .snapshot()
                .projectiles
                .iter()
                .map(|projectile| projectile.id)
                .collect(),
            lives: BTreeMap::new(),
            shields: game
                .controlled_effects()
                .map(|(life, _, c)| (life, c.shield_until))
                .collect(),
            time: game.time,
            queued: [0; 5],
        };
        audio.play(4, None, Vec3::new(0., 1., -5.), 0.2, true)?;
        Ok(audio)
    }
    fn play(
        &mut self,
        cue: usize,
        life: Option<LifeId>,
        position: Vec3,
        gain: f32,
        looping: bool,
    ) -> Result<(), String> {
        self.output.play(
            &self.clips[cue],
            Emitter {
                life,
                position,
                range: if looping { 100. } else { 35. },
                gain,
                pitch: 1.,
                looping,
            },
        )?;
        self.queued[cue] += 1;
        Ok(())
    }
    pub fn update(
        &mut self,
        game: &Game,
        view: View,
        markers: &[MarkerEvent],
    ) -> Result<(), String> {
        self.output
            .listener(view.eye, view.view_proj.row(0).truncate())?;
        if game.time < self.time {
            for life in self.lives.values() {
                self.output.stop_life(*life)?;
            }
            self.serial = None;
            self.flights.clear();
            self.lives.clear();
            self.shields.clear();
        }
        self.time = game.time;
        for event in &game.events {
            if self.serial.is_some_and(|(instance, serial)| {
                instance == event.instance && serial >= event.serial
            }) {
                continue;
            }
            self.serial = Some((event.instance, event.serial));
            if let Some(life) = event.actor {
                let key = (life.instance, life.actor);
                if let Some(old) = self.lives.insert(key, life) {
                    if old != life {
                        self.output.stop_life(old)?;
                    }
                }
                let position = if life == game.player_life() {
                    game.player
                } else {
                    game.actor_position(life.actor).unwrap_or(game.player)
                };
                match event.kind {
                    Kind::Damage { .. } => self.play(2, Some(life), position, 0.65, false)?,
                    Kind::Death => self.output.stop_life(life)?,
                    _ => {}
                }
            }
        }
        for marker in markers {
            let life = marker.event.life;
            if marker.event.marker == verse_engine::markers::FOOTSTEP_LEFT
                || marker.event.marker == verse_engine::markers::FOOTSTEP_RIGHT
            {
                let current = if life == game.player_life() {
                    Some(life)
                } else {
                    game.actor_life(life.actor)
                };
                if current == Some(life) {
                    self.lives.insert((life.instance, life.actor), life);
                    self.play(0, Some(life), marker.position.into(), 0.45, false)?;
                }
            }
        }
        let snapshot = game.snapshot();
        let current: BTreeSet<_> = snapshot.projectiles.iter().map(|p| p.id).collect();
        for projectile in &snapshot.projectiles {
            if !self.flights.contains(&projectile.id)
                && matches!(
                    projectile.kind,
                    ProjectileKind::Fireball | ProjectileKind::Firebolt
                )
            {
                self.play(
                    1,
                    game.projectile_caster_life(projectile.caster),
                    projectile.pos.into(),
                    0.6,
                    false,
                )?;
            }
        }
        self.flights = current;
        let current: BTreeMap<_, _> = game
            .controlled_effects()
            .map(|(life, _, c)| (life, c.shield_until))
            .collect();
        for (life, position, c) in game.controlled_effects() {
            if c.shield_until > self.shields.get(&life).copied().unwrap_or(0.)
                && c.shield_until > game.time
            {
                self.play(3, Some(life), position, 0.55, false)?;
            }
        }
        self.shields = current;
        Ok(())
    }
    pub fn stats(&self) -> serde_json::Value {
        serde_json::json!({"device":self.output.stats(),"queued_by_cue":self.queued,"sample_rate":self.output.rate()})
    }
}
