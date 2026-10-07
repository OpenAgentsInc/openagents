//! The spectral sea on every tier (`docs/verse/water.md`, phase W4): a
//! `physics::water::Spectrum` synthesized on the CPU, because OpenGL ES 3.0
//! and WebGL2 have no compute shaders, and uploaded as one `Rgba16Float`
//! array texture that both water passes sample (`water_waves`).
//!
//! - [`plan`]: what a tier synthesizes. Low runs only cascade 0, the
//!   gameplay band, so the drawn swell is the one `physics::water` floats
//!   bodies on; its fine detail stays the baked tile (`water::tile`).
//!   Medium runs 2 × 64² cascades and High 3 × 128².
//! - [`Synthesis`]: one tick of every cascade, the foam field, and the
//!   half-float texels. Each cascade fills two layers: displacement x,
//!   height, displacement z, and foam; then the slopes, the crest squeeze
//!   `1 − J` from the Jacobian, and 0.
//! - The foam field is the whitecap cover where crests squeeze the surface
//!   most, read from the Jacobian (Tessendorf 2001, section 4.6; Dupuy and
//!   Bruneton, "Real-time Animation and Rendering of Ocean Whitecaps",
//!   SIGGRAPH Asia 2012). How much of the sea breaks follows the wind by
//!   Monahan and O'Muircheartaigh's whitecap coverage, `W = 3.84 × 10⁻⁶
//!   U^3.41` ("Optimal Power-Law Description of Oceanic Whitecap
//!   Coverage", JPO 1980): each tick, the most squeezed share of a tile
//!   breaks. Foam then decays exponentially and drifts downwind by a
//!   semi-Lagrangian step (Stam, "Stable Fluids", SIGGRAPH 1999). It is
//!   visual only and depends on the ticks synthesized before.
//! - [`OceanGpu`]: the texture and a worker thread that synthesizes the
//!   next tick while the frame draws; browsers have no threads, so on
//!   `wasm32` the frame synthesizes in place.

#[cfg(not(target_arch = "wasm32"))]
use std::sync::mpsc;
#[cfg(not(target_arch = "wasm32"))]
use std::thread;

use physics::water::{Cascade, Spectrum, Synth, Tile};
use verse_engine::quality::Tier;

/// No crest whose Jacobian is above this breaks, however strong the wind.
pub const WHITECAP: f32 = 0.97;
/// The Jacobian span below the breaking threshold over which cover goes
/// from none to full.
pub const WHITECAP_SPAN: f32 = 0.08;
/// How long foam lasts, s: its cover falls by `e` in this time.
pub const FOAM_LIFE: f32 = 2.4;
/// Foam drifts downwind at this share of the wind speed (the surface
/// drift of wind-driven water is about 3%).
pub const FOAM_DRIFT: f32 = 0.03;
/// Layers per cascade in the texture.
pub const LAYERS: u32 = 2;
/// Rows of the uniform the shader reads (`water.ocean`).
pub const ROWS: usize = 6;

/// What a tier synthesizes: cascades, texels along a side, and how many
/// cascades move the vertices (the rest only bend normals).
#[must_use]
pub fn plan(tier: Tier) -> Plan {
    match tier {
        Tier::Low => Plan {
            count: 1,
            size: 64,
            displaced: 1,
            surf: false,
        },
        Tier::Medium => Plan {
            count: 2,
            size: 64,
            displaced: 2,
            surf: true,
        },
        Tier::High => Plan {
            count: 3,
            size: 128,
            displaced: 2,
            surf: true,
        },
    }
}

/// One tier's synthesis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plan {
    pub count: usize,
    pub size: usize,
    pub displaced: usize,
    /// Whether the shader draws breaking surf over shoaling water.
    pub surf: bool,
}

impl Plan {
    /// Texture layers.
    #[must_use]
    pub fn layers(&self) -> u32 {
        self.count as u32 * LAYERS
    }

    /// Bytes of one tick's texels.
    #[must_use]
    pub fn bytes(&self) -> u64 {
        u64::from(self.layers()) * (self.size * self.size * 8) as u64
    }
}

