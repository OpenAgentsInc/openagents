//! Portable local illumination, atmosphere, cube-shadow camera construction,
//! the sun's shadow cascades, and the color grade every zone's output pass
//! applies.
use crate::presentation::View;
use glam::{Mat4, Vec3, Vec4};

/// Local-light units and attenuation. Authored chamber values preserve the
/// retained look; they are not calibrated candela. Physical lamps use candela.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum PointProfile {
    Authored,
    Candela,
}
impl PointProfile {
    /// Reference attenuation used by both render paths, with distances in meters.
    /// The finite core avoids a singularity at the source. Returns zero for
    /// invalid queries or beyond the authored range.
    #[must_use]
    pub fn attenuation(self, distance_squared: f32, range: f32) -> f32 {
        if !distance_squared.is_finite()
            || distance_squared < 0.0
            || !range.is_finite()
            || range <= 0.0
            || distance_squared >= range * range
        {
            return 0.0;
        }
        match self {
            Self::Authored => {
                let window = (1.0 - distance_squared.sqrt() / range).max(0.0);
                window * window / (1.0 + distance_squared)
            }
            Self::Candela => {
                let d2 = distance_squared.max(1e-4);
                let window = (1.0 - (d2 / (range * range)).powi(2)).clamp(0.0, 1.0);
                window * window / (d2 + 0.01)
            }
        }
    }
}
/// Applied lighting settings for a captured frame. Exposure is a linear
/// multiplier; the output grade's offset is in stops. Shadow views count active
/// maps, including cached maps, rather than redraws or elapsed GPU time.
#[derive(Clone, Debug, serde::Serialize)]
pub struct FrameLighting {
    pub profile: PointProfile,
    pub exposure: f32,
    pub grade_stops: f32,
    pub selected_points: Vec<usize>,
    pub shadowed_points: Vec<usize>,
    pub shadow_views: usize,
    pub shadow_size: u32,
    pub ambient: &'static str,
}
impl Default for FrameLighting {
    fn default() -> Self {
        Self {
            profile: PointProfile::Authored,
            exposure: 1.0,
            grade_stops: 0.0,
            selected_points: vec![],
            shadowed_points: vec![],
            shadow_views: 0,
            shadow_size: 0,
            ambient: "none",
        }
    }
}

/// Converts photographic EV100 to the linear multiplier applied exactly once
/// before the shared output grade. A grade exposure offset is separate, in stops.
#[must_use]
pub fn exposure(ev100: f32) -> f32 {
    1.0 / (1.2 * 2f32.powf(ev100))
}

pub const MAX_LIGHTS: usize = 32;

/// A point source in meters. Up to four sources receive cube shadow maps;
/// [`select_shadowed`] picks them.
#[derive(Clone, Copy, Debug)]
pub struct Light {
    pub position: Vec3,
    pub color: Vec3,
    /// Authored intensity for the chamber profile; candela for physical lamps.
    pub intensity: f32,
    pub range: f32,
}
/// Linear scene lighting, independent of the UI layer.
#[derive(Clone, Debug)]
pub struct Lighting {
    pub ambient: Vec3,
    pub exposure: f32,
    pub fog: Vec3,
    pub density: f32,
    pub time: f32,
    pub lights: Vec<Light>,
    pub shadowed: usize,
    /// Fog that thins with height, toward `fog`. Without it, fog is uniform
    /// at `density` per meter of distance.
    pub height_fog: Option<HeightFog>,
}
impl Default for Lighting {
    fn default() -> Self {
        Self {
            ambient: Vec3::splat(0.025),
            exposure: 1.0,
            fog: Vec3::new(0.009, 0.012, 0.016),
            density: 0.008,
            time: 0.0,
            lights: vec![],
            shadowed: 4,
            height_fog: None,
        }
    }
}

impl Lighting {
    /// The fog the scene draws: its height fog, or uniform distance fog at
    /// `density`.
    #[must_use]
    pub fn fog_shape(&self) -> HeightFog {
        self.height_fog
            .unwrap_or_else(|| HeightFog::distance(self.density))
    }
}

/// Exponential height fog: density falls off exponentially with height, so
/// the optical depth along a straight view ray has a closed form (Wenzel,
/// "Real-Time Atmospheric Effects in Games Revisited", GDC 2007; Quílez,
/// "Better Fog"). Fog begins `start` meters from the eye, never exceeds
/// `max_opacity`, and scatters sunlight toward the eye in a lobe around the
/// Sun's direction.
///
/// The shaders evaluate the same functions; [`HeightFog::uniform`] packs the
/// parameters they read.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeightFog {
    /// Extinction per meter at height `base`.
    pub density: f32,
    /// The height where the density is `density`, meters.
    pub base: f32,
    /// How fast density falls with height, per meter; zero is uniform fog.
    pub falloff: f32,
    /// Distance from the eye where fog begins, meters.
    pub start: f32,
    /// The most opacity fog reaches, 0 to 1.
    pub max_opacity: f32,
    /// In-scattered sunlight looking straight at the Sun, as a multiple of
    /// the Sun's tint.
    pub sun_strength: f32,
    /// The lobe's sharpness: the power of the cosine to the Sun.
    pub sun_exponent: f32,
}

/// The largest magnitude an exponent in the fog's closed form may take, so
/// single-precision exponentials stay finite.
const FOG_EXPONENT_LIMIT: f32 = 80.0;

impl HeightFog {
    /// Uniform distance fog: `density` per meter everywhere, from the eye,
    /// up to full opacity, without a sun lobe.
    #[must_use]
    pub const fn distance(density: f32) -> Self {
        Self {
            density,
            base: 0.0,
            falloff: 0.0,
            start: 0.0,
            max_opacity: 1.0,
            sun_strength: 0.0,
            sun_exponent: 1.0,
        }
    }

    /// Refuses non-finite or out-of-range parameters.
    pub fn validate(&self) -> Result<(), String> {
        let values = [
            self.density,
            self.base,
            self.falloff,
            self.start,
            self.max_opacity,
            self.sun_strength,
            self.sun_exponent,
        ];
        if values.iter().any(|v| !v.is_finite())
            || !(0.0..=1.0).contains(&self.density)
            || self.base.abs() > 10_000.0
            || !(0.0..=10.0).contains(&self.falloff)
            || !(0.0..=10_000.0).contains(&self.start)
            || !(0.0..=1.0).contains(&self.max_opacity)
            || !(0.0..=16.0).contains(&self.sun_strength)
            || !(1.0..=256.0).contains(&self.sun_exponent)
        {
            return Err("Invalid height fog".into());
        }
        Ok(())
    }

    /// The fog's optical depth between `eye` and `point`: the integral of
    /// `density × exp(−falloff × (y − base))` along the ray from where fog
    /// starts.
    #[must_use]
    pub fn optical_depth(&self, eye: Vec3, point: Vec3) -> f32 {
        let ray = point - eye;
        let length = ray.length();
        let travel = length - self.start;
        if travel <= 0.0 || self.density <= 0.0 {
            return 0.0;
        }
        // The ray from the start distance to the point.
        let first = eye.y + ray.y * (self.start / length);
        let rise = ray.y * (travel / length);
        let at_start = self.density
            * (-self.falloff * (first - self.base))
                .clamp(-FOG_EXPONENT_LIMIT, FOG_EXPONENT_LIMIT)
                .exp();
        // The mean density along the ray relative to its start:
        // (1 − e^(−k)) / k, which tends to 1 − k / 2 as k vanishes.
        let k = (self.falloff * rise).clamp(-FOG_EXPONENT_LIMIT, FOG_EXPONENT_LIMIT);
        let shape = if k.abs() > 1e-4 {
            (1.0 - (-k).exp()) / k
        } else {
            1.0 - 0.5 * k
        };
        at_start * travel * shape
    }

    /// How much of the view toward `point` the fog covers, 0 to
    /// `max_opacity`.
    #[must_use]
    pub fn opacity(&self, eye: Vec3, point: Vec3) -> f32 {
        (1.0 - (-self.optical_depth(eye, point)).exp()).min(self.max_opacity)
    }

    /// The fog's opacity on a stage whose world ends at `end` meters: full
    /// fog over the last 30% of that horizontal distance, so the zone's fog
    /// range still hides the world's edge.
    #[must_use]
    pub fn stage_opacity(&self, eye: Vec3, point: Vec3, end: f32) -> f32 {
        let distance = glam::Vec2::new(point.x - eye.x, point.z - eye.z).length();
        let x = ((distance - 0.7 * end) / (0.3 * end).max(1e-3)).clamp(0.0, 1.0);
        self.opacity(eye, point).max(x * x * (3.0 - 2.0 * x))
    }

    /// In-scattered sunlight along unit view direction `view` toward the
    /// unit Sun direction `sun`, as a multiple of the Sun's tint.
    #[must_use]
    pub fn sun_lobe(&self, view: Vec3, sun: Vec3) -> f32 {
        self.sun_strength * view.dot(sun).max(0.0).powf(self.sun_exponent)
    }

