//! Effect definitions: what a TOML file under `assets/verse/fx/effects/`
//! says, parsed and checked against the sprite sheets.
//!
//! An effect is a list of emitters. Each emitter names a sheet and a frame
//! range, how many particles it throws and how fast, how they move, and how
//! their size, color, opacity, and blend change over their life. Values
//! that change over a particle's life are [`Curve`]s and [`ColorCurve`]s:
//! a constant, three keys at the start, middle, and end of life, or
//! explicit `[t, value]` keys.

use serde::Deserialize;

use super::sheet::{self, Sheet};

/// A scalar over a particle's life, `t` from 0 at birth to 1 at death.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum Curve {
    /// The same value all life.
    Constant(f32),
    /// Start, middle (at `t` = 0.5), and end.
    Three([f32; 3]),
    /// `[t, value]` keys in increasing `t`.
    Keys(Vec<[f32; 2]>),
}

impl Curve {
    /// The value at `t`, linear between keys and held past the ends.
    #[must_use]
    pub fn at(&self, t: f32) -> f32 {
        match self {
            Self::Constant(v) => *v,
            Self::Three([a, b, c]) => {
                let t = t.clamp(0.0, 1.0);
                if t < 0.5 {
                    a + (b - a) * t * 2.0
                } else {
                    b + (c - b) * (t - 0.5) * 2.0
                }
            }
            Self::Keys(keys) => keyed(keys.iter().map(|[t, v]| (*t, *v)), t, |a, b, x| {
                a + (b - a) * x
            })
            .unwrap_or(0.0),
        }
    }

    fn check(&self, what: &str) -> Result<(), String> {
        let values: Vec<f32> = match self {
            Self::Constant(v) => vec![*v],
            Self::Three(v) => v.to_vec(),
            Self::Keys(keys) => {
                check_keys(keys.iter().map(|k| k[0]), what)?;
                keys.iter().map(|k| k[1]).collect()
            }
        };
        if values.iter().any(|v| !v.is_finite()) {
            return Err(format!("{what} has a value that isn't finite"));
        }
        Ok(())
    }
}

/// A linear RGB color over a particle's life.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum ColorCurve {
    Constant([f32; 3]),
    Three([[f32; 3]; 3]),
    Keys(Vec<(f32, [f32; 3])>),
}

impl ColorCurve {
    #[must_use]
    pub fn at(&self, t: f32) -> [f32; 3] {
        let mix = |a: [f32; 3], b: [f32; 3], x: f32| [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * x);
        match self {
            Self::Constant(c) => *c,
            Self::Three([a, b, c]) => {
                let t = t.clamp(0.0, 1.0);
                if t < 0.5 {
                    mix(*a, *b, t * 2.0)
                } else {
                    mix(*b, *c, (t - 0.5) * 2.0)
                }
            }
            Self::Keys(keys) => keyed(keys.iter().copied(), t, mix).unwrap_or([0.0; 3]),
        }
    }

    fn check(&self, what: &str) -> Result<(), String> {
        let values: Vec<[f32; 3]> = match self {
            Self::Constant(c) => vec![*c],
            Self::Three(c) => c.to_vec(),
            Self::Keys(keys) => {
                check_keys(keys.iter().map(|k| k.0), what)?;
                keys.iter().map(|k| k.1).collect()
            }
        };
        if values.iter().flatten().any(|v| !v.is_finite() || *v < 0.0) {
            return Err(format!("{what} has a negative or non-finite channel"));
        }
        Ok(())
    }
}

fn keyed<T: Copy>(
    keys: impl Iterator<Item = (f32, T)>,
    t: f32,
    mix: impl Fn(T, T, f32) -> T,
) -> Option<T> {
    let mut before: Option<(f32, T)> = None;
    for (k, v) in keys {
        if t <= k {
            return Some(match before {
                Some((p, pv)) if k > p => mix(pv, v, (t - p) / (k - p)),
                _ => v,
            });
        }
        before = Some((k, v));
    }
    before.map(|(_, v)| v)
}

fn check_keys(times: impl Iterator<Item = f32>, what: &str) -> Result<(), String> {
    let times: Vec<f32> = times.collect();
    if times.is_empty() {
        return Err(format!("{what} has no keys"));
    }
    if times.windows(2).any(|w| w[1] < w[0]) || times.iter().any(|t| !(0.0..=1.0).contains(t)) {
        return Err(format!("{what} keys must rise from 0 to 1"));
    }
    Ok(())
}

/// How a particle's frames play.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Animate {
    /// The frame range once over the particle's life, blending between
    /// neighboring frames.
    #[default]
    Life,
    /// One frame from the range, chosen at birth.
    Random,
    /// The range over and over at `fps`, from a random first frame.
    Loop,
}

