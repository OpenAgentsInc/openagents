//! The interactive ripple and foam field (`docs/verse/water.md`, phase W6):
//! a height field on a grid centred on the camera, which every mover,
//! impact, and spell in water writes into, and a foam field that keeps the
//! trails they leave and drifts with the current.
//!
//! The techniques are public ones, reimplemented here:
//!
//! - Low and Medium step the damped wave equation `h_tt = c² ∇²h` on the
//!   grid with a fixed step, by symplectic Euler on a height and a vertical
//!   velocity with a reflecting boundary at dry cells (Bridson and
//!   Müller-Fischer, "Fluid Simulation for Computer Graphics", SIGGRAPH
//!   2007 course notes, section on height-field water).
//! - High steps Tessendorf's iWave ("Interactive Water Surfaces", Game
//!   Programming Gems 4, 2004): `h' = h (2 − αΔt)/(1 + αΔt) − h₋/(1 + αΔt)
//!   − gΔt²/(1 + αΔt) · (G ⊛ h)`, where the convolution kernel `G` is the
//!   vertical derivative `√(−∇²)`, summed over the wavenumbers and
//!   tapered to a few cells, so short waves run slower
//!   than long ones as on real water, and a moving source draws a true
//!   Kelvin wake. Dry cells are obstructions: their heights are held at 0.
//! - Every tier draws the Kelvin wedge behind each mover on top of the
//!   simulated field: arms at the Kelvin angle `asin(1/3)` ≈ 19.47° with
//!   transverse waves of wavelength `2πv²/g` between them (Lord Kelvin,
//!   "On Ship Waves", 1887; Thomson's stationary-phase result as given in
//!   Lighthill, *Waves in Fluids*, 1978, section 3.10), so a swimmer and a
//!   boat leave the V shape even on the non-dispersive tiers.
//! - The foam field decays exponentially and is carried by the current by
//!   a semi-Lagrangian step (Stam, "Stable Fluids", SIGGRAPH 1999).
//!
//! The field is visual only. Each client runs its own, from the sources
//! its frame lists, and nothing gameplay reads it. The grid scrolls in
//! whole cells as the camera moves, so it never resamples. A tier's grid
//! matches the water cascades' array texture (`water::ocean`), which holds
//! it as one more layer: height, its two slopes, and foam, in half floats.

use glam::Vec2;
use verse_engine::quality::Tier;

/// The most sources a frame lists (High's budget).
pub const MAX_SOURCES: usize = 32;
/// Gravity, m/s².
const G: f32 = 9.81;
/// The Kelvin wedge's half angle, rad: asin(1/3).
pub const KELVIN: f32 = 0.339_837;
/// How fast ripples run on the wave-equation tiers, m/s: about the speed
/// of a 1 m gravity wave.
pub const WAVE_SPEED: f32 = 1.2;
/// How long a ripple's height takes to fall to 1/e, s.
const RING_LIFE: f32 = 1.6;
/// How long a displaced surface takes to settle back to its level, s: the
/// water a splash pushed aside flows back.
const SETTLE: f32 = 2.5;
/// How long foam takes to fall to 1/e, s.
pub const FOAM_LIFE: f32 = 3.0;
/// How far behind a mover its drawn wedge reaches, s of its travel.
const WAKE_SECONDS: f32 = 3.0;
/// Below this speed a mover draws no wedge, m/s.
pub const WAKE_SPEED: f32 = 0.25;
/// The longest step a frame takes before the field starts over, s.
const GAP: f32 = 0.5;
/// The most steps one frame runs.
const MOST_STEPS: usize = 4;

/// How the field steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kernel {
    /// The damped wave equation.
    Wave,
    /// Tessendorf's iWave, with a kernel `radius` cells wide either side.
    IWave { radius: usize },
}