/// Converts to IEEE half-precision bits, rounding to nearest.
#[must_use]
pub fn half(v: f32) -> u16 {
    half::f16::from_f32(v).to_bits()
}

/// One tick of a sea's cascades as texels.
#[derive(Clone, Debug)]
pub struct Frame {
    pub spectrum: Spectrum,
    pub tick: u64,
    pub plan: Plan,
    /// Every layer's texels in order, `size²` each, half-float RGBA.
    pub texels: Vec<[u16; 4]>,
    /// What synthesizing it took, µs.
    pub micros: f64,
}

/// A sea's cascades and foam field, advanced tick by tick.
pub struct Synthesis {
    synth: Synth,
    plan: Plan,
    tile: Tile,
    /// The tile's fields at the tier's size.
    wide: [Vec<f32>; 6],
    foam: Vec<Vec<f32>>,
    scratch: Vec<f32>,
    last: Option<u64>,
}

impl Synthesis {
    /// `spectrum` as `tier` draws it.
    ///
    /// # Errors
    ///
    /// For a spectrum `physics::water::Synth` refuses.
    pub fn new(spectrum: &Spectrum, tier: Tier) -> Result<Self, String> {
        let plan = plan(tier);
        let synth = Synth::new(spectrum, plan.count, plan.size)?;
        Ok(Self {
            synth,
            plan,
            tile: Tile::default(),
            wide: Default::default(),
            foam: vec![vec![0.0; plan.size * plan.size]; plan.count],
            scratch: Vec::new(),
            last: None,
        })
    }

    #[must_use]
    pub fn spectrum(&self) -> &Spectrum {
        self.synth.spectrum()
    }

    #[must_use]
    pub fn plan(&self) -> Plan {
        self.plan
    }

    #[must_use]
    pub fn cascades(&self) -> Vec<Cascade> {
        self.synth.cascades()
    }

    /// The cascades at `tick`, the foam carried on from the tick before.
    pub fn step(&mut self, tick: u64) -> Frame {
        let start = web_time::Instant::now();
        let spectrum = *self.synth.spectrum();
        let n = self.plan.size;
        // Foam older than a few lives, or from a later tick, is forgotten.
        let elapsed = self
            .last
            .filter(|&last| tick >= last)
            .map(|last| (tick - last) as f32 * spectrum.tick as f32)
            .filter(|&dt| dt < FOAM_LIFE * 4.0);
        self.last = Some(tick);
        let mut texels = Vec::with_capacity(self.plan.layers() as usize * n * n);
        let wind = spectrum.direction();
        for (c, cascade) in self.synth.cascades().iter().enumerate() {
            self.synth.tile(c, tick, &mut self.tile);
            // Cascade 0 is the gameplay grid on every tier; a finer tier
            // gets it doubled bilinearly, which its texture filter then
            // reproduces exactly, so the drawn swell is the physics' own.
            let t = &self.tile;
            let fields: [&[f32]; 6] = [&t.dx, &t.height, &t.dz, &t.sx, &t.sz, &t.jacobian];
            if t.size < n {
                for (wide, field) in self.wide.iter_mut().zip(fields) {
                    double(field, t.size, wide);
                }
            } else {
                for (wide, field) in self.wide.iter_mut().zip(fields) {
                    wide.clear();
                    wide.extend_from_slice(field);
                }
            }
            let [dx, height, dz, sx, sz, jacobian] = &self.wide;
            let threshold = breaking(jacobian, spectrum.wind_speed as f32, &mut self.scratch);
            let foam = &mut self.foam[c];
            match elapsed {
                Some(dt) => {
                    let decay = (-dt / FOAM_LIFE).exp();
                    let drift = spectrum.wind_speed as f32 * FOAM_DRIFT * dt;
                    let texel = cascade.patch as f32 / n as f32;
                    let shift = [wind.x as f32 * drift / texel, wind.y as f32 * drift / texel];
                    advect(foam, &mut self.scratch, n, shift);
                    for (f, j) in foam.iter_mut().zip(jacobian) {
                        *f = (*f * decay).max(cover(*j, threshold));
                    }
                }
                None => {
                    for (f, j) in foam.iter_mut().zip(jacobian) {
                        *f = cover(*j, threshold);
                    }
                }
            }
            texels.extend(
                (0..n * n).map(|i| [half(dx[i]), half(height[i]), half(dz[i]), half(foam[i])]),
            );
            texels.extend((0..n * n).map(|i| {
                [
                    half(sx[i]),
                    half(sz[i]),
                    half((1.0 - jacobian[i]).clamp(-1.0, 2.0)),
                    0,
                ]
            }));
        }
        Frame {
            spectrum,
            tick,
            plan: self.plan,
            texels,
            micros: start.elapsed().as_secs_f64() * 1e6,
        }
    }
}

