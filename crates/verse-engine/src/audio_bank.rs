//! Admitted audio content and captions. Resolve on the control thread.
use crate::{
    audio::{Bus, Clip, Emitter, Prepared},
    audio_cues::{Cue, synthesize},
    core::LifeId,
};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Source {
    /// Canonical stereo little-endian float PCM, verified before publication.
    Stream {
        id: String,
        rate: u32,
        frames: u64,
        sha256: String,
    },
    Synth {
        id: String,
        cue: Cue,
        seed: u64,
    },
    Pcm {
        id: String,
        rate: u32,
        frames: usize,
        sha256: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CueSpec {
    pub id: String,
    pub source: String,
    pub bus: Bus,
    pub priority: u8,
    pub gain: f32,
    pub range: f32,
    pub pitch: f32,
    pub looping: bool,
    pub captions: BTreeMap<String, String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    pub version: u32,
    pub id: String,
    pub default_locale: String,
    pub sources: Vec<Source>,
    pub cues: Vec<CueSpec>,
}
pub struct Bank {
    spec: Spec,
    clips: BTreeMap<String, Clip>,
    streams: BTreeMap<String, (u32, u64, String)>,
    pub digest: String,
}
fn identifier(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
fn pcm_digest(clip: &Clip) -> String {
    let mut hash = Sha256::new();
    for sample in clip.samples() {
        hash.update(sample.to_le_bytes());
    }
    format!("{:x}", hash.finalize())
}
impl Bank {
    /// Synthetic sources resolve here; decoded assets must match their PCM pin.
    /// No path, network request, plugin, or callback decoder is selected by data.
    pub fn admit(spec: Spec, decoded: BTreeMap<String, Clip>) -> Result<Self, String> {
        if spec.version != 1
            || !identifier(&spec.id)
            || !identifier(&spec.default_locale)
            || spec.sources.is_empty()
            || spec.sources.len() > 256
            || spec.cues.is_empty()
            || spec.cues.len() > 256
        {
            return Err("Invalid audio bank header or capacity".into());
        }
        let mut clips = BTreeMap::new();
        let mut streams = BTreeMap::new();
        let mut source_ids = BTreeSet::new();
        let mut bytes = 0usize;
        let mut used_decoded = BTreeSet::new();
        for source in &spec.sources {
            let source_id = match source {
                Source::Stream { id, .. } | Source::Pcm { id, .. } | Source::Synth { id, .. } => id,
            };
            if !identifier(source_id) || !source_ids.insert(source_id.clone()) {
                return Err("Invalid or duplicate audio source".into());
            }
            let (id, clip) = match source {
                Source::Stream {
                    id,
                    rate,
                    frames,
                    sha256,
                } => {
                    if !(8000..=192000).contains(rate)
                        || *frames == 0
                        || *frames > u64::from(*rate) * 86400
                        || sha256.len() != 64
                        || !sha256
                            .bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                    {
                        return Err("Invalid streamed audio source".into());
                    }
                    streams.insert(id.clone(), (*rate, *frames, sha256.clone()));
                    continue;
                }
                Source::Synth { id, cue, seed } => (id, synthesize(*cue, 48000, *seed)?),
                Source::Pcm {
                    id,
                    rate,
                    frames,
                    sha256,
                } => {
                    let clip = decoded.get(id).ok_or("Missing decoded bank source")?;
                    if clip.rate() != *rate
                        || clip.samples().len() != *frames
                        || pcm_digest(clip) != *sha256
                    {
                        return Err("Audio source disagrees with its PCM pin".into());
                    }
                    used_decoded.insert(id.clone());
                    (id, clip.clone())
                }
            };
            if !identifier(id) || clips.contains_key(id) {
                return Err("Invalid or duplicate audio source".into());
            }
            bytes = bytes
                .checked_add(clip.samples().len() * 4)
                .ok_or("Audio bank byte overflow")?;
            if bytes > 32 * 1024 * 1024 {
                return Err("Decoded audio bank exceeds 32 MiB".into());
            }
            clips.insert(id.clone(), clip);
        }
        if used_decoded.len() != decoded.len() {
            return Err("Unreferenced decoded audio source".into());
        }
        let mut cues = BTreeSet::new();
        for cue in &spec.cues {
            if !identifier(&cue.id)
                || !cues.insert(cue.id.clone())
                || !source_ids.contains(&cue.source)
                || (streams.contains_key(&cue.source)
                    && (cue.looping || !matches!(cue.bus, Bus::Music | Bus::Dialogue)))
                || cue.captions.len() > 16
                || cue.captions.iter().any(|(locale, text)| {
                    !identifier(locale)
                        || text.is_empty()
                        || text.len() > 512
                        || text.chars().any(char::is_control)
                })
                || (!cue.captions.is_empty() && !cue.captions.contains_key(&spec.default_locale))
            {
                return Err("Invalid audio cue, source, or caption".into());
            }
            Emitter {
                life: None,
                position: Vec3::ZERO,
                range: cue.range,
                gain: cue.gain,
                pitch: cue.pitch,
                looping: cue.looping,
            }
            .validate()?;
        }
        let mut hash = Sha256::new();
        hash.update(serde_json::to_vec(&spec).map_err(|e| e.to_string())?);
        for (id, clip) in &clips {
            hash.update(id.as_bytes());
            hash.update(pcm_digest(clip).as_bytes());
        }
        Ok(Self {
            spec,
            clips,
            streams,
            digest: format!("{:x}", hash.finalize()),
        })
    }
    /// Verify an admitted reader on a control or decoding worker. No path is
    /// selected here. Returns a bounded feeder and its matching prepared voice.
    pub fn prepare_stream_reader<R: std::io::Read + std::io::Seek>(
        &self,
        id: &str,
        mut reader: R,
        start: u64,
        capacity: usize,
    ) -> Result<(Prepared, crate::audio_stream::Reader<R>), String> {
        let cue = self.cue(id).ok_or("Unknown audio cue")?;
        let (rate, frames, expected) = self
            .streams
            .get(&cue.source)
            .ok_or("Cue is not a streamed source")?;
        if start >= *frames {
            return Err("Stream restore position is past the source".into());
        }
        reader
            .seek(std::io::SeekFrom::Start(0))
            .map_err(|e| e.to_string())?;
        let mut hash = Sha256::new();
        let mut remaining = *frames;
        let mut bytes = [0u8; 8192];
        while remaining > 0 {
            let count = remaining.min(1024) as usize;
            let batch = &mut bytes[..count * 8];
            reader.read_exact(batch).map_err(|e| e.to_string())?;
            for sample in batch.chunks_exact(4) {
                let sample = f32::from_le_bytes(sample.try_into().unwrap());
                if !sample.is_finite() || sample.abs() > 1. {
                    return Err("Invalid streamed PCM sample".into());
                }
            }
            hash.update(batch);
            remaining -= count as u64;
        }
        if reader.read(&mut bytes[..1]).map_err(|e| e.to_string())? != 0
            || format!("{:x}", hash.finalize()) != *expected
        {
            return Err("Streamed PCM disagrees with its pin or length".into());
        }
        reader
            .seek(std::io::SeekFrom::Start(start * 8))
            .map_err(|e| e.to_string())?;
        let (feeder, stream) =
            crate::audio_stream::channel(*rate, capacity).map_err(|_| "Invalid stream capacity")?;
        let prepared = Prepared::stream(
            stream,
            Emitter {
                life: None,
                position: Vec3::ZERO,
                range: cue.range,
                gain: cue.gain,
                pitch: cue.pitch,
                looping: false,
            },
            cue.bus,
            cue.priority,
            start,
        )?;
        let reader = crate::audio_stream::Reader::new(reader, feeder, *frames - start)
            .map_err(|_| "Invalid stream length")?;
        Ok((prepared, reader))
    }
    pub fn original() -> Result<Self, String> {
        Self::admit(
            serde_json::from_str(include_str!("original_audio_bank.json"))
                .map_err(|e| e.to_string())?,
            BTreeMap::new(),
        )
    }
    pub fn cue(&self, id: &str) -> Option<&CueSpec> {
        self.spec.cues.iter().find(|c| c.id == id)
    }
    pub fn clip(&self, id: &str) -> Option<&Clip> {
        self.clips.get(id)
    }
    pub fn prepare(
        &self,
        id: &str,
        life: Option<LifeId>,
        position: Vec3,
        start: u64,
    ) -> Result<Prepared, String> {
        let cue = self.cue(id).ok_or("Unknown audio cue")?;
        Prepared::clip(
            self.clips[&cue.source].clone(),
            Emitter {
                life,
                position,
                range: cue.range,
                gain: cue.gain,
                pitch: cue.pitch,
                looping: cue.looping,
            },
            cue.bus,
            cue.priority,
            start,
        )
    }
    pub fn caption<'a>(&'a self, id: &str, locale: &str) -> Option<&'a str> {
        let cue = self.cue(id)?;
        cue.captions
            .get(locale)
            .or_else(|| cue.captions.get(&self.spec.default_locale))
            .map(String::as_str)
    }
}
#[derive(Clone, Debug)]
pub struct Caption {
    pub cue: String,
    pub life: Option<LifeId>,
    pub text: String,
    pub expires: f64,
    pub priority: u8,
}
/// Captions and volume survive loss of audio output. This owns no device.
pub struct Scene {
    pub bank: Arc<Bank>,
    locale: String,
    captions: Vec<Caption>,
    pub master: f32,
    pub buses: [f32; 4],
    pub output_available: bool,
    pub suspended: bool,
    pub caption_drops: u64,
    zone: Option<String>,
    music: Option<(String, crate::audio::Progress)>,
}
impl Scene {
    pub fn new(bank: Arc<Bank>, locale: &str) -> Result<Self, String> {
        if !identifier(locale) {
            return Err("Invalid audio locale".into());
        }
        Ok(Self {
            bank,
            locale: locale.into(),
            captions: Vec::with_capacity(32),
            master: 1.0,
            buses: [1.0; 4],
            output_available: false,
            suspended: false,
            caption_drops: 0,
            zone: None,
            music: None,
        })
    }
    pub fn volume(&mut self, master: f32) -> Result<(), String> {
        if !master.is_finite() || !(0.0..=1.0).contains(&master) {
            return Err("Invalid master volume".into());
        }
        self.master = master;
        Ok(())
    }
    pub fn bus_volume(&mut self, bus: Bus, volume: f32) -> Result<(), String> {
        if !volume.is_finite() || !(0.0..=1.0).contains(&volume) {
            return Err("Invalid audio bus volume".into());
        }
        self.buses[bus as usize] = volume;
        Ok(())
    }
    pub fn emit(
        &mut self,
        id: &str,
        life: Option<LifeId>,
        position: Vec3,
        now: f64,
    ) -> Result<Prepared, String> {
        let prepared = self.bank.prepare(id, life, position, 0)?;
        self.caption(id, life, now)?;
        Ok(prepared)
    }
    /// Streaming providers can publish the same authored caption independently.
    pub fn caption(&mut self, id: &str, life: Option<LifeId>, now: f64) -> Result<(), String> {
        if !now.is_finite() || now < 0.0 {
            return Err("Invalid audio presentation time".into());
        }
        self.bank.cue(id).ok_or("Unknown audio cue")?;
        self.captions.retain(|c| c.expires > now);
        if let Some(text) = self.bank.caption(id, &self.locale) {
            let priority = self.bank.cue(id).unwrap().priority;
            if let Some(c) = self
                .captions
                .iter_mut()
                .find(|c| c.cue == id && c.life == life)
            {
                c.expires = now + 3.0;
            } else {
                if self.captions.len() == 32 {
                    let victim = self
                        .captions
                        .iter()
                        .enumerate()
                        .filter(|(_, c)| c.priority < priority)
                        .min_by_key(|(_, c)| c.priority)
                        .map(|(i, _)| i);
                    if let Some(i) = victim {
                        self.captions.swap_remove(i);
                    } else {
                        self.caption_drops = self.caption_drops.saturating_add(1);
                        return Ok(());
                    }
                }
                self.captions.push(Caption {
                    cue: id.into(),
                    life,
                    text: text.into(),
                    expires: now + 3.0,
                    priority,
                });
                self.captions
                    .sort_by_key(|caption| std::cmp::Reverse(caption.priority));
            }
        }
        Ok(())
    }
    pub fn clear_captions(&mut self) {
        self.captions.clear();
    }
    pub fn captions(&self, now: f64) -> impl Iterator<Item = &Caption> {
        self.captions.iter().filter(move |c| c.expires > now)
    }
    /// On a new zone, the adapter releases old music before submitting this cue.
    /// A retry within the same zone restores the observed source-frame position.
    pub fn enter_zone(
        &mut self,
        zone: &str,
        music: Option<&str>,
    ) -> Result<Option<Prepared>, String> {
        if !identifier(zone) {
            return Err("Invalid audio zone identity".into());
        }
        if let Some(id) = music {
            let cue = self.bank.cue(id).ok_or("Unknown zone music cue")?;
            if cue.bus != Bus::Music {
                return Err("Zone music cue must use the music bus".into());
            }
            let start = self.zone_music_position(zone, id)?;
            let prepared = self.bank.prepare(id, None, Vec3::ZERO, start)?;
            self.bind_zone_music(zone, id, prepared.progress())?;
            Ok(Some(prepared))
        } else {
            self.zone = Some(zone.into());
            self.music = None;
            Ok(None)
        }
    }
    /// Prepare from this position, submit successfully, then bind the new clock.
    pub fn zone_music_position(&self, zone: &str, id: &str) -> Result<u64, String> {
        if !identifier(zone) || self.bank.cue(id).is_none_or(|c| c.bus != Bus::Music) {
            return Err("Invalid zone music binding".into());
        }
        Ok(if self.zone.as_deref() == Some(zone) {
            self.music
                .as_ref()
                .filter(|(old, _)| old == id)
                .map_or(0, |(_, p)| p.frame())
        } else {
            0
        })
    }
    pub fn bind_zone_music(
        &mut self,
        zone: &str,
        id: &str,
        progress: crate::audio::Progress,
    ) -> Result<(), String> {
        self.zone_music_position(zone, id)?;
        if self.zone.as_deref() != Some(zone) {
            self.clear_captions();
        }
        self.zone = Some(zone.into());
        self.music = Some((id.into(), progress));
        Ok(())
    }
    pub fn music_position(&self) -> Option<u64> {
        self.music.as_ref().map(|(_, p)| p.frame())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authored_bank_is_pinned_and_invalid_edits_refuse() {
        let bank = Bank::original().unwrap();
        let mut spec = bank.spec.clone();
        assert_eq!(Bank::original().unwrap().digest, bank.digest);
        assert_eq!(bank.caption("impact", "unknown"), Some("Impact"));
        spec.cues.push(spec.cues[0].clone());
        assert!(Bank::admit(spec, BTreeMap::new()).is_err());
        let mut spec = bank.spec.clone();
        spec.cues[0].gain = f32::NAN;
        assert!(Bank::admit(spec, BTreeMap::new()).is_err());
        let mut spec = bank.spec.clone();
        spec.cues[0].source = "missing".into();
        assert!(Bank::admit(spec, BTreeMap::new()).is_err());
    }
    #[test]
    fn pcm_pins_localization_and_bank_capacity_are_enforced() {
        let original = Bank::original().unwrap();
        let clip = Clip::new(vec![0.25; 8000], 8000).unwrap();
        let mut spec = original.spec.clone();
        spec.sources = vec![Source::Pcm {
            id: "decoded".into(),
            rate: 8000,
            frames: 8000,
            sha256: pcm_digest(&clip),
        }];
        spec.cues = vec![spec.cues[0].clone()];
        spec.cues[0].source = "decoded".into();
        let decoded = BTreeMap::from([("decoded".into(), clip.clone())]);
        assert!(Bank::admit(spec.clone(), decoded.clone()).is_ok());
        let wrong = BTreeMap::from([("decoded".into(), Clip::new(vec![0.5; 8000], 8000).unwrap())]);
        assert!(Bank::admit(spec.clone(), wrong).is_err());
        spec.cues[0].captions.remove("en");
        spec.cues[0].captions.insert("fr".into(), "Pas".into());
        assert!(Bank::admit(spec.clone(), decoded.clone()).is_err());
        spec.cues[0]
            .captions
            .insert("en".into(), "Footsteps".into());
        let bank = Bank::admit(spec.clone(), decoded.clone()).unwrap();
        assert_eq!(bank.caption("footstep", "fr"), Some("Pas"));
        spec.cues[0]
            .captions
            .insert("en".into(), "Bad\ncaption".into());
        assert!(Bank::admit(spec, decoded).is_err());
        let mut spec = original.spec.clone();
        spec.sources = vec![spec.sources[0].clone(); 257];
        assert!(Bank::admit(spec, BTreeMap::new()).is_err());
    }
    #[test]
    fn authored_long_stream_is_verified_before_a_frame_can_play() {
        let original = Bank::original().unwrap();
        let mut spec = original.spec.clone();
        let bytes = [0.25f32, -0.25]
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .cycle()
            .take(8000)
            .collect::<Vec<_>>();
        let hash = format!("{:x}", Sha256::digest(&bytes));
        spec.sources = vec![Source::Stream {
            id: "track".into(),
            rate: 48000,
            frames: 1000,
            sha256: hash,
        }];
        spec.cues = vec![spec.cues[4].clone()];
        spec.cues[0].source = "track".into();
        spec.cues[0].looping = false;
        let bank = Bank::admit(spec, BTreeMap::new()).unwrap();
        let mut changed = bytes.clone();
        changed[0] = 1;
        assert!(
            bank.prepare_stream_reader("ritual_ambience", std::io::Cursor::new(changed), 0, 128)
                .is_err()
        );
        let bank = Arc::new(bank);
        let mut scene = Scene::new(bank.clone(), "en").unwrap();
        scene.caption("ritual_ambience", None, 1.).unwrap();
        assert_eq!(scene.captions(2.).count(), 1);
        let (prepared, mut reader) = bank
            .prepare_stream_reader("ritual_ambience", std::io::Cursor::new(bytes), 500, 128)
            .unwrap();
        assert_eq!(reader.remaining(), 500);
        let progress = prepared.progress();
        scene
            .bind_zone_music("forest", "ritual_ambience", progress.clone())
            .unwrap();
        reader.pump().unwrap();
        let mut mixer = crate::audio::Mixer::new(48000).unwrap();
        assert!(mixer.play_rt(prepared).is_ok());
        mixer.render_rt(&mut [0.; 128]).unwrap();
        assert_eq!(progress.frame(), 564);
        assert_eq!(
            scene
                .zone_music_position("forest", "ritual_ambience")
                .unwrap(),
            564
        );
        assert_eq!(
            scene
                .zone_music_position("chamber", "ritual_ambience")
                .unwrap(),
            0
        );
    }
    #[test]
    fn a_small_hud_shows_critical_captions_before_crowded_footsteps() {
        let mut scene = Scene::new(Arc::new(Bank::original().unwrap()), "en").unwrap();
        for actor in 1..=32 {
            scene
                .caption(
                    "footstep",
                    Some(LifeId {
                        instance: 1,
                        actor,
                        generation: 1,
                    }),
                    1.,
                )
                .unwrap();
        }
        scene.caption("impact", None, 2.).unwrap();
        let visible: Vec<_> = scene
            .captions(2.)
            .take(4)
            .map(|caption| caption.text.as_str())
            .collect();
        assert_eq!(visible[0], "Impact");
        assert_eq!(visible.len(), 4);
        assert_eq!(scene.captions(2.).count(), 32);
        assert_eq!(scene.captions(4.5).count(), 1);
    }
    #[test]
    fn silent_output_keeps_captions_volumes_and_music_restore_position() {
        let mut scene = Scene::new(Arc::new(Bank::original().unwrap()), "en").unwrap();
        scene.volume(0.0).unwrap();
        assert!(!scene.output_available);
        let cue = scene.emit("impact", None, Vec3::ZERO, 1.0).unwrap();
        drop(cue);
        assert_eq!(scene.captions(2.0).next().unwrap().text, "Impact");
        assert!(scene.captions(5.0).next().is_none());
        let mut mixer = crate::audio::Mixer::new(48000).unwrap();
        let music = scene
            .enter_zone("chamber", Some("ritual_ambience"))
            .unwrap()
            .unwrap();
        mixer.play_rt(music).ok().unwrap();
        mixer.render_rt(&mut [0.0; 1024]).unwrap();
        let position = scene.music_position().unwrap();
        assert_eq!(position, 512);
        mixer.suspend(true);
        mixer.render_rt(&mut [1.0; 1024]).unwrap();
        assert_eq!(scene.music_position(), Some(position));
        let restored = scene
            .enter_zone("chamber", Some("ritual_ambience"))
            .unwrap()
            .unwrap();
        assert_eq!(restored.progress().frame(), position);
        let fresh = scene
            .enter_zone("forest", Some("ritual_ambience"))
            .unwrap()
            .unwrap();
        assert_eq!(fresh.progress().frame(), 0);
    }
}