    /// The parameters as the shaders read them: density, base, falloff, and
    /// start; then the opacity cap, the lobe's strength and exponent, and 1
    /// to mark the fog present.
    #[must_use]
    pub fn uniform(&self) -> [[f32; 4]; 2] {
        [
            [self.density, self.base, self.falloff, self.start],
            [self.max_opacity, self.sun_strength, self.sun_exponent, 1.0],
        ]
    }
}

impl Lighting {
    pub fn shadow_count(&self) -> usize {
        self.lights.len().min(self.shadowed).min(4)
    }
    pub fn validate(&self, view: View) -> Result<(), String> {
        view.validate()?;
        if self.lights.len() > MAX_LIGHTS
            || !self.ambient.is_finite()
            || self.ambient.min_element() < 0.
            || !self.fog.is_finite()
            || self.fog.min_element() < 0.
            || !self.exposure.is_finite()
            || self.exposure <= 0.
            || !self.density.is_finite()
            || self.density < 0.
            || !self.time.is_finite()
        {
            return Err("Invalid scene illumination".into());
        }
        if let Some(fog) = &self.height_fog {
            fog.validate()?;
        }
        for (index, light) in self.lights.iter().enumerate() {
            light.sample(self.time, index)?;
            if index < self.shadow_count() {
                light.shadow_views()?;
            }
        }
        Ok(())
    }
}
impl Light {
    /// Preserve the authored two-frequency local-light flicker.
    pub fn sample(&self, time: f32, index: usize) -> Result<f32, String> {
        if !self.position.is_finite()
            || !self.color.is_finite()
            || self.color.min_element() < 0.
            || !self.intensity.is_finite()
            || self.intensity < 0.
            || !self.range.is_finite()
            || self.range <= 0.2
            || !time.is_finite()
        {
            return Err("Invalid point source".into());
        }
        let flicker = 1.0
            + 0.04 * (time * 13.0 + index as f32).sin()
            + 0.025 * (time * 19.0 + index as f32 * 2.0).sin();
        let intensity = self.intensity * flicker;
        if !intensity.is_finite() {
            return Err("Point source sampling overflow".into());
        }
        Ok(intensity)
    }
    /// Cube faces use +X, -X, +Y, -Y, +Z, -Z ordering and right-handed depth.
    pub fn shadow_views(&self) -> Result<[Mat4; 6], String> {
        self.sample(0., 0)?;
        let directions = [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z];
        let ups = [-Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z, -Vec3::Y, -Vec3::Y];
        let projection = Mat4::perspective_rh(std::f32::consts::FRAC_PI_2, 1.0, 0.15, self.range);
        let views = std::array::from_fn(|face| {
            projection * Mat4::look_to_rh(self.position, directions[face], ups[face])
        });
        if views.iter().any(|matrix| !matrix.is_finite()) {
            return Err("Point source shadow projection overflow".into());
        }
        Ok(views)
    }
}

/// How strongly a light that already holds a shadow map is favored when the
/// maps are handed out again. Two lights of nearly equal contribution then do
/// not trade maps every frame and refill the static shadow cache.
pub const SHADOW_HYSTERESIS: f32 = 1.25;

impl Light {
    /// An estimate of how much this light adds to the view: its intensity
    /// weighted by luminance, times the share of the view its sphere of
    /// influence covers. A light whose sphere lies outside the view frustum
    /// lights nothing visible and contributes zero.
    #[must_use]
    pub fn contribution(&self, view: View) -> f32 {
        let power = self.intensity * self.color.dot(LUMA);
        if !(power > 0.0) || !sphere_in_frustum(view.view_proj, self.position, self.range) {
            return 0.0;
        }
        let distance = (self.position - view.eye).length();
        let coverage = if distance <= self.range {
            1.0
        } else {
            (self.range / distance).powi(2)
        };
        power * coverage
    }
}

/// Whether a sphere reaches into the frustum of `view_proj`, whose clip depth
/// runs from 0 to 1. The planes come from the matrix's rows (Gribb and
/// Hartmann, "Fast Extraction of Viewing Frustum Planes", 2001).
fn sphere_in_frustum(view_proj: Mat4, center: Vec3, radius: f32) -> bool {
    let [x, y, z, w] = [0, 1, 2, 3].map(|row| view_proj.row(row));
    let planes: [Vec4; 6] = [w + x, w - x, w + y, w - y, z, w - z];
    let point = center.extend(1.0);
    planes.iter().all(|plane| {
        let length = plane.truncate().length();
        length <= f32::EPSILON || plane.dot(point) >= -radius * length
    })
}

/// The lights that receive the cube shadow maps, in map order: the `slots`
/// lights with the largest [`Light::contribution`] to `view`, ties going to
/// the earlier light.
///
/// `previous` is the last frame's result. A light in it is favored by
/// [`SHADOW_HYSTERESIS`] and keeps its map, so a light that stays chosen
/// keeps its cached faces; a newly chosen light takes a map that was freed.
#[must_use]
pub fn select_shadowed(
    lights: &[Light],
    view: View,
    slots: usize,
    previous: &[usize],
) -> Vec<usize> {
    let slots = slots.min(lights.len());
    let mut ranked: Vec<(f32, usize)> = lights
        .iter()
        .enumerate()
        .map(|(index, light)| {
            let mut score = light.contribution(view);
            if previous.contains(&index) {
                score *= SHADOW_HYSTERESIS;
            }
            (score, index)
        })
        .collect();
    ranked.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    let chosen: Vec<usize> = ranked.iter().take(slots).map(|&(_, index)| index).collect();
    let mut order: Vec<Option<usize>> = vec![None; slots];
    for &index in &chosen {
        if let Some(slot) = previous.iter().position(|&p| p == index)
            && slot < slots
        {
            order[slot] = Some(index);
        }
    }
    let placed: Vec<usize> = order.iter().flatten().copied().collect();
    let mut rest = chosen
        .iter()
        .copied()
        .filter(|index| !placed.contains(index));
    for slot in &mut order {
        if slot.is_none() {
            *slot = rest.next();
        }
    }
    order.into_iter().flatten().collect()
}

/// The strongest visible point sources, independent of source-list order.
/// Ties retain source order. This uses the same contribution estimate as local
/// shadow priority; selection does not mutate simulation or flicker phases.
#[must_use]
pub fn select_lights(lights: &[Light], view: View, budget: usize) -> Vec<usize> {
    let mut ranked: Vec<_> = lights
        .iter()
        .enumerate()
        .map(|(index, light)| (light.contribution(view), index))
        .filter(|(score, _)| score.is_finite() && *score > 0.0)
        .collect();
    ranked.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    ranked
        .into_iter()
        .take(budget.min(MAX_LIGHTS))
        .map(|(_, index)| index)
        .collect()
}

/// The most sun shadow cascades any quality tier draws.
pub const MAX_CASCADES: usize = 4;

/// A view depth that no scene reaches, m: the end of a shadow that does not
/// follow the camera.
pub const UNBOUNDED: f32 = 1.0e30;

/// What shadow cascades need to know about a perspective camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frustum {
    pub eye: Vec3,
    /// Unit view axis. A point's view depth is its distance along it.
    pub forward: Vec3,
    /// View depths of the near and far planes, m.
    pub near: f32,
    pub far: f32,
    /// The largest ratio of a frustum corner's distance from the view axis
    /// to its view depth: the tangent of the half-diagonal field of view.
    pub spread: f32,
}

impl Frustum {
    /// The frustum of a perspective `view_proj` whose clip depth runs from 0
    /// at the near plane to 1 at the far plane, seen from `eye`. Returns
    /// `None` for a matrix that is not such a projection.
    #[must_use]
    pub fn from_view_proj(view_proj: Mat4, eye: Vec3) -> Option<Self> {
        // In double precision: single-precision noise from inverting a moving
        // camera's matrix is about the size of the lens grid below, and would
        // sometimes cross a grid line and resize a cascade for a frame.
        let inverse = view_proj.as_dmat4().inverse();
        if !inverse.is_finite() || !eye.is_finite() {
            return None;
        }
        let eye64 = eye.as_dvec3();
        let corners = |z: f64| {
            [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)]
                .map(|(x, y)| inverse.project_point3(glam::DVec3::new(x, y, z)))
        };
        let (near, far) = (corners(0.0), corners(1.0));
        let center = |c: &[glam::DVec3; 4]| (c[0] + c[1] + c[2] + c[3]) * 0.25;
        // The view axis and the lens come from the near plane, whose corners
        // invert well; the far plane, 2 km out from a near plane a tenth of a
        // meter away, only sets the far depth.
        let forward = (center(&near) - eye64).try_normalize()?;
        let depth = |p: &glam::DVec3| (*p - eye64).dot(forward);
        let near_depth = near.iter().map(depth).fold(f64::INFINITY, f64::min);
        let far_depth = far.iter().map(depth).fold(f64::INFINITY, f64::min);
        let mut spread = 0.0f64;
        for corner in near {
            let offset = corner - eye64;
            let along = offset.dot(forward);
            if !(along > 0.0) {
                return None;
            }
            spread = spread.max((offset - forward * along).length() / along);
        }
        let forward = forward.as_vec3();
        let (near_depth, far_depth, spread) = (near_depth as f32, far_depth as f32, spread as f32);
        // The lens is rounded to a fine grid. Float noise from inverting a
        // moving camera's matrix then cannot change a cascade's size between
        // frames, which would make every shadow edge crawl.
        let lens = |value: f32| (value * 4096.0).round() / 4096.0;
        let frustum = Self {
            eye,
            forward,
            near: lens(near_depth),
            far: lens(far_depth),
            spread: lens(spread),
        };
        let valid = frustum.near.is_finite()
            && frustum.far.is_finite()
            && frustum.spread.is_finite()
            && frustum.near > 0.0
            && frustum.far > frustum.near
            && frustum.spread > 0.0;
        valid.then_some(frustum)
    }

    /// The view depth of a world point, m.
    #[must_use]
    pub fn depth(&self, point: Vec3) -> f32 {
        (point - self.eye).dot(self.forward)
    }
}