/// Doubles a periodic `n × n` field bilinearly into `out`: each source
/// texel lands on an even texel, and the texels between take the bilinear
/// values there.
fn double(field: &[f32], n: usize, out: &mut Vec<f32>) {
    let m = n * 2;
    out.clear();
    out.resize(m * m, 0.0);
    for j in 0..n {
        let j1 = (j + 1) % n;
        for i in 0..n {
            let i1 = (i + 1) % n;
            let (a, b) = (field[j * n + i], field[j * n + i1]);
            let (c, d) = (field[j1 * n + i], field[j1 * n + i1]);
            let row = 2 * j * m + 2 * i;
            out[row] = a;
            out[row + 1] = 0.5 * (a + b);
            out[row + m] = 0.5 * (a + c);
            out[row + m + 1] = 0.25 * (a + b + c + d);
        }
    }
}

/// The share of the sea whitecaps cover in a wind of `wind_speed` m/s
/// (Monahan and O'Muircheartaigh, 1980), at most a quarter.
#[must_use]
pub fn coverage(wind_speed: f32) -> f32 {
    (3.84e-6 * wind_speed.max(0.0).powf(3.41)).min(0.25)
}

/// The Jacobian below which a tile's crests break this tick: the
/// quantile that leaves half the wind's whitecap coverage breaking anew
/// (the foam that lingers makes up the rest), never above [`WHITECAP`].
fn breaking(jacobian: &[f32], wind_speed: f32, scratch: &mut Vec<f32>) -> f32 {
    let share = coverage(wind_speed) * 0.5;
    let k = (share * jacobian.len() as f32) as usize;
    if k == 0 {
        return f32::NEG_INFINITY;
    }
    scratch.clear();
    scratch.extend_from_slice(jacobian);
    let (_, at, _) = scratch.select_nth_unstable_by(k, f32::total_cmp);
    at.min(WHITECAP)
}

/// The whitecap cover a Jacobian makes against a breaking `threshold`,
/// 0 to 1.
#[must_use]
pub fn cover(jacobian: f32, threshold: f32) -> f32 {
    ((threshold - jacobian) / WHITECAP_SPAN + 0.25).clamp(0.0, 1.0)
}

/// Moves a periodic `n × n` field by `shift` texels (x, z): each texel takes
/// the bilinear value from where its water came from.
fn advect(field: &mut [f32], scratch: &mut Vec<f32>, n: usize, shift: [f32; 2]) {
    if shift[0].abs() < 1e-4 && shift[1].abs() < 1e-4 {
        return;
    }
    scratch.clear();
    scratch.extend_from_slice(field);
    let size = n as f32;
    let (fx, fz) = (shift[0].rem_euclid(size), shift[1].rem_euclid(size));
    let (ix, iz) = (fx.floor() as usize, fz.floor() as usize);
    let (tx, tz) = (fx - fx.floor(), fz - fz.floor());
    for j in 0..n {
        let j0 = (j + n - iz % n) % n;
        let j1 = (j0 + n - 1) % n;
        for i in 0..n {
            let i0 = (i + n - ix % n) % n;
            let i1 = (i0 + n - 1) % n;
            let a = scratch[j0 * n + i0] * (1.0 - tx) + scratch[j0 * n + i1] * tx;
            let b = scratch[j1 * n + i0] * (1.0 - tx) + scratch[j1 * n + i1] * tx;
            field[j * n + i] = a * (1.0 - tz) + b * tz;
        }
    }
}

