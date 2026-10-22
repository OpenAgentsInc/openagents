//! Every sound made in the page with Web Audio, no files, through five
//! buses (`master`, `music`, `ambience`, `sfx`, `voice`) the house
//! document sets live. Effects come from a place in the house through a
//! panner, so the creak at arrival really is behind you.
//!
//! The music, `bassline-minor`, is about 70 bpm in D minor: a walking bass
//! with a tritone step, a low organ pad, and a sparse music-box line that
//! comes in as the intensity rises. The ambience beds are wind, rain on
//! glass, a clock's tick, a fireplace, a bubbling cauldron, and distant
//! thunder.

use glam::Vec3;
use web_sys::{
    AudioContext, AudioNode, BiquadFilterType, DistanceModelType, GainNode, OscillatorType,
    PannerNode, PanningModelType,
};

/// The most effects that play at once; the oldest gives way.
const VOICES: usize = 6;
/// Seconds per beat at 70 bpm.
const BEAT: f64 = 60.0 / 70.0;
/// The walking bass, MIDI notes, one a beat: D minor with the tritone
/// (D to G sharp) in the walk.
const BASS: [u8; 16] = [
    38, 41, 45, 44, 45, 41, 40, 34, 33, 37, 38, 41, 44, 43, 41, 40,
];
/// The organ's chord for each bar of four beats.
const PAD: [[u8; 3]; 4] = [[50, 53, 57], [46, 50, 53], [43, 46, 50], [45, 49, 52]];
/// The music box's notes, from which it picks.
const BOX: [u8; 8] = [74, 77, 81, 80, 77, 74, 73, 76];

fn hz(midi: u8) -> f32 {
    440.0 * 2.0_f32.powf((f32::from(midi) - 69.0) / 12.0)
}

/// The levels the house document sets.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Levels {
    pub master: f32,
    pub music: f32,
    pub ambience: f32,
    pub sfx: f32,
    pub voice: f32,
}

struct Bed {
    id: String,
    gain: GainNode,
    stop: Vec<web_sys::AudioScheduledSourceNode>,
    next: f64,
}

pub struct Sound {
    ctx: Option<AudioContext>,
    master: Option<GainNode>,
    music: Option<GainNode>,
    ambience: Option<GainNode>,
    sfx: Option<GainNode>,
    voice: Option<GainNode>,
    noise: Option<web_sys::AudioBuffer>,
    levels: Levels,
    /// The area's music gain and intensity, 0 to 5.
    area_music: f32,
    intensity: f32,
    music_on: bool,
    next_beat: f64,
    beat: usize,
    bed: Option<Bed>,
    playing: Vec<(f64, GainNode)>,
    /// Effects waiting for their time: when, which, and where.
    later: Vec<(f64, String, Vec3)>,
    seed: u32,
}

impl Sound {
    pub fn new() -> Self {
        Self {
            ctx: None,
            master: None,
            music: None,
            ambience: None,
            sfx: None,
            voice: None,
            noise: None,
            levels: Levels {
                master: 0.8,
                music: 0.6,
                ambience: 0.7,
                sfx: 0.9,
                voice: 1.0,
            },
            area_music: 1.0,
            intensity: 1.0,
            music_on: false,
            next_beat: 0.0,
            beat: 0,
            bed: None,
            playing: Vec::new(),
            later: Vec::new(),
            seed: 1,
        }
    }

    /// Whether the audio is running (it starts on the first click or key).
    pub fn awake(&self) -> bool {
        self.ctx.is_some()
    }

    fn rand(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        (self.seed % 10_000) as f32 / 10_000.0
    }

    /// Starts the audio on a click, tap or key, as browsers require.
    pub fn wake(&mut self) {
        if let Some(ctx) = &self.ctx {
            let _ = ctx.resume();
            return;
        }
        let Ok(ctx) = AudioContext::new() else {
            return;
        };
        let bus = |to: &AudioNode| -> Option<GainNode> {
            let g = ctx.create_gain().ok()?;
            let _ = g.connect_with_audio_node(to);
            Some(g)
        };
        let Some(master) = bus(&ctx.destination()) else {
            return;
        };
        self.music = bus(&master);
        self.ambience = bus(&master);
        self.sfx = bus(&master);
        self.voice = bus(&master);
        let rate = ctx.sample_rate();
        let noise = ctx.create_buffer(1, (rate * 2.0) as u32, rate).ok();
        if let Some(buffer) = &noise {
            let mut data: Vec<f32> = (0..(rate * 2.0) as usize)
                .map(|_| js_sys::Math::random() as f32 * 2.0 - 1.0)
                .collect();
            let _ = buffer.copy_to_channel(&mut data, 0);
        }
        self.next_beat = ctx.current_time() + 0.1;
        self.master = Some(master);
        self.noise = noise;
        self.ctx = Some(ctx);
        self.seed = (js_sys::Math::random() * 1e6) as u32 | 1;
        self.apply_levels(0.0);
    }

