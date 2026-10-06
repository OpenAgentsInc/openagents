//! Owned bounded PCM mixing. Device output and gameplay authority remain adapters.
use crate::core::LifeId;
use glam::Vec3;
use std::sync::Arc;

#[derive(Clone)]
pub struct Clip {
    samples: Arc<[f32]>,
    rate: u32,
}
impl Clip {
    pub fn new(samples: Vec<f32>, rate: u32) -> Result<Self, String> {
        if !(8000..=192000).contains(&rate)
            || samples.is_empty()
            || samples.len() > rate as usize * 60
            || samples
                .iter()
                .any(|sample| !sample.is_finite() || sample.abs() > 1.)
        {
            return Err("Invalid audio PCM samples, rate, or duration".into());
        }
        Ok(Self {
            samples: samples.into(),
            rate,
        })
    }
    pub fn samples(&self) -> &[f32] {
        &self.samples
    }
    pub fn rate(&self) -> u32 {
        self.rate
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VoiceId(u64);
#[derive(Clone, Copy)]
pub struct Emitter {
    pub life: Option<LifeId>,
    pub position: Vec3,
    pub range: f32,
    pub gain: f32,
    pub pitch: f32,
    pub looping: bool,
}
impl Emitter {
    pub fn validate(self) -> Result<(), String> {
        if !self.position.is_finite()
            || self.position.abs().max_element() > 1_000_000.
            || !self.range.is_finite()
            || !(0.01..=10000.).contains(&self.range)
            || !self.gain.is_finite()
            || !(0. ..=1.).contains(&self.gain)
            || !self.pitch.is_finite()
            || !(0.25..=4.).contains(&self.pitch)
        {
            return Err("Invalid spatial audio emitter".into());
        }
        Ok(())
    }
}

pub const MAX_VOICES: usize = 128;
pub const AUDIBLE_VOICES: usize = 32;
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Bus {
    Effects,
    Music,
    Dialogue,
    Interface,
}
impl Bus {
    fn index(self) -> usize {
        self as usize
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Capacity,
    Retirement,
    Identity,
    Block,
}
#[derive(Clone, Default)]
pub struct Progress(Arc<std::sync::atomic::AtomicU64>);
impl Progress {
    fn at(frame: u64) -> Self {
        Self(Arc::new(std::sync::atomic::AtomicU64::new(frame)))
    }
    pub fn frame(&self) -> u64 {
        self.0.load(std::sync::atomic::Ordering::Acquire)
    }
}
enum Source {
    Clip(Clip),
    Stream(crate::audio_stream::Stream),
}
/// Construct on the control thread. Callback admission only moves prepared data.
pub struct Prepared {
    source: Source,
    emitter: Emitter,
    bus: Bus,
    priority: u8,
    start: u64,
    progress: Progress,
}
impl Prepared {
    pub fn clip(
        clip: Clip,
        emitter: Emitter,
        bus: Bus,
        priority: u8,
        start: u64,
    ) -> Result<Self, String> {
        emitter.validate()?;
        if start > (1_u64 << 53) {
            return Err("Audio resume position exceeds exact frame precision".into());
        }
        Ok(Self {
            source: Source::Clip(clip),
            emitter,
            bus,
            priority,
            start,
            progress: Progress::at(start),
        })
    }
    pub fn stream(
        stream: crate::audio_stream::Stream,
        emitter: Emitter,
        bus: Bus,
        priority: u8,
        start: u64,
    ) -> Result<Self, String> {
        emitter.validate()?;
        if emitter.looping || start > (1_u64 << 53) {
            return Err("Streaming loops belong to the feeder; invalid resume position".into());
        }
        Ok(Self {
            source: Source::Stream(stream),
            emitter,
            bus,
            priority,
            start,
            progress: Progress::at(start),
        })
    }
    /// Converts an admitted clip to bounded decoded streaming on the control thread.
    /// The returned feeder can run on an I/O worker; its consumer stays in the voice.
    pub fn into_streaming(self, frames: usize) -> Result<(Self, LoopFeeder), String> {
        let Self {
            source,
            mut emitter,
            bus,
            priority,
            start,
            progress,
        } = self;
        let Source::Clip(clip) = source else {
            return Err("Audio source is already streaming".into());
        };
        let (feeder, stream) = crate::audio_stream::channel(clip.rate, frames)
            .map_err(|_| "Invalid audio stream capacity")?;
        let looping = emitter.looping;
        emitter.looping = false;
        let worker = LoopFeeder {
            feeder,
            clip,
            cursor: start,
            looping,
            finished: false,
        };
        Ok((
            Self {
                source: Source::Stream(stream),
                emitter,
                bus,
                priority,
                start,
                progress,
            },
            worker,
        ))
    }
    pub fn is_streaming(&self) -> bool {
        matches!(self.source, Source::Stream(_))
    }
    pub fn bus(&self) -> Bus {
        self.bus
    }
    pub fn progress(&self) -> Progress {
        self.progress.clone()
    }
    pub fn priority(&self) -> u8 {
        self.priority
    }
}
/// An original or admitted PCM source streamed in at most 1024 frames per pump.
pub struct LoopFeeder {
    feeder: crate::audio_stream::Feeder,
    clip: Clip,
    cursor: u64,
    looping: bool,
    finished: bool,
}
impl LoopFeeder {
    pub fn closed(&self) -> bool {
        self.feeder.closed()
    }
    pub fn pump(&mut self) -> usize {
        if self.finished || self.closed() {
            return 0;
        }
        let count = self.feeder.slots().min(1024);
        let mut written = 0;
        for _ in 0..count {
            if !self.looping && self.cursor >= self.clip.samples.len() as u64 {
                self.feeder.finish();
                self.finished = true;
                break;
            }
            let sample = self.clip.samples[self.cursor as usize % self.clip.samples.len()]
                * std::f32::consts::FRAC_1_SQRT_2;
            if self.feeder.push([sample; 2]).is_err() {
                break;
            }
            self.cursor = self.cursor.saturating_add(1);
            written += 1;
        }
        written
    }
}

/// Drop this value on a control or reclamation thread, never in a callback.
pub struct Retired {
    _prepared: Prepared,
}
struct Voice {
    id: VoiceId,
    prepared: Prepared,
    cursor: f64,
    position: f64,
    age: u64,
    release: Option<u32>,
    release_total: u32,
    done: bool,
}
#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
pub struct MixStats {
    pub audible: usize,
    pub virtual_voices: usize,
    pub stolen: u64,
    pub stream_gaps: u64,
    pub retirement_blocked: u64,
}
pub struct Mixer {
    rate: u32,
    listener: Vec3,
    right: Vec3,
    voices: Vec<Voice>,
    retired: Vec<Retired>,
    next_id: u64,
    volumes: [f32; 4],
    master: f32,
    suspended: bool,
    pub stats: MixStats,
}
impl Mixer {
    pub fn new(rate: u32) -> Result<Self, String> {
        if !(8000..=192000).contains(&rate) {
            return Err("Invalid audio output rate".into());
        }
        Ok(Self {
            rate,
            listener: Vec3::ZERO,
            right: Vec3::X,
            voices: Vec::with_capacity(MAX_VOICES),
            retired: Vec::with_capacity(MAX_VOICES),
            next_id: 0,
            volumes: [1.0; 4],
            master: 1.0,
            suspended: false,
            stats: Default::default(),
        })
    }
    pub fn set_listener(&mut self, position: Vec3, right: Vec3) -> Result<(), String> {
        if !position.is_finite()
            || position.abs().max_element() > 1_000_000.
            || !right.is_finite()
            || right.length_squared() < 1e-8
            || !right.length_squared().is_finite()
        {
            return Err("Invalid audio listener pose".into());
        }
        self.listener = position;
        self.right = right.normalize();
        Ok(())
    }
    pub fn volume(&mut self, bus: Bus, volume: f32) -> Result<(), String> {
        if !volume.is_finite() || !(0.0..=1.0).contains(&volume) {
            return Err("Invalid audio bus volume".into());
        }
        self.volumes[bus.index()] = volume;
        Ok(())
    }
    pub fn master(&mut self, volume: f32) -> Result<(), String> {
        if !volume.is_finite() || !(0.0..=1.0).contains(&volume) {
            return Err("Invalid master volume".into());
        }
        self.master = volume;
        Ok(())
    }
    pub fn suspend(&mut self, suspended: bool) {
        self.suspended = suspended;
    }
    pub fn retirement_slots(&self) -> usize {
        MAX_VOICES - self.retired.len()
    }
    pub fn pop_retired(&mut self) -> Option<Retired> {
        self.retired.pop()
    }
    pub fn return_retired(&mut self, retired: Retired) {
        assert!(self.retired.len() < MAX_VOICES);
        self.retired.push(retired);
    }
    pub fn retire_prepared(&mut self, prepared: Prepared) -> Result<(), Prepared> {
        if self.retirement_slots() == 0 {
            return Err(prepared);
        }
        self.retired.push(Retired {
            _prepared: prepared,
        });
        Ok(())
    }
    fn retire_voice(&mut self, index: usize) {
        let voice = self.voices.swap_remove(index);
        self.retired.push(Retired {
            _prepared: voice.prepared,
        });
    }
    /// No allocation, locking, or resource destruction. Returns refused ownership.
    pub fn play_rt(&mut self, prepared: Prepared) -> Result<VoiceId, (Refusal, Prepared)> {
        let Some(next) = self.next_id.checked_add(1) else {
            return Err((Refusal::Identity, prepared));
        };
        // Preflight every limit before displacing an admitted source.
        let music_victim = (prepared.bus == Bus::Music
            && self
                .voices
                .iter()
                .filter(|v| v.prepared.bus == Bus::Music)
                .count()
                >= 2)
            .then(|| {
                self.voices
                    .iter()
                    .enumerate()
                    .filter(|(_, v)| v.prepared.bus == Bus::Music)
                    .min_by_key(|(_, v)| v.id.0)
                    .unwrap()
                    .0
            });
        let victim = if music_victim.is_some() {
            music_victim
        } else if self.voices.len() == MAX_VOICES {
            let victim = self
                .voices
                .iter()
                .enumerate()
                .filter(|(_, v)| {
                    (v.prepared.bus == Bus::Effects && prepared.bus != Bus::Effects)
                        || (v.prepared.bus == prepared.bus
                            && v.prepared.priority < prepared.priority)
                })
                .min_by_key(|(_, v)| (v.prepared.priority, v.id.0))
                .map(|(i, _)| i);
            if victim.is_none() {
                return Err((Refusal::Capacity, prepared));
            }
            victim
        } else {
            None
        };
        if victim.is_some() && self.retirement_slots() == 0 {
            return Err((Refusal::Retirement, prepared));
        }
        let streams = self
            .voices
            .iter()
            .enumerate()
            .filter(|(i, v)| Some(*i) != victim && matches!(v.prepared.source, Source::Stream(_)))
            .count();
        if matches!(prepared.source, Source::Stream(_)) && streams >= 8 {
            return Err((Refusal::Capacity, prepared));
        }
        if let Some(victim) = victim {
            self.retire_voice(victim);
            self.stats.stolen = self.stats.stolen.saturating_add(1);
        }
        let mut cursor = prepared.start as f64;
        if let Source::Clip(clip) = &prepared.source {
            if prepared.emitter.looping {
                cursor %= clip.samples.len() as f64;
            }
        } else {
            cursor = 0.0;
        }
        let position = prepared.start as f64;
        let voice = Voice {
            id: VoiceId(next),
            prepared,
            cursor,
            position,
            age: 0,
            release: None,
            release_total: 0,
            done: false,
        };
        self.next_id = next;
        self.voices.push(voice);
        Ok(VoiceId(next))
    }
    /// Legacy control-thread entry. Same-priority voices retain refusal behavior.
    pub fn play(&mut self, clip: &Clip, emitter: Emitter) -> Result<VoiceId, String> {
        let prepared = Prepared::clip(clip.clone(), emitter, Bus::Effects, 0, 0)?;
        self.play_rt(prepared)
            .map_err(|(reason, _)| format!("Audio voice refused: {reason:?}"))
    }
    pub fn voice_count(&self) -> usize {
        self.voices.len()
    }
    pub fn stop_life(&mut self, life: LifeId) {
        for voice in &mut self.voices {
            if voice.prepared.emitter.life == Some(life) && voice.release.is_none() {
                voice.release = Some(self.rate / 100);
                voice.release_total = self.rate / 100;
            }
        }
    }
    pub fn stop(&mut self, id: VoiceId) -> bool {
        let Some(voice) = self.voices.iter_mut().find(|v| v.id == id) else {
            return false;
        };
        if voice.release.is_none() {
            voice.release = Some(self.rate / 100);
            voice.release_total = self.rate / 100;
        }
        true
    }
    pub fn stop_bus(&mut self, bus: Bus) {
        for voice in &mut self.voices {
            if voice.prepared.bus == bus && voice.release.is_none() {
                voice.release = Some(self.rate / 5);
                voice.release_total = self.rate / 5;
            }
        }
    }
    pub fn render(&mut self, output: &mut [f32]) -> Result<(), String> {
        self.render_rt(output)
            .map_err(|_| "Invalid audio output block size".into())
    }
    pub fn render_rt(&mut self, output: &mut [f32]) -> Result<(), Refusal> {
        if !output.len().is_multiple_of(2) || output.len() > 16384 {
            return Err(Refusal::Block);
        }
        output.fill(0.0);
        if self.suspended {
            return Ok(());
        }
        let frames = output.len() / 2;
        let mut order = [0usize; MAX_VOICES];
        for (i, index) in order[..self.voices.len()].iter_mut().enumerate() {
            *index = i;
        }
        order[..self.voices.len()].sort_unstable_by_key(|&i| {
            (
                matches!(self.voices[i].prepared.bus, Bus::Effects),
                std::cmp::Reverse(self.voices[i].prepared.priority),
                self.voices[i].id.0,
            )
        });
        let mut selected = [false; MAX_VOICES];
        let mut audible = 0;
        for &i in &order[..self.voices.len()] {
            let voice = &self.voices[i];
            let emitter = voice.prepared.emitter;
            let spatial = matches!(voice.prepared.bus, Bus::Effects);
            if !voice.done
                && emitter.gain > 0.0
                && self.volumes[voice.prepared.bus.index()] > 0.0
                && (!spatial || (emitter.position - self.listener).length() < emitter.range)
            {
                if audible < AUDIBLE_VOICES {
                    selected[i] = true;
                    audible += 1;
                }
            }
        }
        self.stats.audible = audible;
        self.stats.virtual_voices = self.voices.len() - audible;
        let dialogue = self
            .voices
            .iter()
            .enumerate()
            .any(|(i, v)| selected[i] && v.prepared.bus == Bus::Dialogue);
        let fade = f64::from(self.rate) / 100.0;
        for (i, voice) in self.voices.iter_mut().enumerate() {
            if voice.done {
                continue;
            }
            let emitter = voice.prepared.emitter;
            let spatial = matches!(voice.prepared.bus, Bus::Effects);
            let delta = emitter.position - self.listener;
            let distance = delta.length();
            let attenuation = if spatial {
                (1.0 - distance / emitter.range).clamp(0.0, 1.0).powi(2)
            } else {
                1.0
            };
            let pan = if spatial && distance > 1e-8 {
                (delta / distance).dot(self.right).clamp(-1.0, 1.0)
            } else {
                0.0
            };
            let mono = matches!(voice.prepared.source, Source::Clip(_));
            let balance = if spatial || mono {
                [((1.0 - pan) * 0.5).sqrt(), ((1.0 + pan) * 0.5).sqrt()]
            } else {
                [1.0; 2]
            };
            let rate = match &voice.prepared.source {
                Source::Clip(c) => c.rate,
                Source::Stream(s) => s.rate,
            };
            let step = f64::from(rate) / f64::from(self.rate) * f64::from(emitter.pitch);
            if !selected[i] {
                if let Source::Clip(clip) = &voice.prepared.source {
                    let count =
                        frames.min(voice.release.map_or(frames, |remaining| remaining as usize));
                    let count = if emitter.looping {
                        count
                    } else {
                        count.min(
                            ((clip.samples.len() as f64 - voice.cursor).max(0.0) / step).ceil()
                                as usize,
                        )
                    };
                    voice.cursor += count as f64 * step;
                    voice.position += count as f64 * step;
                    voice.age = voice.age.saturating_add(count as u64);
                    if emitter.looping {
                        voice.cursor %= clip.samples.len() as f64;
                    }
                    if let Some(remaining) = &mut voice.release {
                        *remaining = remaining.saturating_sub(count as u32);
                    }
                    voice.done = voice.release == Some(0)
                        || (!emitter.looping && voice.cursor >= clip.samples.len() as f64);
                    voice
                        .prepared
                        .progress
                        .0
                        .store(voice.position as u64, std::sync::atomic::Ordering::Release);
                    continue;
                }
            }
            let bus = self.volumes[voice.prepared.bus.index()]
                * if dialogue && voice.prepared.bus == Bus::Music {
                    0.25
                } else {
                    1.0
                };
            for frame in output.chunks_exact_mut(2) {
                if voice.release == Some(0) {
                    voice.done = true;
                    break;
                }
                let sample = match &mut voice.prepared.source {
                    Source::Clip(clip) => {
                        let length = clip.samples.len();
                        if voice.cursor >= length as f64 {
                            voice.done = true;
                            break;
                        }
                        let index = voice.cursor.floor() as usize;
                        let next = if index + 1 < length {
                            clip.samples[index + 1]
                        } else if emitter.looping {
                            clip.samples[0]
                        } else {
                            0.0
                        };
                        let sample = clip.samples[index]
                            + (next - clip.samples[index]) * voice.cursor.fract() as f32;
                        voice.cursor += step;
                        if emitter.looping {
                            voice.cursor %= length as f64;
                        }
                        [sample; 2]
                    }
                    Source::Stream(stream) => match stream.sample(step) {
                        crate::audio_stream::Sample::Frame(sample) => sample,
                        crate::audio_stream::Sample::End => {
                            voice.done = true;
                            break;
                        }
                        crate::audio_stream::Sample::Gap => {
                            self.stats.stream_gaps = self.stats.stream_gaps.saturating_add(1);
                            voice.age = voice.age.saturating_add(1);
                            if let Some(remaining) = &mut voice.release {
                                *remaining = remaining.saturating_sub(1);
                            }
                            if voice.release == Some(0) {
                                voice.done = true;
                                break;
                            }
                            continue;
                        }
                    },
                };
                let attack = (voice.age as f64 / fade).min(1.0);
                let release = voice.release.map_or(1.0, |left| {
                    f64::from(left) / f64::from(voice.release_total.max(1))
                });
                let tail = match &voice.prepared.source {
                    Source::Clip(clip) if !emitter.looping => {
                        ((clip.samples.len() as f64 - voice.cursor + step) / step / fade)
                            .clamp(0.0, 1.0)
                    }
                    _ => 1.0,
                };
                let gain = emitter.gain
                    * attenuation
                    * self.master
                    * bus
                    * (attack * release * tail) as f32;
                if selected[i] {
                    for channel in 0..2 {
                        frame[channel] += sample[channel] * gain * balance[channel];
                    }
                }
                voice.age = voice.age.saturating_add(1);
                voice.position += step;
                if let Some(left) = &mut voice.release {
                    *left = left.saturating_sub(1);
                }
            }
            voice
                .prepared
                .progress
                .0
                .store(voice.position as u64, std::sync::atomic::Ordering::Release);
            if let Source::Clip(clip) = &voice.prepared.source {
                if !emitter.looping && voice.cursor >= clip.samples.len() as f64 {
                    voice.done = true;
                }
            }
            if voice.release == Some(0) {
                voice.done = true;
            }
        }
        let mut i = 0;
        while i < self.voices.len() {
            if self.voices[i].done && self.retirement_slots() > 0 {
                self.retire_voice(i);
            } else {
                if self.voices[i].done {
                    self.stats.retirement_blocked = self.stats.retirement_blocked.saturating_add(1);
                }
                i += 1;
            }
        }
        for sample in output {
            *sample = sample.clamp(-1.0, 1.0);
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn emitter() -> Emitter {
        Emitter {
            life: Some(LifeId {
                instance: 1,
                actor: 2,
                generation: 0,
            }),
            position: Vec3::X,
            range: 10.,
            gain: 1.,
            pitch: 1.,
            looping: false,
        }
    }
    #[test]
    fn resamples_spatializes_and_retires_finished_pcm() {
        let clip = Clip::new(vec![0.5; 800], 8000).unwrap();
        let mut mixer = Mixer::new(16000).unwrap();
        mixer.play(&clip, emitter()).unwrap();
        let mut output = vec![0.; 3200];
        mixer.render(&mut output).unwrap();
        assert!(output.chunks_exact(2).all(|f| f[0] == 0.));
        assert!(output.iter().any(|v| *v > 0.2));
        assert_eq!(mixer.voice_count(), 0);
    }
    #[test]
    fn exact_life_release_preserves_a_new_generation() {
        let clip = Clip::new(vec![0.5; 800], 8000).unwrap();
        let mut mixer = Mixer::new(8000).unwrap();
        let old = emitter();
        let mut new = old;
        new.life = Some(old.life.unwrap().next().unwrap());
        new.looping = true;
        mixer.play(&clip, old).unwrap();
        let id = mixer.play(&clip, new).unwrap();
        mixer.stop_life(old.life.unwrap());
        mixer.render(&mut [0.; 1600]).unwrap();
        assert_eq!(mixer.voice_count(), 1);
        assert!(mixer.stop(id));
        mixer.render(&mut [0.; 160]).unwrap();
        assert_eq!(mixer.voice_count(), 0);
    }
    #[test]
    fn refuses_invalid_input_and_bounds_capacity_without_stealing() {
        assert!(Clip::new(vec![f32::NAN], 8000).is_err());
        let mut invalid_emitter = emitter();
        invalid_emitter.position = Vec3::splat(f32::MAX);
        assert!(invalid_emitter.validate().is_err());
        let clip = Clip::new(vec![0.; 80], 8000).unwrap();
        let mut mixer = Mixer::new(8000).unwrap();
        let mut e = emitter();
        e.looping = true;
        for _ in 0..128 {
            mixer.play(&clip, e).unwrap();
        }
        assert!(mixer.play(&clip, e).is_err());
        let mut invalid = [0.75; 3];
        assert!(mixer.render(&mut invalid).is_err());
        assert_eq!(invalid, [0.75; 3]);
        assert_eq!(mixer.voice_count(), 128);
    }
}

#[cfg(test)]
mod rt_tests {
    use super::*;
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;
    std::thread_local! {static TRACK:Cell<bool>=const {Cell::new(false)}; static OPS:Cell<(u64,u64)>=const {Cell::new((0,0))};}
    struct Allocator;
    #[global_allocator]
    static ALLOC: Allocator = Allocator;
    unsafe impl GlobalAlloc for Allocator {
        unsafe fn alloc(&self, l: Layout) -> *mut u8 {
            TRACK.with(|t| {
                if t.get() {
                    OPS.with(|o| {
                        let (a, d) = o.get();
                        o.set((a + 1, d));
                    })
                }
            });
            unsafe { System.alloc(l) }
        }
        unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
            TRACK.with(|t| {
                if t.get() {
                    OPS.with(|o| {
                        let (a, d) = o.get();
                        o.set((a, d + 1));
                    })
                }
            });
            unsafe { System.dealloc(p, l) }
        }
        unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
            TRACK.with(|t| {
                if t.get() {
                    OPS.with(|o| {
                        let (a, d) = o.get();
                        o.set((a + 1, d + 1));
                    })
                }
            });
            unsafe { System.realloc(p, l, n) }
        }
    }
    fn emitter(looping: bool) -> Emitter {
        Emitter {
            life: None,
            position: Vec3::ZERO,
            range: 10.,
            gain: 0.01,
            pitch: 1.,
            looping,
        }
    }
    #[test]
    fn callback_admission_render_and_retirement_never_allocate_or_free() {
        let clip = Clip::new(vec![0.5; 480], 48000).unwrap();
        let weak = Arc::downgrade(&clip.samples);
        let prepared = Prepared::clip(clip, emitter(false), Bus::Effects, 1, 0).unwrap();
        let mut mixer = Mixer::new(48000).unwrap();
        let mut pcm = [0.; 1024];
        OPS.with(|o| o.set((0, 0)));
        TRACK.with(|t| t.set(true));
        let admitted = mixer.play_rt(prepared).is_ok();
        let rendered = mixer.render_rt(&mut pcm).is_ok();
        let retired = mixer.pop_retired();
        TRACK.with(|t| t.set(false));
        assert!(admitted && rendered && retired.is_some());
        assert_eq!(OPS.with(Cell::get), (0, 0));
        assert!(weak.upgrade().is_some());
        drop(retired);
        assert!(weak.upgrade().is_none());
    }
    #[test]
    fn crowd_pressure_protects_music_and_critical_effects_and_virtual_time() {
        let clip = Clip::new(vec![0.5; 48000], 48000).unwrap();
        let mut mixer = Mixer::new(48000).unwrap();
        let music = Prepared::clip(clip.clone(), emitter(true), Bus::Music, 1, 0).unwrap();
        let progress = music.progress();
        assert!(mixer.play_rt(music).is_ok());
        for _ in 0..127 {
            assert!(
                mixer
                    .play_rt(
                        Prepared::clip(clip.clone(), emitter(true), Bus::Effects, 20, 0).unwrap()
                    )
                    .is_ok()
            );
        }
        let mut critical_emitter = emitter(false);
        critical_emitter.gain = 0.9;
        let critical =
            Prepared::clip(clip.clone(), critical_emitter, Bus::Effects, 220, 0).unwrap();
        let cp = critical.progress();
        assert!(mixer.play_rt(critical).is_ok());
        let mut pcm = [0.; 1024];
        mixer.render_rt(&mut pcm).unwrap();
        assert_eq!(progress.frame(), 512);
        assert_eq!(cp.frame(), 512);
        assert!(
            pcm.iter().any(|v| *v > 0.4),
            "The critical voice must contribute audible PCM"
        );
        assert_eq!(mixer.stats.audible, 32);
        assert_eq!(mixer.stats.virtual_voices, 96);
        assert_eq!(mixer.stats.stolen, 1);
        assert!(mixer.voices.iter().any(|v| v.prepared.bus == Bus::Music));
        assert!(
            mixer
                .voices
                .iter()
                .all(|v| v.prepared.progress.frame() == 512)
        );
    }
    #[test]
    fn stream_limit_refusal_preserves_admitted_sources() {
        let clip = Clip::new(vec![0.2; 8000], 8000).unwrap();
        let mut mixer = Mixer::new(8000).unwrap();
        let mut feeders = Vec::new();
        for _ in 0..8 {
            let (feeder, stream) = crate::audio_stream::channel(8000, 128).unwrap();
            feeders.push(feeder);
            assert!(
                mixer
                    .play_rt(
                        Prepared::stream(stream, emitter(false), Bus::Dialogue, 10, 0).unwrap()
                    )
                    .is_ok()
            );
        }
        for _ in 0..120 {
            assert!(
                mixer
                    .play_rt(
                        Prepared::clip(clip.clone(), emitter(true), Bus::Effects, 0, 0).unwrap()
                    )
                    .is_ok()
            );
        }
        let (feeder, stream) = crate::audio_stream::channel(8000, 128).unwrap();
        feeders.push(feeder);
        assert!(matches!(
            mixer.play_rt(Prepared::stream(stream, emitter(false), Bus::Music, 1, 0).unwrap()),
            Err((Refusal::Capacity, _))
        ));
        assert_eq!(mixer.voice_count(), 128);
        assert_eq!(mixer.stats.stolen, 0);
        assert!(mixer.pop_retired().is_none());
        mixer.stop_bus(Bus::Dialogue);
        mixer.render_rt(&mut [0.; 3200]).unwrap();
        assert_eq!(mixer.voice_count(), 120);
    }
    #[test]
    fn long_music_release_decays_without_a_gain_spike() {
        let clip = Clip::new(vec![0.5; 48000], 48000).unwrap();
        let mut mixer = Mixer::new(48000).unwrap();
        assert!(
            mixer
                .play_rt(Prepared::clip(clip, emitter(true), Bus::Music, 80, 0).unwrap())
                .is_ok()
        );
        let mut pcm = [0.; 1024];
        mixer.render_rt(&mut pcm).unwrap();
        let baseline = pcm[1022];
        assert!(baseline > 0.);
        mixer.stop_bus(Bus::Music);
        mixer.render_rt(&mut pcm).unwrap();
        assert!(pcm.iter().all(|v| *v <= baseline + 1e-6));
        let early = pcm[1022];
        for _ in 0..17 {
            mixer.render_rt(&mut pcm).unwrap();
        }
        assert!(pcm[1022] < early * 0.1);
        mixer.render_rt(&mut pcm).unwrap();
        assert_eq!(mixer.voice_count(), 0);
    }
    #[test]
    fn streamed_loop_restores_source_clock_and_pause_freezes_it() {
        let clip = Clip::new(vec![0.5; 1000], 48000).unwrap();
        let prepared = Prepared::clip(clip, emitter(true), Bus::Music, 80, 1250).unwrap();
        let progress = prepared.progress();
        let (prepared, mut feeder) = prepared.into_streaming(2048).unwrap();
        while feeder.pump() > 0 {}
        let mut mixer = Mixer::new(48000).unwrap();
        assert!(mixer.play_rt(prepared).is_ok());
        mixer.render_rt(&mut [0.; 1024]).unwrap();
        assert_eq!(progress.frame(), 1762);
        mixer.suspend(true);
        let mut pcm = [1.; 1024];
        mixer.render_rt(&mut pcm).unwrap();
        assert_eq!(progress.frame(), 1762);
        assert!(pcm.iter().all(|v| *v == 0.));
        mixer.suspend(false);
        mixer.render_rt(&mut pcm).unwrap();
        assert_eq!(progress.frame(), 2274);
        assert!(pcm.iter().any(|v| *v > 0.));
    }
}