/// What a tier's field is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plan {
    /// Cells a side; the cascades' array texture's size.
    pub size: usize,
    /// A cell's side, m.
    pub cell: f32,
    pub kernel: Kernel,
    /// Steps a second.
    pub rate: f32,
    /// The most sources the field takes a frame
    /// (`docs/verse/water.md`, Budgets per tier).
    pub sources: usize,
}

impl Plan {
    /// The window's side, m.
    #[must_use]
    pub fn extent(&self) -> f32 {
        self.size as f32 * self.cell
    }
}

/// `tier`'s field: Low 64² at 0.25 m, Medium 64² at 0.3 m, and High 128²
/// at 0.2 m with iWave.
#[must_use]
pub fn plan(tier: Tier) -> Plan {
    let size = super::ocean::plan(tier).size;
    match tier {
        Tier::Low => Plan {
            size,
            cell: 0.25,
            kernel: Kernel::Wave,
            rate: 60.0,
            sources: 8,
        },
        Tier::Medium => Plan {
            size,
            cell: 0.3,
            kernel: Kernel::Wave,
            rate: 60.0,
            sources: 16,
        },
        Tier::High => Plan {
            size,
            cell: 0.2,
            kernel: Kernel::IWave { radius: 4 },
            rate: 30.0,
            sources: 32,
        },
    }
}

/// Something writing into the field this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Source {
    /// Where, x and z, m.
    pub at: [f32; 2],
    /// How fast it moves over the water, m/s; a mover faster than
    /// [`WAKE_SPEED`] draws a Kelvin wedge.
    pub velocity: [f32; 2],
    /// Its footprint's radius, m.
    pub radius: f32,
    /// How hard it pushes the surface down: m/s for a mover, or m at once
    /// for an impact ([`Self::pulse`]).
    pub strength: f32,
    /// Foam it leaves a second (a mover) or at once (an impact), 0 to 1.
    pub foam: f32,
    /// Whether it acts once, at its first step: an impact or a splash.
    pub pulse: bool,
}

impl Source {
    /// A body moving through the water at `velocity`, `radius` wide: a
    /// swimmer, a boat, a duck, or drifting debris. Its push and foam grow
    /// with its speed.
    #[must_use]
    pub fn mover(at: Vec2, velocity: Vec2, radius: f32) -> Self {
        let speed = velocity.length();
        Self {
            at: at.to_array(),
            velocity: velocity.to_array(),
            radius: radius.max(0.05),
            strength: 0.02 + 0.05 * speed.min(3.0),
            foam: (0.9 * speed).min(2.5),
            pulse: false,
        }
    }

    /// An impact at `at`, `radius` wide, that throws the surface down
    /// `depth` m at once: a splash, a stone, or a spell.
    #[must_use]
    pub fn impact(at: Vec2, radius: f32, depth: f32) -> Self {
        Self {
            at: at.to_array(),
            velocity: [0.0; 2],
            radius: radius.max(0.05),
            strength: depth,
            foam: (depth * 6.0).min(1.0),
            pulse: true,
        }
    }

    fn valid(&self) -> bool {
        self.at.iter().chain(&self.velocity).all(|v| v.is_finite())
            && self.radius.is_finite()
            && self.radius > 0.0
            && self.strength.is_finite()
            && self.foam.is_finite()
    }

    /// How much this source matters to a viewer at `eye`, for the budget.
    fn weight(&self, eye: Vec2) -> f32 {
        let d = Vec2::from(self.at).distance(eye);
        let size = self.strength.abs() * if self.pulse { 20.0 } else { 1.0 } + self.foam * 0.05;
        size / (1.0 + d * d * 0.02)
    }
}

/// The water at a point for the field: whether it is wet, and its current
/// (m/s). A zone without one is wet everywhere and still.
#[derive(Clone, Copy)]
pub struct Wet(pub fn(f32, f32) -> Option<[f32; 2]>);

impl PartialEq for Wet {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::fn_addr_eq(self.0, other.0)
    }
}

impl std::fmt::Debug for Wet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Wet")
    }
}