    fn now(&self) -> f64 {
        self.ctx.as_ref().map_or(0.0, AudioContext::current_time)
    }

    /// Sets the five buses; they reach the new levels within `glide`
    /// seconds (0 is at once).
    pub fn set_levels(&mut self, levels: Levels, glide: f64) {
        self.levels = levels;
        self.apply_levels(glide);
    }

    fn apply_levels(&self, glide: f64) {
        let t = self.now();
        let l = self.levels;
        let music = if self.music_on {
            l.music * self.area_music.clamp(0.0, 2.0)
        } else {
            0.0
        };
        for (node, level) in [
            (&self.master, l.master),
            (&self.music, music),
            (&self.ambience, l.ambience),
            (&self.sfx, l.sfx),
            (&self.voice, l.voice),
        ] {
            if let Some(g) = node {
                let level = level.clamp(0.0, 2.0);
                if glide <= 0.0 {
                    let _ = g.gain().cancel_scheduled_values(t);
                    let _ = g.gain().set_value_at_time(level, t);
                } else {
                    let _ = g.gain().set_target_at_time(level, t, glide / 3.0);
                }
            }
        }
    }

    /// The music on or off (it enters after the arrival cue).
    pub fn music(&mut self, on: bool) {
        if self.music_on != on {
            self.music_on = on;
            self.next_beat = self.now() + 0.05;
            self.apply_levels(1.5);
        }
    }

    /// The area's music level and intensity shape the music.
    pub fn area(&mut self, music_gain: f32, intensity: f32) {
        if (self.area_music - music_gain).abs() > 1e-3 || (self.intensity - intensity).abs() > 1e-3
        {
            self.area_music = music_gain;
            self.intensity = intensity;
            self.apply_levels(1.0);
        }
    }

    /// Where the listener is and which way they face, in the house's frame.
    pub fn listener(&self, eye: Vec3, forward: Vec3) {
        let Some(ctx) = &self.ctx else { return };
        let l = ctx.listener();
        // Web Audio's frame is right-handed: mirror z, as the view does.
        l.set_position(f64::from(eye.x), f64::from(eye.y), f64::from(-eye.z));
        l.set_orientation(
            f64::from(forward.x),
            f64::from(forward.y),
            f64::from(-forward.z),
            0.0,
            1.0,
            0.0,
        );
    }

    fn panner(&self, at: Vec3) -> Option<PannerNode> {
        let ctx = self.ctx.as_ref()?;
        let p = ctx.create_panner().ok()?;
        p.set_panning_model(PanningModelType::Hrtf);
        p.set_distance_model(DistanceModelType::Inverse);
        p.set_ref_distance(2.0);
        p.set_rolloff_factor(0.8);
        p.set_position(f64::from(at.x), f64::from(at.y), f64::from(-at.z));
        Some(p)
    }

    /// A voice's output: a gain into a panner (when placed) into the sfx
    /// bus. Old voices give way past the limit.
    fn voice_out(&mut self, at: Option<Vec3>, length: f64) -> Option<GainNode> {
        let ctx = self.ctx.as_ref()?;
        let sfx = self.sfx.as_ref()?;
        let gain = ctx.create_gain().ok()?;
        match at.and_then(|p| self.panner(p)) {
            Some(p) => {
                let _ = gain.connect_with_audio_node(&p);
                let _ = p.connect_with_audio_node(sfx);
            }
            None => {
                let _ = gain.connect_with_audio_node(sfx);
            }
        }
        let now = self.now();
        self.playing.retain(|(end, _)| *end > now);
        while self.playing.len() >= VOICES {
            let (_, old) = self.playing.remove(0);
            let _ = old.gain().set_target_at_time(0.0, now, 0.03);
        }
        self.playing.push((now + length, gain.clone()));
        Some(gain)
    }