/// How a particle composites.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Blend {
    /// Adds light: fire, sparks, glows. Order doesn't matter.
    #[default]
    Additive,
    /// Covers what's behind: smoke and dust, drawn back to front.
    Alpha,
}

/// What a particle's color means.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Light {
    /// Emitted luminance: the color times `luminance`, in cd/m² before
    /// exposure.
    #[default]
    Emit,
    /// A surface color in the scene's display scale, like smoke lit by the
    /// day: the sheet carries its own shading.
    Lit,
}

/// Which way a particle's quad faces.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Orient {
    /// Turned toward the camera.
    #[default]
    Camera,
    /// Lying flat on the ground, like a shockwave or a scorch.
    Ground,
}

/// Where particles are born, around the emitter's position.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Shape {
    #[default]
    Point,
    /// Anywhere inside a ball of `radius`.
    Sphere,
    /// Anywhere on a flat disc of `radius` across the ground.
    Disc,
    /// On the rim of a flat circle of `radius`.
    Ring,
}

/// One emitter.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Emitter {
    pub name: String,
    /// The sheet's name in [`sheet::SHEETS`].
    pub sheet: String,
    /// The first and last frame used, inclusive. Defaults to the whole
    /// sheet.
    pub frames: Option<[u32; 2]>,
    #[serde(default)]
    pub animate: Animate,
    /// Frames a second for [`Animate::Loop`].
    #[serde(default = "default_fps")]
    pub fps: f32,
    #[serde(default)]
    pub blend: Blend,
    /// Overrides the blend over life, from 0 (alpha) to 1 (additive), so
    /// fire can cool into smoke that covers.
    pub additive: Option<Curve>,
    #[serde(default)]
    pub light: Light,
    /// Scales an emitting particle's color, cd/m².
    #[serde(default = "one")]
    pub luminance: f32,
    #[serde(default)]
    pub orient: Orient,
    /// Seconds after the effect starts before this emitter starts.
    #[serde(default)]
    pub delay: f32,
    /// Particles thrown at once when the emitter starts.
    #[serde(default)]
    pub burst: u32,
    /// Particles a second while the emitter runs.
    #[serde(default)]
    pub rate: f32,
    /// Seconds the emitter runs after its delay; 0 runs it until the effect
    /// is stopped.
    #[serde(default)]
    pub duration: f32,
    /// A particle's life, s, from the first to the second at random.
    pub life: [f32; 2],
    #[serde(default)]
    pub shape: Shape,
    #[serde(default)]
    pub radius: f32,
    /// The direction particles leave in, in the effect's frame (+Y is the
    /// effect's axis).
    #[serde(default = "up")]
    pub direction: [f32; 3],
    /// The widest angle from `direction`, degrees; 180 throws every way.
    #[serde(default)]
    pub spread: f32,
    /// The speed particles leave at, m/s, at random between the two.
    #[serde(default)]
    pub speed: [f32; 2],
    /// Speed away from the effect's center through the birth point, m/s,
    /// at random between the two: a ring that spreads, a ball that bursts.
    #[serde(default)]
    pub radial: [f32; 2],
    /// How much of the effect's own velocity particles take.
    #[serde(default)]
    pub inherit: f32,
    /// Upward acceleration, m/s²: negative falls, positive rises like hot
    /// smoke.
    #[serde(default)]
    pub gravity: f32,
    /// How fast speed bleeds away, 1/s.
    #[serde(default)]
    pub drag: f32,
    /// Half the quad's size at birth, m, at random between the two.
    pub size: [f32; 2],
    /// The size's factor over life.
    #[serde(default = "one_curve")]
    pub scale: Curve,
    #[serde(default = "white")]
    pub color: ColorCurve,
    #[serde(default = "one_curve")]
    pub alpha: Curve,
    /// Turning speed, radians a second, at random between the two.
    #[serde(default)]
    pub spin: [f32; 2],
    /// Whether each particle starts at a random angle.
    #[serde(default = "yes")]
    pub rotate: bool,
    /// Lays the quad along its velocity: the tail is the distance the
    /// particle covers in this many seconds.
    #[serde(default)]
    pub stretch: f32,
    /// The longest tail, m.
    #[serde(default = "far")]
    pub stretch_max: f32,
    /// Whether particles bounce off the ground, keeping this much of their
    /// speed; 0 lets them pass through.
    #[serde(default)]
    pub bounce: f32,
    /// Higher priorities are drawn first when a frame's budget runs out.
    #[serde(default)]
    pub priority: u8,
}

fn default_fps() -> f32 {
    12.0
}
fn one() -> f32 {
    1.0
}
fn one_curve() -> Curve {
    Curve::Constant(1.0)
}
fn white() -> ColorCurve {
    ColorCurve::Constant([1.0; 3])
}
fn up() -> [f32; 3] {
    [0.0, 1.0, 0.0]
}
fn yes() -> bool {
    true
}
fn far() -> f32 {
    50.0
}

