//! 2D drawings (#11647): standard views of a built part or assembly
//! projected from its solids, with visible and hidden lines told apart,
//! silhouettes of curved faces, section views (hatched), dimensions and a
//! title block, laid out on a sheet and written as SVG, PDF or DXF.
//!
//! The views are orthographic projections of the solids' triangles and
//! their B-rep edges (as fine as the display tolerance on curved faces):
//!
//! - **front** looks at the model's front (+X) with Z up; **top** looks down
//!   with the front at the bottom of the view; **right** looks from the
//!   right of the front view (+Y, the model's left); **left**, **bottom**
//!   and **back** are their opposites; **iso** looks from the front, the
//!   right and above (1, 1, 1).
//! - **Lines**: every B-rep edge that isn't a seam or a tangent join, and
//!   the silhouettes of curved faces (where their triangles turn from facing
//!   the viewer to facing away). A line's visibility changes only where it
//!   passes behind another line, so each line is split there and each piece
//!   tested: hidden when a triangle in front covers it. Where lines fall on
//!   each other, a section outline wins over a visible line and a visible
//!   line over a hidden one.
//! - **Sections** cut the model with a plane square to a view (through the
//!   middle unless `at` says where) and take away what is in front: the
//!   cut's outline is drawn and hatched at 45°, with the visible lines
//!   behind it; a chain line marks the cut on a view it crosses.
//! - **Dimensions**: the overall width and height of the front view and the
//!   depth of the top (or right) view, the diameters of the circles a view
//!   shows (`4× Ø6` when four match), and distances between named flat
//!   faces (`{"part": "plate", "faces": ["body:start", "body:top"],
//!   "view": "front"}`, the names of [`crate::refs`]).
//! - **Sheet**: A4, A3 (the default) or A2, landscape; third-angle
//!   projection (top above the front, right to its right) or first-angle;
//!   one standard scale for every view (`2:1`, `1:1`, `1:2`, `1:5`, ...);
//!   a title block with the title, the document, the scale and the units.

use std::collections::HashMap;
use std::fmt::Write as _;

use serde_json::{Value, json};

use crate::build::Built;

type P2 = [f64; 2];
type P3 = [f64; 3];

/// What a line of the drawing is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    /// An edge or silhouette the viewer sees.
    Visible,
    /// One behind the model (dashed).
    Hidden,
    /// The outline of a section's cut.
    Section,
    /// The hatching inside a cut.
    Hatch,
    /// A chain line where a section cuts another view.
    Cutting,
    /// Dimension and extension lines, arrows and leaders.
    Dimension,
    /// The sheet's border and title block.
    Frame,
}

impl Kind {
    /// Its name in reports and its DXF layer (upper case there).
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Kind::Visible => "visible",
            Kind::Hidden => "hidden",
            Kind::Section => "section",
            Kind::Hatch => "hatch",
            Kind::Cutting => "cutting",
            Kind::Dimension => "dimension",
            Kind::Frame => "frame",
        }
    }

    const ALL: [Kind; 7] = [
        Kind::Visible,
        Kind::Hidden,
        Kind::Section,
        Kind::Hatch,
        Kind::Cutting,
        Kind::Dimension,
        Kind::Frame,
    ];

    /// Stroke width on the sheet, mm.
    fn width(self) -> f64 {
        match self {
            Kind::Visible | Kind::Section | Kind::Frame => 0.5,
            Kind::Hidden | Kind::Cutting => 0.35,
            Kind::Hatch | Kind::Dimension => 0.18,
        }
    }

    /// Dash pattern on the sheet, mm (none for a solid line).
    fn dashes(self) -> &'static [f64] {
        match self {
            Kind::Hidden => &[3.0, 1.5],
            Kind::Cutting => &[8.0, 2.0, 1.5, 2.0],
            _ => &[],
        }
    }

    fn dxf_linetype(self) -> &'static str {
        match self {
            Kind::Hidden => "DASHED",
            Kind::Cutting => "CENTER",
            _ => "CONTINUOUS",
        }
    }

    /// AutoCAD colour index of its layer.
    fn dxf_color(self) -> i32 {
        match self {
            Kind::Visible | Kind::Frame => 7,
            Kind::Hidden => 8,
            Kind::Section => 1,
            Kind::Hatch => 9,
            Kind::Cutting => 5,
            Kind::Dimension => 3,
        }
    }
}

/// One line of a view: a polyline in the view's plane (model millimetres,
/// `u` right and `v` up as the view is seen).
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub kind: Kind,
    pub points: Vec<P2>,
    /// A curved face's silhouette rather than a B-rep edge.
    pub silhouette: bool,
}

/// A projected view.
#[derive(Clone, Debug)]
pub struct View {
    /// `front`, `top`, ..., or `section-a` for the first section.
    pub name: String,
    /// What the sheet calls it: "Front", "Section A–A".
    pub title: String,
    /// Towards the viewer, and the view's right and up, in model axes.
    pub eye: P3,
    pub u: P3,
    pub v: P3,
    /// For a section: where the plane cuts, measured along `eye`.
    pub cut: Option<f64>,
    pub lines: Vec<Line>,
    /// The box of its lines (u, v).
    pub min: P2,
    pub max: P2,
}

impl View {
    /// How many lines of a kind it has (silhouettes counted with their kind).
    #[must_use]
    pub fn count(&self, kind: Kind) -> usize {
        self.lines.iter().filter(|l| l.kind == kind).count()
    }
}

/// Where text hangs from its point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    Start,
    Middle,
    End,
}

/// One thing on the sheet, in sheet millimetres (origin bottom left, y up).
#[derive(Clone, Debug, PartialEq)]
pub enum Mark {
    Line {
        kind: Kind,
        points: Vec<P2>,
    },
    Text {
        at: P2,
        /// Cap height, mm.
        size: f64,
        text: String,
        anchor: Anchor,
        /// Degrees anticlockwise.
        angle: f64,
    },
}

/// A dimension the sheet shows.
#[derive(Clone, Debug, PartialEq)]
pub struct Dimension {
    pub view: String,
    pub text: String,
    /// The measured length (or diameter), mm.
    pub value: f64,
}

/// A laid-out drawing.
#[derive(Clone, Debug)]
pub struct Drawing {
    pub title: String,
    /// The document's id and version line, for the title block.
    pub subtitle: String,
    pub sheet: String,
    /// Sheet width and height, mm.
    pub size: P2,
    /// Sheet mm per model mm.
    pub scale: f64,
    pub angle: String,
    pub views: Vec<View>,
    /// Each view's origin on the sheet (where its u = 0, v = 0 lands).
    pub origins: Vec<P2>,
    pub dimensions: Vec<Dimension>,
    pub marks: Vec<Mark>,
    /// How far below each view (sheet mm) its dimensions reach.
    below: Vec<f64>,
    /// Whether circles get diameter callouts (a part, not an assembly).
    callouts: bool,
}

// ---------------------------------------------------------------------------
// Small vector helpers.

fn dot(a: P3, b: P3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: P3, b: P3) -> P3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn sub3(a: P3, b: P3) -> P3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn unit(a: P3) -> P3 {
    let l = dot(a, a).sqrt();
    if l > 0.0 {
        [a[0] / l, a[1] / l, a[2] / l]
    } else {
        a
    }
}

fn sub(a: P2, b: P2) -> P2 {
    [a[0] - b[0], a[1] - b[1]]
}

fn add(a: P2, b: P2) -> P2 {
    [a[0] + b[0], a[1] + b[1]]
}

fn mul(a: P2, k: f64) -> P2 {
    [a[0] * k, a[1] * k]
}

fn dot2(a: P2, b: P2) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

fn cross2(a: P2, b: P2) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}

fn len2(a: P2) -> f64 {
    dot2(a, a).sqrt()
}