/// What one frame's step did, for the budget checks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Sources the frame listed.
    pub offered: usize,
    /// Sources the field took, at most the plan's budget.
    pub applied: usize,
    /// Movers whose Kelvin wedge is drawn.
    pub wakes: usize,
    /// Steps run.
    pub steps: usize,
}

/// The ripple and foam field around the camera.
#[derive(Clone, Debug)]
pub struct Ripples {
    plan: Plan,
    /// The grid's first cell, in whole cells from the origin.
    origin: [i64; 2],
    /// Heights now and a step ago, m; for the wave equation the second is
    /// the vertical velocity, m/s.
    h: Vec<f32>,
    prev: Vec<f32>,
    next: Vec<f32>,
    foam: Vec<f32>,
    scratch: Vec<f32>,
    /// 1 over water, 0 over dry ground.
    wet: Vec<f32>,
    flow: Vec<[f32; 2]>,
    /// The water function the mask was built with.
    water: Option<Wet>,
    /// iWave's kernel weights by offset, `(2r + 1)²`.
    kernel: Vec<f32>,
    /// The field's clock, s, and the time it has stepped to.
    clock: Option<f32>,
    /// The movers drawn as wedges this frame.
    wakes: Vec<Source>,
    stats: Stats,
}

impl Ripples {
    /// A still field for `plan`.
    #[must_use]
    pub fn new(plan: Plan) -> Self {
        let n = plan.size * plan.size;
        let kernel = match plan.kernel {
            Kernel::IWave { radius } => iwave_kernel(radius),
            Kernel::Wave => Vec::new(),
        };
        Self {
            plan,
            origin: [0; 2],
            h: vec![0.0; n],
            prev: vec![0.0; n],
            next: vec![0.0; n],
            foam: vec![0.0; n],
            scratch: vec![0.0; n],
            wet: vec![1.0; n],
            flow: vec![[0.0; 2]; n],
            water: None,
            kernel,
            clock: None,
            wakes: Vec::new(),
            stats: Stats::default(),
        }
    }

    /// `tier`'s field.
    #[must_use]
    pub fn for_tier(tier: Tier) -> Self {
        Self::new(plan(tier))
    }

    #[must_use]
    pub fn plan(&self) -> Plan {
        self.plan
    }

    /// CPU grids, kernel, and retained wake records, bytes.
    #[must_use]
    pub fn heap_bytes(&self) -> u64 {
        ((self.h.capacity()
            + self.prev.capacity()
            + self.next.capacity()
            + self.foam.capacity()
            + self.scratch.capacity()
            + self.wet.capacity()
            + self.kernel.capacity())
            * 4
            + self.flow.capacity() * 8
            + self.wakes.capacity() * std::mem::size_of::<Source>()) as u64
    }

    /// What the last frame did.
    #[must_use]
    pub fn stats(&self) -> Stats {
        self.stats
    }

    /// The window's lowest corner, x and z, m.
    #[must_use]
    pub fn corner(&self) -> Vec2 {
        Vec2::new(self.origin[0] as f32, self.origin[1] as f32) * self.plan.cell
    }

    /// The uniform's `ripple` row for the texture's layer `layer`: the
    /// window's corner, its side, and the layer.
    #[must_use]
    pub fn window(&self, layer: u32) -> [f32; 4] {
        let c = self.corner();
        [c.x, c.y, self.plan.extent(), layer as f32]
    }

    /// Clears the field.
    pub fn clear(&mut self) {
        self.h.fill(0.0);
        self.prev.fill(0.0);
        self.foam.fill(0.0);
        self.wakes.clear();
        self.clock = None;
    }