/// How the sun's shadow follows the camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CascadeSettings {
    /// Cascades across the view, from 1 to [`MAX_CASCADES`]; the quality
    /// tier sets it.
    pub count: usize,
    /// View depth where the shadow ends, m.
    pub distance: f32,
    /// Where the splits sit between uniform (0) and logarithmic (1) spacing
    /// (Zhang et al., "Parallel-Split Shadow Maps", 2006).
    pub split_lambda: f32,
    /// Texels along each side of a cascade's map.
    pub resolution: u32,
    /// The band at the end of each cascade where shading blends into the
    /// next, as a fraction of the cascade's slice.
    pub blend: f32,
    /// The stretch at the end of the shadow distance where the shadow fades
    /// out, as a fraction of the distance.
    pub fade: f32,
    /// How far beyond a cascade's slice, toward the light, its map still
    /// takes casters, m.
    pub caster_reach: f32,
    /// The cell, in texels, to which every cascade after the first snaps
    /// its center, so its map changes only when the camera crosses a cell
    /// and its static casters can be cached. Zero snaps to single texels
    /// and caches nothing.
    pub cache_cell: u32,
}

impl CascadeSettings {
    /// `count` cascades out to `distance` meters, with 2048² maps.
    #[must_use]
    pub fn new(count: u32, distance: f32) -> Self {
        Self {
            count: count as usize,
            distance,
            split_lambda: 0.8,
            resolution: 2048,
            blend: 0.1,
            fade: 0.1,
            caster_reach: 60.0,
            cache_cell: 64,
        }
    }

    /// Refuses values that cannot fit a cascade.
    pub fn validate(&self) -> Result<(), String> {
        let finite = self.distance.is_finite()
            && self.split_lambda.is_finite()
            && self.blend.is_finite()
            && self.fade.is_finite()
            && self.caster_reach.is_finite();
        if !finite
            || !(1..=MAX_CASCADES).contains(&self.count)
            || self.distance <= 0.0
            || !(0.0..=1.0).contains(&self.split_lambda)
            || self.resolution < 64
            || u64::from(self.cache_cell) * 4 >= u64::from(self.resolution)
            || !(0.0..=0.5).contains(&self.blend)
            || !(0.0..=1.0).contains(&self.fade)
            || self.caster_reach < 0.0
        {
            return Err("Invalid shadow cascade settings".into());
        }
        Ok(())
    }
}

/// One shadow map of the sun.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cascade {
    /// World to the map's clip space; depth 0 is nearest the light.
    pub matrix: Mat4,
    /// Edge of one map texel, m.
    pub texel: f32,
    /// Distance along the light that the map's depth spans, m.
    pub depth_range: f32,
    /// View depth where shading hands over to the next cascade, m.
    pub end: f32,
    /// Whether the map holds static casters only, so it can be kept until
    /// its matrix or the static scene changes.
    pub cached: bool,
}

/// The sun's shadow maps for one frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Cascades {
    /// Near to far, at least one.
    pub cascades: Vec<Cascade>,
    /// The camera's view axis, which view depth is measured along; zero for
    /// a shadow that does not follow the camera.
    pub forward: Vec3,
    /// The blend band, as in [`CascadeSettings::blend`].
    pub blend: f32,
    /// View depth where the shadow starts to fade out, m.
    pub fade_start: f32,
    /// View depth where the shadow ends, m.
    pub distance: f32,
}

impl Cascades {
    /// The cascade a point at view depth `depth` reads, and how far it has
    /// blended into the next, from 0 to 1. `sun_shadow` in the physical
    /// path's `photo.wgsl` makes the same choice on the GPU.
    #[must_use]
    pub fn select(&self, depth: f32) -> (usize, f32) {
        let last = self.cascades.len().saturating_sub(1);
        let index = self
            .cascades
            .iter()
            .position(|cascade| depth <= cascade.end)
            .unwrap_or(last);
        if index >= last {
            return (index, 0.0);
        }
        let previous = if index == 0 {
            0.0
        } else {
            self.cascades[index - 1].end
        };
        let end = self.cascades[index].end;
        let band = (end - previous) * self.blend;
        let weight = ((depth - (end - band)) / band.max(1e-4)).clamp(0.0, 1.0);
        (index, weight)
    }

    /// How much of the shadow remains at view depth `depth`: 1 before the
    /// fade, falling to 0 at the shadow distance.
    #[must_use]
    pub fn presence(&self, depth: f32) -> f32 {
        1.0 - ((depth - self.fade_start) / (self.distance - self.fade_start).max(1e-4))
            .clamp(0.0, 1.0)
    }
}

/// View depths that split `near`..`far` into `count` slices: `count + 1`
/// values from `near` to `far`, spaced `lambda` of the way from uniform to
/// logarithmic. The logarithmic part starts at 1 m, so a near plane a few
/// centimeters out does not crowd the first cascade.
#[must_use]
pub fn cascade_splits(near: f32, far: f32, count: usize, lambda: f32) -> Vec<f32> {
    let count = count.clamp(1, MAX_CASCADES);
    let base = near.max(1.0).min(far);
    (0..=count)
        .map(|i| {
            if i == 0 {
                return near;
            }
            if i == count {
                return far;
            }
            let t = i as f32 / count as f32;
            let logarithmic = base * (far / base).powf(t);
            let uniform = near + (far - near) * t;
            lambda * logarithmic + (1.0 - lambda) * uniform
        })
        .collect()
}

/// The light's view: from the origin, looking along `-toward`.
fn light_view(toward: Vec3) -> Mat4 {
    let up = if toward.y.abs() > 0.9 {
        Vec3::X
    } else {
        Vec3::Y
    };
    Mat4::look_to_rh(Vec3::ZERO, -toward, up)
}

/// An orthographic map `half` meters each side of `center` in the light's
/// view `look`, reaching `toward_light` meters toward the light and `away`
/// meters away from it. The center snaps to whole multiples of `cell`, so a
/// world point keeps its place within its texel as the center moves.
fn snapped_map(
    look: Mat4,
    center: Vec3,
    half: f32,
    cell: f32,
    toward_light: f32,
    away: f32,
) -> Mat4 {
    let c = look.transform_point3(center);
    let s = (c / cell).round() * cell;
    Mat4::orthographic_rh(
        s.x - half,
        s.x + half,
        s.y - half,
        s.y + half,
        -s.z - toward_light,
        -s.z + away,
    ) * look
}

/// The smallest sphere centered on the view axis that holds the frustum
/// slice from view depth `a` to `b`, whose corners stand `spread` times their
/// depth off the axis: its center's view depth and its radius. It depends on
/// the slice and the lens only, never on where the camera stands or looks, so
/// a cascade's texel size does not change as the camera turns.
fn slice_sphere(a: f32, b: f32, spread: f32) -> (f32, f32) {
    let k2 = spread * spread;
    let z = (a + b) * (1.0 + k2) * 0.5;
    if z >= b {
        (b, b * spread)
    } else {
        (z, ((z - a) * (z - a) + a * a * k2).sqrt())
    }
}

/// One map along `toward` (unit, toward the light) over the fixed cube of
/// half extent `half` about `center`, for a shadow that does not follow the
/// camera. It reaches a cube's width beyond the cube on both sides.
#[must_use]
pub fn fit_box(toward: Vec3, center: Vec3, half: f32, resolution: u32) -> Cascades {
    let half = half.max(1.0);
    let reach = half * 2.0;
    let texel = 2.0 * half / resolution.max(1) as f32;
    let matrix = snapped_map(light_view(toward), center, half, texel, reach, reach);
    Cascades {
        cascades: vec![Cascade {
            matrix,
            texel,
            depth_range: reach * 2.0,
            end: UNBOUNDED,
            cached: false,
        }],
        forward: Vec3::ZERO,
        blend: 0.0,
        fade_start: UNBOUNDED,
        distance: UNBOUNDED,
    }
}