fn lerp(a: P2, b: P2, t: f64) -> P2 {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

/// A length for the sheet: whole millimetres plain, else up to two places.
#[must_use]
pub fn number(x: f64) -> String {
    let r = (x * 100.0).round() / 100.0;
    let mut s = format!("{r:.2}");
    while s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    if s == "-0" { "0".into() } else { s }
}

// ---------------------------------------------------------------------------
// Views.

/// The named views a drawing can show.
pub const VIEWS: [&str; 7] = ["front", "top", "right", "left", "bottom", "back", "iso"];

/// (towards the viewer, up on the sheet) for a named view.
fn direction(name: &str) -> Option<(P3, P3)> {
    let z = [0.0, 0.0, 1.0];
    Some(match name {
        "front" => ([1.0, 0.0, 0.0], z),
        "back" => ([-1.0, 0.0, 0.0], z),
        "right" => ([0.0, 1.0, 0.0], z),
        "left" => ([0.0, -1.0, 0.0], z),
        "top" => ([0.0, 0.0, 1.0], [-1.0, 0.0, 0.0]),
        "bottom" => ([0.0, 0.0, -1.0], [1.0, 0.0, 0.0]),
        "iso" => (unit([1.0, 1.0, 1.0]), z),
        _ => return None,
    })
}

fn title_of(name: &str) -> String {
    match name {
        "iso" => "Isometric".into(),
        other => {
            let mut c = other.chars();
            c.next()
                .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                .unwrap_or_default()
        }
    }
}

/// A projected triangle: corners on the view plane and their depths.
struct Tri {
    p: [P2; 3],
    d: [f64; 3],
    /// Twice its signed area.
    area: f64,
}

/// A uniform grid over the view plane, each cell listing the items whose
/// boxes touch it.
struct Grid {
    min: P2,
    cell: f64,
    nx: usize,
    ny: usize,
    cells: Vec<Vec<u32>>,
}

impl Grid {
    fn new(min: P2, max: P2, items: usize) -> Grid {
        let w = (max[0] - min[0]).max(1e-9);
        let h = (max[1] - min[1]).max(1e-9);
        let target = (items.max(1) as f64).sqrt().clamp(1.0, 256.0);
        let cell = (w.max(h) / target).max(1e-9);
        let nx = ((w / cell).ceil() as usize).clamp(1, 512);
        let ny = ((h / cell).ceil() as usize).clamp(1, 512);
        Grid {
            min,
            cell,
            nx,
            ny,
            cells: vec![Vec::new(); nx * ny],
        }
    }

    fn range(&self, lo: P2, hi: P2) -> (usize, usize, usize, usize) {
        let at = |x: f64, o: f64, n: usize| -> usize {
            (((x - o) / self.cell).floor().max(0.0) as usize).min(n - 1)
        };
        (
            at(lo[0], self.min[0], self.nx),
            at(hi[0], self.min[0], self.nx),
            at(lo[1], self.min[1], self.ny),
            at(hi[1], self.min[1], self.ny),
        )
    }

    fn insert(&mut self, id: u32, lo: P2, hi: P2) {
        let (x0, x1, y0, y1) = self.range(lo, hi);
        for y in y0..=y1 {
            for x in x0..=x1 {
                self.cells[y * self.nx + x].push(id);
            }
        }
    }

    /// Items near a box, each once (`seen` is scratch, as long as the items).
    fn query(&self, lo: P2, hi: P2, seen: &mut [u32], stamp: u32, out: &mut Vec<u32>) {
        out.clear();
        let (x0, x1, y0, y1) = self.range(lo, hi);
        for y in y0..=y1 {
            for x in x0..=x1 {
                for &id in &self.cells[y * self.nx + x] {
                    if seen[id as usize] != stamp {
                        seen[id as usize] = stamp;
                        out.push(id);
                    }
                }
            }
        }
    }
}

fn bbox2(points: impl IntoIterator<Item = P2>) -> (P2, P2) {
    let mut lo = [f64::INFINITY; 2];
    let mut hi = [f64::NEG_INFINITY; 2];
    for p in points {
        for k in 0..2 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    (lo, hi)
}

/// A projected segment with depths at its ends, from contour line `line`.
#[derive(Clone, Copy)]
struct Seg {
    a: P2,
    b: P2,
    da: f64,
    db: f64,
}

/// Clips a 3D segment to the half-space `depth <= cut`.
fn clip_seg(a: P3, b: P3, eye: P3, cut: Option<f64>) -> Option<(P3, P3)> {
    let Some(cut) = cut else { return Some((a, b)) };
    let (da, db) = (dot(a, eye) - cut, dot(b, eye) - cut);
    match (da <= 0.0, db <= 0.0) {
        (true, true) => Some((a, b)),
        (false, false) => None,
        (ina, _) => {
            let t = da / (da - db);
            let m = [
                a[0] + (b[0] - a[0]) * t,
                a[1] + (b[1] - a[1]) * t,
                a[2] + (b[2] - a[2]) * t,
            ];
            if ina { Some((a, m)) } else { Some((m, b)) }
        }
    }
}

/// The circular arc a mesh edge from `a` to `b` with vertex normals `na`
/// and `nb` stands for (a cylinder's or a sphere's, between two of its
/// vertices): its point at angle `x` from `a` (of `phi` in all), or `None`
/// when the ends and normals describe no such arc.
struct Arc {
    a: P3,
    na: P3,
    nb: P3,
    phi: f64,
    sigma: f64,
    r: f64,
    d_orth: P3,
}

impl Arc {
    fn new(a: P3, b: P3, na: P3, nb: P3) -> Option<Arc> {
        let c = dot(na, nb).clamp(-1.0, 1.0);
        let phi = c.acos();
        if !(1e-6..=std::f64::consts::FRAC_PI_2).contains(&phi) {
            return None;
        }
        let d = sub3(b, a);
        let e2 = unit(sub3(nb, [na[0] * c, na[1] * c, na[2] * c]));
        let (d1, d2) = (dot(d, na), dot(d, e2));
        let d_in: P3 = std::array::from_fn(|k| na[k] * d1 + e2[k] * d2);
        let chord = dot(d_in, d_in).sqrt();
        let sigma = if dot(sub3(nb, na), d) >= 0.0 {
            1.0
        } else {
            -1.0
        };
        let r = chord / (2.0 * (phi / 2.0).sin());
        // The two ends must lie on one circle with these normals.
        let off: P3 = std::array::from_fn(|k| d_in[k] - (nb[k] - na[k]) * sigma * r);
        if chord <= 0.0 || dot(off, off).sqrt() > chord * 0.05 {
            return None;
        }
        Some(Arc {
            a,
            na,
            nb,
            phi,
            sigma,
            r,
            d_orth: sub3(d, d_in),
        })
    }

    fn normal(&self, x: f64) -> P3 {
        let s = self.phi.sin();
        let (wa, wb) = ((self.phi - x).sin() / s, x.sin() / s);
        std::array::from_fn(|k| self.na[k] * wa + self.nb[k] * wb)
    }

    fn at(&self, x: f64) -> P3 {
        let n = self.normal(x);
        let t = x / self.phi;
        std::array::from_fn(|k| {
            self.a[k] - self.sigma * self.r * self.na[k]
                + self.sigma * self.r * n[k]
                + t * self.d_orth[k]
        })
    }
}

fn lerp3(a: P3, b: P3, t: f64) -> P3 {
    std::array::from_fn(|k| a[k] + (b[k] - a[k]) * t)
}

/// Where normal·eye is zero on the mesh edge from `a` to `b` (normals `na`
/// and `nb` facing opposite ways): on the arc the edge stands for, or along
/// the straight edge when it stands for none.
fn on_arc(a: P3, b: P3, na: P3, nb: P3, eye: P3) -> P3 {
    let (mut fa, mut fb) = (dot(na, eye), dot(nb, eye));
    let Some(arc) = Arc::new(a, b, na, nb) else {
        return lerp3(a, b, (fa / (fa - fb)).clamp(0.0, 1.0));
    };
    if fa < 0.0 {
        fa = -fa;
        fb = -fb;
    }
    let c = arc.phi.cos();
    let x = (arc.phi.sin() * fa).atan2(c * fa - fb);
    if !(0.0..=arc.phi).contains(&x) {
        return lerp3(a, b, (fa / (fa - fb)).clamp(0.0, 1.0));
    }
    arc.at(x)
}

/// Where the mesh edge from `a` to `b` crosses depth `cut` along `eye`: on
/// the arc it stands for when it stands for one.
fn cut_point(a: P3, b: P3, na: Option<P3>, nb: Option<P3>, eye: P3, cut: f64) -> P3 {
    let (da, db) = (dot(a, eye) - cut, dot(b, eye) - cut);
    let straight = lerp3(a, b, da / (da - db));
    let (Some(na), Some(nb)) = (na, nb) else {
        return straight;
    };
    let Some(arc) = Arc::new(a, b, na, nb) else {
        return straight;
    };
    let depth = |x: f64| dot(arc.at(x), eye) - cut;
    let (mut lo, mut hi) = (0.0, arc.phi);
    let (dlo, dhi) = (depth(lo), depth(hi));
    if (dlo > 0.0) == (dhi > 0.0) {
        return straight;
    }
    for _ in 0..60 {
        let mid = (lo + hi) / 2.0;
        if (depth(mid) > 0.0) == (dlo > 0.0) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    arc.at((lo + hi) / 2.0)
}

/// Projects the built solids for one view: every line, visible or hidden,
/// or for a section (`cut`) the cut's outline and the visible lines behind.
#[must_use]
pub fn project(built: &Built, name: &str, title: &str, eye: P3, up: P3, cut: Option<f64>) -> View {
    let u = unit(cross(up, eye));
    let v = cross(eye, u);
    let proj = |p: P3| -> (P2, f64) { ([dot(p, u), dot(p, v)], dot(p, eye)) };

    // The model's size sets the tolerances.
    let mut lo3 = [f64::INFINITY; 3];
    let mut hi3 = [f64::NEG_INFINITY; 3];
    for s in &built.solids {
        for p in &s.mesh.positions {
            for (k, c) in [p.x, p.y, p.z].into_iter().enumerate() {
                lo3[k] = lo3[k].min(c);
                hi3[k] = hi3[k].max(c);
            }
        }
    }
    let size = dot(sub3(hi3, lo3), sub3(hi3, lo3)).sqrt().max(1e-6);
    let tol = size * 1e-7;
    let depth_eps = size * 1e-6;
    let same = size * 1e-5;

    // Triangles (clipped for a section) and the cut's segments.
    let mut tris: Vec<Tri> = Vec::new();
    let mut cut_segs: Vec<(P2, P2)> = Vec::new();
    let mut contours: Vec<(Vec<Seg>, bool, Kind)> = Vec::new();
    for s in &built.solids {
        let m = &s.mesh;
        let pos = |i: u32| {
            let q = m.positions[i as usize];
            [q.x, q.y, q.z]
        };
        // Vertex normals by position and B-rep face: where two faces meet
        // with the same normals the join is smooth (a seam of a cylinder
        // made in two halves, a tangent fillet) and isn't drawn.
        let q = size * 1e-6;
        let key = |p: P3| -> (i64, i64, i64) {
            (
                (p[0] / q).round() as i64,
                (p[1] / q).round() as i64,
                (p[2] / q).round() as i64,
            )
        };
        let has_normals = m.normals.len() == m.positions.len();
        let normal_at = |i: u32| -> P3 {
            let n = m.normals[i as usize];
            unit([n.x, n.y, n.z])
        };
        let mut by_face: HashMap<((i64, i64, i64), u32), P3> = HashMap::new();
        if has_normals {
            for (t, tri) in m.triangles.iter().enumerate() {
                let face = m.tri_face.get(t).copied().unwrap_or(0);
                for &i in tri {
                    by_face
                        .entry((key(pos(i)), face))
                        .or_insert_with(|| normal_at(i));
                }
            }
        }
        // Faces at each vertex position, and consecutive points of each
        // B-rep edge: a straight mesh edge between two of those stands for
        // a piece of the curve, whose normals come from the curved face.
        let mut faces_at: HashMap<(i64, i64, i64), Vec<(u32, P3)>> = HashMap::new();
        for (&(k, f), n) in &by_face {
            faces_at.entry(k).or_default().push((f, *n));
        }
        for list in faces_at.values_mut() {
            list.sort_by_key(|x| x.0);
        }
        let mut along_edges: std::collections::HashSet<((i64, i64, i64), (i64, i64, i64))> =
            std::collections::HashSet::new();
        for e in &m.edges {
            for w in e.windows(2) {
                let (ka, kb) = (key([w[0].x, w[0].y, w[0].z]), key([w[1].x, w[1].y, w[1].z]));
                along_edges.insert((ka.min(kb), ka.max(kb)));
            }
        }
        let curve_normals = |a: P3, b: P3| -> Option<(P3, P3)> {
            let (ka, kb) = (key(a), key(b));
            if !along_edges.contains(&(ka.min(kb), ka.max(kb))) {
                return None;
            }
            let (la, lb) = (faces_at.get(&ka)?, faces_at.get(&kb)?);
            la.iter().find_map(|(f, na)| {
                let nb = lb.iter().find(|(g, _)| g == f)?.1;
                (dot(*na, nb) < 0.999_999).then_some((*na, nb))
            })
        };
        // The normals a mesh edge's ends have on the curve it stands for:
        // its own face's when curved, else the B-rep curve's it runs along.
        let edge_normals = |a: P3, b: P3, ia: u32, ib: u32| -> (Option<P3>, Option<P3>) {
            if has_normals {
                let (na, nb) = (normal_at(ia), normal_at(ib));
                if dot(na, nb) < 0.999_999 {
                    return (Some(na), Some(nb));
                }
            }
            curve_normals(a, b).map_or((None, None), |(na, nb)| (Some(na), Some(nb)))
        };
        for t in &m.triangles {
            let w = t.map(pos);
            let mut poly: Vec<P3> = w.to_vec();
            if let Some(c) = cut {
                let d: Vec<f64> = w.iter().map(|p| dot(*p, eye) - c).collect();
                if d.iter().all(|x| *x > 0.0) {
                    continue;
                }
                if d.iter().any(|x| *x > 0.0) {
                    let mut out = Vec::new();
                    let mut crossing = Vec::new();
                    for i in 0..3 {
                        let j = (i + 1) % 3;
                        if d[i] <= 0.0 {
                            out.push(w[i]);
                        }
                        if (d[i] <= 0.0) != (d[j] <= 0.0) {
                            let k = d[i] / (d[i] - d[j]);
                            let p = [
                                w[i][0] + (w[j][0] - w[i][0]) * k,
                                w[i][1] + (w[j][1] - w[i][1]) * k,
                                w[i][2] + (w[j][2] - w[i][2]) * k,
                            ];
                            out.push(p);
                            let (na, nb) = edge_normals(w[i], w[j], t[i], t[j]);
                            let on = cut_point(w[i], w[j], na, nb, eye, c);
                            crossing.push(proj(on).0);
                        }
                    }
                    if let [a, b] = crossing[..]
                        && len2(sub(a, b)) > tol
                    {
                        cut_segs.push((a, b));
                    }
                    poly = out;
                }
            }
            for k in 1..poly.len().saturating_sub(1) {
                let corners = [poly[0], poly[k], poly[k + 1]];
                let pd = corners.map(proj);
                let p = pd.map(|x| x.0);
                let area = cross2(sub(p[1], p[0]), sub(p[2], p[0]));
                if area.abs() > tol * tol {
                    tris.push(Tri {
                        p,
                        d: pd.map(|x| x.1),
                        area,
                    });
                }
            }
        }

        let smooth = |i: usize| -> bool {
            let (Some(e), Some(faces)) = (m.edges.get(i), m.edge_faces.get(i)) else {
                return false;
            };
            // One face on both sides: a seam closing a curved face.
            let (f1, f2) = match faces[..] {
                [f] => return e.len() >= 2 && has_normals && f == f,
                [f1, f2] => (f1, f2),
                _ => return false,
            };
            !e.is_empty()
                && e.iter().all(|p| {
                    let k = key([p.x, p.y, p.z]);
                    match (by_face.get(&(k, f1)), by_face.get(&(k, f2))) {
                        (Some(a), Some(b)) => dot(*a, *b) > 0.9998,
                        _ => false,
                    }
                })
        };

        // B-rep edges, seams and tangent joins left out.
        for (i, e) in m.edges.iter().enumerate() {
            if !m.shaded_edge_visible(i) || smooth(i) {
                continue;
            }
            let mut run: Vec<Seg> = Vec::new();
            for w in e.windows(2) {
                let (a, b) = ([w[0].x, w[0].y, w[0].z], [w[1].x, w[1].y, w[1].z]);
                let clipped = match cut {
                    Some(c) if (dot(a, eye) > c) != (dot(b, eye) > c) => {
                        let (na, nb) = curve_normals(a, b).unzip();
                        let on = cut_point(a, b, na, nb, eye, c);
                        Some(if dot(a, eye) <= c { (a, on) } else { (on, b) })
                    }
                    _ => clip_seg(a, b, eye, cut),
                };
                let Some((a, b)) = clipped else {
                    if !run.is_empty() {
                        contours.push((std::mem::take(&mut run), false, Kind::Visible));
                    }
                    continue;
                };
                let ((pa, da), (pb, db)) = (proj(a), proj(b));
                run.push(Seg {
                    a: pa,
                    b: pb,
                    da,
                    db,
                });
            }
            if !run.is_empty() {
                contours.push((run, false, Kind::Visible));
            }
        }

        // Silhouettes: where a curved face turns from facing the viewer to
        // facing away. Inside each triangle the zero of normal·eye is found
        // on the two edges whose ends face opposite ways, on the circular
        // arc through the edge's ends with their normals (so a cylinder's
        // silhouette lies on the cylinder, not on the chord between two
        // vertices).
        if has_normals {
            for t in &m.triangles {
                let f = t.map(|i| dot(normal_at(i), eye));
                if f.iter().all(|x| x.abs() < 1e-3) {
                    continue; // seen edge on: its outline is its edges
                }
                let mut ends: Vec<P3> = Vec::new();
                for (i, j) in [(0, 1), (1, 2), (2, 0)] {
                    if (f[i] >= 0.0) != (f[j] >= 0.0) {
                        ends.push(on_arc(
                            pos(t[i]),
                            pos(t[j]),
                            normal_at(t[i]),
                            normal_at(t[j]),
                            eye,
                        ));
                    }
                }
                if let [a, b] = ends[..]
                    && let Some((a, b)) = clip_seg(a, b, eye, cut)
                {
                    let ((pa, da), (pb, db)) = (proj(a), proj(b));
                    if len2(sub(pa, pb)) > tol {
                        contours.push((
                            vec![Seg {
                                a: pa,
                                b: pb,
                                da,
                                db,
                            }],
                            true,
                            Kind::Visible,
                        ));
                    }
                }
            }
        }
    }
    for (a, b) in &cut_segs {
        let d = cut.unwrap_or(0.0);
        contours.push((
            vec![Seg {
                a: *a,
                b: *b,
                da: d,
                db: d,
            }],
            false,
            Kind::Section,
        ));
    }

    // Grids: triangles for the cover test, contour segments for crossings.
    let (gmin, gmax) = bbox2(
        tris.iter().flat_map(|t| t.p).chain(
            contours
                .iter()
                .flat_map(|c| c.0.iter().flat_map(|s| [s.a, s.b])),
        ),
    );
    let mut tri_grid = Grid::new(gmin, gmax, tris.len());
    for (i, t) in tris.iter().enumerate() {
        let (lo, hi) = bbox2(t.p);
        tri_grid.insert(i as u32, lo, hi);
    }
    let all_segs: Vec<Seg> = contours.iter().flat_map(|c| c.0.iter().copied()).collect();
    let mut seg_grid = Grid::new(gmin, gmax, all_segs.len());
    for (i, s) in all_segs.iter().enumerate() {
        let (lo, hi) = bbox2([s.a, s.b]);
        seg_grid.insert(i as u32, lo, hi);
    }
    let mut seen_t = vec![0u32; tris.len()];
    let mut seen_s = vec![0u32; all_segs.len()];
    let mut stamp = 0u32;
    let mut found = Vec::new();

    // Behind the cut's face: inside its outline and not on it.
    let inside_cut = |m: P2| -> bool {
        let near = cut_segs.iter().any(|(a, b)| {
            let d = sub(*b, *a);
            let t = (dot2(sub(m, *a), d) / dot2(d, d).max(1e-300)).clamp(0.0, 1.0);
            len2(sub(m, lerp(*a, *b, t))) <= same
        });
        if near {
            return false;
        }
        // A ray in no particular direction, so it doesn't run along the
        // outline's straight edges.
        let dir = [0.954_3, 0.298_8];
        let mut inside = false;
        for (a, b) in &cut_segs {
            let e = sub(*b, *a);
            let den = cross2(dir, e);
            if den.abs() < 1e-300 {
                continue;
            }
            let w = sub(*a, m);
            let s = cross2(w, e) / den;
            let r = cross2(w, dir) / den;
            if s > 0.0 && (0.0..1.0).contains(&r) {
                inside = !inside;
            }
        }
        inside
    };

    // Each contour split where it passes behind another, each piece tested.
    let mut pieces: Vec<(Kind, bool, usize, Vec<(P2, P2)>)> = Vec::new();
    for (ci, (segs, silhouette, kind)) in contours.iter().enumerate() {
        let mut run: Vec<(P2, P2)> = Vec::new();
        let mut run_hidden = false;
        let mut flush = |run: &mut Vec<(P2, P2)>, hidden: bool| {
            if !run.is_empty() {
                let k = if *kind == Kind::Section {
                    Kind::Section
                } else if hidden {
                    Kind::Hidden
                } else {
                    Kind::Visible
                };
                pieces.push((k, *silhouette, ci, std::mem::take(run)));
            }
        };
        for s in segs {
            let d = sub(s.b, s.a);
            let l = len2(d);
            if l <= tol {
                continue;
            }
            let mut ts = vec![0.0, 1.0];
            if *kind != Kind::Section {
                stamp = stamp.wrapping_add(1);
                let (lo, hi) = bbox2([s.a, s.b]);
                seg_grid.query(lo, hi, &mut seen_s, stamp, &mut found);
                for &j in &found {
                    let o = all_segs[j as usize];
                    let e = sub(o.b, o.a);
                    let den = cross2(d, e);
                    if den.abs() <= 1e-12 * l * len2(e) {
                        continue;
                    }
                    let w = sub(o.a, s.a);
                    let t = cross2(w, e) / den;
                    let r = cross2(w, d) / den;
                    if t > 1e-9 && t < 1.0 - 1e-9 && (-1e-9..=1.0 + 1e-9).contains(&r) {
                        ts.push(t);
                    }
                }
                ts.sort_by(f64::total_cmp);
                ts.dedup_by(|a, b| (*a - *b).abs() * l <= tol);
            }
            for w in ts.windows(2) {
                let (t0, t1) = (w[0], w[1]);
                let (p0, p1) = (lerp(s.a, s.b, t0), lerp(s.a, s.b, t1));
                let hidden = if *kind == Kind::Section {
                    false
                } else {
                    let tm = (t0 + t1) / 2.0;
                    let m = lerp(s.a, s.b, tm);
                    let dm = s.da + (s.db - s.da) * tm;
                    let capped = cut.is_some_and(|c| dm < c - depth_eps && inside_cut(m));
                    capped || {
                        stamp = stamp.wrapping_add(1);
                        tri_grid.query(m, m, &mut seen_t, stamp, &mut found);
                        found.iter().any(|&i| {
                            let t = &tris[i as usize];
                            let l0 = cross2(sub(t.p[1], m), sub(t.p[2], m)) / t.area;
                            let l1 = cross2(sub(t.p[2], m), sub(t.p[0], m)) / t.area;
                            let l2 = 1.0 - l0 - l1;
                            let inside = l0 > 1e-9 && l1 > 1e-9 && l2 > 1e-9;
                            inside && l0 * t.d[0] + l1 * t.d[1] + l2 * t.d[2] > dm + depth_eps
                        })
                    }
                };
                if hidden != run_hidden && !run.is_empty() {
                    flush(&mut run, run_hidden);
                }
                run_hidden = hidden;
                run.push((p0, p1));
            }
        }
        flush(&mut run, run_hidden);
    }

    // Where lines fall on each other: section over visible over hidden.
    let rank = |k: Kind| match k {
        Kind::Section => 0,
        Kind::Visible => 1,
        _ => 2,
    };
    let length = |p: &Vec<(P2, P2)>| p.iter().map(|(a, b)| len2(sub(*b, *a))).sum::<f64>();
    let mut order: Vec<usize> = (0..pieces.len()).collect();
    order.sort_by(|&a, &b| {
        rank(pieces[a].0)
            .cmp(&rank(pieces[b].0))
            .then(length(&pieces[b].3).total_cmp(&length(&pieces[a].3)))
            .then(a.cmp(&b))
    });
    let mut kept: Vec<(P2, P2)> = Vec::new();
    let mut kept_grid = Grid::new(gmin, gmax, all_segs.len().max(16));
    let mut seen_k: Vec<u32> = Vec::new();
    let mut lines: Vec<Line> = Vec::new();
    if cut.is_some() {
        // A section shows no hidden lines.
        order.retain(|&i| pieces[i].0 != Kind::Hidden);
    }
    for i in order {
        let (kind, silhouette, _, segs) = &pieces[i];
        let mut current: Vec<P2> = Vec::new();
        let mut out: Vec<Vec<P2>> = Vec::new();
        for (a, b) in segs {
            let d = sub(*b, *a);
            let l = len2(d);
            let dir = mul(d, 1.0 / l);
            // Parts of a..b already drawn by a collinear kept segment.
            let mut covered: Vec<(f64, f64)> = Vec::new();
            stamp = stamp.wrapping_add(1);
            seen_k.resize(kept.len(), 0);
            let (lo, hi) = bbox2([*a, *b]);
            kept_grid.query(
                [lo[0] - same, lo[1] - same],
                [hi[0] + same, hi[1] + same],
                &mut seen_k,
                stamp,
                &mut found,
            );
            for &j in &found {
                let (c, e) = kept[j as usize];
                if cross2(dir, sub(c, *a)).abs() > same || cross2(dir, sub(e, *a)).abs() > same {
                    continue;
                }
                let (s0, s1) = (dot2(sub(c, *a), dir) / l, dot2(sub(e, *a), dir) / l);
                let (s0, s1) = (s0.min(s1), s0.max(s1));
                if s1 > 0.0 && s0 < 1.0 {
                    covered.push((s0.max(0.0), s1.min(1.0)));
                }
            }
            covered.sort_by(|x, y| x.0.total_cmp(&y.0));
            let mut free: Vec<(f64, f64)> = Vec::new();
            let mut at = 0.0;
            for (s0, s1) in covered {
                if s0 > at {
                    free.push((at, s0));
                }
                at = at.max(s1);
            }
            if at < 1.0 {
                free.push((at, 1.0));
            }
            for (s0, s1) in free {
                if (s1 - s0) * l <= same {
                    if !current.is_empty() {
                        out.push(std::mem::take(&mut current));
                    }
                    continue;
                }
                let (p, q) = (lerp(*a, *b, s0), lerp(*a, *b, s1));
                match current.last() {
                    Some(last) if len2(sub(*last, p)) <= same => current.push(q),
                    _ => {
                        if !current.is_empty() {
                            out.push(std::mem::take(&mut current));
                        }
                        current = vec![p, q];
                    }
                }
            }
            if free_is_partial(&current, *b, same) {
                out.push(std::mem::take(&mut current));
            }
        }
        if !current.is_empty() {
            out.push(current);
        }
        for points in out {
            for w in points.windows(2) {
                let id = kept.len() as u32;
                kept.push((w[0], w[1]));
                let (lo, hi) = bbox2([w[0], w[1]]);
                kept_grid.insert(id, lo, hi);
            }
            lines.push(Line {
                kind: *kind,
                points,
                silhouette: *silhouette,
            });
        }
    }

    // Silhouette pieces and the cut's segments chain into whole lines.
    lines = chain(lines, same);
    for line in &mut lines {
        simplify(&mut line.points, same);
    }
    lines.retain(|l| {
        l.points.len() >= 2
            && l.points
                .windows(2)
                .map(|w| len2(sub(w[1], w[0])))
                .sum::<f64>()
                > same
    });
    lines.sort_by(|a, b| {
        a.kind
            .cmp(&b.kind)
            .then(a.points[0][0].total_cmp(&b.points[0][0]))
            .then(a.points[0][1].total_cmp(&b.points[0][1]))
    });
    let (min, max) = bbox2(lines.iter().flat_map(|l| l.points.iter().copied()));
    View {
        name: name.to_owned(),
        title: title.to_owned(),
        eye,
        u,
        v,
        cut,
        lines,
        min,
        max,
    }
}

/// Whether the polyline being built stopped short of the segment's end (a
/// covered tail), so it must not run on into the next segment.
fn free_is_partial(current: &[P2], end: P2, same: f64) -> bool {
    current
        .last()
        .is_some_and(|last| len2(sub(*last, end)) > same)
}

/// Joins silhouette pieces end to end, and the section's cut segments, into
/// whole lines.
fn chain(lines: Vec<Line>, same: f64) -> Vec<Line> {
    let (mut joinable, mut out): (Vec<Line>, Vec<Line>) = lines
        .into_iter()
        .partition(|l| l.silhouette || l.kind == Kind::Section);
    let key = |p: P2| ((p[0] / same).round() as i64, (p[1] / same).round() as i64);
    loop {
        let mut joined = false;
        let mut i = 0;
        while i < joinable.len() {
            let mut j = i + 1;
            while j < joinable.len() {
                let (a, b) = (&joinable[i], &joinable[j]);
                if a.kind != b.kind || a.silhouette != b.silhouette {
                    j += 1;
                    continue;
                }
                let (a0, a1) = (
                    key(a.points[0]),
                    key(*a.points.last().unwrap_or(&a.points[0])),
                );
                let (b0, b1) = (
                    key(b.points[0]),
                    key(*b.points.last().unwrap_or(&b.points[0])),
                );
                let closed = a0 == a1 && a.points.len() > 2;
                let mut other = joinable[j].points.clone();
                let merged = if closed {
                    None
                } else if a1 == b0 {
                    Some((false, false))
                } else if a1 == b1 {
                    other.reverse();
                    Some((false, false))
                } else if a0 == b1 {
                    Some((true, false))
                } else if a0 == b0 {
                    other.reverse();
                    Some((true, false))
                } else {
                    None
                };
                if let Some((front, _)) = merged {
                    joinable.remove(j);
                    let a = &mut joinable[i].points;
                    if front {
                        other.pop();
                        other.extend(a.iter().copied());
                        *a = other;
                    } else {
                        a.extend(other.into_iter().skip(1));
                    }
                    joined = true;
                    continue;
                }
                j += 1;
            }
            i += 1;
        }
        if !joined {
            break;
        }
    }
    out.extend(joinable);
    out
}

/// Drops points in the middle of straight runs.
fn simplify(points: &mut Vec<P2>, same: f64) {
    let mut out: Vec<P2> = Vec::with_capacity(points.len());
    for &p in points.iter() {
        if let Some(&last) = out.last()
            && len2(sub(p, last)) <= same
        {
            continue;
        }
        while out.len() >= 2 {
            let (a, b) = (out[out.len() - 2], out[out.len() - 1]);
            let d = sub(p, a);
            let l = len2(d);
            if l > 0.0
                && (cross2(d, sub(b, a)) / l).abs() <= same
                && dot2(sub(b, a), d) > 0.0
                && dot2(sub(p, b), d) > 0.0
            {
                out.pop();
            } else {
                break;
            }
        }
        out.push(p);
    }
    // A closed loop starting in the middle of a side starts at a corner.
    while out.len() >= 4 && len2(sub(out[0], out[out.len() - 1])) <= same {
        let (a, b, c) = (out[out.len() - 2], out[0], out[1]);
        let d = sub(c, a);
        let l = len2(d);
        if l > 0.0 && (cross2(d, sub(b, a)) / l).abs() <= same && dot2(sub(b, a), d) > 0.0 {
            out.remove(0);
            out.pop();
            out.push(out[0]);
        } else {
            break;
        }
    }
    *points = out;
}

/// A circle (or a whole ring of arcs) a view shows: its centre and radius.
#[must_use]
pub fn circles(view: &View) -> Vec<(P2, f64)> {
    let mut arcs: Vec<(P2, f64, f64)> = Vec::new();
    for l in view
        .lines
        .iter()
        .filter(|l| l.kind == Kind::Visible && !l.silhouette)
    {
        let p = &l.points;
        if p.len() < 4 {
            continue;
        }
        let (a, b, c) = (p[0], p[p.len() / 2], p[p.len() - 1]);
        let (a, b, c) = if len2(sub(a, c)) < 1e-9 {
            (p[0], p[p.len() / 3], p[2 * p.len() / 3])
        } else {
            (a, b, c)
        };
        let Some((centre, r)) = circle_through(a, b, c) else {
            continue;
        };
        if p.iter()
            .all(|q| (len2(sub(*q, centre)) - r).abs() <= r * 0.01)
        {
            let sweep: f64 = p
                .windows(2)
                .map(|w| {
                    let (x, y) = (sub(w[0], centre), sub(w[1], centre));
                    cross2(x, y).atan2(dot2(x, y)).abs()
                })
                .sum();
            arcs.push((centre, r, sweep));
        }
    }
    // Arcs of one circle add up to a whole turn.
    let mut out: Vec<(P2, f64, f64)> = Vec::new();
    for (c, r, sweep) in arcs {
        match out
            .iter_mut()
            .find(|(c2, r2, _)| len2(sub(c, *c2)) <= r * 0.01 && (r - *r2).abs() <= r * 0.01)
        {
            Some(found) => found.2 += sweep,
            None => out.push((c, r, sweep)),
        }
    }
    out.into_iter()
        .filter(|(_, _, sweep)| *sweep > std::f64::consts::TAU * 0.98)
        .map(|(c, r, _)| (c, r))
        .collect()
}

fn circle_through(a: P2, b: P2, c: P2) -> Option<(P2, f64)> {
    let d = 2.0 * (a[0] * (b[1] - c[1]) + b[0] * (c[1] - a[1]) + c[0] * (a[1] - b[1]));
    if d.abs() < 1e-12 {
        return None;
    }
    let (a2, b2, c2) = (dot2(a, a), dot2(b, b), dot2(c, c));
    let x = (a2 * (b[1] - c[1]) + b2 * (c[1] - a[1]) + c2 * (a[1] - b[1])) / d;
    let y = (a2 * (c[0] - b[0]) + b2 * (a[0] - c[0]) + c2 * (b[0] - a[0])) / d;
    let centre = [x, y];
    Some((centre, len2(sub(a, centre))))
}

// ---------------------------------------------------------------------------
// Options and layout.

/// A section asked for: the view it is seen like and where it cuts.
#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    pub view: String,
    pub at: Option<f64>,
}

/// A dimension between two named flat faces of a part, in a view.
#[derive(Clone, Debug, PartialEq)]
pub struct FaceDimension {
    pub view: String,
    /// The two faces' centres and the first one's normal (world mm).
    pub a: P3,
    pub b: P3,
    pub normal: P3,
    pub label: String,
}

/// What to draw.
#[derive(Clone, Debug)]
pub struct Options {
    pub views: Vec<String>,
    pub sections: Vec<Section>,
    /// `third` or `first`.
    pub angle: String,
    /// `A4`, `A3` or `A2`.
    pub sheet: String,
    pub title: String,
    pub subtitle: String,
    pub hidden: bool,
    pub faces: Vec<FaceDimension>,
    /// Diameter callouts for the circles the views show.
    pub callouts: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            views: vec!["front".into(), "top".into(), "right".into(), "iso".into()],
            sections: Vec::new(),
            angle: "third".into(),
            sheet: "A3".into(),
            title: String::new(),
            subtitle: String::new(),
            hidden: true,
            faces: Vec::new(),
            callouts: true,
        }
    }
}