    /// A tone sliding from one pitch to another into `out`.
    #[allow(clippy::too_many_arguments)]
    fn tone(
        &self,
        out: &AudioNode,
        kind: OscillatorType,
        from: f32,
        to: f32,
        start: f64,
        length: f64,
        volume: f32,
        attack: f64,
    ) {
        let Some(ctx) = &self.ctx else { return };
        let (Ok(osc), Ok(env)) = (ctx.create_oscillator(), ctx.create_gain()) else {
            return;
        };
        osc.set_type(kind);
        let _ = osc.frequency().set_value_at_time(from.max(1.0), start);
        let _ = osc
            .frequency()
            .exponential_ramp_to_value_at_time(to.max(1.0), start + length);
        let _ = env.gain().set_value_at_time(0.0001, start);
        let _ = env
            .gain()
            .linear_ramp_to_value_at_time(volume, start + attack.max(0.005));
        let _ = env
            .gain()
            .exponential_ramp_to_value_at_time(0.0001, start + length);
        let _ = osc.connect_with_audio_node(&env);
        let _ = env.connect_with_audio_node(out);
        let _ = osc.start_with_when(start);
        let _ = osc.stop_with_when(start + length + 0.05);
    }

    /// Filtered noise with a sweeping filter into `out`.
    #[allow(clippy::too_many_arguments)]
    fn hiss(
        &self,
        out: &AudioNode,
        kind: BiquadFilterType,
        from: f32,
        to: f32,
        start: f64,
        length: f64,
        volume: f32,
        q: f32,
    ) {
        let (Some(ctx), Some(buffer)) = (&self.ctx, &self.noise) else {
            return;
        };
        let (Ok(source), Ok(filter), Ok(env)) = (
            ctx.create_buffer_source(),
            ctx.create_biquad_filter(),
            ctx.create_gain(),
        ) else {
            return;
        };
        source.set_buffer(Some(buffer));
        source.set_loop(true);
        filter.set_type(kind);
        filter.q().set_value(q);
        let _ = filter.frequency().set_value_at_time(from, start);
        let _ = filter
            .frequency()
            .exponential_ramp_to_value_at_time(to.max(1.0), start + length);
        let _ = env.gain().set_value_at_time(0.0001, start);
        let _ = env
            .gain()
            .linear_ramp_to_value_at_time(volume, start + 0.01);
        let _ = env
            .gain()
            .exponential_ramp_to_value_at_time(0.0001, start + length);
        let _ = source.connect_with_audio_node(&filter);
        let _ = filter.connect_with_audio_node(&env);
        let _ = env.connect_with_audio_node(out);
        let _ = source.start_with_when_and_grain_offset(start, js_sys::Math::random() * 1.5);
        let node: &web_sys::AudioScheduledSourceNode = source.as_ref();
        let _ = node.stop_with_when(start + length + 0.05);
    }

    /// A pulsing voice-like sound (laughs, cackles): `count` syllables.
    #[allow(clippy::too_many_arguments)]
    fn syllables(
        &self,
        out: &AudioNode,
        kind: OscillatorType,
        start_hz: f32,
        end_hz: f32,
        count: usize,
        gap: f64,
        volume: f32,
        t: f64,
    ) {
        for i in 0..count {
            let k = i as f32 / count.max(1) as f32;
            let f = start_hz + (end_hz - start_hz) * k;
            self.tone(
                out,
                kind,
                f * 1.08,
                f,
                t + i as f64 * gap,
                gap * 0.8,
                volume,
                0.01,
            );
        }
    }

    /// Plays a sound effect by id, from `at` when given. Unknown ids are
    /// quiet.
    /// Whether the page has the effect.
    pub fn play(&mut self, id: &str, at: Option<Vec3>) -> bool {
        if !crate::sounds::EFFECTS.contains(&id) {
            return false;
        }
        self.play_made(id, at);
        true
    }