/// What the shader reads about the sea, `water.ocean` (body 0 only):
///
/// - rows 0 to 2, per cascade: `1 / patch` (1/m), the shortest wavelength
///   (m), the band's characteristic wavenumber for shoaling (rad/m), and
///   the gain (0 for a cascade the tier lacks);
/// - row 3: cascade count, cascades that move vertices, 1 when surf is
///   drawn, and the significant height (m);
/// - row 4: half a texel in texture coordinates, the peak's angular
///   frequency (rad/s) the surf's bores run at, and two spare;
/// - row 5: each cascade's slope variance (for the roughness its faded
///   slopes leave behind), and one spare.
#[must_use]
pub fn rows(spectrum: &Spectrum, plan: Plan, gain: f32, significant: f32) -> [[f32; 4]; ROWS] {
    let mut rows = [[0.0; 4]; ROWS];
    let peak = spectrum.wavenumber(spectrum.peak_omega());
    for (c, cascade) in spectrum.cascades(plan.count, plan.size).iter().enumerate() {
        let k = if c == 0 {
            peak.min(cascade.high)
        } else {
            (cascade.low * cascade.high).sqrt()
        };
        rows[c] = [
            (1.0 / cascade.patch) as f32,
            cascade.shortest() as f32,
            k as f32,
            gain,
        ];
        rows[5][c] = slope_variance(spectrum, cascade) as f32;
    }
    rows[3] = [
        plan.count as f32,
        plan.displaced.min(plan.count) as f32,
        if plan.surf { 1.0 } else { 0.0 },
        significant * gain,
    ];
    rows[4] = [
        0.5 / plan.size as f32,
        (spectrum.peak_omega() * spectrum.time_scale) as f32,
        0.0,
        0.0,
    ];
    rows
}

/// The mean squared slope of a cascade's band, `∫ k² E(k) dk`, on its
/// lattice.
fn slope_variance(spectrum: &Spectrum, cascade: &Cascade) -> f64 {
    let dk = std::f64::consts::TAU / cascade.patch;
    let reach = (cascade.high / dk).ceil() as i64;
    let mut sum = 0.0;
    for m in -reach..=reach {
        for n in -reach..=reach {
            let k = glam::DVec2::new(n as f64, m as f64) * dk;
            let length = k.length();
            if length > 0.0 && length >= cascade.low && length < cascade.high {
                sum += length * length * spectrum.density(k) * dk * dk;
            }
        }
    }
    sum
}

/// The surface's gain over water `depth` deep for a band of wavenumber
/// `k`: linear shoaling, `K_s = 1 / √(tanh(kd) (1 + 2kd / sinh 2kd))`,
/// which first lowers and then raises a wave as the water shallows, capped
/// so no wave stands higher than 0.78 of the depth (McCowan, 1894). The
/// shader's `water_ocean_gain`; visual only, as `physics::water` is deep
/// water.
#[must_use]
pub fn gain(k: f32, depth: f32, significant: f32) -> f32 {
    let x = (k * depth.max(0.0)).clamp(0.02, 10.0);
    let shoal = (1.0 / (x.tanh() * (1.0 + 2.0 * x / (2.0 * x).sinh())).sqrt()).min(2.0);
    let cap = (0.78 * depth.max(0.0) / (significant * shoal).max(1e-3)).clamp(0.0, 1.0);
    shoal * cap
}

enum Worker {
    #[cfg(not(target_arch = "wasm32"))]
    Thread {
        jobs: mpsc::Sender<(Spectrum, u64)>,
        frames: mpsc::Receiver<Result<Frame, String>>,
        busy: bool,
    },
    #[allow(dead_code)]
    Inline(Option<Synthesis>),
}