/// A JSON argument given as JSON or as text holding JSON.
fn json_value(v: &Value) -> Result<Value, String> {
    match v {
        Value::String(text) if text.trim_start().starts_with(['{', '[']) => {
            serde_json::from_str(text).map_err(|e| format!("not valid JSON ({e})"))
        }
        other => Ok(other.clone()),
    }
}

fn list_of(v: &Value) -> Result<Vec<Value>, String> {
    Ok(match json_value(v)? {
        Value::Null => Vec::new(),
        Value::Array(items) => items,
        Value::String(s) => s
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| json!(s))
            .collect(),
        other => vec![other],
    })
}

impl Options {
    /// Options from a tool's arguments: `views`, `sections`, `angle`,
    /// `sheet`, `title`, `hidden`. Named-face `dimensions` are resolved by
    /// [`of_document`].
    ///
    /// # Errors
    /// An unknown view, sheet or projection angle, or a malformed section.
    pub fn from_args(args: &Value) -> Result<Options, String> {
        let mut o = Options::default();
        let views = list_of(&args["views"]).map_err(|e| format!("views: {e}"))?;
        if !views.is_empty() {
            o.views = Vec::new();
            for v in views {
                let name = v.as_str().unwrap_or_default().to_lowercase();
                if !VIEWS.contains(&name.as_str()) {
                    return Err(format!("The views are: {}.", VIEWS.join(", ")));
                }
                if !o.views.contains(&name) {
                    o.views.push(name);
                }
            }
        }
        for s in list_of(&args["sections"]).map_err(|e| format!("sections: {e}"))? {
            let (view, at) = match &s {
                Value::String(text) => match text.split_once('@') {
                    Some((v, at)) => (
                        v.trim().to_lowercase(),
                        Some(at.trim().parse::<f64>().map_err(|_| {
                            format!("A section is a view and where it cuts, like front@0 ({text}).")
                        })?),
                    ),
                    None => (text.trim().to_lowercase(), None),
                },
                Value::Object(_) => (
                    s["view"].as_str().unwrap_or("front").to_lowercase(),
                    s["at"].as_f64(),
                ),
                _ => return Err("A section is {\"view\": \"front\", \"at\": 0}.".into()),
            };
            if view == "iso" || direction(&view).is_none() {
                return Err(format!(
                    "A section is seen like one of: {}.",
                    VIEWS[..6].join(", ")
                ));
            }
            o.sections.push(Section { view, at });
        }
        if let Some(angle) = args["angle"].as_str() {
            let angle = angle.to_lowercase();
            let angle = angle.trim_end_matches("-angle").trim_end_matches(" angle");
            if angle != "third" && angle != "first" {
                return Err("The projection angle is third or first.".into());
            }
            angle.clone_into(&mut o.angle);
        }
        if let Some(sheet) = args["sheet"].as_str() {
            let sheet = sheet.to_uppercase();
            if sheet_size(&sheet).is_none() {
                return Err("The sheet is A4, A3 or A2.".into());
            }
            o.sheet = sheet;
        }
        if let Some(title) = args["title"].as_str() {
            o.title = title.chars().take(80).collect();
        }
        if let Some(subtitle) = args["subtitle"].as_str() {
            o.subtitle = subtitle.chars().take(80).collect();
        }
        if let Some(hidden) = args["hidden"].as_bool() {
            o.hidden = hidden;
        }
        if o.views.is_empty() && o.sections.is_empty() {
            return Err("Ask for at least one view or section.".into());
        }
        Ok(o)
    }
}

