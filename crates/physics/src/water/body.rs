//! Water body records: what kind of water, where, how high, how dense, its
//! waves, and its current.

use glam::{DVec2, DVec3};
use serde::{Deserialize, Serialize};

use super::flow::FlowGrid;
use super::surface::{Phases, WaveSet};

/// Index of a water body in its zone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct WaterId(pub u32);

/// What a body of water is, for the rules and the look. The physics treats
/// every kind alike.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Pond,
    River,
    Waterfall,
    Ocean,
    Pool,
    Puddle,
    Marsh,
}

/// Fresh water's density, kg/m³.
pub const FRESH: f64 = 1000.0;
/// Seawater's density, kg/m³.
pub const SALT: f64 = 1025.0;

/// A river's course: a centerline with a full width at each point.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Course {
    pub points: Vec<DVec2>,
    /// Full width at each point, m.
    pub widths: Vec<f64>,
    /// Distance along the course to each point, m.
    pub lengths: Vec<f64>,
}

/// Where a point sits relative to a course.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Station {
    /// Distance along the course, m, clamped to it.
    pub along: f64,
    /// Signed distance from the centerline, positive to the left of the
    /// direction of flow, m.
    pub offset: f64,
    /// Half the width there, m.
    pub half_width: f64,
    /// Direction of flow, unit.
    pub tangent: DVec2,
    /// Whether the point lies past either end.
    pub beyond: bool,
}

impl Course {
    /// A course through `points` with a full width at each.
    ///
    /// # Panics
    ///
    /// When there are fewer than two points or the widths do not match.
    #[must_use]
    pub fn new(points: Vec<DVec2>, widths: Vec<f64>) -> Self {
        assert!(points.len() >= 2, "a course needs two points");
        assert_eq!(points.len(), widths.len(), "one width per point");
        let mut lengths = vec![0.0];
        for w in points.windows(2) {
            lengths.push(lengths.last().unwrap() + w[0].distance(w[1]));
        }
        Self {
            points,
            widths,
            lengths,
        }
    }

    /// A course along a uniform Catmull–Rom spline through `controls`, with
    /// `per_segment` points between each pair, widths interpolated linearly.
    #[must_use]
    pub fn spline(controls: &[DVec2], widths: &[f64], per_segment: usize) -> Self {
        assert!(controls.len() >= 2 && controls.len() == widths.len());
        let n = controls.len();
        let at = |i: isize| controls[i.clamp(0, n as isize - 1) as usize];
        let mut points = Vec::new();
        let mut out_widths = Vec::new();
        let steps = per_segment.max(1);
        for i in 0..n - 1 {
            let (p0, p1, p2, p3) = (
                at(i as isize - 1),
                at(i as isize),
                at(i as isize + 1),
                at(i as isize + 2),
            );
            for s in 0..steps {
                let t = s as f64 / steps as f64;
                let (t2, t3) = (t * t, t * t * t);
                points.push(
                    0.5 * (2.0 * p1
                        + (p2 - p0) * t
                        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
                        + (3.0 * p1 - p0 - 3.0 * p2 + p3) * t3),
                );
                out_widths.push(widths[i] + (widths[i + 1] - widths[i]) * t);
            }
        }
        points.push(controls[n - 1]);
        out_widths.push(widths[n - 1]);
        Self::new(points, out_widths)
    }

    /// Total length, m.
    #[must_use]
    pub fn length(&self) -> f64 {
        *self.lengths.last().unwrap()
    }

