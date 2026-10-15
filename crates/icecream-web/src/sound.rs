//! Placeholder sounds made with Web Audio, no files: the ice cream truck's
//! jingle (a little out of tune, louder as he gets closer), the freezer
//! hum, squelchy fudge, comic farts, a bloop for sprinkles, and short
//! chimes for keys and pickups. The audio starts on the first key or tap,
//! as browsers require.

use web_sys::{AudioContext, BiquadFilterType, GainNode, OscillatorType};

/// The jingle's notes (semitones above A4) and lengths in beats: a short
/// tune of our own, played on a music box.
const TUNE: [(i32, f32); 16] = [
    (3, 0.5),
    (7, 0.5),
    (10, 0.5),
    (7, 0.5),
    (8, 0.5),
    (12, 0.5),
    (10, 1.0),
    (5, 0.5),
    (8, 0.5),
    (12, 0.5),
    (8, 0.5),
    (7, 0.5),
    (3, 0.5),
    (5, 1.0),
    (0, 0.5),
    (-2, 0.5),
];

pub struct Sound {
    ctx: Option<AudioContext>,
    master: Option<GainNode>,
    jingle: Option<GainNode>,
    hum: Option<GainNode>,
    noise: Option<web_sys::AudioBuffer>,
    next_note: f64,
    note: usize,
    /// The fluorescent buzz, 0 to 1.
    buzz: f32,
}

/// The effects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    Fudge,
    Splat,
    Fart,
    Bloop,
    Hit,
    Grab,
    Key,
    Pickup,
    Frozen,
    Jingle,
    Locked,
    Jump,
    Empty,
    Escape,
    Scooped,
    /// The jumpscare's sting: a loud, clashing shriek over a thump, made
    /// here like every other sound.
    Jumpscare,
}

impl Sound {
    pub fn new() -> Self {
        Self {
            ctx: None,
            master: None,
            jingle: None,
            hum: None,
            noise: None,
            next_note: 0.0,
            note: 0,
            buzz: 0.0,
        }
    }

    /// How loud the fluorescent lights buzz, 0 to 1, over the freezer hum.
    pub fn set_buzz(&mut self, buzz: f32) {
        self.buzz = buzz.clamp(0.0, 1.0);
        self.quiet(false);
    }

    fn hum_level(&self) -> f32 {
        0.025 + 0.11 * self.buzz
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
        master.gain().set_value(0.7);
        let _ = master.connect_with_audio_node(&ctx.destination());
        let jingle = ctx.create_gain().ok();
        if let Some(j) = &jingle {
            j.gain().set_value(0.0);
            let _ = j.connect_with_audio_node(&master);
        }
        // The freezer hum: a low, quiet buzz.
        let hum = ctx.create_gain().ok();
        if let Some(h) = &hum {
            h.gain().set_value(0.025 + 0.11 * self.buzz);
            let _ = h.connect_with_audio_node(&master);
            for (freq, kind) in [
                (60.0, OscillatorType::Sine),
                (120.0, OscillatorType::Triangle),
                (240.0, OscillatorType::Sawtooth),
            ] {
                if let Ok(osc) = ctx.create_oscillator() {
                    osc.set_type(kind);
                    osc.frequency().set_value(freq);
                    let _ = osc.connect_with_audio_node(h);
                    let _ = osc.start();
                }
            }
        }
        let rate = ctx.sample_rate();
        let noise = ctx.create_buffer(1, rate as u32, rate).ok();
        if let Some(buffer) = &noise {
            let mut data: Vec<f32> = (0..rate as usize)
                .map(|_| js_sys::Math::random() as f32 * 2.0 - 1.0)
                .collect();
            let _ = buffer.copy_to_channel(&mut data, 0);
        }
        self.next_note = ctx.current_time();
        self.ctx = Some(ctx);
        self.master = Some(master);
        self.jingle = jingle;
        self.hum = hum;
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
        let _ = gain.gain().set_value_at_time(volume, t);
        let _ = gain
            .gain()
            .exponential_ramp_to_value_at_time(0.001, t + length);
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
        filter.set_type(kind);
        filter.q().set_value(6.0);
        let _ = filter.frequency().set_value_at_time(from, t);
        let _ = filter
            .frequency()
            .exponential_ramp_to_value_at_time(to.max(1.0), t + length);
        let _ = gain.gain().set_value_at_time(volume, t);
        let _ = gain
            .gain()
            .exponential_ramp_to_value_at_time(0.001, t + length);
        let _ = source.connect_with_audio_node(&filter);
        let _ = filter.connect_with_audio_node(&gain);
        let _ = gain.connect_with_audio_node(master);
        let node: &web_sys::AudioScheduledSourceNode = source.as_ref();
        let _ = node.start();
        let _ = node.stop_with_when(t + length + 0.05);
    }