fn sheet_size(name: &str) -> Option<P2> {
    match name {
        "A4" => Some([297.0, 210.0]),
        "A3" => Some([420.0, 297.0]),
        "A2" => Some([594.0, 420.0]),
        _ => None,
    }
}

/// The standard scales, largest first (sheet mm per model mm).
const SCALES: [(f64, &str); 17] = [
    (20.0, "20:1"),
    (10.0, "10:1"),
    (5.0, "5:1"),
    (2.0, "2:1"),
    (1.0, "1:1"),
    (0.5, "1:2"),
    (0.2, "1:5"),
    (0.1, "1:10"),
    (0.05, "1:20"),
    (0.02, "1:50"),
    (0.01, "1:100"),
    (0.005, "1:200"),
    (0.002, "1:500"),
    (0.001, "1:1000"),
    (0.0005, "1:2000"),
    (0.0002, "1:5000"),
    (0.0001, "1:10000"),
];

/// The scale's label, like `1:2`.
#[must_use]
pub fn scale_label(scale: f64) -> String {
    SCALES
        .iter()
        .find(|(s, _)| (s - scale).abs() < 1e-12)
        .map_or_else(|| format!("{scale}:1"), |(_, l)| (*l).to_owned())
}

/// Where each view sits: (row, column), rows counted down from the top.
fn cell(name: &str, first: bool) -> (i32, i32) {
    let third = match name {
        "top" => (0, 1),
        "front" => (1, 1),
        "right" => (1, 2),
        "left" => (1, 0),
        "bottom" => (2, 1),
        "back" => (1, 3),
        _ => (0, 3), // iso: top right
    };
    if !first {
        return third;
    }
    match name {
        "top" => (2, 1),
        "bottom" => (0, 1),
        "right" => (1, 0),
        "left" => (1, 2),
        "back" => (1, 3),
        _ => (0, 3),
    }
}