    /// The nearest station on the course to `p`.
    #[must_use]
    pub fn locate(&self, p: DVec2) -> Station {
        let mut best = (f64::INFINITY, 0usize, 0.0);
        for i in 0..self.points.len() - 1 {
            let (a, b) = (self.points[i], self.points[i + 1]);
            let d = b - a;
            let t = if d.length_squared() > 0.0 {
                ((p - a).dot(d) / d.length_squared()).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let dist = p.distance_squared(a + d * t);
            if dist < best.0 {
                best = (dist, i, t);
            }
        }
        let (_, i, t) = best;
        let (a, b) = (self.points[i], self.points[i + 1]);
        let tangent = (b - a).normalize_or(DVec2::X);
        let normal = DVec2::new(-tangent.y, tangent.x);
        let foot = a + (b - a) * t;
        let last = self.points.len() - 2;
        let rel = (p - a).dot(b - a);
        let beyond = (i == 0 && rel < 0.0) || (i == last && (p - b).dot(b - a) > 0.0 && t >= 1.0);
        Station {
            along: self.lengths[i] + (self.lengths[i + 1] - self.lengths[i]) * t,
            offset: (p - foot).dot(normal),
            half_width: 0.5 * (self.widths[i] + (self.widths[i + 1] - self.widths[i]) * t),
            tangent,
            beyond,
        }
    }

    /// The point on the centerline at distance `along`.
    #[must_use]
    pub fn point_at(&self, along: f64) -> DVec2 {
        let along = along.clamp(0.0, self.length());
        let i = self
            .lengths
            .windows(2)
            .position(|w| along <= w[1])
            .unwrap_or(self.points.len() - 2);
        let span = self.lengths[i + 1] - self.lengths[i];
        let t = if span > 0.0 {
            (along - self.lengths[i]) / span
        } else {
            0.0
        };
        self.points[i].lerp(self.points[i + 1], t)
    }

    fn bounds(&self) -> (DVec2, DVec2) {
        let reach = self.widths.iter().fold(0.0f64, |m, w| m.max(*w)) * 0.5;
        let (lo, hi) = self.points.iter().fold(
            (DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY)),
            |(lo, hi), p| (lo.min(*p), hi.max(*p)),
        );
        (lo - reach, hi + reach)
    }
}

/// Where a body of water lies in the (x, z) plane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outline", rename_all = "snake_case")]
pub enum Outline {
    /// No edge: the open ocean.
    Everywhere,
    /// A closed polygon.
    Polygon { points: Vec<DVec2> },
    /// A river along its course, as wide as the course says.
    River { course: Course },
}

impl Outline {
    /// Whether `(x, z)` lies in the water.
    #[must_use]
    pub fn contains(&self, p: DVec2) -> bool {
        match self {
            Self::Everywhere => true,
            Self::Polygon { points } => {
                // Even-odd crossing test.
                let mut inside = false;
                let n = points.len();
                for i in 0..n {
                    let (a, b) = (points[i], points[(i + n - 1) % n]);
                    if (a.y > p.y) != (b.y > p.y)
                        && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x
                    {
                        inside = !inside;
                    }
                }
                inside
            }
            Self::River { course } => {
                let station = course.locate(p);
                !station.beyond && station.offset.abs() <= station.half_width
            }
        }
    }

    /// The outline's bounding box, or none for an unbounded outline.
    #[must_use]
    pub fn bounds(&self) -> Option<(DVec2, DVec2)> {
        match self {
            Self::Everywhere => None,
            Self::Polygon { points } => Some(points.iter().fold(
                (DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY)),
                |(lo, hi), p| (lo.min(*p), hi.max(*p)),
            )),
            Self::River { course } => Some(course.bounds()),
        }
    }
}

/// The water's rest level.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "level", rename_all = "snake_case")]
pub enum Level {
    /// One height everywhere, m.
    Constant { height: f64 },
    /// Heights at distances along a river's course, `[along, height]`,
    /// interpolated linearly and held past either end. On an outline
    /// without a course, the first height.
    Profile { points: Vec<[f64; 2]> },
}

impl Level {
    /// The height and its slope along the course at distance `along`.
    #[must_use]
    pub fn at(&self, along: f64) -> (f64, f64) {
        match self {
            Self::Constant { height } => (*height, 0.0),
            Self::Profile { points } => {
                let Some(first) = points.first() else {
                    return (0.0, 0.0);
                };
                if along <= first[0] {
                    return (first[1], 0.0);
                }
                for w in points.windows(2) {
                    if along <= w[1][0] {
                        let span = w[1][0] - w[0][0];
                        let slope = if span > 0.0 {
                            (w[1][1] - w[0][1]) / span
                        } else {
                            0.0
                        };
                        return (w[0][1] + slope * (along - w[0][0]), slope);
                    }
                }
                (points.last().unwrap()[1], 0.0)
            }
        }
    }
}

/// One body of water.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WaterBody {
    pub id: WaterId,
    pub kind: Kind,
    pub outline: Outline,
    pub level: Level,
    /// kg/m³: [`FRESH`] or [`SALT`].
    pub density: f64,
    pub waves: WaveSet,
    /// The current, in the body's (world) frame.
    pub flow: Option<FlowGrid>,
}