/// The sun's cascades for one frame: the view from the near plane to the
/// shadow distance, split by [`cascade_splits`], each slice in a map fitted
/// to its bounding sphere and snapped to whole texels (Valient, "Stable
/// Cascaded Shadow Maps", ShaderX6, 2008).
///
/// Each slice after the first starts inside the previous cascade's blend
/// band, so both maps cover the band. Each map is padded by its snapping
/// cell, so the slice stays inside it wherever the snapped center lands.
/// With [`CascadeSettings::cache_cell`] set, every cascade after the first
/// snaps to that coarser cell and is marked cached.
///
/// # Errors
///
/// Refuses invalid settings, a zero light direction, and a shadow distance
/// that ends before the near plane.
pub fn fit_cascades(
    frustum: &Frustum,
    toward: Vec3,
    settings: &CascadeSettings,
) -> Result<Cascades, String> {
    settings.validate()?;
    let toward = toward
        .try_normalize()
        .ok_or("The sun has no direction for its shadow")?;
    let distance = settings.distance.min(frustum.far);
    if !(distance > frustum.near) {
        return Err("The shadow ends before the camera's near plane".into());
    }
    let count = settings.count;
    let splits = cascade_splits(frustum.near, distance, count, settings.split_lambda);
    let look = light_view(toward);
    let resolution = settings.resolution as f32;
    let mut cascades = Vec::with_capacity(count);
    for i in 0..count {
        let end = splits[i + 1];
        let start = if i == 0 {
            frustum.near
        } else {
            let before = if i >= 2 { splits[i - 1] } else { 0.0 };
            (splits[i] - settings.blend * (splits[i] - before)).max(frustum.near)
        };
        let (center_depth, radius) = slice_sphere(start, end, frustum.spread);
        // Rounding up to 1/64 m keeps float noise in the camera matrix from
        // changing the texel size between frames.
        let radius = (radius * 64.0).ceil() / 64.0;
        let center = frustum.eye + frustum.forward * center_depth;
        let cached = settings.cache_cell > 0 && i > 0;
        let cell_texels = (if cached { settings.cache_cell } else { 1 }) as f32;
        let half = radius / (1.0 - 2.0 * cell_texels / resolution);
        let texel = 2.0 * half / resolution;
        let reach = half + settings.caster_reach;
        let matrix = snapped_map(look, center, half, texel * cell_texels, reach, half);
        if !matrix.is_finite() {
            return Err("Shadow cascade projection overflow".into());
        }
        cascades.push(Cascade {
            matrix,
            texel,
            depth_range: reach + half,
            end,
            cached,
        });
    }
    Ok(Cascades {
        cascades,
        forward: frustum.forward,
        blend: settings.blend,
        fade_start: distance * (1.0 - settings.fade),
        distance,
    })
}

/// Edge length of the color-grading lookup table, in texels.
pub const GRADE_LUT_SIZE: usize = 32;
/// The table's shaper is linear below this scene value and logarithmic above.
pub const GRADE_LUT_FLOOR: f32 = 1.0 / 1024.0;
/// The scene value the table's last texel holds; brighter values clamp to it.
pub const GRADE_LUT_TOP: f32 = 256.0;

/// Rec. 709 luminance weights for linear scene color.
const LUMA: Vec3 = Vec3::new(0.2126, 0.7152, 0.0722);
/// Mid-grey, the pivot of a contrast change.
const MID_GREY: f32 = 0.18;

/// The shaper's span: the number of stops between the floor and the top.
#[must_use]
pub fn grade_lut_range() -> f32 {
    (GRADE_LUT_TOP / GRADE_LUT_FLOOR + 1.0).log2()
}

/// A linear scene value as a table coordinate in `[0, 1]`.
///
/// The shaper is `log2(x / floor + 1)`, normalized so the top maps to 1. Zero
/// maps to zero, so black stays black, and the table spends its texels
/// evenly across stops above the floor.
#[must_use]
pub fn grade_lut_shape(x: f32) -> f32 {
    ((x.max(0.0) / GRADE_LUT_FLOOR + 1.0).log2() / grade_lut_range()).clamp(0.0, 1.0)
}

/// The linear scene value at table coordinate `u`, the inverse of
/// [`grade_lut_shape`].
#[must_use]
pub fn grade_lut_unshape(u: f32) -> f32 {
    GRADE_LUT_FLOOR * ((u.clamp(0.0, 1.0) * grade_lut_range()).exp2() - 1.0)
}

/// The tone curve that maps graded scene color to display color.
///
/// Both curves scale a color by its peak channel, so they keep its hue. A
/// table the size of [`GRADE_LUT_SIZE`] interpolates such a curve poorly at
/// the shoulder, so the output pass evaluates the curve analytically after
/// the table; a per-channel curve could be baked into the table instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Curve {
    /// Khronos PBR Neutral, its shoulder generalized to approach `ceiling`.
    Neutral,
    /// A hue-preserving shoulder alone, for neon stages: a bright amber core
    /// compresses along its own hue instead of washing toward white.
    HueShoulder,
}

impl Curve {
    /// The display color for a graded scene color, at most `ceiling`: 1 on a
    /// standard display, the headroom over reference white on an
    /// extended-range one. The output pass's WGSL is the GPU form of this.
    #[must_use]
    pub fn apply(self, color: Vec3, ceiling: f32) -> Vec3 {
        let color = color.max(Vec3::ZERO);
        let start = 0.8 - 0.04;
        match self {
            Self::Neutral => {
                let x = color.min_element();
                let offset = if x < 0.08 { x - 6.25 * x * x } else { 0.04 };
                let c = color - Vec3::splat(offset);
                let peak = c.max_element();
                if peak < start {
                    return c;
                }
                let d = ceiling - start;
                let new_peak = ceiling - d * d / (peak + d - start);
                let c = c * (new_peak / peak);
                let g = 1.0 - 1.0 / (0.15 * (peak - new_peak) + 1.0);
                c.lerp(Vec3::splat(new_peak), g)
            }
            Self::HueShoulder => {
                let peak = color.max_element();
                if peak < start {
                    return color;
                }
                let d = ceiling - start;
                color * ((ceiling - d * d / (peak + d - start)) / peak)
            }
        }
    }
}

/// A zone's color grade: what the output pass does to exposed scene color
/// before it reaches the display.
///
/// The scene-referred part (exposure offset, white balance, saturation,
/// contrast, and color gain) is baked into a 3D lookup table, so a new look
/// is data rather than shader code. The curve and its ceiling are evaluated
/// analytically after the table.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grade {
    /// Exposure offset, stops, applied first.
    pub exposure: f32,
    /// Per-channel white-balance gains on linear scene color.
    pub balance: Vec3,
    /// Saturation about Rec. 709 luminance: 1 keeps color, 0 is grey.
    pub saturation: f32,
    /// Contrast about mid-grey, per channel in log space: 1 keeps it.
    pub contrast: f32,
    /// A per-channel color filter after contrast.
    pub gain: Vec3,
    /// Split toning: per-channel gains on the shadows and on the
    /// highlights, blended by luminance in log space about mid-grey, so a
    /// look can push cool shadows against warm highlights. Ones leave the
    /// color alone.
    pub shadows: Vec3,
    pub highlights: Vec3,
    pub curve: Curve,
    /// The display's highest value: 1 in standard range, the headroom over
    /// reference white on an extended-range display.
    pub ceiling: f32,
}

impl Grade {
    /// No grade: the Neutral curve on exposed scene color.
    pub const NEUTRAL: Self = Self {
        exposure: 0.0,
        balance: Vec3::ONE,
        saturation: 1.0,
        contrast: 1.0,
        gain: Vec3::ONE,
        shadows: Vec3::ONE,
        highlights: Vec3::ONE,
        curve: Curve::Neutral,
        ceiling: 1.0,
    };

    /// The neon stages (the plaza and Everglade): the hue-preserving shoulder,
    /// standard range even on an extended-range display.
    pub const STAGE: Self = Self {
        curve: Curve::HueShoulder,
        ..Self::NEUTRAL
    };

    /// The summoning chamber. Its fragments used to tone-map themselves with
    /// an ACES curve fit, which lifts mid-grey to 0.267; Neutral maps 0.18 to
    /// 0.14. The 0.77-stop offset keeps the chamber's mid-tones where the
    /// retired curve put them, so its authored light levels still read.
    pub const CHAMBER: Self = Self {
        exposure: 0.77,
        ..Self::NEUTRAL
    };