    /// Steps the field to `time` (s, the water clock) around `eye` (x, z)
    /// with this frame's `sources`, over `water`. Steps are fixed; a jump
    /// back in time or a long gap starts the field over.
    pub fn advance(&mut self, time: f32, eye: Vec2, sources: &[Source], water: Option<Wet>) {
        self.stats = Stats {
            offered: sources.len(),
            ..Stats::default()
        };
        if !time.is_finite() || !eye.is_finite() {
            return;
        }
        if water != self.water {
            self.water = water;
            self.rebuild_mask(0..self.plan.size, 0..self.plan.size);
        }
        self.recenter(eye);
        let dt = 1.0 / self.plan.rate;
        let clock = match self.clock {
            Some(c) if time >= c && time - c < GAP => c,
            _ => {
                self.clear();
                time
            }
        };
        // The budget: the sources that matter most to this viewer.
        let mut chosen: Vec<Source> = sources.iter().copied().filter(Source::valid).collect();
        chosen.sort_by(|a, b| b.weight(eye).total_cmp(&a.weight(eye)));
        chosen.truncate(self.plan.sources);
        self.stats.applied = chosen.len();
        let mut steps = 0;
        let mut now = clock;
        let mut pulsed = false;
        while now + dt <= time && steps < MOST_STEPS {
            for s in &chosen {
                if s.pulse && pulsed {
                    continue;
                }
                self.inject(s, dt);
            }
            pulsed = true;
            self.step(dt);
            now += dt;
            steps += 1;
        }
        if steps == MOST_STEPS {
            now = time;
        }
        // Pulses that arrived between steps act at the next one.
        if !pulsed {
            for s in chosen.iter().filter(|s| s.pulse) {
                self.inject(s, dt);
            }
        }
        self.clock = Some(now);
        self.wakes = chosen
            .iter()
            .filter(|s| !s.pulse && Vec2::from(s.velocity).length() > WAKE_SPEED)
            .copied()
            .collect();
        self.stats.wakes = self.wakes.len();
        self.stats.steps = steps;
    }

    /// Moves the window so `eye` lies at its middle, in whole cells.
    fn recenter(&mut self, eye: Vec2) {
        let n = self.plan.size as i64;
        let want = [
            (eye.x / self.plan.cell).floor() as i64 - n / 2,
            (eye.y / self.plan.cell).floor() as i64 - n / 2,
        ];
        let shift = [want[0] - self.origin[0], want[1] - self.origin[1]];
        if shift == [0, 0] {
            return;
        }
        self.origin = want;
        if shift[0].abs() >= n || shift[1].abs() >= n {
            self.h.fill(0.0);
            self.prev.fill(0.0);
            self.foam.fill(0.0);
            self.rebuild_mask(0..self.plan.size, 0..self.plan.size);
            return;
        }
        for field in [&mut self.h, &mut self.prev, &mut self.foam] {
            shift_grid(field, &mut self.scratch, self.plan.size, shift, 0.0);
        }
        let mut wet = std::mem::take(&mut self.wet);
        shift_grid(&mut wet, &mut self.scratch, self.plan.size, shift, 1.0);
        self.wet = wet;
        let mut flow: Vec<f32> = Vec::new();
        for axis in 0..2 {
            flow.clear();
            flow.extend(self.flow.iter().map(|f| f[axis]));
            shift_grid(&mut flow, &mut self.scratch, self.plan.size, shift, 0.0);
            for (f, v) in self.flow.iter_mut().zip(&flow) {
                f[axis] = *v;
            }
        }
        // The strips that came into view.
        let size = self.plan.size;
        let fresh = |s: i64| -> std::ops::Range<usize> {
            if s > 0 {
                size - (s as usize).min(size)..size
            } else {
                0..((-s) as usize).min(size)
            }
        };
        if shift[0] != 0 {
            self.rebuild_mask(fresh(shift[0]), 0..size);
        }
        if shift[1] != 0 {
            self.rebuild_mask(0..size, fresh(shift[1]));
        }
    }