    /// A comic fart: a wobbling low buzz through a muffling filter.
    fn fart(&self) {
        let (Some(ctx), Some(master)) = (&self.ctx, &self.master) else {
            return;
        };
        let (Ok(osc), Ok(wobble), Ok(depth), Ok(filter), Ok(gain)) = (
            ctx.create_oscillator(),
            ctx.create_oscillator(),
            ctx.create_gain(),
            ctx.create_biquad_filter(),
            ctx.create_gain(),
        ) else {
            return;
        };
        let t = ctx.current_time();
        let length = 0.75;
        osc.set_type(OscillatorType::Sawtooth);
        let _ = osc.frequency().set_value_at_time(110.0, t);
        let _ = osc
            .frequency()
            .linear_ramp_to_value_at_time(62.0, t + length);
        wobble.frequency().set_value(23.0);
        depth.gain().set_value(28.0);
        let _ = wobble.connect_with_audio_node(&depth);
        let _ = depth.connect_with_audio_param(&osc.frequency());
        filter.set_type(BiquadFilterType::Lowpass);
        filter.frequency().set_value(520.0);
        let _ = gain.gain().set_value_at_time(0.0, t);
        let _ = gain.gain().linear_ramp_to_value_at_time(0.5, t + 0.04);
        let _ = gain
            .gain()
            .linear_ramp_to_value_at_time(0.35, t + length * 0.7);
        let _ = gain.gain().linear_ramp_to_value_at_time(0.0, t + length);
        let _ = osc.connect_with_audio_node(&filter);
        let _ = filter.connect_with_audio_node(&gain);
        let _ = gain.connect_with_audio_node(master);
        let _ = osc.start_with_when(t);
        let _ = wobble.start_with_when(t);
        let _ = osc.stop_with_when(t + length + 0.05);
        let _ = wobble.stop_with_when(t + length + 0.05);
        self.hiss(BiquadFilterType::Bandpass, 300.0, 120.0, 0.5, 0.25);
    }