    /// Refuses non-finite or out-of-range values before a table is baked.
    pub fn validate(&self) -> Result<(), String> {
        let finite = self.exposure.is_finite()
            && self.balance.is_finite()
            && self.saturation.is_finite()
            && self.contrast.is_finite()
            && self.gain.is_finite()
            && self.shadows.is_finite()
            && self.highlights.is_finite()
            && self.ceiling.is_finite();
        if !finite
            || self.exposure.abs() > 16.0
            || self.balance.min_element() <= 0.0
            || self.gain.min_element() < 0.0
            || self.shadows.min_element() <= 0.0
            || self.highlights.min_element() <= 0.0
            || self.shadows.max_element() > 4.0
            || self.highlights.max_element() > 4.0
            || self.saturation < 0.0
            || self.contrast <= 0.0
            || self.ceiling < 1.0
        {
            return Err("Invalid color grade".into());
        }
        Ok(())
    }

    /// Whether `other` bakes the same table: the curve and ceiling are
    /// uniforms, not table contents.
    #[must_use]
    pub fn same_table(&self, other: &Self) -> bool {
        self.exposure == other.exposure
            && self.balance == other.balance
            && self.saturation == other.saturation
            && self.contrast == other.contrast
            && self.gain == other.gain
            && self.shadows == other.shadows
            && self.highlights == other.highlights
    }

    /// The scene-referred grade of one linear color: the function the table
    /// holds.
    #[must_use]
    pub fn scene(&self, color: Vec3) -> Vec3 {
        // Identity terms are skipped, so an identity grade is exact.
        let mut c = color.max(Vec3::ZERO) * self.balance;
        if self.exposure != 0.0 {
            c *= self.exposure.exp2();
        }
        if self.saturation != 1.0 {
            let luma = Vec3::splat(c.dot(LUMA));
            c = (luma + (c - luma) * self.saturation).max(Vec3::ZERO);
        }
        let c = if self.contrast == 1.0 {
            c
        } else {
            let pivot = |v: f32| {
                if v > 0.0 {
                    MID_GREY * (v / MID_GREY).powf(self.contrast)
                } else {
                    0.0
                }
            };
            Vec3::new(pivot(c.x), pivot(c.y), pivot(c.z))
        };
        let c = c * self.gain;
        if self.shadows == Vec3::ONE && self.highlights == Vec3::ONE {
            return c;
        }
        // From all shadow tint four stops under mid-grey to all highlight
        // tint four stops over it.
        let luma = c.dot(LUMA).max(1e-6);
        let t = ((luma / MID_GREY).log2() / 8.0 + 0.5).clamp(0.0, 1.0);
        let t = t * t * (3.0 - 2.0 * t);
        c * self.shadows.lerp(self.highlights, t)
    }

    /// The full transform of one exposed scene color to display color,
    /// without the table's interpolation.
    #[must_use]
    pub fn display(&self, color: Vec3) -> Vec3 {
        self.curve.apply(self.scene(color), self.ceiling)
    }

    /// Bakes the scene-referred grade into a [`GRADE_LUT_SIZE`]³ table of
    /// RGBA texels, red fastest, then green, then blue.
    ///
    /// Each texel holds the grade's change to its shaped input, not the graded
    /// color, so an identity grade is exact under trilinear filtering and a
    /// mild grade carries only a small interpolation error.
    #[must_use]
    pub fn bake(&self) -> Vec<[f32; 4]> {
        let n = GRADE_LUT_SIZE;
        let axis: Vec<f32> = (0..n)
            .map(|i| grade_lut_unshape(i as f32 / (n - 1) as f32))
            .collect();
        let mut texels = Vec::with_capacity(n * n * n);
        for &b in &axis {
            for &g in &axis {
                for &r in &axis {
                    let input = Vec3::new(r, g, b);
                    let change = self.scene(input) - input;
                    texels.push([change.x, change.y, change.z, 0.0]);
                }
            }
        }
        texels
    }
}

/// The graded scene color as the output pass reads it from a baked table:
/// trilinear filtering between texel centers, as the GPU samples it.
///
/// # Panics
///
/// Panics when `table` is not [`GRADE_LUT_SIZE`]³ texels.
#[must_use]
pub fn sample_grade_lut(table: &[[f32; 4]], color: Vec3) -> Vec3 {
    let n = GRADE_LUT_SIZE;
    assert_eq!(table.len(), n * n * n, "grade table size");
    let color = color.max(Vec3::ZERO);
    let mut base = [0usize; 3];
    let mut t = [0.0f32; 3];
    for (k, value) in color.to_array().into_iter().enumerate() {
        let position = grade_lut_shape(value) * (n - 1) as f32;
        base[k] = (position.floor() as usize).min(n - 2);
        t[k] = position - base[k] as f32;
    }
    let mut change = Vec3::ZERO;
    for corner in 0..8 {
        let step = [corner & 1, (corner >> 1) & 1, (corner >> 2) & 1];
        let mut weight = 1.0;
        for (s, f) in step.iter().zip(t) {
            weight *= if *s == 1 { f } else { 1.0 - f };
        }
        let index = (base[0] + step[0]) + n * ((base[1] + step[1]) + n * (base[2] + step[2]));
        let texel = table[index];
        change += Vec3::new(texel[0], texel[1], texel[2]) * weight;
    }
    color + change
}

#[cfg(test)]
mod tests {
    use super::*;
    fn view() -> View {
        View {
            view_proj: Mat4::IDENTITY,
            eye: Vec3::ZERO,
        }
    }
    fn light() -> Light {
        Light {
            position: Vec3::ZERO,
            color: Vec3::ONE,
            intensity: 12.,
            range: 20.,
        }
    }
    #[test]
    fn every_cube_face_projects_outward_with_finite_depth() {
        let light = light();
        for (matrix, direction) in light.shadow_views().unwrap().iter().zip([
            Vec3::X,
            -Vec3::X,
            Vec3::Y,
            -Vec3::Y,
            Vec3::Z,
            -Vec3::Z,
        ]) {
            let p = *matrix * (direction * 2.).extend(1.);
            assert!(p.w > 0. && p.x.abs() < 1e-5 && p.y.abs() < 1e-5);
            assert!(p.z / p.w > 0. && p.z / p.w < 1.);
        }
    }
    #[test]
    fn capacity_and_derived_overflow_are_admitted_before_submission() {
        let mut lighting = Lighting::default();
        lighting.lights = vec![light(); MAX_LIGHTS];
        assert!(lighting.validate(view()).is_ok());
        assert_eq!(lighting.shadow_count(), 4);
        lighting.lights.push(light());
        assert!(lighting.validate(view()).is_err());
        lighting.lights.pop();
        lighting.time = f32::MAX;
        assert!(lighting.validate(view()).is_err());
        lighting.time = 0.;
        lighting.lights[0].intensity = f32::MAX;
        lighting.time = 0.1;
        assert!(lighting.validate(view()).is_err());
    }
    #[test]
    fn camera_atmosphere_and_point_sources_reject_invalid_values() {
        let mut v = view();
        v.eye.x = f32::NAN;
        assert!(Lighting::default().validate(v).is_err());
        v = view();
        v.view_proj.x_axis.x = f32::INFINITY;
        assert!(Lighting::default().validate(v).is_err());
        let mut lighting = Lighting::default();
        lighting.fog.x = -1.;
        assert!(lighting.validate(view()).is_err());
        let mut bad = light();
        bad.range = 0.2;
        assert!(bad.shadow_views().is_err());
        bad = light();
        bad.position = Vec3::splat(f32::MAX);
        assert!(bad.shadow_views().is_err());
    }

    /// A camera at `eye` looking along `dir` through a 16:9 lens.
    fn camera(eye: Vec3, dir: Vec3) -> (Mat4, Frustum) {
        let view_proj = Mat4::perspective_rh(1.0, 16.0 / 9.0, 0.1, 2000.0)
            * Mat4::look_to_rh(eye, dir.normalize(), Vec3::Y);
        (view_proj, Frustum::from_view_proj(view_proj, eye).unwrap())
    }

    /// Everglade's afternoon sun.
    fn sun() -> Vec3 {
        Vec3::new(-0.35, 0.8, -0.45).normalize()
    }

    #[test]
    fn a_frustum_reads_the_lens_from_the_camera_matrix() {
        let dir = Vec3::new(1.0, -0.2, 0.3);
        let (_, frustum) = camera(Vec3::new(3.0, 2.0, -1.0), dir);
        assert!((frustum.near - 0.1).abs() < 1e-3);
        assert!((frustum.far - 2000.0).abs() < 20.0);
        let tan = 0.5f32.tan();
        let diagonal = (tan * tan * (1.0 + (16.0f32 / 9.0).powi(2))).sqrt();
        assert!((frustum.spread - diagonal).abs() < 1e-3);
        assert!((frustum.forward - dir.normalize()).length() < 1e-4);
        assert!(Frustum::from_view_proj(Mat4::IDENTITY, Vec3::ZERO).is_none());
    }