    fn rebuild_mask(&mut self, cols: std::ops::Range<usize>, rows: std::ops::Range<usize>) {
        let n = self.plan.size;
        for j in rows {
            for i in cols.clone() {
                let k = j * n + i;
                let p = self.center(i, j);
                let (wet, flow) = match self.water.map(|w| (w.0)(p.x, p.y)) {
                    None => (1.0, [0.0; 2]),
                    Some(None) => (0.0, [0.0; 2]),
                    Some(Some(flow)) => (1.0, flow),
                };
                self.wet[k] = wet;
                self.flow[k] = flow;
                if wet == 0.0 {
                    self.h[k] = 0.0;
                    self.prev[k] = 0.0;
                    self.foam[k] = 0.0;
                }
            }
        }
    }

    /// Cell `(i, j)`'s center, m.
    fn center(&self, i: usize, j: usize) -> Vec2 {
        (Vec2::new(
            (self.origin[0] + i as i64) as f32,
            (self.origin[1] + j as i64) as f32,
        ) + 0.5)
            * self.plan.cell
    }

    /// Writes `s` into the field for a step of `dt`.
    fn inject(&mut self, s: &Source, dt: f32) {
        let n = self.plan.size as i64;
        let cell = self.plan.cell;
        // A footprint narrower than two cells still reaches its neighbors.
        let radius = s.radius.max(cell * 1.2);
        let reach = (radius * 2.0 / cell).ceil() as i64;
        let at = Vec2::from(s.at);
        let ci = (at.x / cell).floor() as i64 - self.origin[0];
        let cj = (at.y / cell).floor() as i64 - self.origin[1];
        let (push, foam) = if s.pulse {
            (s.strength, s.foam)
        } else {
            (s.strength * dt, s.foam * dt)
        };
        for j in (cj - reach).max(0)..(cj + reach + 1).min(n) {
            for i in (ci - reach).max(0)..(ci + reach + 1).min(n) {
                let k = (j * n + i) as usize;
                if self.wet[k] == 0.0 {
                    continue;
                }
                let d = self.center(i as usize, j as usize).distance(at) / radius;
                let w = (-d * d * 2.0).exp();
                if w < 0.01 {
                    continue;
                }
                // The body presses the surface down under its footprint.
                self.h[k] -= push * w;
                self.foam[k] = (self.foam[k] + foam * w).min(1.5);
            }
        }
    }

