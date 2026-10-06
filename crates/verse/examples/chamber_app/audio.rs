//! Presentation routing over committed world results and sampled animation cues.
use glam::Vec3;
use std::collections::{BTreeMap, BTreeSet};
use verse::{audio_native::Output, imported::MarkerEvent, render::View};
use verse_engine::{
    audio::Bus,
    audio_bank::{Bank, Scene},
    core::LifeId,
};
use verse_world::{events::Kind, play::Game, rules::ProjectileKind};

pub struct Audio {
    output: Option<Output>,
    scene: Scene,
    retry_at: std::time::Instant,
    pub status: String,
    serial: Option<(u64, u64)>,
    flights: BTreeSet<u32>,
    lives: BTreeMap<(u64, u64), LifeId>,
    shields: BTreeMap<LifeId, f32>,
    time: f32,
    queued: [u64; 5],
}
impl Audio {
    pub fn open(game: &Game) -> Result<Self, String> {
        let scene = Scene::new(std::sync::Arc::new(Bank::original()?), "en")?;
        let mut audio = Self {
            output: None,
            scene,
            retry_at: std::time::Instant::now(),
            status: String::new(),
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
        audio.resume();
        Ok(audio)
    }
    fn play(
        &mut self,
        cue: usize,
        life: Option<LifeId>,
        position: Vec3,
        _gain: f32,
        _looping: bool,
    ) -> Result<(), String> {
        let id = [
            "footstep",
            "fire_launch",
            "impact",
            "shield",
            "ritual_ambience",
        ][cue];
        let prepared = self
            .scene
            .emit(id, life, position, f64::from(self.time.max(0.)))?;
        if let Some(output) = &self.output {
            if let Err(error) = output.play_prepared(prepared) {
                self.status = error;
            } else {
                self.queued[cue] = self.queued[cue].saturating_add(1);
            }
        }
        Ok(())
    }
    fn stop_life(&mut self, life: LifeId) {
        if let Some(output) = &self.output {
            if let Err(error) = output.stop_life(life) {
                self.status = error;
            }
        }
    }
    pub fn focus(&mut self, focused: bool) {
        self.scene.suspended = !focused;
        if let Some(output) = &self.output {
            output.suspend(!focused);
        }
    }
    pub fn suspend(&mut self) {
        self.focus(false);
        self.output = None;
        self.scene.output_available = false;
    }
    pub fn resume(&mut self) {
        self.scene.suspended = false;
        self.retry_at = std::time::Instant::now() + std::time::Duration::from_secs(3);
        if let Some(output) = &self.output {
            output.suspend(false);
            return;
        }
        let result = (|| -> Result<Output, String> {
            let output = Output::open()?;
            output.volume(self.scene.master, self.scene.buses)?;
            let start = self
                .scene
                .zone_music_position("chamber", "ritual_ambience")?;
            let music = self
                .scene
                .bank
                .prepare("ritual_ambience", None, Vec3::ZERO, start)?;
            let progress = music.progress();
            output.play_prepared(music)?;
            self.scene
                .bind_zone_music("chamber", "ritual_ambience", progress)?;
            Ok(output)
        })();
        match result {
            Ok(output) => {
                self.output = Some(output);
                self.scene.output_available = true;
                self.status.clear();
            }
            Err(error) => {
                self.scene.output_available = false;
                self.status = error;
            }
        }
    }
    pub fn adjust_volume(&mut self, delta: f32) {
        let _ = self.scene.volume((self.scene.master + delta).clamp(0., 1.));
        if let Some(output) = &self.output {
            let _ = output.volume(self.scene.master, self.scene.buses);
        }
    }
    pub fn adjust_music(&mut self, delta: f32) {
        let _ = self
            .scene
            .bus_volume(Bus::Music, (self.scene.buses[1] + delta).clamp(0., 1.));
        if let Some(output) = &self.output {
            let _ = output.volume(self.scene.master, self.scene.buses);
        }
    }
    pub fn hud(&self) -> Vec<String> {
        let mut lines = vec![format!(
            "Volume {:.0}% [F9/F10]  Music {:.0}% [F11/F12]{}",
            self.scene.master * 100.,
            self.scene.buses[1] * 100.,
            if self.scene.output_available {
                ""
            } else {
                "  Audio unavailable; captions active"
            }
        )];
        lines.extend(
            self.scene
                .captions(f64::from(self.time.max(0.)))
                .take(4)
                .map(|c| c.text.clone()),
        );
        lines
    }
    pub fn update(
        &mut self,
        game: &Game,
        view: View,
        markers: &[MarkerEvent],
    ) -> Result<(), String> {
        if self
            .output
            .as_ref()
            .is_some_and(|o| o.stats().device_errors > 0)
        {
            self.output = None;
            self.scene.output_available = false;
        }
        if self.output.is_none()
            && !self.scene.suspended
            && std::time::Instant::now() >= self.retry_at
        {
            self.resume();
        }
        if let Some(output) = &self.output {
            if let Err(error) = output.listener(view.eye, view.view_proj.row(0).truncate()) {
                self.status = error;
            }
        }
        if game.time < self.time {
            if let Some(output) = &self.output {
                let _ = output.stop_bus(Bus::Effects);
            }
            self.scene.clear_captions();
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
                        self.stop_life(old);
                    }
                }
                let position = if life == game.player_life() {
                    game.player
                } else {
                    game.actor_position(life.actor).unwrap_or(game.player)
                };
                match event.kind {
                    Kind::Damage { .. } => self.play(2, Some(life), position, 0.65, false)?,
                    Kind::Death => self.stop_life(life),
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
        serde_json::json!({"device":self.output.as_ref().map(Output::stats),"queued_by_cue":self.queued,"sample_rate":self.output.as_ref().map(Output::rate),"bank":self.scene.bank.digest,"master":self.scene.master,"buses":self.scene.buses,"music_position":self.scene.music_position(),"caption_drops":self.scene.caption_drops,"status":self.status})
    }
}
