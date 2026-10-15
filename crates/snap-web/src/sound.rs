//! Gentle sounds made with Web Audio, no files: a two-note click when a
//! piece snaps on, a soft clunk for a bolt, a boop when a piece can't go
//! there, a rising chime for Go, a whoosh while thrusters burn, a twang
//! when magnets let go, a pop when a square pops, the storm's wind, and
//! short tunes for a job done or not quite. The audio starts on the first
//! key or tap, as browsers require.

use web_sys::{AudioContext, BiquadFilterType, GainNode, OscillatorType};

/// A sound the game makes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    Lift,
    Snap,
    Brace,
    Bolt,
    Nope,
    Remove,
    Go,
    Thrust,
    Storm,
    Break,
    Pop,
    Bump,
    Won,
    Failed,
}

pub struct Sound {
    ctx: Option<AudioContext>,
    master: Option<GainNode>,
    noise: Option<web_sys::AudioBuffer>,
    pub on: bool,
}

impl Sound {
    pub fn new(on: bool) -> Self {
        Self {
            ctx: None,
            master: None,
            noise: None,
            on,
        }
    }

    /// Starts the audio on a key press or tap.
    pub fn wake(&mut self) {
        if let Some(ctx) = &self.ctx {
            let _ = ctx.resume();
            return;
        }
        let Ok(ctx) = AudioContext::new() else {
            return;
        };
        let Ok(master) = ctx.create_gain() else {
            return;
        };
        master.gain().set_value(0.6);
        let _ = master.connect_with_audio_node(&ctx.destination());
        let rate = ctx.sample_rate();
        let noise = ctx.create_buffer(1, rate as u32, rate).ok();
        if let Some(buffer) = &noise {
            let mut data: Vec<f32> = (0..rate as usize)
                .map(|_| js_sys::Math::random() as f32 * 2.0 - 1.0)
                .collect();
            let _ = buffer.copy_to_channel(&mut data, 0);
        }
        self.ctx = Some(ctx);
        self.master = Some(master);
        self.noise = noise;
    }

    /// A tone sliding from one pitch to another.
    fn tone(&self, kind: OscillatorType, from: f32, to: f32, length: f64, volume: f32, delay: f64) {
        let (Some(ctx), Some(master)) = (&self.ctx, &self.master) else {
            return;
        };
        let (Ok(osc), Ok(gain)) = (ctx.create_oscillator(), ctx.create_gain()) else {
            return;
        };
        let t = ctx.current_time() + delay;
        osc.set_type(kind);
        let _ = osc.frequency().set_value_at_time(from, t);
        let _ = osc
            .frequency()
            .exponential_ramp_to_value_at_time(to.max(1.0), t + length);
        let _ = gain.gain().set_value_at_time(0.0001, t);
        let _ = gain.gain().linear_ramp_to_value_at_time(volume, t + 0.01);
        let _ = gain
            .gain()
            .exponential_ramp_to_value_at_time(0.0001, t + length);
        let _ = osc.connect_with_audio_node(&gain);
        let _ = gain.connect_with_audio_node(master);
        let _ = osc.start_with_when(t);
        let _ = osc.stop_with_when(t + length + 0.05);
    }

    /// A burst of filtered noise with a sweeping filter.
    fn hiss(&self, kind: BiquadFilterType, from: f32, to: f32, length: f64, volume: f32) {
        let (Some(ctx), Some(master), Some(buffer)) = (&self.ctx, &self.master, &self.noise) else {
            return;
        };
        let (Ok(source), Ok(filter), Ok(gain)) = (
            ctx.create_buffer_source(),
            ctx.create_biquad_filter(),
            ctx.create_gain(),
        ) else {
            return;
        };
        let t = ctx.current_time();
        source.set_buffer(Some(buffer));
        source.set_loop(true);
        filter.set_type(kind);
        filter.q().set_value(2.0);
        let _ = filter.frequency().set_value_at_time(from, t);
        let _ = filter
            .frequency()
            .exponential_ramp_to_value_at_time(to.max(1.0), t + length);
        let _ = gain.gain().set_value_at_time(0.0001, t);
        let _ = gain
            .gain()
            .linear_ramp_to_value_at_time(volume, t + length * 0.25);
        let _ = gain
            .gain()
            .exponential_ramp_to_value_at_time(0.0001, t + length);
        let _ = source.connect_with_audio_node(&filter);
        let _ = filter.connect_with_audio_node(&gain);
        let _ = gain.connect_with_audio_node(master);
        let node: &web_sys::AudioScheduledSourceNode = source.as_ref();
        let _ = node.start();
        let _ = node.stop_with_when(t + length + 0.05);
    }

    /// Plays an effect, unless the sound is off.
    pub fn play(&self, effect: Effect) {
        if !self.on {
            return;
        }
        use OscillatorType::{Sawtooth, Sine, Square, Triangle};
        match effect {
            Effect::Lift => self.tone(Sine, 520.0, 600.0, 0.08, 0.05, 0.0),
            Effect::Snap => {
                self.tone(Sine, 880.0, 880.0, 0.07, 0.12, 0.0);
                self.tone(Sine, 1320.0, 1320.0, 0.1, 0.1, 0.05);
            }
            Effect::Brace => self.tone(Triangle, 660.0, 990.0, 0.12, 0.1, 0.0),
            Effect::Bolt => {
                self.tone(Square, 190.0, 150.0, 0.12, 0.05, 0.0);
                self.tone(Sine, 1600.0, 1200.0, 0.04, 0.05, 0.0);
            }
            Effect::Nope => {
                self.tone(Sine, 300.0, 240.0, 0.12, 0.08, 0.0);
                self.tone(Sine, 240.0, 200.0, 0.14, 0.08, 0.12);
            }
            Effect::Remove => self.tone(Sine, 600.0, 400.0, 0.12, 0.08, 0.0),
            Effect::Go => {
                for (i, f) in [392.0, 523.0, 659.0].iter().enumerate() {
                    self.tone(Triangle, *f, *f, 0.18, 0.1, i as f64 * 0.09);
                }
            }
            Effect::Thrust => self.hiss(BiquadFilterType::Lowpass, 500.0, 1400.0, 2.4, 0.12),
            Effect::Storm => self.hiss(BiquadFilterType::Bandpass, 300.0, 900.0, 3.5, 0.1),
            Effect::Break => {
                self.tone(Sawtooth, 340.0, 120.0, 0.3, 0.05, 0.0);
                self.hiss(BiquadFilterType::Highpass, 2000.0, 800.0, 0.15, 0.05);
            }
            Effect::Pop => self.tone(Sine, 900.0, 300.0, 0.1, 0.1, 0.0),
            Effect::Bump => self.tone(Sine, 140.0, 90.0, 0.25, 0.14, 0.0),
            Effect::Won => {
                for (i, f) in [523.0, 659.0, 784.0, 1047.0].iter().enumerate() {
                    self.tone(Triangle, *f, *f, 0.3, 0.1, i as f64 * 0.12);
                }
            }
            Effect::Failed => {
                for (i, f) in [392.0, 330.0, 262.0].iter().enumerate() {
                    self.tone(Triangle, *f, *f, 0.28, 0.08, i as f64 * 0.16);
                }
            }
        }
    }
}