impl Emitter {
    /// The emitter's sheet.
    #[must_use]
    pub fn sheet(&self) -> &'static Sheet {
        sheet::find(&self.sheet).unwrap_or(&sheet::SHEETS[0])
    }

    /// The first and last frame.
    #[must_use]
    pub fn frame_range(&self) -> (u32, u32) {
        let s = self.sheet();
        let [a, b] = self.frames.unwrap_or([0, s.frames - 1]);
        (a, b)
    }

    /// The blend at `t`, 0 for alpha to 1 for additive.
    #[must_use]
    pub fn additive_at(&self, t: f32) -> f32 {
        match &self.additive {
            Some(c) => c.at(t).clamp(0.0, 1.0),
            None => match self.blend {
                Blend::Additive => 1.0,
                Blend::Alpha => 0.0,
            },
        }
    }

    /// The most particles this emitter can have alive at once.
    #[must_use]
    pub fn peak(&self) -> f32 {
        let running = if self.duration > 0.0 {
            self.duration.min(self.life[1])
        } else {
            self.life[1]
        };
        self.burst as f32 + self.rate * running
    }

    fn check(&self, effect: &str) -> Result<(), String> {
        let what = |field: &str| format!("{effect}: emitter {}: {field}", self.name);
        let Some(sheet) = sheet::find(&self.sheet) else {
            return Err(what(&format!("no sheet named {}", self.sheet)));
        };
        if let Some([a, b]) = self.frames
            && (a > b || b >= sheet.frames)
        {
            return Err(what(&format!(
                "frames {a}..={b} outside the {} frames of {}",
                sheet.frames, sheet.name
            )));
        }
        let ranged = |pair: [f32; 2], name: &str| -> Result<(), String> {
            if pair.iter().any(|v| !v.is_finite() || *v < 0.0) || pair[1] < pair[0] {
                Err(what(&format!(
                    "{name} must be two rising values of 0 or more"
                )))
            } else {
                Ok(())
            }
        };
        ranged(self.life, "life")?;
        ranged(self.size, "size")?;
        ranged(self.speed, "speed")?;
        ranged(self.radial, "radial")?;
        if self.life[1] <= 0.0 {
            return Err(what("life must be above 0"));
        }
        if self.spin.iter().any(|v| !v.is_finite()) {
            return Err(what("spin isn't finite"));
        }
        for (v, name) in [
            (self.fps, "fps"),
            (self.luminance, "luminance"),
            (self.delay, "delay"),
            (self.rate, "rate"),
            (self.duration, "duration"),
            (self.radius, "radius"),
            (self.drag, "drag"),
            (self.stretch, "stretch"),
            (self.stretch_max, "stretch_max"),
            (self.bounce, "bounce"),
            (self.spread, "spread"),
        ] {
            if !v.is_finite() || v < 0.0 {
                return Err(what(&format!("{name} must be 0 or more")));
            }
        }
        if !self.gravity.is_finite() || !self.inherit.is_finite() {
            return Err(what("gravity and inherit must be finite"));
        }
        if self.spread > 180.0 {
            return Err(what("spread is at most 180 degrees"));
        }
        if self.direction.iter().any(|v| !v.is_finite())
            || glam::Vec3::from(self.direction).length_squared() < 1e-6
        {
            return Err(what("direction must be a nonzero vector"));
        }
        if self.burst == 0 && self.rate <= 0.0 {
            return Err(what("needs a burst or a rate"));
        }
        self.scale.check(&what("scale"))?;
        self.alpha.check(&what("alpha"))?;
        self.color.check(&what("color"))?;
        if let Some(a) = &self.additive {
            a.check(&what("additive"))?;
        }
        Ok(())
    }
}

/// One effect: a name and its emitters.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Effect {
    #[serde(skip)]
    pub name: String,
    /// What the effect is for, one sentence.
    pub description: String,
    #[serde(rename = "emitter")]
    pub emitters: Vec<Emitter>,
}

impl Effect {
    /// Parses and checks the TOML of the effect called `name`.
    ///
    /// # Errors
    ///
    /// Returns what is malformed or names a missing sheet or frame.
    pub fn parse(name: &str, source: &str) -> Result<Self, String> {
        let mut effect: Self = toml::from_str(source).map_err(|e| format!("{name}: {e}"))?;
        effect.name = name.to_owned();
        effect.check()?;
        Ok(effect)
    }

    fn check(&self) -> Result<(), String> {
        if self.emitters.is_empty() {
            return Err(format!("{}: no emitters", self.name));
        }
        for e in &self.emitters {
            e.check(&self.name)?;
        }
        Ok(())
    }

    /// The most particles one instance can have alive at once, an upper
    /// bound the budgets are checked against.
    #[must_use]
    pub fn peak(&self) -> f32 {
        self.emitters.iter().map(Emitter::peak).sum()
    }
}
