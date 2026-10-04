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
    fn validate(self) -> Result<(), String> {
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
struct Voice {
    id: VoiceId,
    clip: Clip,
    emitter: Emitter,
    cursor: f64,
    age: u64,
    release: Option<u32>,
}
pub struct Mixer {
    rate: u32,
    listener: Vec3,
    right: Vec3,
    voices: Vec<Voice>,
    next_id: u64,
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
            voices: Vec::with_capacity(128),
            next_id: 0,
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
    pub fn play(&mut self, clip: &Clip, emitter: Emitter) -> Result<VoiceId, String> {
        emitter.validate()?;
        if self.voices.len() >= 128 {
            return Err("Audio voice capacity exhausted".into());
        }
        let next = self
            .next_id
            .checked_add(1)
            .ok_or("Audio voice identity exhausted")?;
        let id = VoiceId(next);
        self.next_id = next;
        self.voices.push(Voice {
            id,
            clip: clip.clone(),
            emitter,
            cursor: 0.,
            age: 0,
            release: None,
        });
        Ok(id)
    }
    pub fn voice_count(&self) -> usize {
        self.voices.len()
    }
    /// Stops only this exact actor life, with a ten-millisecond release envelope.
    pub fn stop_life(&mut self, life: LifeId) {
        let release = self.rate / 100;
        for voice in &mut self.voices {
            if voice.emitter.life == Some(life) && voice.release.is_none() {
                voice.release = Some(release);
            }
        }
    }
    pub fn stop(&mut self, id: VoiceId) -> bool {
        let Some(voice) = self.voices.iter_mut().find(|voice| voice.id == id) else {
            return false;
        };
        if voice.release.is_none() {
            voice.release = Some(self.rate / 100);
        }
        true
    }
    /// Writes at most 8192 interleaved stereo frames without allocating. Invalid
    /// output lengths refuse before changing either voices or the output buffer.
    pub fn render(&mut self, output: &mut [f32]) -> Result<(), String> {
        if !output.len().is_multiple_of(2) || output.len() > 16384 {
            return Err("Invalid audio output block size".into());
        }
        output.fill(0.);
        let fade = f64::from(self.rate) / 100.;
        for voice in &mut self.voices {
            let delta = voice.emitter.position - self.listener;
            let distance = delta.length();
            let attenuation = (1. - distance / voice.emitter.range).clamp(0., 1.).powi(2);
            let pan = if distance > 1e-8 {
                (delta / distance).dot(self.right).clamp(-1., 1.)
            } else {
                0.
            };
            let left = ((1. - pan) * 0.5).sqrt();
            let right = ((1. + pan) * 0.5).sqrt();
            let step =
                f64::from(voice.clip.rate) / f64::from(self.rate) * f64::from(voice.emitter.pitch);
            let length = voice.clip.samples.len();
            for frame in output.chunks_exact_mut(2) {
                if voice.release == Some(0)
                    || (!voice.emitter.looping && voice.cursor >= length as f64)
                {
                    break;
                }
                let index = voice.cursor.floor() as usize;
                let next = if index + 1 < length {
                    voice.clip.samples[index + 1]
                } else if voice.emitter.looping {
                    voice.clip.samples[0]
                } else {
                    0.
                };
                let sample = voice.clip.samples[index]
                    + (next - voice.clip.samples[index]) * voice.cursor.fract() as f32;
                let attack = (voice.age as f64 / fade).min(1.);
                let release = voice
                    .release
                    .map_or(1., |remaining| f64::from(remaining) / fade);
                let tail = if voice.emitter.looping {
                    1.
                } else {
                    ((length as f64 - voice.cursor) / step / fade).clamp(0., 1.)
                };
                let gain = voice.emitter.gain * attenuation * (attack * release * tail) as f32;
                frame[0] += sample * gain * left;
                frame[1] += sample * gain * right;
                voice.age = voice.age.saturating_add(1);
                if let Some(remaining) = &mut voice.release {
                    *remaining = remaining.saturating_sub(1);
                }
                voice.cursor += step;
                if voice.emitter.looping {
                    voice.cursor %= length as f64;
                }
            }
        }
        self.voices.retain(|voice| {
            voice.release != Some(0)
                && (voice.emitter.looping || voice.cursor < voice.clip.samples.len() as f64)
        });
        for sample in output {
            *sample = sample.clamp(-1., 1.);
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