/// The water at one point, at one tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    pub body: WaterId,
    /// Surface height, m.
    pub height: f64,
    /// Surface normal, unit, up.
    pub normal: DVec3,
    /// Velocity of the surface's water particles from the waves, m/s.
    pub surface_velocity: DVec3,
    /// The current, horizontal, m/s.
    pub flow: DVec3,
    /// kg/m³.
    pub density: f64,
}

impl Sample {
    /// The water's velocity at the surface: waves plus current.
    #[must_use]
    pub fn velocity(&self) -> DVec3 {
        self.surface_velocity + self.flow
    }

    /// Whether the water here is still: no waves and no current.
    #[must_use]
    pub fn still(&self) -> bool {
        self.surface_velocity == DVec3::ZERO && self.flow.length_squared() < 1e-12
    }
}

/// A body's gameplay surface: level plus waves plus current, a pure
/// function of the body and the tick.
#[derive(Clone, Copy, Debug)]
pub struct Surface<'a> {
    pub body: &'a WaterBody,
}

impl Surface<'_> {
    /// The water over `(x, z)` at `tick`, or none outside the outline.
    #[must_use]
    pub fn sample(&self, x: f64, z: f64, tick: u64) -> Option<Sample> {
        self.sample_with(x, z, &self.body.waves.phases(tick))
    }

    /// As [`Surface::sample`], with phase counters the caller advanced.
    #[must_use]
    pub fn sample_with(&self, x: f64, z: f64, phases: &Phases) -> Option<Sample> {
        let body = self.body;
        let p = DVec2::new(x, z);
        let (along, tangent) = match &body.outline {
            Outline::Everywhere => (0.0, DVec2::X),
            Outline::Polygon { points } => {
                if !body.outline.contains(p) || points.len() < 3 {
                    return None;
                }
                (0.0, DVec2::X)
            }
            Outline::River { course } => {
                let station = course.locate(p);
                if station.beyond || station.offset.abs() > station.half_width {
                    return None;
                }
                (station.along, station.tangent)
            }
        };
        let (level, slope) = body.level.at(along);
        let wave = body.waves.at(p, phases);
        // Height H = level + waves; the normal is (−∂H/∂x, 1, −∂H/∂z).
        let n = wave.normal;
        let normal = DVec3::new(
            n.x / n.y - slope * tangent.x,
            1.0,
            n.z / n.y - slope * tangent.y,
        )
        .normalize();
        let flow = body
            .flow
            .as_ref()
            .map_or(DVec2::ZERO, |grid| grid.sample(p));
        Some(Sample {
            body: body.id,
            height: level + wave.height,
            normal,
            surface_velocity: wave.velocity,
            flow: DVec3::new(flow.x, 0.0, flow.y),
            density: body.density,
        })
    }
}

impl WaterBody {
    /// Still fresh water inside `outline` at `level`.
    #[must_use]
    pub fn new(id: WaterId, kind: Kind, outline: Outline, level: Level) -> Self {
        Self {
            id,
            kind,
            outline,
            level,
            density: FRESH,
            waves: WaveSet::calm(),
            flow: None,
        }
    }

    /// A pond: a polygon at a constant level.
    #[must_use]
    pub fn pond(id: WaterId, points: Vec<DVec2>, height: f64) -> Self {
        Self::new(
            id,
            Kind::Pond,
            Outline::Polygon { points },
            Level::Constant { height },
        )
    }

    /// An ocean of seawater at `height` everywhere.
    #[must_use]
    pub fn ocean(id: WaterId, height: f64) -> Self {
        let mut body = Self::new(
            id,
            Kind::Ocean,
            Outline::Everywhere,
            Level::Constant { height },
        );
        body.density = SALT;
        body
    }

    #[must_use]
    pub fn with_waves(mut self, waves: WaveSet) -> Self {
        self.waves = waves;
        self
    }

    #[must_use]
    pub fn with_flow(mut self, flow: FlowGrid) -> Self {
        self.flow = Some(flow);
        self
    }

    #[must_use]
    pub fn with_density(mut self, density: f64) -> Self {
        self.density = density;
        self
    }

    /// The gameplay surface.
    #[must_use]
    pub fn surface(&self) -> Surface<'_> {
        Surface { body: self }
    }
}