    pub fn play(&self, effect: Effect) {
        use OscillatorType::{Sawtooth, Sine, Square, Triangle};
        match effect {
            Effect::Fudge => {
                self.hiss(BiquadFilterType::Lowpass, 1400.0, 180.0, 0.16, 0.35);
                self.tone(Sine, 240.0, 90.0, 0.12, 0.2, 0.0);
            }
            Effect::Splat => self.hiss(BiquadFilterType::Lowpass, 700.0, 100.0, 0.2, 0.3),
            Effect::Fart => self.fart(),
            Effect::Bloop => {
                for i in 0..3 {
                    self.tone(Sine, 420.0, 980.0, 0.08, 0.16, f64::from(i) * 0.06);
                }
            }
            Effect::Hit => self.tone(Square, 330.0, 140.0, 0.16, 0.18, 0.0),
            Effect::Grab => {
                self.tone(Sawtooth, 180.0, 70.0, 0.35, 0.3, 0.0);
                self.hiss(BiquadFilterType::Lowpass, 900.0, 120.0, 0.3, 0.3);
            }
            Effect::Key => {
                self.tone(Sine, 1318.5, 1318.5, 0.18, 0.25, 0.0);
                self.tone(Sine, 1760.0, 1760.0, 0.3, 0.25, 0.1);
            }
            Effect::Pickup => {
                for (i, f) in [523.25_f32, 659.25, 783.99].iter().enumerate() {
                    self.tone(Triangle, *f, *f, 0.14, 0.22, i as f64 * 0.07);
                }
            }
            Effect::Frozen => {
                self.hiss(BiquadFilterType::Highpass, 3000.0, 6000.0, 0.4, 0.3);
                self.tone(Triangle, 1200.0, 300.0, 0.4, 0.15, 0.0);
            }
            Effect::Jingle => {
                for i in 0..4 {
                    self.tone(
                        Triangle,
                        2400.0 + i as f32 * 200.0,
                        2000.0,
                        0.06,
                        0.12,
                        f64::from(i) * 0.05,
                    );
                }
            }
            Effect::Locked => self.hiss(BiquadFilterType::Bandpass, 400.0, 300.0, 0.25, 0.4),
            Effect::Jump => self.tone(Sine, 300.0, 520.0, 0.12, 0.12, 0.0),
            Effect::Empty => self.tone(Square, 160.0, 150.0, 0.08, 0.1, 0.0),
            Effect::Escape => {
                for (i, f) in [523.25_f32, 659.25, 783.99, 1046.5].iter().enumerate() {
                    self.tone(Triangle, *f, *f, 0.3, 0.25, i as f64 * 0.12);
                }
            }
            Effect::Scooped => self.tone(Sine, 900.0, 160.0, 0.9, 0.3, 0.0),
            Effect::Jumpscare => {
                // Clashing notes a semitone apart, sliding up as he lunges.
                for (f, kind) in [
                    (880.0_f32, Sawtooth),
                    (932.3, Square),
                    (1244.5, Sawtooth),
                    (1318.5, Square),
                ] {
                    self.tone(kind, f, f * 1.5, 0.9, 0.22, 0.0);
                }
                self.hiss(BiquadFilterType::Bandpass, 4200.0, 700.0, 0.8, 0.9);
                // The thump as his face hits the screen.
                self.tone(Sine, 140.0, 38.0, 0.6, 0.9, 0.4);
                self.tone(Sawtooth, 2093.0, 2637.0, 0.5, 0.2, 0.4);
            }
        }
    }

    /// Keeps the truck's jingle going: `closeness` from 0 (far or none) to
    /// 1 (right here), faster while he chases.
    pub fn truck(&mut self, closeness: f32, chasing: bool) {
        let (Some(ctx), Some(jingle)) = (&self.ctx, &self.jingle) else {
            return;
        };
        let now = ctx.current_time();
        let volume = if closeness <= 0.0 {
            0.0
        } else {
            0.03 + 0.3 * closeness * closeness
        };
        let _ = jingle.gain().set_target_at_time(volume, now, 0.15);
        if closeness <= 0.0 {
            self.next_note = now;
            return;
        }
        let beat = if chasing { 0.2 } else { 0.3 };
        if self.next_note < now {
            self.next_note = now;
        }
        while self.next_note < now + 0.4 {
            let (step, length) = TUNE[self.note % TUNE.len()];
            // Out of tune on purpose: a little flat, and wobbling.
            let detune = -0.25 + ((self.note * 7) % 5) as f32 * 0.08;
            let freq = 440.0 * 2.0_f32.powf((step as f32 + detune) / 12.0);
            if let (Ok(osc), Ok(env)) = (ctx.create_oscillator(), ctx.create_gain()) {
                let t = self.next_note;
                let d = f64::from(length) * beat;
                osc.set_type(OscillatorType::Triangle);
                osc.frequency().set_value(freq);
                let _ = env.gain().set_value_at_time(0.6, t);
                let _ = env.gain().exponential_ramp_to_value_at_time(0.01, t + d);
                let _ = osc.connect_with_audio_node(&env);
                let _ = env.connect_with_audio_node(jingle);
                let _ = osc.start_with_when(t);
                let _ = osc.stop_with_when(t + d + 0.02);
            }
            self.next_note += f64::from(length) * beat;
            self.note += 1;
        }
    }

    /// Quiets the hum and the jingle (menus, the end of a run).
    pub fn quiet(&mut self, quiet: bool) {
        if let (Some(ctx), Some(hum)) = (&self.ctx, &self.hum) {
            let _ = hum.gain().set_target_at_time(
                if quiet { 0.0 } else { self.hum_level() },
                ctx.current_time(),
                0.2,
            );
        }
        if quiet {
            self.truck(0.0, false);
        }
    }
}