    /// One fixed step of the heights and the foam.
    fn step(&mut self, dt: f32) {
        let n = self.plan.size;
        match self.plan.kernel {
            Kernel::Wave => {
                // Symplectic Euler on height and vertical velocity
                // (`prev` holds the velocity), reflecting at dry cells.
                let c2 = WAVE_SPEED * WAVE_SPEED / (self.plan.cell * self.plan.cell);
                let keep = (-dt / RING_LIFE).exp();
                for j in 0..n {
                    for i in 0..n {
                        let k = j * n + i;
                        if self.wet[k] == 0.0 {
                            continue;
                        }
                        let h = self.h[k];
                        let at = |ii: usize, jj: usize| {
                            let q = jj * n + ii;
                            // A dry or missing neighbor mirrors this cell.
                            if self.wet[q] == 0.0 { h } else { self.h[q] }
                        };
                        let l = if i > 0 { at(i - 1, j) } else { h };
                        let r = if i + 1 < n { at(i + 1, j) } else { h };
                        let d = if j > 0 { at(i, j - 1) } else { h };
                        let u = if j + 1 < n { at(i, j + 1) } else { h };
                        let lap = l + r + d + u - 4.0 * h;
                        self.prev[k] = (self.prev[k] + dt * c2 * lap) * keep;
                    }
                }
                let settle = (-dt / SETTLE).exp();
                for k in 0..n * n {
                    self.h[k] = (self.h[k] + dt * self.prev[k]) * self.wet[k] * settle;
                }
            }
            Kernel::IWave { radius } => {
                let alpha = 1.0 / RING_LIFE;
                let a = (2.0 - alpha * dt) / (1.0 + alpha * dt);
                let b = 1.0 / (1.0 + alpha * dt);
                let c = G * dt * dt / self.plan.cell / (1.0 + alpha * dt);
                let r = radius as i64;
                let side = 2 * radius + 1;
                let size = n as i64;
                for j in 0..size {
                    for i in 0..size {
                        let k = (j * size + i) as usize;
                        if self.wet[k] == 0.0 {
                            self.next[k] = 0.0;
                            continue;
                        }
                        let mut conv = 0.0;
                        for dj in -r..=r {
                            let jj = (j + dj).clamp(0, size - 1);
                            let row = (jj * size) as usize;
                            let krow = ((dj + r) as usize) * side;
                            for di in -r..=r {
                                let ii = (i + di).clamp(0, size - 1) as usize;
                                conv += self.kernel[krow + (di + r) as usize] * self.h[row + ii];
                            }
                        }
                        self.next[k] = (a * self.h[k] - b * self.prev[k] - c * conv) * self.wet[k];
                    }
                }
                let settle = (-dt / SETTLE).exp();
                for (h, p) in self.next.iter_mut().zip(self.h.iter_mut()) {
                    *h *= settle;
                    *p *= settle;
                }
                std::mem::swap(&mut self.prev, &mut self.h);
                std::mem::swap(&mut self.h, &mut self.next);
            }
        }
        // Foam: decays, and drifts with the current (semi-Lagrangian).
        let fade = (-dt / FOAM_LIFE).exp();
        let cell = self.plan.cell;
        let moving = self.flow.iter().any(|f| f[0] != 0.0 || f[1] != 0.0);
        if moving {
            for j in 0..n {
                for i in 0..n {
                    let k = j * n + i;
                    let f = self.flow[k];
                    let x = i as f32 - f[0] * dt / cell;
                    let z = j as f32 - f[1] * dt / cell;
                    self.scratch[k] = bilinear(&self.foam, n, x, z) * fade * self.wet[k];
                }
            }
            std::mem::swap(&mut self.foam, &mut self.scratch);
        } else {
            for (f, w) in self.foam.iter_mut().zip(&self.wet) {
                *f *= fade * w;
            }
        }
    }

    /// The simulated height at `(x, z)`, m: zero outside the window.
    #[must_use]
    pub fn height_at(&self, x: f32, z: f32) -> f32 {
        let (i, j) = self.cell_of(x, z);
        bilinear(&self.h, self.plan.size, i, j)
    }

    /// The foam at `(x, z)`, 0 and up.
    #[must_use]
    pub fn foam_at(&self, x: f32, z: f32) -> f32 {
        let (i, j) = self.cell_of(x, z);
        bilinear(&self.foam, self.plan.size, i, j)
    }

    fn cell_of(&self, x: f32, z: f32) -> (f32, f32) {
        let c = self.corner();
        (
            (x - c.x) / self.plan.cell - 0.5,
            (z - c.y) / self.plan.cell - 0.5,
        )
    }