const TEXT: f64 = 3.5;
const PAD: f64 = 13.0;
const FRAME_MARGIN: f64 = 10.0;
const BLOCK_W: f64 = 170.0;
const BLOCK_H: f64 = 28.0;

/// Draws a built document: its views projected and laid out on a sheet.
#[must_use]
pub fn draw(built: &Built, o: &Options) -> Drawing {
    let first = o.angle == "first";
    let mut views: Vec<View> = Vec::new();
    for name in &o.views {
        let Some((eye, up)) = direction(name) else {
            continue;
        };
        let mut v = project(built, name, &title_of(name), eye, up, None);
        if !o.hidden || name == "iso" {
            v.lines.retain(|l| l.kind != Kind::Hidden);
        }
        views.push(v);
    }
    let letters = "ABCDEFGHJKLMNPRSTUVWXYZ";
    for (i, s) in o.sections.iter().enumerate() {
        let Some((eye, up)) = direction(&s.view) else {
            continue;
        };
        let letter = letters.chars().nth(i % letters.len()).unwrap_or('A');
        // Through the middle of the model unless told.
        let at = s.at.unwrap_or_else(|| {
            let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
            for solid in &built.solids {
                for p in &solid.mesh.positions {
                    let d = dot([p.x, p.y, p.z], eye);
                    lo = lo.min(d);
                    hi = hi.max(d);
                }
            }
            if lo.is_finite() { (lo + hi) / 2.0 } else { 0.0 }
        });
        let v = project(
            built,
            &format!("section-{}", letter.to_ascii_lowercase()),
            &format!("Section {letter}\u{2013}{letter}"),
            eye,
            up,
            Some(at),
        );
        views.push(v);
    }
    views.retain(|v| !v.lines.is_empty());

    // The grid: named views in their cells, sections after them in the
    // front's row and then the row above.
    let mut cells: Vec<(i32, i32)> = Vec::new();
    let mut next_col = 3;
    for v in &views {
        if v.cut.is_some() {
            cells.push((1, next_col));
            next_col += 1;
        } else {
            cells.push(cell(&v.name, first));
        }
    }
    let mut rows: Vec<i32> = cells.iter().map(|c| c.0).collect();
    let mut cols: Vec<i32> = cells.iter().map(|c| c.1).collect();
    rows.sort_unstable();
    rows.dedup();
    cols.sort_unstable();
    cols.dedup();
    let row_of = |r: i32| rows.iter().position(|x| *x == r).unwrap_or(0);
    let col_of = |c: i32| cols.iter().position(|x| *x == c).unwrap_or(0);
    let mut widths = vec![0.0f64; cols.len()];
    let mut heights = vec![0.0f64; rows.len()];
    for (v, c) in views.iter().zip(&cells) {
        let (w, h) = (v.max[0] - v.min[0], v.max[1] - v.min[1]);
        widths[col_of(c.1)] = widths[col_of(c.1)].max(w);
        heights[row_of(c.0)] = heights[row_of(c.0)].max(h);
    }
    let size = sheet_size(&o.sheet).unwrap_or([420.0, 297.0]);
    let area_lo = [FRAME_MARGIN + 4.0, FRAME_MARGIN + BLOCK_H + 6.0];
    let area_hi = [size[0] - FRAME_MARGIN - 4.0, size[1] - FRAME_MARGIN - 4.0];
    let avail = sub(area_hi, area_lo);
    let pads_w = 2.0 * PAD * widths.len() as f64;
    let pads_h = 2.0 * PAD * heights.len() as f64;
    let fit_w = (avail[0] - pads_w) / widths.iter().sum::<f64>().max(1e-9);
    let fit_h = (avail[1] - pads_h) / heights.iter().sum::<f64>().max(1e-9);
    let fit = fit_w.min(fit_h);
    let scale = SCALES
        .iter()
        .map(|(s, _)| *s)
        .find(|s| *s <= fit)
        .unwrap_or(SCALES[SCALES.len() - 1].0);

    // Centre the grid in the drawing area; each view in its cell.
    let total_w: f64 = widths.iter().map(|w| w * scale + 2.0 * PAD).sum();
    let total_h: f64 = heights.iter().map(|h| h * scale + 2.0 * PAD).sum();
    let x0 = area_lo[0] + (avail[0] - total_w) / 2.0;
    let top = area_hi[1] - (avail[1] - total_h) / 2.0;
    let mut col_x = Vec::new();
    let mut x = x0;
    for w in &widths {
        col_x.push(x + PAD + w * scale / 2.0);
        x += w * scale + 2.0 * PAD;
    }
    let mut row_y = Vec::new();
    let mut y = top;
    for h in &heights {
        row_y.push(y - PAD - h * scale / 2.0);
        y -= h * scale + 2.0 * PAD;
    }
    let origins: Vec<P2> = views
        .iter()
        .zip(&cells)
        .map(|(v, c)| {
            let centre = [col_x[col_of(c.1)], row_y[row_of(c.0)]];
            let mid = mul(add(v.min, v.max), 0.5 * scale);
            sub(centre, mid)
        })
        .collect();

    let mut d = Drawing {
        title: if o.title.is_empty() {
            "Drawing".into()
        } else {
            o.title.clone()
        },
        subtitle: o.subtitle.clone(),
        sheet: o.sheet.clone(),
        size,
        scale,
        angle: o.angle.clone(),
        views,
        origins,
        dimensions: Vec::new(),
        marks: Vec::new(),
        below: Vec::new(),
        callouts: o.callouts,
    };
    d.below = vec![0.0; d.views.len()];
    d.mark_views();
    d.mark_cutting_planes();
    d.mark_dimensions(&o.faces);
    d.mark_titles();
    d.mark_frame();
    d
}

impl Drawing {
    fn to_sheet(&self, i: usize, p: P2) -> P2 {
        add(self.origins[i], mul(p, self.scale))
    }

    fn line(&mut self, kind: Kind, points: Vec<P2>) {
        self.marks.push(Mark::Line { kind, points });
    }

    fn text(&mut self, at: P2, size: f64, text: impl Into<String>, anchor: Anchor, angle: f64) {
        self.marks.push(Mark::Text {
            at,
            size,
            text: text.into(),
            anchor,
            angle,
        });
    }

    /// Each view's lines, hatching for sections, and its title under it.
    fn mark_views(&mut self) {
        for i in 0..self.views.len() {
            let lines = self.views[i].lines.clone();
            for l in &lines {
                let pts = l.points.iter().map(|p| self.to_sheet(i, *p)).collect();
                self.line(l.kind, pts);
            }
            if self.views[i].cut.is_some() {
                for (a, b) in hatch(&lines, 3.0 / self.scale) {
                    let (a, b) = (self.to_sheet(i, a), self.to_sheet(i, b));
                    self.line(Kind::Hatch, vec![a, b]);
                }
            }
        }
    }

    /// Each view's title under it, below its dimensions.
    fn mark_titles(&mut self) {
        for i in 0..self.views.len() {
            let v = &self.views[i];
            let foot = [(v.min[0] + v.max[0]) / 2.0, v.min[1]];
            let drop = self.below[i].max(2.0) + 4.0 + TEXT;
            let at = sub(self.to_sheet(i, foot), [0.0, drop]);
            let title = v.title.clone();
            self.text(at, TEXT, title, Anchor::Middle, 0.0);
        }
    }

    /// A chain line across each view a section's plane is square to, with
    /// the section's letter at both ends.
    fn mark_cutting_planes(&mut self) {
        for s in 0..self.views.len() {
            let Some(at) = self.views[s].cut else {
                continue;
            };
            let eye = self.views[s].eye;
            let letter = self.views[s].title.chars().nth(8).unwrap_or('A');
            let order = ["top", "front", "right", "left", "bottom", "back"];
            let Some(w) = order.iter().find_map(|name| {
                self.views.iter().position(|v| {
                    v.name == *name && v.cut.is_none() && dot(v.eye, eye).abs() <= 1e-6
                })
            }) else {
                continue;
            };
            let v = &self.views[w];
            // u·(u_w·eye) + v·(v_w·eye) = at.
            let (cu, cv) = (dot(v.u, eye), dot(v.v, eye));
            let margin = 3.0 / self.scale;
            let (a, b) = if cu.abs() > cv.abs() {
                let x = at / cu;
                ([x, v.min[1] - margin], [x, v.max[1] + margin])
            } else {
                let y = at / cv;
                ([v.min[0] - margin, y], [v.max[0] + margin, y])
            };
            let (a, b) = (self.to_sheet(w, a), self.to_sheet(w, b));
            self.line(Kind::Cutting, vec![a, b]);
            let along = mul(sub(b, a), 1.0 / len2(sub(b, a)).max(1e-9));
            // The letter beside each end, off the line.
            let aside = [along[1].abs(), along[0].abs()];
            for p in [a, b] {
                let q = add(p, mul(aside, 1.5));
                let q = if aside[0] > 0.5 {
                    sub(q, [0.0, TEXT / 2.0])
                } else {
                    q
                };
                self.text(q, TEXT, letter.to_string(), Anchor::Start, 0.0);
            }
        }
    }

    /// A linear dimension of `value` between view points `pa` and `pb`
    /// measured along `dir`, its line `level` steps out on the `side` of
    /// view `i`.
    #[allow(clippy::too_many_arguments)]
    fn linear(
        &mut self,
        i: usize,
        pa: P2,
        pb: P2,
        dir: P2,
        side: P2,
        level: usize,
        text: String,
        value: f64,
    ) {
        let v = &self.views[i];
        let corners = [v.min, [v.max[0], v.min[1]], v.max, [v.min[0], v.max[1]]];
        let out = corners
            .iter()
            .map(|c| dot2(*c, side))
            .fold(f64::NEG_INFINITY, f64::max);
        let k = out + (8.0 + 7.0 * level as f64) / self.scale;
        let a_on = add(mul(dir, dot2(pa, dir)), mul(side, k));
        let b_on = add(mul(dir, dot2(pb, dir)), mul(side, k));
        let (sa, sb) = (self.to_sheet(i, a_on), self.to_sheet(i, b_on));
        let gap = 1.5;
        let over = 2.0;
        for (from, to) in [(pa, sa), (pb, sb)] {
            let from = self.to_sheet(i, from);
            let along = sub(to, from);
            let l = len2(along);
            if l > gap {
                let unit_along = mul(along, 1.0 / l);
                self.line(
                    Kind::Dimension,
                    vec![
                        add(from, mul(unit_along, gap)),
                        add(to, mul(unit_along, over)),
                    ],
                );
            }
        }
        self.line(Kind::Dimension, vec![sa, sb]);
        let ab = sub(sb, sa);
        let l = len2(ab).max(1e-9);
        let u = mul(ab, 1.0 / l);
        arrow(self, sa, u);
        arrow(self, sb, mul(u, -1.0));
        // Text above the line, reading from the bottom or the right.
        let mut angle = u[1].atan2(u[0]).to_degrees();
        if angle > 90.0 + 1e-6 {
            angle -= 180.0;
        } else if angle < -90.0 + 1e-6 {
            angle += 180.0;
        }
        let normal = [-(angle.to_radians().sin()), angle.to_radians().cos()];
        let mid = add(mul(add(sa, sb), 0.5), mul(normal, 1.2));
        self.text(mid, TEXT, text.clone(), Anchor::Middle, angle);
        if side[1] < -0.5 {
            self.below[i] = self.below[i].max(8.0 + 7.0 * level as f64 + 1.2 + TEXT);
        }
        self.dimensions.push(Dimension {
            view: self.views[i].name.clone(),
            text,
            value,
        });
    }