    fn play_made(&mut self, id: &str, at: Option<Vec3>) {
        use BiquadFilterType::{Bandpass, Highpass, Lowpass};
        use OscillatorType::{Sawtooth, Sine, Square, Triangle};
        if self.ctx.is_none() {
            return;
        }
        let length = 3.0;
        let Some(out) = self.voice_out(at, length) else {
            return;
        };
        let _ = out.gain().set_value(1.0);
        let o: &AudioNode = out.as_ref();
        let t = self.now() + 0.01;
        match id {
            "creak-door" => {
                self.hiss(o, Bandpass, 380.0, 520.0, t, 1.3, 0.5, 18.0);
                self.tone(o, Sawtooth, 92.0, 148.0, t, 1.2, 0.08, 0.2);
                self.tone(o, Sawtooth, 96.0, 70.0, t + 1.0, 0.6, 0.06, 0.1);
            }
            "creak-rocker" => {
                self.hiss(o, Bandpass, 300.0, 420.0, t, 0.5, 0.35, 16.0);
            }
            "footsteps-upstairs" => {
                for i in 0..6 {
                    let s = t + i as f64 * 0.55;
                    self.tone(o, Sine, 80.0, 45.0, s, 0.18, 0.7, 0.005);
                    self.hiss(o, Lowpass, 300.0, 120.0, s, 0.12, 0.25, 1.0);
                }
            }
            "howl-wolf" => {
                let (Some(ctx), true) = (&self.ctx, true) else {
                    return;
                };
                if let (Ok(osc), Ok(lfo), Ok(depth), Ok(env)) = (
                    ctx.create_oscillator(),
                    ctx.create_oscillator(),
                    ctx.create_gain(),
                    ctx.create_gain(),
                ) {
                    osc.set_type(Sine);
                    let f = osc.frequency();
                    let _ = f.set_value_at_time(330.0, t);
                    let _ = f.linear_ramp_to_value_at_time(620.0, t + 0.6);
                    let _ = f.linear_ramp_to_value_at_time(560.0, t + 1.6);
                    let _ = f.linear_ramp_to_value_at_time(300.0, t + 2.4);
                    lfo.frequency().set_value(5.5);
                    depth.gain().set_value(14.0);
                    let _ = lfo.connect_with_audio_node(&depth);
                    let _ = depth.connect_with_audio_param(&osc.frequency());
                    let _ = env.gain().set_value_at_time(0.0001, t);
                    let _ = env.gain().linear_ramp_to_value_at_time(0.45, t + 0.4);
                    let _ = env.gain().linear_ramp_to_value_at_time(0.35, t + 1.8);
                    let _ = env.gain().linear_ramp_to_value_at_time(0.0001, t + 2.5);
                    let _ = osc.connect_with_audio_node(&env);
                    let _ = env.connect_with_audio_node(o);
                    let _ = osc.start_with_when(t);
                    let _ = lfo.start_with_when(t);
                    let _ = osc.stop_with_when(t + 2.6);
                    let _ = lfo.stop_with_when(t + 2.6);
                }
                self.hiss(o, Bandpass, 900.0, 700.0, t, 2.4, 0.08, 4.0);
            }
            "thunder" => {
                self.hiss(o, Lowpass, 4000.0, 300.0, t, 0.25, 0.9, 0.7);
                self.hiss(o, Lowpass, 220.0, 50.0, t + 0.1, 2.8, 0.9, 0.5);
            }
            "crow" => {
                for i in 0..2 {
                    let s = t + i as f64 * 0.38;
                    self.tone(o, Square, 720.0, 520.0, s, 0.26, 0.12, 0.01);
                    self.hiss(o, Bandpass, 1400.0, 900.0, s, 0.24, 0.3, 3.0);
                }
            }
            "owl" => {
                self.tone(o, Sine, 410.0, 390.0, t, 0.35, 0.4, 0.06);
                self.tone(o, Sine, 380.0, 350.0, t + 0.5, 0.6, 0.35, 0.08);
            }
            "hiss-cat" => {
                self.hiss(o, Highpass, 3500.0, 5000.0, t, 0.7, 0.55, 0.8);
                self.tone(o, Sawtooth, 620.0, 380.0, t, 0.25, 0.06, 0.01);
            }
            "laugh-evil" => {
                self.syllables(o, Sawtooth, 150.0, 120.0, 6, 0.2, 0.18, t);
                self.hiss(o, Bandpass, 500.0, 400.0, t, 1.2, 0.08, 2.0);
            }
            "cackle-witch" => {
                self.syllables(o, Square, 900.0, 600.0, 9, 0.11, 0.1, t);
            }
            "chains" => {
                for i in 0..8 {
                    let s = t + i as f64 * 0.07 + f64::from(self.rand()) * 0.04;
                    let f = 2200.0 + self.rand() * 1600.0;
                    self.tone(o, Triangle, f, f * 0.98, s, 0.18, 0.12, 0.002);
                }
            }
            "heartbeat" => {
                for i in 0..4 {
                    let s = t + i as f64 * 0.8;
                    self.tone(o, Sine, 70.0, 40.0, s, 0.16, 0.9, 0.005);
                    self.tone(o, Sine, 64.0, 38.0, s + 0.2, 0.14, 0.7, 0.005);
                }
            }
            "organ-sting" => {
                for midi in [50, 53, 57, 62, 56] {
                    let f = hz(midi);
                    self.tone(o, Sawtooth, f, f, t, 1.8, 0.07, 0.03);
                    self.tone(o, Square, f * 2.0, f * 2.0, t, 1.6, 0.03, 0.03);
                }
            }
            "scream-kid" => {
                self.tone(o, Triangle, 600.0, 1400.0, t, 0.5, 0.18, 0.02);
                self.tone(o, Sine, 650.0, 1500.0, t + 0.02, 0.5, 0.12, 0.02);
            }
            "whisper" => {
                for i in 0..5 {
                    let s = t + i as f64 * 0.22;
                    let f = 1800.0 + self.rand() * 1500.0;
                    self.hiss(o, Bandpass, f, f * 0.8, s, 0.2, 0.35, 6.0);
                }
            }
            "chime-clock" => {
                for i in 0..3 {
                    let s = t + i as f64 * 0.9;
                    self.tone(o, Sine, 587.3, 587.3, s, 1.4, 0.25, 0.005);
                    self.tone(o, Sine, 1174.7, 1174.7, s, 0.8, 0.08, 0.005);
                }
            }
            "bubbling" => {
                for i in 0..8 {
                    let s = t + i as f64 * 0.12 + f64::from(self.rand()) * 0.06;
                    let f = 180.0 + self.rand() * 220.0;
                    self.tone(o, Sine, f, f * 2.2, s, 0.08, 0.3, 0.005);
                }
            }
            "flutter-bat" => {
                for i in 0..14 {
                    let s = t + i as f64 * 0.045;
                    self.hiss(o, Bandpass, 2200.0, 1800.0, s, 0.03, 0.4, 3.0);
                }
            }
            "music-box" => {
                for (i, midi) in [74, 77, 81, 80, 77].iter().enumerate() {
                    let f = hz(*midi);
                    self.tone(o, Sine, f, f, t + i as f64 * 0.3, 0.9, 0.18, 0.003);
                }
            }
            "candle-gust" => {
                self.hiss(o, Lowpass, 900.0, 200.0, t, 0.9, 0.35, 0.7);
            }
            "door-slam" => {
                self.hiss(o, Lowpass, 3000.0, 400.0, t, 0.18, 0.9, 0.7);
                self.tone(o, Sine, 110.0, 40.0, t, 0.35, 0.9, 0.003);
            }
            "rattle-bones" => {
                for i in 0..10 {
                    let s = t + i as f64 * 0.06;
                    let f = 700.0 + self.rand() * 500.0;
                    self.tone(o, Triangle, f, f * 0.7, s, 0.05, 0.22, 0.002);
                }
            }
            "groan" => {
                self.tone(o, Sawtooth, 95.0, 70.0, t, 1.6, 0.14, 0.3);
                self.hiss(o, Bandpass, 400.0, 300.0, t, 1.5, 0.12, 5.0);
            }
            "skitter" => {
                for i in 0..12 {
                    let s = t + i as f64 * 0.035 + f64::from(self.rand()) * 0.02;
                    self.hiss(o, Highpass, 4000.0, 5000.0, s, 0.02, 0.4, 1.0);
                }
            }
            "gallop" => {
                for i in 0..9 {
                    let s = t + (i / 3) as f64 * 0.42 + (i % 3) as f64 * 0.09;
                    self.tone(o, Sine, 120.0, 60.0, s, 0.1, 0.6, 0.003);
                }
            }
            "squeak" => {
                self.tone(o, Sine, 3000.0, 3600.0, t, 0.07, 0.15, 0.003);
                self.tone(o, Sine, 3200.0, 3800.0, t + 0.12, 0.07, 0.15, 0.003);
            }
            "piano-tune" => {
                for (i, midi) in [62, 65, 69, 68, 69, 65, 62, 61].iter().enumerate() {
                    let f = hz(*midi);
                    self.tone(o, Triangle, f, f, t + i as f64 * 0.42, 1.2, 0.2, 0.004);
                }
            }
            "book-thud" => {
                self.hiss(o, Highpass, 2000.0, 1200.0, t, 0.15, 0.25, 1.0);
                self.tone(o, Sine, 140.0, 60.0, t + 0.25, 0.25, 0.7, 0.003);
            }
            "drip" => {
                self.tone(o, Sine, 1400.0, 700.0, t, 0.12, 0.3, 0.003);
            }
            _ => {}
        }
    }