    /// The Kelvin wedges the frame's movers draw: height (m) and foam over
    /// every cell.
    fn wedges(&self, height: &mut [f32], foam: &mut [f32]) {
        let n = self.plan.size as i64;
        let cell = self.plan.cell;
        let tan = KELVIN.tan();
        for s in &self.wakes {
            let v = Vec2::from(s.velocity);
            let speed = v.length();
            let dir = v / speed;
            let side = Vec2::new(-dir.y, dir.x);
            // Transverse waves of the speed's wavelength, at least a few
            // cells long so the grid can show them.
            let wavelength = (std::f32::consts::TAU * speed * speed / G).max(cell * 4.0);
            let k = std::f32::consts::TAU / wavelength;
            let length = (speed * WAKE_SECONDS).clamp(1.5, 8.0);
            let amp = s.strength * 0.4;
            let width = (s.radius * 0.6).max(cell * 1.5);
            let at = Vec2::from(s.at);
            // The wedge's bounds, in cells.
            let far = at - dir * length;
            let spread = side * (length * tan + width * 2.0);
            let corners = [at, far + spread, far - spread];
            let lo = corners.iter().fold(Vec2::INFINITY, |m, c| m.min(*c));
            let hi = corners.iter().fold(Vec2::NEG_INFINITY, |m, c| m.max(*c));
            let i0 = ((lo.x / cell).floor() as i64 - self.origin[0]).max(0);
            let i1 = ((hi.x / cell).ceil() as i64 - self.origin[0]).min(n - 1);
            let j0 = ((lo.y / cell).floor() as i64 - self.origin[1]).max(0);
            let j1 = ((hi.y / cell).ceil() as i64 - self.origin[1]).min(n - 1);
            for j in j0..=j1 {
                for i in i0..=i1 {
                    let kk = (j * n + i) as usize;
                    if self.wet[kk] == 0.0 {
                        continue;
                    }
                    let p = self.center(i as usize, j as usize) - at;
                    let behind = -p.dot(dir);
                    if behind <= 0.0 || behind > length {
                        continue;
                    }
                    let across = p.dot(side).abs();
                    let arm = behind * tan;
                    let fade = (1.0 - behind / length).powi(2) / (1.0 + 0.6 * behind);
                    // The divergent arms, and the transverse waves inside.
                    let off = (across - arm) / width;
                    let ridge = (-off * off).exp();
                    let inside = if across < arm {
                        1.0 - (across / arm).powi(2)
                    } else {
                        0.0
                    };
                    let wave = (k * behind).cos();
                    height[kk] += amp * fade * (ridge * 1.4 + 0.5 * inside * wave);
                    foam[kk] += s.foam * 0.5 * fade * ridge;
                }
            }
        }
    }

    /// The layer's texels, row by row: height (m), its slopes along x and
    /// z, and foam, as half floats, the movers' wedges drawn in.
    #[must_use]
    pub fn texels(&self) -> Vec<[u16; 4]> {
        let n = self.plan.size;
        let mut height = self.h.clone();
        let mut foam = self.foam.clone();
        self.wedges(&mut height, &mut foam);
        let cell = self.plan.cell;
        let mut out = Vec::with_capacity(n * n);
        for j in 0..n {
            for i in 0..n {
                let k = j * n + i;
                let at = |ii: usize, jj: usize| height[jj * n + ii];
                let sx = (at((i + 1).min(n - 1), j) - at(i.saturating_sub(1), j))
                    / (cell * (((i + 1).min(n - 1) - i.saturating_sub(1)).max(1)) as f32);
                let sz = (at(i, (j + 1).min(n - 1)) - at(i, j.saturating_sub(1)))
                    / (cell * (((j + 1).min(n - 1) - j.saturating_sub(1)).max(1)) as f32);
                out.push([
                    super::ocean::half(height[k]),
                    super::ocean::half(sx.clamp(-4.0, 4.0)),
                    super::ocean::half(sz.clamp(-4.0, 4.0)),
                    super::ocean::half(foam[k].clamp(0.0, 1.0)),
                ]);
            }
        }
        out
    }

    /// Whether the field holds no ripple or foam worth drawing.
    #[must_use]
    pub fn is_quiet(&self) -> bool {
        self.h.iter().all(|h| h.abs() < 1e-5) && self.foam.iter().all(|f| *f < 1e-3)
    }

    /// The field's total height energy, Σh², for tests.
    #[must_use]
    pub fn energy(&self) -> f32 {
        self.h.iter().map(|h| h * h).sum()
    }
}