    fn mark_dimensions(&mut self, faces: &[FaceDimension]) {
        let index = |name: &str, views: &[View]| views.iter().position(|v| v.name == name);
        let mut levels: HashMap<(usize, i64, i64), usize> = HashMap::new();
        let mut level = |i: usize, side: P2| -> usize {
            let key = (i, side[0].round() as i64, side[1].round() as i64);
            let n = levels.entry(key).or_insert(0);
            *n += 1;
            *n - 1
        };
        // Overall sizes: the front's width and height, the depth from the
        // top view (or the right one).
        if let Some(f) = index("front", &self.views) {
            let v = self.views[f].clone();
            let w = v.max[0] - v.min[0];
            let h = v.max[1] - v.min[1];
            let l = level(f, [0.0, -1.0]);
            self.linear(
                f,
                v.min,
                [v.max[0], v.min[1]],
                [1.0, 0.0],
                [0.0, -1.0],
                l,
                number(w),
                w,
            );
            let l = level(f, [-1.0, 0.0]);
            self.linear(
                f,
                v.min,
                [v.min[0], v.max[1]],
                [0.0, 1.0],
                [-1.0, 0.0],
                l,
                number(h),
                h,
            );
        }
        if let Some(t) = index("top", &self.views) {
            let v = self.views[t].clone();
            let depth = v.max[1] - v.min[1];
            let l = level(t, [-1.0, 0.0]);
            self.linear(
                t,
                v.min,
                [v.min[0], v.max[1]],
                [0.0, 1.0],
                [-1.0, 0.0],
                l,
                number(depth),
                depth,
            );
        } else if let Some(r) = index("right", &self.views) {
            let v = self.views[r].clone();
            let depth = v.max[0] - v.min[0];
            let l = level(r, [0.0, -1.0]);
            self.linear(
                r,
                v.min,
                [v.max[0], v.min[1]],
                [1.0, 0.0],
                [0.0, -1.0],
                l,
                number(depth),
                depth,
            );
        }
        // Named faces.
        for fd in faces {
            let Some(i) = index(&fd.view, &self.views) else {
                continue;
            };
            let v = self.views[i].clone();
            let n2 = [dot(fd.normal, v.u), dot(fd.normal, v.v)];
            let ln = len2(n2);
            if ln < 0.99 {
                continue;
            }
            let dir = mul(n2, 1.0 / ln);
            let pa = [dot(fd.a, v.u), dot(fd.a, v.v)];
            let pb = [dot(fd.b, v.u), dot(fd.b, v.v)];
            let value = dot(sub3(fd.b, fd.a), fd.normal).abs();
            let mut side = [-dir[1], dir[0]];
            let centre = mul(add(v.min, v.max), 0.5);
            if dot2(sub(mul(add(pa, pb), 0.5), centre), side) < 0.0 {
                side = mul(side, -1.0);
            }
            let l = level(i, side);
            let text = if fd.label.is_empty() {
                number(value)
            } else {
                format!("{} {}", fd.label, number(value))
            };
            self.linear(i, pa, pb, dir, side, l, text, value);
        }
        // Circles: each size once on the drawing, with how many the view
        // shows; on a part's drawing (an assembly's would bury the views).
        let mut done: Vec<f64> = Vec::new();
        for i in 0..self.views.len() {
            if !self.callouts {
                break;
            }
            if self.views[i].cut.is_some() || self.views[i].name == "iso" {
                continue;
            }
            let found = circles(&self.views[i]);
            let mut sizes: Vec<(f64, Vec<P2>)> = Vec::new();
            for (c, r) in found {
                let dia = (2.0 * r * 100.0).round() / 100.0;
                match sizes.iter_mut().find(|(d, _)| (*d - dia).abs() < 0.011) {
                    Some(s) => s.1.push(c),
                    None => sizes.push((dia, vec![c])),
                }
            }
            sizes.sort_by(|a, b| b.0.total_cmp(&a.0));
            let mut k = 0.0;
            for (dia, centres) in sizes {
                if done.iter().any(|d| (d - dia).abs() < 0.011) {
                    continue;
                }
                done.push(dia);
                let text = if centres.len() > 1 {
                    format!("{}\u{d7} \u{d8}{}", centres.len(), number(dia))
                } else {
                    format!("\u{d8}{}", number(dia))
                };
                // A leader from the circle up and to the right.
                let mut c = centres[0];
                for q in &centres {
                    if q[0] + q[1] > c[0] + c[1] {
                        c = *q;
                    }
                }
                let r = dia / 2.0;
                let edge = add(c, mul([1.0, 1.0], r / 2f64.sqrt()));
                let p = self.to_sheet(i, edge);
                let knee = add(p, [6.0 + 5.0 * k, 6.0 + 5.0 * k]);
                k += 1.0;
                let end = add(knee, [4.0, 0.0]);
                self.line(Kind::Dimension, vec![p, knee, end]);
                arrow(self, p, unit2(sub(knee, p)));
                self.text(
                    add(end, [1.0, -TEXT / 2.0]),
                    TEXT,
                    text.clone(),
                    Anchor::Start,
                    0.0,
                );
                self.dimensions.push(Dimension {
                    view: self.views[i].name.clone(),
                    text,
                    value: dia,
                });
            }
        }
    }

    /// The border and the title block.
    fn mark_frame(&mut self) {
        let [w, h] = self.size;
        let m = FRAME_MARGIN;
        self.line(
            Kind::Frame,
            vec![[m, m], [w - m, m], [w - m, h - m], [m, h - m], [m, m]],
        );
        let (x0, y0) = (w - m - BLOCK_W, m);
        let (x1, y1) = (w - m, m + BLOCK_H);
        self.line(Kind::Frame, vec![[x0, y0], [x0, y1], [x1, y1]]);
        let row = BLOCK_H / 3.0;
        self.line(Kind::Frame, vec![[x0, y0 + row], [x1, y0 + row]]);
        self.line(
            Kind::Frame,
            vec![[x0, y0 + 2.0 * row], [x1, y0 + 2.0 * row]],
        );
        let mid = x0 + BLOCK_W / 2.0;
        self.line(Kind::Frame, vec![[mid, y0], [mid, y0 + row]]);
        let title = self.title.clone();
        self.text(
            [x0 + 3.0, y0 + 2.0 * row + 2.6],
            5.0,
            title,
            Anchor::Start,
            0.0,
        );
        let subtitle = self.subtitle.clone();
        self.text(
            [x0 + 3.0, y0 + row + 3.0],
            TEXT,
            subtitle,
            Anchor::Start,
            0.0,
        );
        let scale = format!(
            "Scale {}   Units mm   {}-angle projection",
            scale_label(self.scale),
            if self.angle == "first" {
                "First"
            } else {
                "Third"
            }
        );
        self.text([x0 + 3.0, y0 + 3.0], 2.8, scale, Anchor::Start, 0.0);
        let sheet = format!("Sheet {}   OpenAgents CAD", self.sheet);
        self.text([mid + 3.0, y0 + 3.0], 2.8, sheet, Anchor::Start, 0.0);
    }

    /// What the drawing holds, for a tool's answer.
    #[must_use]
    pub fn report(&self) -> Value {
        let views: Vec<Value> = self
            .views
            .iter()
            .map(|v| {
                let mut o = json!({"view": v.name, "title": v.title,
                    "visible": v.count(Kind::Visible), "hidden": v.count(Kind::Hidden),
                    "silhouettes": v.lines.iter().filter(|l| l.silhouette).count(),
                    "size_mm": [round2(v.max[0] - v.min[0]), round2(v.max[1] - v.min[1])]});
                if let Some(at) = v.cut {
                    o["section"] = json!(v.count(Kind::Section));
                    o["cut_at_mm"] = json!(round2(at));
                }
                o
            })
            .collect();
        let mut kinds = serde_json::Map::new();
        for k in Kind::ALL {
            let n = self
                .marks
                .iter()
                .filter(|m| matches!(m, Mark::Line { kind, .. } if *kind == k))
                .count();
            kinds.insert(k.name().into(), json!(n));
        }
        json!({"sheet": self.sheet, "scale": scale_label(self.scale), "angle": self.angle,
               "views": views,
               "dimensions": self.dimensions.iter().map(|d| json!({"view": d.view, "text": d.text, "mm": round2(d.value)})).collect::<Vec<_>>(),
               "lines": kinds})
    }

    // -- Writers -----------------------------------------------------------