    /// The arrival cue's sounds, from behind the player.
    pub fn cue(&mut self, sounds: &[String], at: Vec3) {
        for (i, s) in sounds.iter().enumerate() {
            // The crow answers the creak a moment later, from a little off.
            let p = at + Vec3::new(i as f32 * 3.0, i as f32 * 4.0, 0.0);
            if i == 0 {
                self.play(s, Some(p));
            } else {
                let id = s.clone();
                self.later.push((self.now() + 0.9 * i as f64, id, p));
            }
        }
    }

    /// Changes the ambience bed, crossfading.
    pub fn bed(&mut self, id: Option<&str>) {
        if self.bed.as_ref().map(|b| b.id.as_str()) == id {
            return;
        }
        let now = self.now();
        if let Some(old) = self.bed.take() {
            let _ = old.gain.gain().set_target_at_time(0.0, now, 0.4);
            for s in old.stop {
                let _ = s.stop_with_when(now + 2.0);
            }
        }
        let (Some(id), Some(ctx), Some(amb)) = (id, &self.ctx, &self.ambience) else {
            return;
        };
        let Ok(gain) = ctx.create_gain() else { return };
        let _ = gain.gain().set_value_at_time(0.0, now);
        let _ = gain.gain().set_target_at_time(1.0, now, 0.6);
        let _ = gain.connect_with_audio_node(amb);
        // The steady part of each bed: filtered noise, its filter swept
        // slowly for the wind.
        let steady = |kind: BiquadFilterType, f: f32, q: f32, level: f32, sweep: f32| {
            let mut nodes: Vec<web_sys::AudioScheduledSourceNode> = Vec::new();
            let (Some(buffer), Ok(source), Ok(filter), Ok(level_node)) = (
                self.noise.as_ref(),
                ctx.create_buffer_source(),
                ctx.create_biquad_filter(),
                ctx.create_gain(),
            ) else {
                return nodes;
            };
            source.set_buffer(Some(buffer));
            source.set_loop(true);
            filter.set_type(kind);
            filter.frequency().set_value(f);
            filter.q().set_value(q);
            level_node.gain().set_value(level);
            if sweep > 0.0
                && let (Ok(lfo), Ok(depth)) = (ctx.create_oscillator(), ctx.create_gain())
            {
                lfo.frequency().set_value(0.08);
                depth.gain().set_value(sweep);
                let _ = lfo.connect_with_audio_node(&depth);
                let _ = depth.connect_with_audio_param(&filter.frequency());
                let node: &web_sys::AudioScheduledSourceNode = lfo.as_ref();
                let _ = node.start();
                nodes.push(node.clone());
            }
            let _ = source.connect_with_audio_node(&filter);
            let _ = filter.connect_with_audio_node(&level_node);
            let _ = level_node.connect_with_audio_node(&gain);
            let node: &web_sys::AudioScheduledSourceNode = source.as_ref();
            let _ = node.start();
            nodes.push(node.clone());
            nodes
        };
        let stop = match id {
            "wind" => steady(BiquadFilterType::Bandpass, 500.0, 1.2, 0.5, 300.0),
            "rain" => steady(BiquadFilterType::Highpass, 2500.0, 0.5, 0.18, 0.0),
            "fireplace" => steady(BiquadFilterType::Lowpass, 500.0, 0.7, 0.35, 0.0),
            "cauldron" => steady(BiquadFilterType::Lowpass, 300.0, 0.7, 0.15, 0.0),
            "thunder-distant" => steady(BiquadFilterType::Lowpass, 120.0, 0.5, 0.2, 0.0),
            _ => Vec::new(),
        };
        self.bed = Some(Bed {
            id: id.to_owned(),
            gain,
            stop,
            next: now + 0.5,
        });
    }