impl Worker {
    fn new(tier: Tier) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let (jobs, inbox) = mpsc::channel::<(Spectrum, u64)>();
            let (outbox, frames) = mpsc::channel();
            let spawned = thread::Builder::new()
                .name("verse water synthesis".into())
                .spawn(move || {
                    let mut synthesis: Option<Synthesis> = None;
                    while let Ok((spectrum, tick)) = inbox.recv() {
                        if synthesis.as_ref().is_none_or(|s| *s.spectrum() != spectrum) {
                            synthesis = match Synthesis::new(&spectrum, tier) {
                                Ok(s) => Some(s),
                                Err(e) => {
                                    if outbox.send(Err(e)).is_err() {
                                        return;
                                    }
                                    continue;
                                }
                            };
                        }
                        let frame = synthesis.as_mut().map(|s| s.step(tick));
                        if let Some(frame) = frame
                            && outbox.send(Ok(frame)).is_err()
                        {
                            return;
                        }
                    }
                });
            if spawned.is_ok() {
                return Self::Thread {
                    jobs,
                    frames,
                    busy: false,
                };
            }
        }
        let _ = tier;
        Self::Inline(None)
    }
}

/// The sea's texture and the worker that fills it, and the ripple field
/// that rides in its last layer.
pub struct OceanGpu {
    plan: Plan,
    /// The ripple and foam field around the camera ([`super::ripple`]).
    pub ripples: super::ripple::Ripples,
    tier: Tier,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    worker: Worker,
    /// The spectrum and tick the texture holds.
    shown: Option<(Spectrum, u64)>,
    /// The tick asked for last, to predict the next.
    asked: Option<u64>,
    /// The spectrum's significant height, computed once per spectrum.
    significant: Option<(Spectrum, f32)>,
    /// Wait for the exact tick each frame instead of drawing the last one
    /// ready (captures and tests).
    pub exact: bool,
    /// The worker's cost of the tick shown, µs.
    pub micros: f64,
}