    /// The sheet as SVG (millimetres; hidden lines dashed).
    #[must_use]
    pub fn svg(&self) -> String {
        let [w, h] = self.size;
        let mut s = String::new();
        let _ = write!(
            s,
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}mm\" height=\"{h}mm\" viewBox=\"0 0 {w} {h}\">\
<rect width=\"{w}\" height=\"{h}\" fill=\"#ffffff\"/>"
        );
        for k in Kind::ALL {
            let dash = k.dashes();
            let _ = write!(
                s,
                "<g fill=\"none\" stroke=\"#1a1a1a\" stroke-width=\"{}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"{} data-kind=\"{}\">",
                k.width(),
                if dash.is_empty() {
                    String::new()
                } else {
                    format!(
                        " stroke-dasharray=\"{}\"",
                        dash.iter()
                            .map(|d| d.to_string())
                            .collect::<Vec<_>>()
                            .join(" ")
                    )
                },
                k.name()
            );
            for m in &self.marks {
                if let Mark::Line { kind, points } = m
                    && *kind == k
                {
                    let pts: Vec<String> = points
                        .iter()
                        .map(|p| format!("{:.3},{:.3}", p[0], h - p[1]))
                        .collect();
                    let _ = write!(s, "<polyline points=\"{}\"/>", pts.join(" "));
                }
            }
            s.push_str("</g>");
        }
        s.push_str("<g fill=\"#1a1a1a\" font-family=\"Helvetica, Arial, sans-serif\">");
        for m in &self.marks {
            if let Mark::Text {
                at,
                size,
                text,
                anchor,
                angle,
            } = m
            {
                let anchor = match anchor {
                    Anchor::Start => "start",
                    Anchor::Middle => "middle",
                    Anchor::End => "end",
                };
                let (x, y) = (at[0], h - at[1]);
                let rotate = if angle.abs() > 1e-9 {
                    format!(" transform=\"rotate({:.3} {x:.3} {y:.3})\"", -angle)
                } else {
                    String::new()
                };
                let _ = write!(
                    s,
                    "<text x=\"{x:.3}\" y=\"{y:.3}\" font-size=\"{:.2}\" text-anchor=\"{anchor}\"{rotate}>{}</text>",
                    size / 0.72,
                    xml(text)
                );
            }
        }
        s.push_str("</g></svg>\n");
        s
    }

    /// The sheet as a one-page PDF (vector lines, Helvetica text).
    #[must_use]
    pub fn pdf(&self) -> Vec<u8> {
        let pt = 72.0 / 25.4;
        let mut c: Vec<u8> = Vec::new();
        c.extend_from_slice(b"1 J 1 j 0.1 0.1 0.1 RG 0.1 0.1 0.1 rg\n");
        for k in Kind::ALL {
            let dash = k.dashes();
            let _ = writeln!(Bytes(&mut c), "{:.3} w", k.width() * pt);
            if dash.is_empty() {
                c.extend_from_slice(b"[] 0 d\n");
            } else {
                let d: Vec<String> = dash.iter().map(|x| format!("{:.2}", x * pt)).collect();
                let _ = writeln!(Bytes(&mut c), "[{}] 0 d", d.join(" "));
            }
            for m in &self.marks {
                if let Mark::Line { kind, points } = m
                    && *kind == k
                    && points.len() >= 2
                {
                    for (i, p) in points.iter().enumerate() {
                        let _ = write!(
                            Bytes(&mut c),
                            "{:.3} {:.3} {} ",
                            p[0] * pt,
                            p[1] * pt,
                            if i == 0 { "m" } else { "l" }
                        );
                    }
                    c.extend_from_slice(b"S\n");
                }
            }
        }
        for m in &self.marks {
            if let Mark::Text {
                at,
                size,
                text,
                anchor,
                angle,
            } = m
            {
                let fs = size / 0.72 * pt;
                let width = text_width(text) * fs;
                let shift = match anchor {
                    Anchor::Start => 0.0,
                    Anchor::Middle => width / 2.0,
                    Anchor::End => width,
                };
                let (sa, ca) = angle.to_radians().sin_cos();
                let (x, y) = (at[0] * pt - ca * shift, at[1] * pt - sa * shift);
                let _ = write!(
                    Bytes(&mut c),
                    "BT /F1 {fs:.2} Tf {ca:.5} {sa:.5} {:.5} {ca:.5} {x:.3} {y:.3} Tm (",
                    -sa
                );
                c.extend(win_ansi(text));
                c.extend_from_slice(b") Tj ET\n");
            }
        }
        let (w, h) = (self.size[0] * pt, self.size[1] * pt);
        let objects: Vec<Vec<u8>> = vec![
            b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
            b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w:.2} {h:.2}] /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>"
            )
            .into_bytes(),
            {
                let mut s = format!("<< /Length {} >>\nstream\n", c.len()).into_bytes();
                s.extend_from_slice(&c);
                s.extend_from_slice(b"endstream");
                s
            },
            b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"
                .to_vec(),
            format!("<< /Title ({}) /Producer (OpenAgents CAD) >>", pdf_escape(&self.title))
                .into_bytes(),
        ];
        let mut out: Vec<u8> = b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n".to_vec();
        let mut offsets = Vec::new();
        for (i, o) in objects.iter().enumerate() {
            offsets.push(out.len());
            out.extend(format!("{} 0 obj\n", i + 1).into_bytes());
            out.extend_from_slice(o);
            out.extend_from_slice(b"\nendobj\n");
        }
        let xref = out.len();
        out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).into_bytes());
        for o in offsets {
            out.extend(format!("{o:010} 00000 n \n").into_bytes());
        }
        out.extend(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R /Info 6 0 R >>\nstartxref\n{xref}\n%%EOF\n",
                objects.len() + 1
            )
            .into_bytes(),
        );
        out
    }

    /// The sheet as DXF (AutoCAD R12, sheet millimetres): a layer per kind
    /// of line, hidden lines DASHED and cutting planes CENTER.
    #[must_use]
    pub fn dxf(&self) -> String {
        let mut s = String::new();
        let mut g = |code: i32, value: &str| {
            let _ = write!(s, "{code}\n{value}\n");
        };
        g(0, "SECTION");
        g(2, "HEADER");
        g(9, "$ACADVER");
        g(1, "AC1009");
        g(9, "$INSUNITS");
        g(70, "4");
        g(9, "$EXTMIN");
        g(10, "0");
        g(20, "0");
        g(9, "$EXTMAX");
        g(10, &format!("{}", self.size[0]));
        g(20, &format!("{}", self.size[1]));
        g(0, "ENDSEC");
        g(0, "SECTION");
        g(2, "TABLES");
        g(0, "TABLE");
        g(2, "LTYPE");
        g(70, "3");
        for (name, text, dashes) in [
            ("CONTINUOUS", "Solid line", &[][..]),
            ("DASHED", "Dashed __ __ __", &[3.0, -1.5][..]),
            ("CENTER", "Center ____ _ ____", &[8.0, -2.0, 1.5, -2.0][..]),
        ] {
            g(0, "LTYPE");
            g(2, name);
            g(70, "0");
            g(3, text);
            g(72, "65");
            g(73, &dashes.len().to_string());
            g(
                40,
                &format!("{}", dashes.iter().map(|d: &f64| d.abs()).sum::<f64>()),
            );
            for d in dashes {
                g(49, &format!("{d}"));
            }
        }
        g(0, "ENDTAB");
        g(0, "TABLE");
        g(2, "LAYER");
        g(70, &(Kind::ALL.len() + 1).to_string());
        for k in Kind::ALL {
            g(0, "LAYER");
            g(2, &k.name().to_uppercase());
            g(70, "0");
            g(62, &k.dxf_color().to_string());
            g(6, k.dxf_linetype());
        }
        g(0, "LAYER");
        g(2, "TEXT");
        g(70, "0");
        g(62, "7");
        g(6, "CONTINUOUS");
        g(0, "ENDTAB");
        g(0, "ENDSEC");
        g(0, "SECTION");
        g(2, "ENTITIES");
        let f = |x: f64| format!("{x:.4}");
        for m in &self.marks {
            match m {
                Mark::Line { kind, points } if points.len() == 2 => {
                    g(0, "LINE");
                    g(8, &kind.name().to_uppercase());
                    g(10, &f(points[0][0]));
                    g(20, &f(points[0][1]));
                    g(30, "0");
                    g(11, &f(points[1][0]));
                    g(21, &f(points[1][1]));
                    g(31, "0");
                }
                Mark::Line { kind, points } if points.len() > 2 => {
                    let layer = kind.name().to_uppercase();
                    g(0, "POLYLINE");
                    g(8, &layer);
                    g(66, "1");
                    g(10, "0");
                    g(20, "0");
                    g(30, "0");
                    g(70, "0");
                    for p in points {
                        g(0, "VERTEX");
                        g(8, &layer);
                        g(10, &f(p[0]));
                        g(20, &f(p[1]));
                        g(30, "0");
                    }
                    g(0, "SEQEND");
                    g(8, &layer);
                }
                Mark::Line { .. } => {}
                Mark::Text {
                    at,
                    size,
                    text,
                    anchor,
                    angle,
                } => {
                    g(0, "TEXT");
                    g(8, "TEXT");
                    g(10, &f(at[0]));
                    g(20, &f(at[1]));
                    g(30, "0");
                    g(40, &f(*size));
                    g(1, &dxf_text(text));
                    if angle.abs() > 1e-9 {
                        g(50, &f(*angle));
                    }
                    let h = match anchor {
                        Anchor::Start => 0,
                        Anchor::Middle => 1,
                        Anchor::End => 2,
                    };
                    if h != 0 {
                        g(72, &h.to_string());
                        g(11, &f(at[0]));
                        g(21, &f(at[1]));
                        g(31, "0");
                    }
                }
            }
        }
        g(0, "ENDSEC");
        g(0, "EOF");
        s
    }

    /// The drawing as one of `svg`, `pdf` or `dxf`.
    ///
    /// # Errors
    /// Another format.
    pub fn bytes(&self, format: &str) -> Result<Vec<u8>, String> {
        match format {
            "svg" => Ok(self.svg().into_bytes()),
            "pdf" => Ok(self.pdf()),
            "dxf" => Ok(self.dxf().into_bytes()),
            other => Err(format!("A drawing is svg, pdf or dxf, not {other}.")),
        }
    }
}

fn unit2(a: P2) -> P2 {
    let l = len2(a);
    if l > 0.0 { mul(a, 1.0 / l) } else { a }
}

/// An open arrowhead at `tip`, pointing back along `-dir`... drawn as two
/// short strokes opening along `dir`.
fn arrow(d: &mut Drawing, tip: P2, dir: P2) {
    let (len, half) = (2.5, 0.9);
    let n = [-dir[1], dir[0]];
    let base = add(tip, mul(dir, len));
    d.line(
        Kind::Dimension,
        vec![add(base, mul(n, half)), tip, sub(base, mul(n, half))],
    );
}

fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

/// Hatching at 45° inside a section's cut outline, `step` apart (model mm).
fn hatch(lines: &[Line], step: f64) -> Vec<(P2, P2)> {
    // Closed outlines only: an open one has no inside.
    let segs: Vec<(P2, P2)> = lines
        .iter()
        .filter(|l| {
            l.kind == Kind::Section
                && l.points.len() >= 4
                && len2(sub(l.points[0], l.points[l.points.len() - 1])) < 1e-6
        })
        .flat_map(|l| {
            l.points
                .windows(2)
                .map(|w| (w[0], w[1]))
                .collect::<Vec<_>>()
        })
        .collect();
    if segs.is_empty() || step <= 0.0 {
        return Vec::new();
    }
    // Rotate by -45° so the hatch lines are horizontal: y' = (y - x)/√2.
    let r = std::f64::consts::FRAC_1_SQRT_2;
    let rot = |p: P2| [(p[0] + p[1]) * r, (p[1] - p[0]) * r];
    let back = |p: P2| [(p[0] - p[1]) * r, (p[0] + p[1]) * r];
    let rs: Vec<(P2, P2)> = segs.iter().map(|(a, b)| (rot(*a), rot(*b))).collect();
    let (lo, hi) = bbox2(rs.iter().flat_map(|(a, b)| [*a, *b]));
    let mut out = Vec::new();
    let first = (lo[1] / step).floor() as i64 + 1;
    let last = (hi[1] / step).ceil() as i64;
    for k in first..last {
        let y = k as f64 * step;
        let mut xs: Vec<f64> = rs
            .iter()
            .filter(|(a, b)| (a[1] > y) != (b[1] > y))
            .map(|(a, b)| a[0] + (b[0] - a[0]) * (y - a[1]) / (b[1] - a[1]))
            .collect();
        xs.sort_by(f64::total_cmp);
        for pair in xs.chunks_exact(2) {
            if pair[1] - pair[0] > 1e-9 {
                out.push((back([pair[0], y]), back([pair[1], y])));
            }
        }
    }
    out
}

fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Text as Helvetica's WinAnsi bytes, escaped for a PDF string.
fn win_ansi(s: &str) -> Vec<u8> {
    let mut out = Vec::new();
    for ch in s.chars() {
        let b: u8 = match ch {
            '(' | ')' | '\\' => {
                out.push(b'\\');
                ch as u8
            }
            '\u{d8}' => 0xd8,
            '\u{d7}' => 0xd7,
            '\u{b0}' => 0xb0,
            '\u{2013}' => 0x96,
            '\u{b1}' => 0xb1,
            c if c.is_ascii() && !c.is_ascii_control() => c as u8,
            _ => b'?',
        };
        out.push(b);
    }
    out
}

fn pdf_escape(s: &str) -> String {
    s.chars()
        .filter(char::is_ascii)
        .map(|c| match c {
            '(' | ')' | '\\' => format!("\\{c}"),
            c => c.to_string(),
        })
        .collect()
}

/// DXF R12 text: the diameter and degree signs as %%c and %%d.
fn dxf_text(s: &str) -> String {
    s.replace('\u{d8}', "%%c")
        .replace('\u{b0}', "%%d")
        .replace('\u{d7}', "x")
        .replace('\u{2013}', "-")
}

/// Helvetica's width of a string, in ems (near enough to centre text).
fn text_width(s: &str) -> f64 {
    s.chars()
        .map(|c| match c {
            'i' | 'j' | 'l' | '.' | ',' | ':' | ';' | '\'' | '|' => 0.24,
            'f' | 't' | 'r' | 'I' | ' ' | '(' | ')' | '-' => 0.32,
            'm' | 'w' | 'M' | 'W' => 0.84,
            c if c.is_ascii_uppercase() => 0.68,
            c if c.is_ascii_digit() => 0.556,
            _ => 0.54,
        })
        .sum()
}

/// `write!` into a byte buffer.
struct Bytes<'a>(&'a mut Vec<u8>);

impl std::fmt::Write for Bytes<'_> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.0.extend_from_slice(s.as_bytes());
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// From a document.