    /// Keeps the music and the beds' ticks going a little ahead. Call
    /// once a frame.
    pub fn tick(&mut self) {
        if self.ctx.is_none() {
            return;
        }
        let now = self.now();
        let due: Vec<(f64, String, Vec3)> = {
            let (due, keep): (Vec<_>, Vec<_>) = std::mem::take(&mut self.later)
                .into_iter()
                .partition(|(t, _, _)| *t <= now);
            self.later = keep;
            due
        };
        for (_, id, at) in due {
            self.play(&id, Some(at));
        }
        self.tick_bed(now);
        if !self.music_on {
            return;
        }
        let Some(music) = self.music.clone() else {
            return;
        };
        let out: &AudioNode = music.as_ref();
        if self.next_beat < now {
            self.next_beat = now + 0.05;
        }
        while self.next_beat < now + 0.3 {
            let t = self.next_beat;
            let b = self.beat;
            // The walking bass.
            let f = hz(BASS[b % BASS.len()]);
            self.tone(
                out,
                OscillatorType::Triangle,
                f,
                f,
                t,
                BEAT * 0.9,
                0.5,
                0.01,
            );
            self.tone(
                out,
                OscillatorType::Sine,
                f / 2.0,
                f / 2.0,
                t,
                BEAT * 0.8,
                0.35,
                0.01,
            );
            // The organ pad, a bar at a time.
            if b % 4 == 0 {
                for midi in PAD[(b / 4) % PAD.len()] {
                    let p = hz(midi);
                    self.tone(
                        out,
                        OscillatorType::Sawtooth,
                        p,
                        p,
                        t,
                        BEAT * 4.0,
                        0.035,
                        0.6,
                    );
                    self.tone(
                        out,
                        OscillatorType::Sawtooth,
                        p * 1.003,
                        p * 1.003,
                        t,
                        BEAT * 4.0,
                        0.03,
                        0.6,
                    );
                }
            }
            // The music box, sparse, more as intensity rises.
            let every = if self.intensity >= 4.0 {
                1
            } else if self.intensity >= 3.0 {
                2
            } else if self.intensity >= 2.0 {
                4
            } else {
                0
            };
            if every > 0 && b % every == 0 {
                let note = BOX[(b / every.max(1) + b / 7) % BOX.len()];
                let f = hz(note);
                self.tone(out, OscillatorType::Sine, f, f, t, BEAT * 1.4, 0.07, 0.003);
            }
            self.next_beat += BEAT;
            self.beat += 1;
        }
    }