    #[test]
    fn splits_run_from_near_to_far_and_lean_logarithmic() {
        for count in 1..=MAX_CASCADES {
            let splits = cascade_splits(0.1, 150.0, count, 0.8);
            assert_eq!(splits.len(), count + 1);
            assert_eq!(splits[0], 0.1);
            assert_eq!(splits[count], 150.0);
            assert!(
                splits.windows(2).all(|pair| pair[1] > pair[0]),
                "{splits:?}"
            );
        }
        let uniform = cascade_splits(0.0, 90.0, 3, 0.0);
        assert!((uniform[1] - 30.0).abs() < 1e-4 && (uniform[2] - 60.0).abs() < 1e-4);
        assert!(cascade_splits(0.1, 150.0, 2, 0.8)[1] < cascade_splits(0.1, 150.0, 2, 0.0)[1]);
    }

    #[test]
    fn cascades_cover_the_view_their_blend_bands_and_their_casters() {
        let views = [
            (Vec3::new(0.0, 1.7, -20.0), Vec3::new(0.0, -0.1, 1.0)),
            (Vec3::new(30.0, 6.0, 12.0), Vec3::new(-1.0, -0.3, -0.4)),
            (Vec3::new(-5.0, 2.0, 5.0), Vec3::new(0.2, 0.9, 0.1)),
        ];
        for count in 1..=MAX_CASCADES {
            let settings = CascadeSettings::new(count as u32, 150.0);
            for (eye, dir) in views {
                let (view_proj, frustum) = camera(eye, dir);
                let cascades = fit_cascades(&frustum, sun(), &settings).unwrap();
                assert_eq!(cascades.cascades.len(), count);
                for (index, cascade) in cascades.cascades.iter().enumerate() {
                    assert_eq!(cascade.cached, index > 0);
                }
                let inverse = view_proj.inverse();
                for step in 0..=60 {
                    let depth =
                        frustum.near + (cascades.distance - frustum.near) * step as f32 / 60.0;
                    for (x, y) in [
                        (-1.0, -1.0),
                        (1.0, -1.0),
                        (-1.0, 1.0),
                        (1.0, 1.0),
                        (0.0, 0.0),
                        (0.3, -0.7),
                    ] {
                        let ray = inverse.project_point3(Vec3::new(x, y, 0.0)) - eye;
                        let point = eye + ray * (depth / ray.dot(frustum.forward));
                        let (index, weight) = cascades.select(frustum.depth(point));
                        let mut readers = vec![index];
                        if weight > 0.0 {
                            readers.push(index + 1);
                        }
                        for reader in readers {
                            let matrix = cascades.cascades[reader].matrix;
                            // The receiver, and a caster above it toward the sun.
                            let caster = point + sun() * settings.caster_reach * 0.9;
                            for p in [point, caster] {
                                let clip = matrix.project_point3(p);
                                assert!(
                                    clip.x.abs() <= 1.0
                                        && clip.y.abs() <= 1.0
                                        && (0.0..=1.0).contains(&clip.z),
                                    "{count} cascades: {p} at depth {depth} falls outside \
                                     cascade {reader} at {clip}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn texel_snapping_holds_shadow_texels_still_under_sub_texel_motion() {
        let settings = CascadeSettings::new(3, 150.0);
        let dir = Vec3::new(0.3, -0.15, 1.0);
        let fit = |eye: Vec3| fit_cascades(&camera(eye, dir).1, sun(), &settings).unwrap();
        let start = Vec3::new(2.0, 1.7, -20.0);
        let first = fit(start);
        let probe = Vec3::new(4.0, 0.0, -10.0);
        let resolution = settings.resolution as f32;
        let texel_of = |matrix: Mat4| {
            let clip = matrix.project_point3(probe);
            glam::Vec2::new(clip.x, clip.y) * 0.5 * resolution
        };
        let mut changes = [0; 3];
        let mut previous = first.clone();
        for step in 1..=200 {
            // A walk of a fifth of a near texel per frame.
            let walk = Vec3::new(0.7, 0.05, 0.4).normalize() * first.cascades[0].texel * 0.2;
            let now = fit(start + walk * step as f32);
            for (i, (before, after)) in previous.cascades.iter().zip(&now.cascades).enumerate() {
                assert_eq!(before.texel, after.texel, "cascade {i} changed its texel");
                if before.matrix != after.matrix {
                    changes[i] += 1;
                }
                // A world point keeps its place within its texel: the map
                // moves only by whole texels.
                let shift = texel_of(after.matrix) - texel_of(before.matrix);
                assert!(
                    (shift - shift.round()).abs().max_element() < 0.02,
                    "cascade {i} moved by {shift} texels at step {step}"
                );
            }
            previous = now;
        }
        assert!(changes[0] > 0, "the near cascade never followed the camera");
        // The cached cascades move only when the camera crosses a cache cell.
        for (i, &count) in changes.iter().enumerate().skip(1) {
            assert!(count <= 6, "cascade {i} changed {count} times");
        }
        // Turning the camera keeps every cascade's texel.
        let turned = fit_cascades(
            &camera(start, Vec3::new(-1.0, -0.1, 0.2)).1,
            sun(),
            &settings,
        )
        .unwrap();
        for (a, b) in first.cascades.iter().zip(&turned.cascades) {
            assert_eq!(a.texel, b.texel);
        }
    }

    #[test]
    fn cascades_blend_at_their_ends_and_fade_at_the_distance() {
        let (_, frustum) = camera(Vec3::ZERO, Vec3::Z);
        let cascades = fit_cascades(&frustum, sun(), &CascadeSettings::new(2, 100.0)).unwrap();
        let end = cascades.cascades[0].end;
        assert_eq!(cascades.select(end * 0.5), (0, 0.0));
        let (index, weight) = cascades.select(end - 1e-3);
        assert_eq!(index, 0);
        assert!(weight > 0.99);
        assert_eq!(cascades.select(end + 1.0), (1, 0.0));
        assert_eq!(cascades.select(1_000.0).0, 1);
        assert_eq!(cascades.presence(10.0), 1.0);
        assert_eq!(cascades.presence(100.0), 0.0);
        assert!((cascades.presence(95.0) - 0.5).abs() < 1e-4);
        // A fixed box never fades and has one map.
        let fixed = fit_box(sun(), Vec3::ZERO, 40.0, 2048);
        assert_eq!(fixed.cascades.len(), 1);
        assert_eq!(fixed.select(500.0), (0, 0.0));
        assert_eq!(fixed.presence(500.0), 1.0);
        assert!((fixed.cascades[0].texel - 80.0 / 2048.0).abs() < 1e-6);
    }

    #[test]
    fn cascade_settings_refuse_what_cannot_be_fitted() {
        let (_, frustum) = camera(Vec3::ZERO, Vec3::Z);
        let good = CascadeSettings::new(3, 150.0);
        assert!(good.validate().is_ok());
        for bad in [
            CascadeSettings { count: 0, ..good },
            CascadeSettings {
                count: MAX_CASCADES + 1,
                ..good
            },
            CascadeSettings {
                distance: f32::NAN,
                ..good
            },
            CascadeSettings { blend: 0.9, ..good },
            CascadeSettings {
                cache_cell: 1024,
                ..good
            },
        ] {
            assert!(fit_cascades(&frustum, sun(), &bad).is_err(), "{bad:?}");
        }
        assert!(fit_cascades(&frustum, Vec3::ZERO, &good).is_err());
        let short = CascadeSettings {
            distance: 0.05,
            ..good
        };
        assert!(fit_cascades(&frustum, sun(), &short).is_err());
    }

    #[test]
    fn shadow_maps_go_to_the_lights_that_matter_most() {
        let (view_proj, _) = camera(Vec3::ZERO, Vec3::Z);
        let view = View {
            view_proj,
            eye: Vec3::ZERO,
        };
        let at = |z: f32, x: f32, intensity: f32| Light {
            position: Vec3::new(x, 1.0, z),
            color: Vec3::ONE,
            intensity,
            range: 4.0,
        };
        let lights = vec![
            at(-20.0, 0.0, 50.0), // behind the camera
            at(6.0, 0.0, 10.0),
            at(30.0, 0.0, 10.0),
            at(8.0, 3.0, 40.0),
            at(5.0, 200.0, 40.0), // far off to the side
            at(12.0, -2.0, 12.0),
        ];
        assert_eq!(lights[0].contribution(view), 0.0);
        assert_eq!(lights[4].contribution(view), 0.0);
        let chosen = select_shadowed(&lights, view, 4, &[]);
        assert_eq!(chosen, vec![3, 1, 5, 2]);
        // Held lights keep their maps whatever order they were ranked in.
        let held = [2, 5, 1, 3];
        assert_eq!(select_shadowed(&lights, view, 4, &held), held);
        // A slightly brighter newcomer does not take a held map ...
        let mut more = lights.clone();
        more.push(at(30.0, 0.5, 11.0));
        assert!(select_shadowed(&more, view, 4, &[]).contains(&6));
        assert_eq!(select_shadowed(&more, view, 4, &chosen), chosen);
        // ... a much brighter one takes the weakest light's map, and the
        // other lights keep theirs.
        more[6].intensity = 30.0;
        let replaced = select_shadowed(&more, view, 4, &chosen);
        assert_eq!(replaced, vec![3, 1, 5, 6]);
        // With fewer lights than maps, every light gets one.
        assert_eq!(select_shadowed(&lights[..2], view, 4, &[]), vec![1, 0]);
    }

    /// A mild grade with every table term away from identity.
    fn graded() -> Grade {
        Grade {
            exposure: 0.77,
            balance: Vec3::new(1.1, 1.0, 0.85),
            saturation: 0.9,
            contrast: 1.05,
            gain: Vec3::new(1.0, 0.97, 0.92),
            ..Grade::NEUTRAL
        }
    }

    /// Colors from deep shadow to bright highlights in three hues.
    fn test_colors() -> Vec<Vec3> {
        (0..200)
            .flat_map(|step| {
                let x = 0.0005 * (step as f32 / 200.0 * 18.0).exp2();
                [
                    Vec3::splat(x),
                    Vec3::new(x, x * 0.5, x * 0.1),
                    Vec3::new(x * 0.2, x, x * 0.6),
                    Vec3::new(0.0, x, 0.0),
                ]
            })
            .collect()
    }

    #[test]
    fn the_table_shaper_keeps_black_and_round_trips() {
        assert_eq!(grade_lut_shape(0.0), 0.0);
        assert_eq!(grade_lut_shape(-1.0), 0.0);
        assert_eq!(grade_lut_unshape(0.0), 0.0);
        assert!((grade_lut_shape(GRADE_LUT_TOP) - 1.0).abs() < 1e-6);
        assert!((grade_lut_unshape(1.0) - GRADE_LUT_TOP).abs() < GRADE_LUT_TOP * 1e-5);
        assert_eq!(grade_lut_shape(GRADE_LUT_TOP * 4.0), 1.0);
        let mut previous = -1.0;
        for step in 0..400 {
            let x = 1e-4 * (step as f32 / 400.0 * 21.0).exp2();
            let u = grade_lut_shape(x);
            assert!(u > previous, "the shaper rises at {x}");
            previous = u;
            if x <= GRADE_LUT_TOP {
                let back = grade_lut_unshape(u);
                assert!(
                    (back - x).abs() <= x * 1e-4 + 1e-7,
                    "{x} came back as {back}"
                );
            }
        }
    }

    #[test]
    fn an_identity_grade_bakes_an_exact_table() {
        let table = Grade::NEUTRAL.bake();
        assert_eq!(
            table.len(),
            GRADE_LUT_SIZE * GRADE_LUT_SIZE * GRADE_LUT_SIZE
        );
        assert!(table.iter().all(|texel| *texel == [0.0; 4]));
        for color in test_colors() {
            assert_eq!(sample_grade_lut(&table, color), color);
        }
        assert_eq!(Grade::STAGE.bake(), table);
    }

    #[test]
    fn a_baked_grade_matches_the_analytic_grade() {
        let grade = graded();
        let table = grade.bake();
        let n = GRADE_LUT_SIZE;
        // At texel centers the table holds the grade itself.
        for (r, g, b) in [
            (0, 0, 0),
            (5, 9, 2),
            (17, 17, 17),
            (24, 30, 11),
            (31, 31, 31),
        ] {
            let at = |i: usize| grade_lut_unshape(i as f32 / (n - 1) as f32);
            let color = Vec3::new(at(r), at(g), at(b));
            let expected = grade.scene(color);
            let sampled = sample_grade_lut(&table, color);
            let tolerance = expected.max_element() * 1e-4 + 1e-6;
            assert!(
                (sampled - expected).abs().max_element() <= tolerance,
                "{color} graded to {sampled}, not {expected}"
            );
        }
        // Between texels, trilinear filtering stays within 3% of the
        // brightest channel.
        for color in test_colors() {
            let expected = grade.scene(color);
            let sampled = sample_grade_lut(&table, color);
            let tolerance = expected.max_element() * 0.03 + 1e-4;
            assert!(
                (sampled - expected).abs().max_element() <= tolerance,
                "{color} graded to {sampled}, not {expected}"
            );
        }
    }

    #[test]
    fn grade_terms_do_what_they_name() {
        let grey = Vec3::splat(0.18);
        let brighter = Grade {
            exposure: 1.0,
            ..Grade::NEUTRAL
        };
        assert!(
            (brighter.scene(grey) - Vec3::splat(0.36))
                .abs()
                .max_element()
                < 1e-6
        );
        let flat = Grade {
            saturation: 0.0,
            ..Grade::NEUTRAL
        };
        let red = flat.scene(Vec3::new(1.0, 0.0, 0.0));
        assert!((red - Vec3::splat(0.2126)).abs().max_element() < 1e-6);
        // Contrast pivots on mid-grey.
        let punchy = Grade {
            contrast: 1.5,
            ..Grade::NEUTRAL
        };
        assert!((punchy.scene(grey) - grey).abs().max_element() < 1e-6);
        assert!(punchy.scene(Vec3::splat(0.36)).x > 0.36);
        assert!(punchy.scene(Vec3::splat(0.09)).x < 0.09);
        // White balance is a per-channel gain, and black stays black.
        let warm = Grade {
            balance: Vec3::new(1.2, 1.0, 0.8),
            ..Grade::NEUTRAL
        };
        assert!(
            (warm.scene(Vec3::ONE) - Vec3::new(1.2, 1.0, 0.8))
                .abs()
                .max_element()
                < 1e-6
        );
        assert_eq!(graded().scene(Vec3::ZERO), Vec3::ZERO);
    }

    #[test]
    fn output_curves_keep_black_hue_and_their_ceiling() {
        for curve in [Curve::Neutral, Curve::HueShoulder] {
            assert_eq!(curve.apply(Vec3::ZERO, 1.0), Vec3::ZERO);
            for ceiling in [1.0, 4.0, 16.0] {
                let mut previous = -1.0;
                for step in 0..300 {
                    let x = 1e-3 * (step as f32 / 300.0 * 20.0).exp2();
                    let y = curve.apply(Vec3::splat(x), ceiling).x;
                    assert!(y >= previous, "{curve:?} falls at {x}");
                    assert!(y <= ceiling, "{curve:?} passes its ceiling at {x}");
                    previous = y;
                }
            }
        }
        // Below the shoulder, Neutral subtracts its small toe offset.
        let mid = Curve::Neutral.apply(Vec3::splat(0.5), 1.0);
        assert!((mid - Vec3::splat(0.46)).abs().max_element() < 1e-6);
        // The hue-preserving shoulder keeps channel ratios.
        let amber = Curve::HueShoulder.apply(Vec3::new(8.0, 4.0, 1.0), 1.0);
        assert!((amber.x / amber.y - 2.0).abs() < 1e-5);
        assert!((amber.x / amber.z - 8.0).abs() < 1e-4);
        assert!(amber.x < 1.0);
    }

    #[test]
    fn the_chamber_grade_keeps_the_retired_curves_mid_grey() {
        // The ACES curve fit the chamber's fragments used to apply.
        let aces = |x: f32| (x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14);
        let retired = aces(0.18);
        let graded = Grade::CHAMBER.display(Vec3::splat(0.18));
        assert!(
            (graded.x - retired).abs() < 0.003,
            "{graded} against {retired}"
        );
        let plain = Grade::NEUTRAL.display(Vec3::splat(0.18));
        assert!((plain.x - 0.14).abs() < 1e-6);
    }

    /// Split toning tints shadows and highlights apart, leaves mid-grey
    /// between the two, and bakes into the table.
    #[test]
    fn split_toning_cools_shadows_and_warms_highlights() {
        let split = Grade {
            shadows: Vec3::new(0.9, 1.0, 1.1),
            highlights: Vec3::new(1.1, 1.0, 0.9),
            ..Grade::NEUTRAL
        };
        assert!(split.validate().is_ok());
        let dark = split.scene(Vec3::splat(0.01));
        assert!(dark.z > dark.x, "{dark}");
        let bright = split.scene(Vec3::splat(3.0));
        assert!(bright.x > bright.z, "{bright}");
        let mid = split.scene(Vec3::splat(MID_GREY));
        assert!(
            (mid - Vec3::splat(MID_GREY)).abs().max_element() < 1e-5,
            "{mid}"
        );
        assert!(!split.same_table(&Grade::NEUTRAL));
        let table = split.bake();
        let sampled = sample_grade_lut(&table, Vec3::splat(3.0));
        assert!((sampled - bright).abs().max_element() < 0.02 * bright.max_element());
        for bad in [Vec3::ZERO, Vec3::splat(5.0), Vec3::NAN] {
            assert!(
                Grade {
                    shadows: bad,
                    ..Grade::NEUTRAL
                }
                .validate()
                .is_err()
            );
        }
    }

    #[test]
    fn grades_refuse_invalid_values_and_compare_tables() {
        for grade in [Grade::NEUTRAL, Grade::STAGE, Grade::CHAMBER, graded()] {
            assert!(grade.validate().is_ok());
        }
        let bad = [
            Grade {
                exposure: f32::NAN,
                ..Grade::NEUTRAL
            },
            Grade {
                balance: Vec3::new(1.0, 0.0, 1.0),
                ..Grade::NEUTRAL
            },
            Grade {
                saturation: -0.5,
                ..Grade::NEUTRAL
            },
            Grade {
                contrast: 0.0,
                ..Grade::NEUTRAL
            },
            Grade {
                ceiling: 0.5,
                ..Grade::NEUTRAL
            },
            Grade {
                gain: Vec3::new(1.0, f32::INFINITY, 1.0),
                ..Grade::NEUTRAL
            },
        ];
        for grade in bad {
            assert!(grade.validate().is_err(), "{grade:?}");
        }
        let hdr = Grade {
            ceiling: 4.0,
            ..Grade::STAGE
        };
        assert!(hdr.same_table(&Grade::NEUTRAL));
        assert!(!Grade::CHAMBER.same_table(&Grade::NEUTRAL));
    }

    fn glade_fog() -> HeightFog {
        HeightFog {
            density: 0.005,
            base: 0.0,
            falloff: 0.12,
            start: 40.0,
            max_opacity: 0.92,
            sun_strength: 0.4,
            sun_exponent: 3.0,
        }
    }

    /// The trapezoid rule along the ray, against which the closed form is
    /// checked.
    fn integrated_depth(fog: &HeightFog, eye: Vec3, point: Vec3) -> f32 {
        let ray = point - eye;
        let length = ray.length();
        let density = |t: f32| {
            if t * length < fog.start {
                0.0
            } else {
                let y = eye.y + ray.y * t;
                fog.density * (-fog.falloff * (y - fog.base)).exp()
            }
        };
        let steps = 4_000;
        let mut depth = 0.0;
        for i in 0..steps {
            let a = i as f32 / steps as f32;
            let b = (i + 1) as f32 / steps as f32;
            depth += (density(a) + density(b)) * 0.5 * (b - a) * length;
        }
        depth
    }

    #[test]
    fn height_fog_has_the_closed_form_of_its_density() {
        let fog = glade_fog();
        let eye = Vec3::new(0.0, 1.7, 0.0);
        for point in [
            Vec3::new(120.0, 0.0, 0.0),
            Vec3::new(0.0, 9.0, 150.0),
            Vec3::new(60.0, 40.0, 60.0),
            Vec3::new(-30.0, -5.0, 80.0),
            Vec3::new(100.0, 1.7, 0.0),
        ] {
            let closed = fog.optical_depth(eye, point);
            let numeric = integrated_depth(&fog, eye, point);
            assert!(
                (closed - numeric).abs() < 1e-3 * numeric.max(1.0),
                "{point}: {closed} against {numeric}"
            );
        }
    }

    #[test]
    fn uniform_distance_fog_is_the_chambers_exponential() {
        let fog = HeightFog::distance(0.012);
        let eye = Vec3::new(1.0, 2.0, 3.0);
        for point in [Vec3::new(30.0, 2.0, 3.0), Vec3::new(1.0, 50.0, -40.0)] {
            let expected = 1.0 - (-(point - eye).length() * 0.012).exp();
            assert!((fog.opacity(eye, point) - expected).abs() < 1e-6);
        }
        let mut lighting = Lighting::default();
        assert_eq!(lighting.fog_shape(), HeightFog::distance(lighting.density));
        lighting.height_fog = Some(glade_fog());
        assert_eq!(lighting.fog_shape(), glade_fog());
    }

    #[test]
    fn height_fog_starts_late_caps_and_thins_with_height() {
        let fog = glade_fog();
        let eye = Vec3::new(0.0, 1.7, 0.0);
        // Nothing nearer than the start distance.
        assert_eq!(fog.opacity(eye, Vec3::new(0.0, 0.0, 39.0)), 0.0);
        assert!(fog.opacity(eye, Vec3::new(0.0, 0.0, 80.0)) > 0.0);
        // The cap holds however far the ray runs through dense fog.
        let dense = HeightFog {
            density: 0.5,
            ..fog
        };
        assert_eq!(dense.opacity(eye, Vec3::new(0.0, 0.0, 900.0)), 0.92);
        // At equal distance, a hilltop is clearer than the valley floor, and
        // an eye up on the hill sees less fog than one in the hollow.
        let floor = fog.opacity(eye, Vec3::new(0.0, 0.0, 120.0));
        let hill = fog.opacity(eye, Vec3::new(0.0, 10.0, 120.0));
        assert!(hill < floor, "{hill} {floor}");
        let high_eye = Vec3::new(0.0, 11.7, 0.0);
        let from_hill = fog.opacity(high_eye, Vec3::new(0.0, 10.0, 120.0));
        assert!(from_hill < hill, "{from_hill} {hill}");
        // The stage's range is still the limit: fog is total at its end.
        let edge = fog.stage_opacity(eye, Vec3::new(0.0, 0.0, 180.0), 170.0);
        assert_eq!(edge, 1.0);
        let near = Vec3::new(0.0, 0.0, 100.0);
        assert_eq!(fog.stage_opacity(eye, near, 170.0), fog.opacity(eye, near));
    }

    #[test]
    fn the_sun_lobe_glows_toward_the_sun_only() {
        let fog = glade_fog();
        let sun = Vec3::new(-0.35, 0.8, -0.45).normalize();
        let toward = Vec3::new(sun.x, 0.0, sun.z).normalize();
        assert!((fog.sun_lobe(sun, sun) - fog.sun_strength).abs() < 1e-5);
        assert!(fog.sun_lobe(toward, sun) > fog.sun_lobe(-toward, sun));
        assert_eq!(fog.sun_lobe(-sun, sun), 0.0);
        let packed = fog.uniform();
        assert_eq!(packed[0], [0.005, 0.0, 0.12, 40.0]);
        assert_eq!(packed[1], [0.92, 0.4, 3.0, 1.0]);
    }

    #[test]
    fn invalid_height_fog_is_refused() {
        assert!(glade_fog().validate().is_ok());
        assert!(HeightFog::distance(0.008).validate().is_ok());
        let bad = [
            HeightFog {
                density: f32::NAN,
                ..glade_fog()
            },
            HeightFog {
                falloff: -0.1,
                ..glade_fog()
            },
            HeightFog {
                max_opacity: 1.5,
                ..glade_fog()
            },
            HeightFog {
                sun_exponent: 0.0,
                ..glade_fog()
            },
        ];
        for fog in bad {
            assert!(fog.validate().is_err(), "{fog:?}");
        }
        let lighting = Lighting {
            height_fog: Some(HeightFog {
                start: -1.0,
                ..glade_fog()
            }),
            ..Lighting::default()
        };
        assert!(lighting.validate(view()).is_err());
    }
}

#[cfg(test)]
mod visual_contract_tests {
    use super::*;
    #[test]
    fn point_profiles_preserve_units_range_and_finite_source_core() {
        assert!((PointProfile::Authored.attenuation(1.0, 10.0) - 0.405).abs() < 1e-6);
        let physical = PointProfile::Candela.attenuation(4.0, 10.0);
        assert!((physical - 0.9984_f32.powi(2) / 4.01).abs() < 1e-6);
        for profile in [PointProfile::Authored, PointProfile::Candela] {
            assert!(profile.attenuation(0.0, 10.0).is_finite());
            assert_eq!(profile.attenuation(100.0, 10.0), 0.0);
            assert_eq!(profile.attenuation(f32::NAN, 10.0), 0.0);
            assert_eq!(profile.attenuation(1.0, 0.0), 0.0);
        }
        assert!((exposure(1.0) * 2.0 - exposure(0.0)).abs() < 1e-6);
    }
    #[test]
    fn visible_foreground_lamps_survive_every_tier_budget() {
        let eye = Vec3::new(0.0, 1.0, 5.0);
        let view = View {
            eye,
            view_proj: Mat4::perspective_rh(1.0, 1.0, 0.1, 100.0)
                * Mat4::look_at_rh(eye, Vec3::ZERO, Vec3::Y),
        };
        let dim = Light {
            position: Vec3::ZERO,
            color: Vec3::ONE,
            intensity: 1.0,
            range: 2.0,
        };
        let mut lights = vec![dim; MAX_LIGHTS];
        lights[31].intensity = 100.0;
        lights[30].position = Vec3::new(0.0, 0.0, 500.0);
        lights[30].intensity = 1_000_000.0;
        for budget in [8, 16, 32] {
            let selected = select_lights(&lights, view, budget);
            assert_eq!(selected[0], 31);
            assert!(!selected.contains(&30));
            assert!(selected.len() <= budget);
            assert_eq!(selected[1], 0);
        }
    }
}