impl OceanGpu {
    /// An empty sea for `tier`.
    #[must_use]
    pub fn new(device: &wgpu::Device, tier: Tier) -> Self {
        let plan = plan(tier);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Verse water cascades"),
            size: wgpu::Extent3d {
                width: plan.size as u32,
                height: plan.size as u32,
                depth_or_array_layers: plan.layers() + 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        Self {
            plan,
            ripples: super::ripple::Ripples::for_tier(tier),
            tier,
            texture,
            view,
            worker: Worker::new(tier),
            shown: None,
            asked: None,
            significant: None,
            exact: false,
            micros: 0.0,
        }
    }

    /// The array view both water passes bind as `water_waves`.
    #[must_use]
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    #[must_use]
    pub fn plan(&self) -> Plan {
        self.plan
    }

    /// The texture's bytes on the GPU, the ripple layer's included.
    #[must_use]
    pub fn bytes(&self) -> u64 {
        self.plan.bytes() + (self.plan.size * self.plan.size * 8) as u64
    }

    /// Steps the ripple field to `water`'s clock with its sources around
    /// what `view` looks at, uploads it to the last layer, and returns the
    /// uniform's `ripple` row (all zero while the field is still and empty).
    /// The window sits ahead of the eye, so it covers the water the camera
    /// sees and the eye stays inside it.
    pub fn field(
        &mut self,
        queue: &wgpu::Queue,
        water: &super::Water,
        view: verse_engine::presentation::View,
    ) -> [f32; 4] {
        let eye = view.eye;
        let ahead = view
            .view_proj
            .inverse()
            .project_point3(glam::Vec3::new(0.0, 0.0, 0.5))
            - eye;
        let ahead = glam::Vec2::new(ahead.x, ahead.z).normalize_or_zero();
        let focus = glam::Vec2::new(eye.x, eye.z) + ahead * self.ripples.plan().extent() * 0.3;
        self.ripples
            .advance(water.time, focus, water.sources(), water.wet);
        let busy = self.ripples.stats().wakes > 0 || !self.ripples.is_quiet();
        if !busy {
            return [0.0; 4];
        }
        let n = self.plan.size as u32;
        let layer = self.plan.layers();
        let texels = self.ripples.texels();
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: 0,
                    z: layer,
                },
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(&texels),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(n * 8),
                rows_per_image: Some(n),
            },
            wgpu::Extent3d {
                width: n,
                height: n,
                depth_or_array_layers: 1,
            },
        );
        self.ripples.window(layer)
    }

    fn upload(&mut self, queue: &wgpu::Queue, frame: &Frame) {
        let n = self.plan.size as u32;
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(&frame.texels),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(n * 8),
                rows_per_image: Some(n),
            },
            wgpu::Extent3d {
                width: n,
                height: n,
                depth_or_array_layers: self.plan.layers(),
            },
        );
        self.shown = Some((frame.spectrum, frame.tick));
        self.micros = frame.micros;
    }

    /// Shows `spectrum` (the sea's, or none) at `time` s scaled by `gain`,
    /// and returns the uniform's `ocean` rows (all zero without a sea).
    pub fn prepare(
        &mut self,
        queue: &wgpu::Queue,
        spectrum: Option<&Spectrum>,
        time: f64,
        gain: f32,
    ) -> [[f32; 4]; ROWS] {
        let Some(spectrum) = spectrum.filter(|s| s.validate().is_ok()) else {
            self.micros = 0.0;
            return [[0.0; 4]; ROWS];
        };
        let tick = spectrum.tick_at(time);
        self.show(queue, spectrum, tick);
        if self.shown.is_none_or(|(s, _)| s != *spectrum) {
            return [[0.0; 4]; ROWS];
        }
        let significant = match self.significant {
            Some((s, h)) if s == *spectrum => h,
            _ => {
                let h = spectrum.significant_height() as f32;
                self.significant = Some((*spectrum, h));
                h
            }
        };
        rows(spectrum, self.plan, gain, significant)
    }

    fn show(&mut self, queue: &wgpu::Queue, spectrum: &Spectrum, tick: u64) {
        // The tick the frame after this one will likely ask for.
        #[cfg_attr(target_arch = "wasm32", allow(unused_variables))]
        let predicted = self
            .asked
            .map_or(tick, |last| tick + tick.saturating_sub(last).clamp(1, 8));
        self.asked = Some(tick);
        let (exact, shown, tier) = (self.exact, self.shown, self.tier);
        // Whether what the texture would hold is the wrong sea, or the
        // wrong tick when every frame must be exact.
        let stale = |held: Option<(Spectrum, u64)>| {
            held.is_none_or(|(s, t)| s != *spectrum || (exact && t != tick))
        };
        let latest = match &mut self.worker {
            #[cfg(not(target_arch = "wasm32"))]
            Worker::Thread { jobs, frames, busy } => {
                let mut latest = None;
                while let Ok(frame) = frames.try_recv() {
                    *busy = false;
                    latest = frame.ok().or(latest);
                }
                let held = |latest: &Option<Frame>| {
                    latest.as_ref().map(|f| (f.spectrum, f.tick)).or(shown)
                };
                // A new sea, or an exact frame, waits for its own tick.
                if stale(held(&latest)) && *busy {
                    latest = frames.recv().ok().and_then(Result::ok).or(latest);
                    *busy = false;
                }
                if stale(held(&latest)) && jobs.send((*spectrum, tick)).is_ok() {
                    latest = frames.recv().ok().and_then(Result::ok).or(latest);
                }
                if !*busy && jobs.send((*spectrum, predicted)).is_ok() {
                    *busy = true;
                }
                latest
            }
            Worker::Inline(synthesis) => {
                if !stale(shown) && shown.is_some_and(|(_, t)| t == tick) {
                    return;
                }
                if synthesis.as_ref().is_none_or(|s| s.spectrum() != spectrum) {
                    *synthesis = Synthesis::new(spectrum, tier).ok();
                }
                synthesis.as_mut().map(|s| s.step(tick))
            }
        };
        if let Some(frame) = latest {
            self.upload(queue, &frame);
        }
    }
}