    fn tick_bed(&mut self, now: f64) {
        let Some((id, next)) = self.bed.as_ref().map(|b| (b.id.clone(), b.next)) else {
            return;
        };
        if next > now + 0.2 {
            return;
        }
        let Some(gain) = self.bed.as_ref().map(|b| b.gain.clone()) else {
            return;
        };
        let out: &AudioNode = gain.as_ref();
        let t = next.max(now);
        let wait = match id.as_str() {
            "clock-tick" => {
                let f = if self.beat % 2 == 0 { 2400.0 } else { 2000.0 };
                self.hiss(out, BiquadFilterType::Bandpass, f, f, t, 0.03, 0.6, 8.0);
                1.0
            }
            "rain" => {
                let f = 2500.0 + self.rand() * 2500.0;
                self.tone(out, OscillatorType::Sine, f, f * 0.6, t, 0.03, 0.05, 0.002);
                0.05 + f64::from(self.rand()) * 0.12
            }
            "fireplace" => {
                self.hiss(
                    out,
                    BiquadFilterType::Highpass,
                    2000.0,
                    3000.0,
                    t,
                    0.02,
                    0.5,
                    1.0,
                );
                0.08 + f64::from(self.rand()) * 0.5
            }
            "cauldron" => {
                let f = 150.0 + self.rand() * 200.0;
                self.tone(out, OscillatorType::Sine, f, f * 2.0, t, 0.09, 0.25, 0.005);
                0.12 + f64::from(self.rand()) * 0.35
            }
            "thunder-distant" => {
                self.hiss(
                    out,
                    BiquadFilterType::Lowpass,
                    180.0,
                    50.0,
                    t,
                    3.0,
                    0.5,
                    0.5,
                );
                12.0 + f64::from(self.rand()) * 12.0
            }
            "wind" => {
                self.hiss(
                    out,
                    BiquadFilterType::Bandpass,
                    700.0,
                    380.0,
                    t,
                    3.5,
                    0.2,
                    2.0,
                );
                4.0 + f64::from(self.rand()) * 4.0
            }
            _ => 5.0,
        };
        if let Some(b) = self.bed.as_mut() {
            b.next = t + wait;
        }
    }
}