/// A drawing of a document: built (from `cache`), its named-face
/// dimensions found, and laid out. `args` are a tool's: `views`,
/// `sections`, `angle`, `sheet`, `title`, `hidden` and `dimensions`
/// (`[{"part": "plate", "faces": ["body:start", "body:top"], "view":
/// "front"}]`).
///
/// # Errors
/// The document doesn't build, an option is unknown, or a named face isn't
/// there or isn't flat.
pub fn of_document(
    document: &Value,
    args: &Value,
    cache: &std::path::Path,
) -> Result<Drawing, Vec<String>> {
    let mut o = Options::from_args(args).map_err(|e| vec![e])?;
    let built = crate::build::build_with(document, cache)?;
    let assembly = crate::doc::as_assembly(document);
    o.callouts = built.placements.len() <= 1;
    if o.title.is_empty() {
        o.title = assembly["title"]
            .as_str()
            .or_else(|| document["title"].as_str())
            .or_else(|| document["id"].as_str())
            .unwrap_or("Drawing")
            .to_owned();
    }
    if o.subtitle.is_empty() {
        o.subtitle = document["id"].as_str().unwrap_or_default().to_owned();
    }
    let wanted = list_of(&args["dimensions"]).map_err(|e| vec![format!("dimensions: {e}")])?;
    for d in wanted {
        let part = d["part"]
            .as_str()
            .or_else(|| {
                assembly["parts"]
                    .as_object()
                    .filter(|p| p.len() == 1)
                    .and_then(|p| p.keys().next().map(String::as_str))
            })
            .ok_or_else(|| vec!["A face dimension names its part (part).".to_owned()])?
            .to_owned();
        let names: Vec<String> = d["faces"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|f| f.as_str().map(str::to_owned))
            .collect();
        if names.len() != 2 {
            return Err(vec![
                "A face dimension names two faces: {\"faces\": [\"body:start\", \"body:top\"]}."
                    .into(),
            ]);
        }
        let view = d["view"].as_str().unwrap_or("front").to_lowercase();
        if direction(&view).is_none() || view == "iso" {
            return Err(vec![format!(
                "A face dimension is shown in one of: {}.",
                VIEWS[..6].join(", ")
            )]);
        }
        let faces = crate::build::faces_of(document, &part, None, cache)?;
        let find = |name: &str| -> Result<(P3, P3), String> {
            let face = faces["solids"]
                .as_array()
                .into_iter()
                .flatten()
                .flat_map(|s| s["faces"].as_array().into_iter().flatten())
                .find(|f| {
                    f["name"].as_str().is_some_and(|n| {
                        n == name || n.strip_prefix(name).is_some_and(|r| r.starts_with('#'))
                    })
                })
                .ok_or_else(|| format!("{part} has no face {name} (cad_faces lists them)."))?;
            let p = |v: &Value| -> Option<P3> {
                Some([v[0].as_f64()?, v[1].as_f64()?, v[2].as_f64()?])
            };
            let centre = p(&face["centre"]).ok_or_else(|| format!("{name} has no centre."))?;
            let normal = p(&face["normal"])
                .ok_or_else(|| format!("{name} isn't flat, so it has no distance to measure."))?;
            Ok((centre, normal))
        };
        let (ca, na) = find(&names[0]).map_err(|e| vec![e])?;
        let (cb, nb) = find(&names[1]).map_err(|e| vec![e])?;
        if dot(unit(na), unit(nb)).abs() < 0.999 {
            return Err(vec![format!(
                "{} and {} aren't parallel, so they have no one distance.",
                names[0], names[1]
            )]);
        }
        // Into the world: the part's first instance.
        let instance = assembly["instances"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|i| i["part"] == part.as_str())
            .and_then(|i| i["id"].as_str())
            .unwrap_or(&part)
            .to_owned();
        let xf = built
            .placements
            .get(&instance)
            .copied()
            .unwrap_or(crate::xf::Xf::IDENTITY);
        let (eye, _) = direction(&view).unwrap_or(([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]));
        let normal = unit(xf.rotate(na));
        if dot(normal, eye).abs() > 0.01 {
            return Err(vec![format!(
                "{} and {} face the viewer in the {view} view; measure them in a view from the side.",
                names[0], names[1]
            )]);
        }
        o.faces.push(FaceDimension {
            view,
            a: xf.apply(ca),
            b: xf.apply(cb),
            normal,
            label: d["label"]
                .as_str()
                .unwrap_or_default()
                .chars()
                .take(24)
                .collect(),
        });
    }
    Ok(draw(&built, &o))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 60 × 40 × 30 mm block (X, Y, Z) with a 10 mm hole straight down
    /// through its middle.
    fn drilled_block() -> Value {
        json!({"kind": "part.v1", "id": "block", "title": "Drilled block", "material": "Aluminum",
            "features": [
                {"id": "outline", "op": "sketch", "params": {"plane": "XY", "draw": [
                    {"op": "sketch.rectangle.center", "params": {"center": [0, 0], "corner": [30, 20]}}]}},
                {"id": "body", "op": "solid.extrude", "params": {"sketch": "outline", "distance": 30}},
                {"id": "hole", "op": "solid.cylinder", "params": {"diameter": 10, "height": 40, "base": [0, 0, -5], "operation": "cut"}}]})
    }

    fn cache() -> std::path::PathBuf {
        crate::build::default_cache()
    }

    fn near(a: P2, b: P2) -> bool {
        len2(sub(a, b)) < 1e-6
    }

    /// The line from `a` to `b` (either way round) is in `lines`.
    fn has(lines: &[&Line], a: P2, b: P2) -> bool {
        lines.iter().any(|l| {
            l.points.len() == 2
                && ((near(l.points[0], a) && near(l.points[1], b))
                    || (near(l.points[0], b) && near(l.points[1], a)))
        })
    }

    fn of(view: &View, kind: Kind) -> Vec<&Line> {
        view.lines.iter().filter(|l| l.kind == kind).collect()
    }

    #[test]
    fn a_drilled_blocks_views_have_exactly_the_lines_they_should() {
        let d = of_document(&drilled_block(), &json!({"sections": ["front"]}), &cache())
            .unwrap_or_else(|p| panic!("{p:?}"));
        let view = |name: &str| d.views.iter().find(|v| v.name == name).unwrap();
        // Front (Y right, Z up): the block's outline, and the hole's two
        // sides hidden behind its face, exactly where the hole is.
        let front = view("front");
        let (vis, hid) = (of(front, Kind::Visible), of(front, Kind::Hidden));
        assert_eq!((vis.len(), hid.len()), (4, 2), "{:?}", front.lines);
        assert!(has(&vis, [-20.0, 0.0], [20.0, 0.0]) && has(&vis, [-20.0, 30.0], [20.0, 30.0]));
        assert!(has(&vis, [-20.0, 0.0], [-20.0, 30.0]) && has(&vis, [20.0, 0.0], [20.0, 30.0]));
        assert!(has(&hid, [-5.0, 0.0], [-5.0, 30.0]) && has(&hid, [5.0, 0.0], [5.0, 30.0]));
        assert!(hid.iter().all(|l| l.silhouette));
        // Top (Y right, front down): the outline and the hole's rim, in two
        // halves; the rim underneath falls on it and isn't drawn twice.
        let top = view("top");
        let vis = of(top, Kind::Visible);
        assert_eq!(
            (vis.len(), top.count(Kind::Hidden)),
            (6, 0),
            "{:?}",
            top.lines
        );
        assert!(has(&vis, [-20.0, -30.0], [-20.0, 30.0]) && has(&vis, [20.0, -30.0], [20.0, 30.0]));
        assert!(has(&vis, [-20.0, -30.0], [20.0, -30.0]) && has(&vis, [-20.0, 30.0], [20.0, 30.0]));
        let rim = circles(top);
        assert_eq!(rim.len(), 1);
        assert!(
            near(rim[0].0, [0.0, 0.0]) && (rim[0].1 - 5.0).abs() < 1e-9,
            "{rim:?}"
        );
        // Right (from +Y: X to the left): the same, 60 mm wide.
        let right = view("right");
        let (vis, hid) = (of(right, Kind::Visible), of(right, Kind::Hidden));
        assert_eq!((vis.len(), hid.len()), (4, 2), "{:?}", right.lines);
        assert!(has(&vis, [-30.0, 0.0], [30.0, 0.0]) && has(&vis, [-30.0, 30.0], [30.0, 30.0]));
        assert!(has(&hid, [-5.0, 0.0], [-5.0, 30.0]) && has(&hid, [5.0, 0.0], [5.0, 30.0]));
        // Iso: the block's nine edges in sight and the rim; nothing hidden.
        let iso = view("iso");
        assert_eq!((iso.count(Kind::Visible), iso.count(Kind::Hidden)), (11, 0));
        // Section A–A through the middle: two closed rectangles of cut
        // material either side of the hole, the hole's far rims across it,
        // hatching only inside the cut, no hidden lines.
        let sec = view("section-a");
        assert_eq!(sec.cut, Some(0.0));
        let cut = of(sec, Kind::Section);
        assert_eq!(cut.len(), 2, "{:?}", sec.lines);
        for l in &cut {
            assert_eq!(l.points.len(), 5, "{l:?}");
            assert!(near(l.points[0], l.points[4]));
            let (lo, hi) = bbox2(l.points.iter().copied());
            let left = near(lo, [-20.0, 0.0]) && near(hi, [-5.0, 30.0]);
            let right = near(lo, [5.0, 0.0]) && near(hi, [20.0, 30.0]);
            assert!(left || right, "{lo:?} {hi:?}");
        }
        let vis = of(sec, Kind::Visible);
        assert_eq!((vis.len(), sec.count(Kind::Hidden)), (2, 0));
        assert!(has(&vis, [-5.0, 0.0], [5.0, 0.0]) && has(&vis, [-5.0, 30.0], [5.0, 30.0]));
        let hatch = hatch(&sec.lines, 3.0);
        assert!(hatch.len() >= 10, "{}", hatch.len());
        for (a, b) in &hatch {
            for p in [a, b] {
                assert!(
                    p[0].abs() >= 5.0 - 1e-6 && p[0].abs() <= 20.0 + 1e-6,
                    "{p:?}"
                );
            }
        }
        // The sheet: one scale, the overall sizes and the hole's diameter.
        assert_eq!(scale_label(d.scale), "1:1");
        let texts: Vec<&str> = d.dimensions.iter().map(|x| x.text.as_str()).collect();
        assert_eq!(texts, ["40", "30", "60", "\u{d8}10"]);
        let report = d.report();
        assert_eq!(report["views"][0]["hidden"], 2);
        assert_eq!(report["lines"]["cutting"], 1);
    }

    #[test]
    fn the_sheet_writes_as_svg_pdf_and_dxf() {
        let d = of_document(
            &drilled_block(),
            &json!({"views": "front,top", "sheet": "A4"}),
            &cache(),
        )
        .unwrap();
        let svg = d.svg();
        assert!(svg.starts_with("<svg") && svg.contains("viewBox=\"0 0 297 210\""));
        assert!(svg.contains("stroke-dasharray=\"3 1.5\" data-kind=\"hidden\""));
        assert!(svg.contains(">\u{d8}10</text>"));
        let pdf = d.pdf();
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.starts_with("%PDF-1.4") && text.trim_end().ends_with("%%EOF"));
        assert!(text.contains("/MediaBox [0 0 841.89 595.28]"));
        // The cross-reference table points at each object.
        let xref: usize = text
            .split("startxref\n")
            .nth(1)
            .unwrap()
            .lines()
            .next()
            .unwrap()
            .parse()
            .unwrap();
        assert!(pdf[xref..].starts_with(b"xref"));
        let dxf = d.dxf();
        assert!(dxf.contains("HIDDEN\n70\n0\n62\n8\n6\nDASHED"));
        // The kernel's DXF reader reads it back: every straight line of the
        // sheet, and the text.
        let read = solidcraft_engine::io::read_dxf(&dxf).unwrap();
        let lines = d
            .marks
            .iter()
            .filter(|m| matches!(m, Mark::Line { points, .. } if points.len() == 2))
            .count();
        let read_lines = read
            .iter()
            .filter(|g| matches!(g, solidcraft_engine::io::Geom2::Line(..)))
            .count();
        assert!(read_lines >= lines, "{read_lines} < {lines}");
        assert!(d.bytes("png").is_err());
    }

    #[test]
    fn named_faces_are_dimensioned_and_first_angle_puts_the_top_below() {
        let d = of_document(
            &drilled_block(),
            &json!({"angle": "first", "views": ["front", "top"],
                    "dimensions": [{"faces": ["body:start", "body:top"], "view": "front", "label": "H"}]}),
            &cache(),
        )
        .unwrap_or_else(|p| panic!("{p:?}"));
        let named = d
            .dimensions
            .iter()
            .find(|x| x.text.starts_with('H'))
            .unwrap();
        assert_eq!(named.text, "H 30");
        assert!((named.value - 30.0).abs() < 1e-6);
        // First angle: the top view under the front.
        let (f, t) = (d.origins[0], d.origins[1]);
        assert!(t[1] < f[1], "{f:?} {t:?}");
        let bad = of_document(
            &drilled_block(),
            &json!({"dimensions": [{"faces": ["body:start", "body:side:l1"], "view": "front"}]}),
            &cache(),
        );
        assert!(bad.unwrap_err()[0].contains("aren't parallel"));
        let unknown = of_document(&drilled_block(), &json!({"views": ["under"]}), &cache());
        assert!(unknown.unwrap_err()[0].contains("The views are"));
    }
}