/// Shifts a row-major `n`×`n` grid by `shift` cells (the window moved by
/// that much), filling cells that come into view with `fill`.
fn shift_grid(field: &mut [f32], scratch: &mut [f32], n: usize, shift: [i64; 2], fill: f32) {
    let n_i = n as i64;
    for j in 0..n_i {
        for i in 0..n_i {
            let (si, sj) = (i + shift[0], j + shift[1]);
            scratch[(j * n_i + i) as usize] = if (0..n_i).contains(&si) && (0..n_i).contains(&sj) {
                field[(sj * n_i + si) as usize]
            } else {
                fill
            };
        }
    }
    field.copy_from_slice(&scratch[..n * n]);
}

/// Bilinear sample of a row-major `n`×`n` grid at cell coordinates;
/// outside it is zero.
fn bilinear(field: &[f32], n: usize, x: f32, z: f32) -> f32 {
    if !(x > -1.0 && z > -1.0 && x < n as f32 && z < n as f32) {
        return 0.0;
    }
    let (x0, z0) = (x.floor(), z.floor());
    let (fx, fz) = (x - x0, z - z0);
    let get = |i: f32, j: f32| {
        if i < 0.0 || j < 0.0 || i >= n as f32 || j >= n as f32 {
            0.0
        } else {
            field[j as usize * n + i as usize]
        }
    };
    let a = get(x0, z0) * (1.0 - fx) + get(x0 + 1.0, z0) * fx;
    let b = get(x0, z0 + 1.0) * (1.0 - fx) + get(x0 + 1.0, z0 + 1.0) * fx;
    a * (1.0 - fz) + b * fz
}

/// iWave's kernel, `(2r + 1)²` weights in cells: the vertical derivative
/// `√(−∇²)`, whose response to a plane wave of wavenumber `q` is `|q|`
/// (Tessendorf 2004). Its weights are the inverse transform of `|q|` over a
/// 64² lattice of wavenumbers, tapered by a Gaussian to the kernel's width,
/// made to sum to zero (still water stays still), and scaled so a
/// wavenumber of 0.8 rad a cell answers 0.8. The taper keeps the response
/// positive and rising at every wavenumber the grid holds, so no mode grows.
#[must_use]
pub fn iwave_kernel(radius: usize) -> Vec<f32> {
    const N: usize = 64;
    let r = radius as i64;
    let side = 2 * radius + 1;
    let sigma = radius as f64 * 0.55;
    let ks: Vec<f64> = (0..N)
        .map(|i| std::f64::consts::TAU * (i as f64 - (N / 2) as f64) / N as f64)
        .collect();
    let mut weights = vec![0.0f64; side * side];
    for dj in -r..=r {
        for di in -r..=r {
            let mut sum = 0.0;
            for &kz in &ks {
                for &kx in &ks {
                    sum += kx.hypot(kz) * (kx * di as f64 + kz * dj as f64).cos();
                }
            }
            let taper = (-((di * di + dj * dj) as f64) / (2.0 * sigma * sigma)).exp();
            weights[((dj + r) as usize) * side + (di + r) as usize] = sum / (N * N) as f64 * taper;
        }
    }
    let total: f64 = weights.iter().sum();
    weights[radius * side + radius] -= total;
    let response = |q: f64| -> f64 {
        let mut s = 0.0;
        for dj in -r..=r {
            for di in -r..=r {
                s +=
                    weights[((dj + r) as usize) * side + (di + r) as usize] * (q * di as f64).cos();
            }
        }
        s
    };
    let q0 = 0.8;
    let scale = q0 / response(q0);
    weights.iter().map(|w| (w * scale) as f32).collect()
}

/// The kernel's response to a plane wave of wavenumber `q` (rad a cell).
#[must_use]
pub fn kernel_response(kernel: &[f32], q: f32) -> f32 {
    let side = (kernel.len() as f64).sqrt() as usize;
    let r = (side / 2) as i64;
    let mut s = 0.0;
    for dj in -r..=r {
        for di in -r..=r {
            s += kernel[((dj + r) as usize) * side + (di + r) as usize] * (q * di as f32).cos();
        }
    }
    s
}

#[cfg(test)]
#[path = "ripple_tests.rs"]
mod tests;