/// The array texture's layout entry at `binding`, for both stages (the
/// vertex stage reads displacement).
#[must_use]
pub fn entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2Array,
            multisampled: false,
        },
        count: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sea() -> Spectrum {
        Spectrum {
            seed: 3,
            wind_speed: 12.0,
            fetch: 60_000.0,
            choppiness: 1.1,
            ..Spectrum::default()
        }
    }

    /// Each tier fills its layers; the foam field keeps whitecaps after
    /// the crest that made them has passed and lets them fade.
    #[test]
    fn tiers_fill_their_layers_and_foam_persists() {
        for tier in [Tier::Low, Tier::Medium, Tier::High] {
            let mut s = Synthesis::new(&sea(), tier).unwrap();
            let frame = s.step(100);
            let plan = plan(tier);
            assert_eq!(frame.texels.len() as u64 * 8, plan.bytes());
            assert_eq!(frame.plan, plan);
        }
        let mut s = Synthesis::new(&sea(), Tier::Medium).unwrap();
        // The mean foam cover over every cascade's displacement layer.
        let cover_now = |s: &mut Synthesis, tick| -> f32 {
            let frame = s.step(tick);
            let n = s.plan.size * s.plan.size;
            frame
                .texels
                .chunks(n)
                .step_by(2)
                .flatten()
                .map(|t| half::f16::from_bits(t[3]).to_f32())
                .sum::<f32>()
                / n as f32
        };
        let fresh = {
            let mut once = Synthesis::new(&sea(), Tier::Medium).unwrap();
            cover_now(&mut once, 400)
        };
        for tick in (0..400).step_by(4) {
            s.step(tick);
        }
        let kept = cover_now(&mut s, 400);
        assert!(fresh > 0.0, "{fresh}");
        assert!(kept > fresh * 1.2, "{kept} vs {fresh}");
    }

    /// The tier's cascade 0 at High's 128 texels and the gameplay band at
    /// 64, both half floats and both filtered bilinearly as the GPU does,
    /// agree with `Surface::sample`'s band within 2 cm over open water.
    #[test]
    fn the_drawn_band_matches_the_physics_band() {
        let spectrum = sea();
        let field = physics::water::spectrum::field(&spectrum, 777).unwrap();
        for tier in [Tier::Low, Tier::High] {
            let mut s = Synthesis::new(&spectrum, tier).unwrap();
            let frame = s.step(777);
            let n = s.plan.size;
            let patch = s.cascades()[0].patch;
            let texel = |i: usize, j: usize| {
                let t = frame.texels[(j % n) * n + i % n];
                [0, 1, 2].map(|c| f64::from(half::f16::from_bits(t[c]).to_f32()))
            };
            let mut worst: f64 = 0.0;
            for p in 0..200 {
                let x = (p as f64 * 37.7) % (patch * 2.0) - patch;
                let z = (p as f64 * 61.3) % (patch * 2.0) - patch;
                let (u, v) = (x / patch * n as f64, z / patch * n as f64);
                let (i, j) = (u.floor(), v.floor());
                let (tu, tv) = (u - i, v - j);
                let (i, j) = (
                    i.rem_euclid(n as f64) as usize,
                    j.rem_euclid(n as f64) as usize,
                );
                let (a, b, c, d) = (
                    texel(i, j),
                    texel(i + 1, j),
                    texel(i, j + 1),
                    texel(i + 1, j + 1),
                );
                let exact = field.sample(glam::DVec2::new(x, z));
                for k in 0..3 {
                    let drawn = (a[k] * (1.0 - tu) + b[k] * tu) * (1.0 - tv)
                        + (c[k] * (1.0 - tu) + d[k] * tu) * tv;
                    worst = worst.max((drawn - exact[k]).abs());
                }
            }
            assert!(worst < 0.02, "{tier:?}: {worst}");
        }
    }

    /// Shoaling lowers a wave a little over middling depth, raises it over
    /// shallow water, and the breaker limit takes it to nothing at the
    /// waterline.
    #[test]
    fn shoaling_and_the_breaker_limit() {
        let k = 0.2;
        assert!((gain(k, 500.0, 1.0) - 1.0).abs() < 1e-3);
        assert!(gain(k, 7.0, 0.5) < 1.0);
        assert!(gain(k, 1.2, 0.3) > 1.0);
        assert_eq!(gain(k, 0.0, 1.0), 0.0);
        assert!(gain(k, 0.5, 2.0) * 2.0 <= 0.78 * 0.5 + 1e-4);
    }
}
