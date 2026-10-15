//! The assembly solver (#11646): joints that close loops, one joint driven
//! with the others following, joint limits, gear couplings between joints,
//! and the degrees of freedom that are left.
//!
//! An assembly is a tree: each instance hangs from one parent by at most
//! one joint (its *tree joint*), and is placed by its parent's frame, that
//! joint's motion at its value, then its own `at` and `rotate`. A joint
//! with `"closes": true` adds a loop instead of placing anything: it joins
//! two instances the tree already places (a four-bar's coupler and rocker)
//! and asks that a frame on each meet the way its type says:
//!
//! | type | holds |
//! | --- | --- |
//! | `fastened` | the points meet, the axes line up, no twist |
//! | `revolute` | the points meet, the axes line up |
//! | `cylindrical` | the child's point stays on the parent's axis line, the axes line up |
//! | `slider` | as cylindrical, and no twist |
//! | `planar` | the child's point stays in the parent's plane, the axes line up |
//! | `spherical` | the points meet |
//! | `parallel` | the axes line up |
//! | `gear` | joint `driven`'s value is `ratio` times joint `driver`'s (a coupling, no instances) |
//!
//! The parent's frame is `origin`, `axis` and `x_dir` in the parent's frame
//! (no `parent`: the assembly's origin, which is fastened-to-origin); the
//! child's is `child_origin`, `child_axis` (default `axis`) and
//! `child_x_dir` in the child's.
//!
//! The unknowns are the values of the tree joints around each loop (and the
//! joints a coupling names). A tree joint with `"drive": true` keeps its
//! value and the others follow it. The solver runs damped Newton steps
//! (Levenberg–Marquardt) on the loop equations from the values written,
//! held inside each joint's limits, so a mechanism keeps the branch it was
//! drawn in. It reports, per loop, the residual, the degrees of freedom
//! left once the driven joints are held (`dof`), the mechanism's own
//! mobility (`mobility`: a four-bar's is 1), and equations that repeat
//! others (`redundant`: a planar linkage's out-of-plane equations). A loop
//! that can't close (a link too short, a joint at its limit, more joints
//! driven than the loop can take) is a problem the edit is refused for,
//! never forced.
//!
//! Pure arithmetic, no kernel: it runs the same natively and in wasm32.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use crate::doc;
use crate::xf::Xf;

/// A loop counts as closed when every residual is under this (mm, or
/// radians for directions).
pub const CLOSED: f64 = 1e-6;
/// The solver aims well under it.
const TARGET: f64 = 1e-11;
/// Direction residuals are weighed as millimetres at this lever arm, so a
/// step trades them fairly against point residuals.
const LEVER: f64 = 100.0;

/// The kinds a closing joint may have.
pub const CLOSURES: [&str; 8] = [
    "fastened",
    "revolute",
    "slider",
    "cylindrical",
    "planar",
    "spherical",
    "parallel",
    "gear",
];

/// Whether a joint closes a loop rather than placing its child.
#[must_use]
pub fn closes(joint: &Value) -> bool {
    joint["closes"] == true
}

/// One unknown: component `k` of tree joint `joint`'s value.
#[derive(Clone, Debug)]
struct Var {
    joint: usize,
    k: usize,
    lo: f64,
    hi: f64,
    held: bool,
}

/// One closing joint, read once.
struct Closure {
    id: String,
    kind: String,
    parent: Option<String>,
    child: String,
    origin: [f64; 3],
    axis: [f64; 3],
    x: [f64; 3],
    /// Two directions across the parent's axis, in the parent's frame.
    across: [[f64; 3]; 2],
    child_origin: [f64; 3],
    child_axis: [f64; 3],
    child_x: [f64; 3],
    /// A coupling: (driver joint, driven joint, ratio).
    coupling: Option<(usize, usize, f64)>,
}

fn unit(a: [f64; 3]) -> [f64; 3] {
    let n = dot(a, a).sqrt().max(1e-300);
    [a[0] / n, a[1] / n, a[2] / n]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// A direction across `n`: along `seed` made perpendicular, or a fixed one.
fn across(n: [f64; 3], seed: Option<[f64; 3]>) -> [f64; 3] {
    let seed = seed.unwrap_or(if n[0].abs() < 0.9 {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 1.0, 0.0]
    });
    let d = dot(seed, n);
    unit([seed[0] - d * n[0], seed[1] - d * n[1], seed[2] - d * n[2]])
}

/// How many values a tree joint of this type moves.
fn values_of(kind: &str) -> usize {
    doc::JOINTS
        .iter()
        .find(|(k, _)| *k == kind)
        .map_or(0, |(_, n)| *n)
}

/// The kinematic part of an assembly: its instances and joints only, so a
/// trial value costs a placement, not a copy of every part.
struct Model {
    slim: Value,
}

impl Model {
    fn new(assembly: &Value) -> Self {
        Model {
            slim: json!({"instances": assembly["instances"], "joints": assembly["joints"]}),
        }
    }

    fn joints(&self) -> &[Value] {
        self.slim["joints"].as_array().map_or(&[], Vec::as_slice)
    }

    fn value(&self, v: &Var) -> f64 {
        doc::joint_values(&self.joints()[v.joint])
            .get(v.k)
            .copied()
            .unwrap_or(0.0)
    }

    fn set(&mut self, v: &Var, x: f64) {
        let joint = &mut self.slim["joints"][v.joint];
        let n = values_of(joint["type"].as_str().unwrap_or_default());
        if n <= 1 {
            joint["value"] = json!(x);
        } else {
            let mut values = doc::joint_values(joint);
            values.resize(n, 0.0);
            values[v.k] = x;
            joint["value"] = json!(values);
        }
    }

    fn set_all(&mut self, vars: &[Var], x: &[f64]) {
        for (v, x) in vars.iter().zip(x) {
            self.set(v, *x);
        }
    }

    fn worlds(&self) -> BTreeMap<String, Xf> {
        crate::build::placements(&self.slim)
    }
}

/// The residuals of every closure at the model's current values; `rows`
/// gets, per residual, the closure it belongs to and whether it is a
/// direction (radians) rather than a distance (mm).
fn residuals(model: &Model, closures: &[Closure], rows: &mut Vec<(usize, bool)>) -> Vec<f64> {
    let worlds = model.worlds();
    let joints = model.joints();
    let mut out = Vec::new();
    rows.clear();
    for (c, cl) in closures.iter().enumerate() {
        if let Some((driver, driven, ratio)) = cl.coupling {
            let first = |j: usize| {
                doc::joint_values(&joints[j])
                    .first()
                    .copied()
                    .unwrap_or(0.0)
            };
            out.push(first(driven) - ratio * first(driver));
            rows.push((c, false));
            continue;
        }
        let p = cl
            .parent
            .as_ref()
            .and_then(|p| worlds.get(p))
            .copied()
            .unwrap_or(Xf::IDENTITY);
        let ch = worlds.get(&cl.child).copied().unwrap_or(Xf::IDENTITY);
        let pp = p.apply(cl.origin);
        let pc = ch.apply(cl.child_origin);
        let ap = unit(p.rotate(cl.axis));
        let ac = unit(ch.rotate(cl.child_axis));
        let u = [p.rotate(cl.across[0]), p.rotate(cl.across[1])];
        let d = sub(pc, pp);
        let mut push = |x: f64, direction: bool| {
            out.push(x);
            rows.push((c, direction));
        };
        let kind = cl.kind.as_str();
        match kind {
            "fastened" | "revolute" | "spherical" => {
                for x in d {
                    push(x, false);
                }
            }
            "cylindrical" | "slider" => {
                push(dot(d, u[0]), false);
                push(dot(d, u[1]), false);
            }
            "planar" => push(dot(d, ap), false),
            _ => {}
        }
        if kind != "spherical" {
            push(dot(ac, u[0]), true);
            push(dot(ac, u[1]), true);
        }
        if matches!(kind, "fastened" | "slider") {
            let xp = p.rotate(cl.x);
            let xc = ch.rotate(cl.child_x);
            push(dot(xc, cross(ap, xp)), true);
        }
    }
    out
}

fn weighted(r: &[f64], rows: &[(usize, bool)]) -> Vec<f64> {
    r.iter()
        .zip(rows)
        .map(|(x, (_, dir))| if *dir { x * LEVER } else { *x })
        .collect()
}

fn norm2(r: &[f64]) -> f64 {
    r.iter().map(|x| x * x).sum()
}

/// The Jacobian of the weighted residuals by the free unknowns, by central
/// differences.
fn jacobian(
    model: &mut Model,
    closures: &[Closure],
    vars: &[Var],
    x: &[f64],
    free: &[usize],
) -> Vec<Vec<f64>> {
    let mut rows = Vec::new();
    let mut columns = Vec::with_capacity(free.len());
    for &i in free {
        let h = 1e-6 * (1.0 + x[i].abs());
        model.set(&vars[i], x[i] + h);
        let plus = residuals(model, closures, &mut rows);
        model.set(&vars[i], x[i] - h);
        let minus = residuals(model, closures, &mut rows);
        model.set(&vars[i], x[i]);
        let column: Vec<f64> = weighted(&plus, &rows)
            .iter()
            .zip(weighted(&minus, &rows))
            .map(|(a, b)| (a - b) / (2.0 * h))
            .collect();
        columns.push(column);
    }
    let m = columns.first().map_or(0, Vec::len);
    (0..m)
        .map(|r| columns.iter().map(|c| c[r]).collect())
        .collect()
}

/// Solves `a x = b` (square, small) by elimination with partial pivoting.
fn linear(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Option<Vec<f64>> {
    let n = b.len();
    for c in 0..n {
        let p = (c..n).max_by(|&i, &j| a[i][c].abs().total_cmp(&a[j][c].abs()))?;
        if a[p][c].abs() < 1e-300 {
            return None;
        }
        a.swap(c, p);
        b.swap(c, p);
        for r in c + 1..n {
            let f = a[r][c] / a[c][c];
            if f != 0.0 {
                for k in c..n {
                    a[r][k] -= f * a[c][k];
                }
                b[r] -= f * b[c];
            }
        }
    }
    let mut x = vec![0.0; n];
    for c in (0..n).rev() {
        let s: f64 = (c + 1..n).map(|k| a[c][k] * x[k]).sum();
        x[c] = (b[c] - s) / a[c][c];
    }
    Some(x)
}

/// The numerical rank of a matrix (rows × columns), by elimination with
/// full pivoting relative to its largest entry.
fn rank(m: &[Vec<f64>]) -> usize {
    let mut a: Vec<Vec<f64>> = m.to_vec();
    let rows = a.len();
    let cols = a.first().map_or(0, Vec::len);
    let scale = a
        .iter()
        .flatten()
        .fold(0.0f64, |s, x| s.max(x.abs()))
        .max(1e-300);
    let tol = scale * 1e-7;
    let mut r = 0;
    let mut used = vec![false; cols];
    for _ in 0..rows.min(cols) {
        let mut best = (0.0, 0, 0);
        for (i, row) in a.iter().enumerate().skip(r) {
            for (j, x) in row.iter().enumerate() {
                if !used[j] && x.abs() > best.0 {
                    best = (x.abs(), i, j);
                }
            }
        }
        if best.0 <= tol {
            break;
        }
        let (_, i, j) = best;
        a.swap(r, i);
        used[j] = true;
        for k in r + 1..rows {
            let f = a[k][j] / a[r][j];
            for c in 0..cols {
                a[k][c] -= f * a[r][c];
            }
        }
        r += 1;
    }
    r
}

/// What the solver found.
pub struct Solved {
    /// The assembly's joints with the followers' solved values.
    pub joints: Vec<Value>,
    /// The loops, their residuals and degrees of freedom; null when the
    /// assembly closes no loop.
    pub report: Value,
    /// Loops that can't close, in plain words.
    pub problems: Vec<String>,
}

/// Reads the closing joints; what's wrong with them is [`doc`]'s to say,
/// so a malformed one is skipped here.
fn read_closures(joints: &[Value]) -> Vec<Closure> {
    let index: BTreeMap<&str, usize> = joints
        .iter()
        .enumerate()
        .filter(|(_, j)| !closes(j))
        .filter_map(|(i, j)| Some((j["id"].as_str()?, i)))
        .collect();
    let mut out = Vec::new();
    for j in joints.iter().filter(|j| closes(j)) {
        let id = j["id"].as_str().unwrap_or_default().to_owned();
        let kind = j["type"].as_str().unwrap_or_default().to_owned();
        let coupling = if kind == "gear" {
            let (Some(a), Some(b)) = (
                j["driver"].as_str().and_then(|d| index.get(d)),
                j["driven"].as_str().and_then(|d| index.get(d)),
            ) else {
                continue;
            };
            Some((*a, *b, j["ratio"].as_f64().unwrap_or(1.0)))
        } else {
            None
        };
        let axis = unit(doc::triple(&j["axis"]).unwrap_or([0.0, 0.0, 1.0]));
        let child_axis = unit(doc::triple(&j["child_axis"]).unwrap_or(axis));
        let x = across(axis, doc::triple(&j["x_dir"]));
        let child_x = across(
            child_axis,
            doc::triple(&j["child_x_dir"]).or(doc::triple(&j["x_dir"])),
        );
        let a0 = across(axis, None);
        out.push(Closure {
            id,
            kind,
            parent: j["parent"].as_str().map(str::to_owned),
            child: j["child"].as_str().unwrap_or_default().to_owned(),
            origin: doc::triple(&j["origin"]).unwrap_or([0.0; 3]),
            axis,
            x,
            across: [a0, cross(axis, a0)],
            child_origin: doc::triple(&j["child_origin"]).unwrap_or([0.0; 3]),
            child_axis,
            child_x,
            coupling,
        });
    }
    out
}

/// The tree joints each closure moves: those on the path between its two
/// instances (and, through gears there, the joints driving them), or the
/// two a coupling names.
fn loop_joints(assembly: &Value, joints: &[Value], cl: &Closure) -> BTreeSet<usize> {
    if let Some((a, b, _)) = cl.coupling {
        return [a, b].into_iter().collect();
    }
    let parent_of: BTreeMap<&str, &str> = assembly["instances"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|i| Some((i["id"].as_str()?, i["parent"].as_str()?)))
        .collect();
    let chain = |start: Option<&str>| -> Vec<String> {
        let mut out = Vec::new();
        let mut at = start;
        while let Some(id) = at {
            if out.iter().any(|x| x == id) || out.len() > parent_of.len() {
                break;
            }
            out.push(id.to_owned());
            at = parent_of.get(id).copied();
        }
        out
    };
    let a = chain(cl.parent.as_deref());
    let b = chain(Some(&cl.child));
    let on_path: BTreeSet<&String> = a
        .iter()
        .filter(|x| !b.contains(x))
        .chain(b.iter().filter(|x| !a.contains(x)))
        .collect();
    let mut out = BTreeSet::new();
    for (i, j) in joints.iter().enumerate() {
        if closes(j)
            || !j["child"]
                .as_str()
                .is_some_and(|c| on_path.iter().any(|p| *p == c))
        {
            continue;
        }
        // A gear on the path turns with its driver: the driver moves the loop.
        let mut at = i;
        for _ in 0..=joints.len() {
            if joints[at]["type"] != "gear" {
                out.insert(at);
                break;
            }
            match joints[at]["driver"]
                .as_str()
                .and_then(|d| joints.iter().position(|k| k["id"] == d && !closes(k)))
            {
                Some(next) => at = next,
                None => break,
            }
        }
    }
    out
}

/// Joins sets that share a member: each loop group, as indices into `sets`.
fn groups(sets: &[BTreeSet<usize>]) -> Vec<Vec<usize>> {
    let mut parent: Vec<usize> = (0..sets.len()).collect();
    fn find(p: &mut [usize], i: usize) -> usize {
        let mut r = i;
        while p[r] != r {
            r = p[r];
        }
        p[i] = r;
        r
    }
    for i in 0..sets.len() {
        for j in i + 1..sets.len() {
            if !sets[i].is_disjoint(&sets[j]) {
                let (a, b) = (find(&mut parent, i), find(&mut parent, j));
                parent[a] = b;
            }
        }
    }
    let mut out: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for i in 0..sets.len() {
        let r = find(&mut parent, i);
        out.entry(r).or_default().push(i);
    }
    out.into_values().collect()
}

/// Solves the assembly's loops (an assembly as [`doc::as_assembly`] gives
/// it: subassemblies expanded). Without closing joints it changes nothing
/// and reports null.
#[must_use]
pub fn solve(assembly: &Value) -> Solved {
    let joints: Vec<Value> = assembly["joints"].as_array().cloned().unwrap_or_default();
    let closures = read_closures(&joints);
    if closures.is_empty() {
        return Solved {
            joints,
            report: Value::Null,
            problems: Vec::new(),
        };
    }
    let started = crate::sys::Timer::start();
    let per_closure: Vec<BTreeSet<usize>> = closures
        .iter()
        .map(|c| loop_joints(assembly, &joints, c))
        .collect();
    let moved: BTreeSet<usize> = per_closure.iter().flatten().copied().collect();
    let mut vars = Vec::new();
    for &j in &moved {
        let joint = &joints[j];
        let n = values_of(joint["type"].as_str().unwrap_or_default());
        for k in 0..n {
            let limit = &joint["limits"][k];
            vars.push(Var {
                joint: j,
                k,
                lo: limit[0].as_f64().unwrap_or(f64::NEG_INFINITY),
                hi: limit[1].as_f64().unwrap_or(f64::INFINITY),
                held: joint["drive"] == true,
            });
        }
    }
    let mut model = Model::new(assembly);
    let mut x: Vec<f64> = vars
        .iter()
        .map(|v| model.value(v).clamp(v.lo, v.hi))
        .collect();
    model.set_all(&vars, &x);
    let free: Vec<usize> = (0..vars.len()).filter(|&i| !vars[i].held).collect();
    let mut rows = Vec::new();
    let mut r = residuals(&model, &closures, &mut rows);
    let mut cost = norm2(&weighted(&r, &rows));
    let mut lambda = 1e-3;
    let mut iterations = 0;
    while iterations < 200 && r.iter().any(|v| v.abs() > TARGET) && !free.is_empty() {
        iterations += 1;
        let jac = jacobian(&mut model, &closures, &vars, &x, &free);
        let w = weighted(&r, &rows);
        let n = free.len();
        let mut jtj = vec![vec![0.0; n]; n];
        let mut jtr = vec![0.0; n];
        for (row, rv) in jac.iter().zip(&w) {
            for a in 0..n {
                jtr[a] -= row[a] * rv;
                for b in 0..n {
                    jtj[a][b] += row[a] * row[b];
                }
            }
        }
        let mut improved = false;
        while lambda < 1e12 {
            let mut m = jtj.clone();
            for (a, row) in m.iter_mut().enumerate() {
                row[a] += lambda * (jtj[a][a] + 1e-9);
            }
            let Some(step) = linear(m, jtr.clone()) else {
                lambda *= 10.0;
                continue;
            };
            let mut trial = x.clone();
            for (a, &i) in free.iter().enumerate() {
                trial[i] = (x[i] + step[a]).clamp(vars[i].lo, vars[i].hi);
            }
            model.set_all(&vars, &trial);
            let mut trial_rows = Vec::new();
            let tr = residuals(&model, &closures, &mut trial_rows);
            let trial_cost = norm2(&weighted(&tr, &trial_rows));
            if trial_cost < cost {
                x = trial;
                r = tr;
                rows = trial_rows;
                cost = trial_cost;
                lambda = (lambda / 5.0).max(1e-12);
                improved = true;
                break;
            }
            lambda *= 4.0;
        }
        model.set_all(&vars, &x);
        if !improved {
            break;
        }
    }
    // An angle with no limits comes back to (−180°, 180°].
    for (v, xi) in vars.iter().zip(x.iter_mut()) {
        let kind = joints[v.joint]["type"].as_str().unwrap_or_default();
        let angle = matches!(
            (kind, v.k),
            ("revolute", 0) | ("cylindrical", 0) | ("planar", 2)
        );
        if angle && v.lo.is_infinite() && v.hi.is_infinite() {
            *xi = 180.0 - (180.0 - *xi).rem_euclid(360.0);
        }
    }
    model.set_all(&vars, &x);
    let r = residuals(&model, &closures, &mut rows);
    let all: Vec<usize> = (0..vars.len()).collect();
    let jac_all = jacobian(&mut model, &closures, &vars, &x, &all);
    let mut loops = Vec::new();
    let mut problems = Vec::new();
    let (mut dof_total, mut mobility_total) = (0usize, 0usize);
    for group in groups(&per_closure) {
        let in_group: BTreeSet<usize> = group.iter().copied().collect();
        let row_ids: Vec<usize> = (0..rows.len())
            .filter(|&k| in_group.contains(&rows[k].0))
            .collect();
        let group_joints: BTreeSet<usize> = group
            .iter()
            .flat_map(|c| per_closure[*c].iter().copied())
            .collect();
        let cols: Vec<usize> = (0..vars.len())
            .filter(|&i| group_joints.contains(&vars[i].joint))
            .collect();
        let sub_matrix = |columns: &[usize]| -> Vec<Vec<f64>> {
            row_ids
                .iter()
                .map(|&k| columns.iter().map(|&c| jac_all[k][c]).collect())
                .collect()
        };
        let free_cols: Vec<usize> = cols.iter().copied().filter(|&i| !vars[i].held).collect();
        let rank_all = rank(&sub_matrix(&cols));
        let rank_free = rank(&sub_matrix(&free_cols));
        let dof = free_cols.len() - rank_free;
        let mobility = cols.len() - rank_all;
        let redundant = row_ids.len() - rank_all;
        dof_total += dof;
        mobility_total += mobility;
        let residual = row_ids.iter().map(|&k| r[k].abs()).fold(0.0, f64::max);
        let ids: Vec<&str> = group.iter().map(|c| closures[*c].id.as_str()).collect();
        let held: Vec<&str> = cols
            .iter()
            .filter(|&&i| vars[i].held)
            .filter_map(|&i| joints[vars[i].joint]["id"].as_str())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let mut values = serde_json::Map::new();
        for j in &group_joints {
            let id = joints[*j]["id"].as_str().unwrap_or_default();
            values.insert(id.to_owned(), model.joints()[*j]["value"].clone());
        }
        let closed = residual <= CLOSED;
        if !closed {
            let at_limit: Vec<String> = cols
                .iter()
                .filter(|&&i| {
                    !vars[i].held
                        && ((x[i] - vars[i].lo).abs() < 1e-9 || (x[i] - vars[i].hi).abs() < 1e-9)
                })
                .map(|&i| {
                    format!(
                        "{} is at its limit ({})",
                        joints[vars[i].joint]["id"].as_str().unwrap_or_default(),
                        if (x[i] - vars[i].lo).abs() < 1e-9 {
                            vars[i].lo
                        } else {
                            vars[i].hi
                        }
                    )
                })
                .collect();
            // Would it close with nothing driven? Then the driving is the trouble.
            let closes_released = !held.is_empty() && {
                let mut released = assembly.clone();
                for j in released["joints"].as_array_mut().into_iter().flatten() {
                    if let Some(o) = j.as_object_mut() {
                        o.remove("drive");
                    }
                }
                solve(&released).problems.is_empty()
            };
            let why = if !at_limit.is_empty() {
                at_limit.join(", ")
            } else if closes_released {
                format!(
                    "the driven joints ({}) ask for a position the loop can't take; drive fewer joints or another value",
                    held.join(", ")
                )
            } else {
                "its links can't reach each other".to_owned()
            };
            problems.push(format!(
                "The loop closed by {} can't close: {why} (still {residual:.3} mm apart).",
                ids.join(", ")
            ));
        }
        loops.push(json!({
            "closes": ids,
            "joints": values,
            "driven": held,
            "closed": closed,
            "residual": residual,
            "dof": dof,
            "mobility": mobility,
            "redundant": redundant,
            "equations": row_ids.len(),
            "unknowns": cols.len(),
        }));
    }
    let mut solved = joints.clone();
    for v in &vars {
        solved[v.joint]["value"] = model.joints()[v.joint]["value"].clone();
    }
    let report = json!({
        "loops": loops,
        "dof": dof_total,
        "mobility": mobility_total,
        "residual": r.iter().map(|v| v.abs()).fold(0.0, f64::max),
        "iterations": iterations,
        "solve_ms": started.ms(),
    });
    Solved {
        joints: solved,
        report,
        problems,
    }
}

/// The joints that move with `joint` in its loops (itself included), by
/// id: what driving it moves.
#[must_use]
pub fn loop_of(assembly: &Value, joint: &str) -> BTreeSet<String> {
    let joints: Vec<Value> = assembly["joints"].as_array().cloned().unwrap_or_default();
    let closures = read_closures(&joints);
    let sets: Vec<BTreeSet<usize>> = closures
        .iter()
        .map(|c| loop_joints(assembly, &joints, c))
        .collect();
    let Some(me) = joints.iter().position(|j| j["id"] == joint) else {
        return BTreeSet::new();
    };
    let mut out = BTreeSet::new();
    for group in groups(&sets) {
        let members: BTreeSet<usize> = group.iter().flat_map(|c| sets[*c].clone()).collect();
        if members.contains(&me) {
            out.extend(
                members
                    .iter()
                    .filter_map(|j| joints[*j]["id"].as_str().map(str::to_owned)),
            );
        }
    }
    out.insert(joint.to_owned());
    out
}

/// Drives `joint` to `value` in `assembly`'s joints: sets it, holds it
/// (`drive: true`) and lets go of the other joints of its loops, so they
/// follow. Works on a document's own `joints` list (top-level joints).
///
/// # Errors
/// No such joint, or it takes no value.
pub fn drive(document: &mut Value, joint: &str, value: &Value) -> Result<(), String> {
    let flat = doc::as_assembly(document);
    let together = loop_of(&flat, joint);
    let list = document["joints"]
        .as_array_mut()
        .ok_or("The document has no joints.")?;
    let Some(target) = list.iter().position(|j| j["id"] == joint) else {
        return Err(format!("There's no joint {joint} to drive."));
    };
    let kind = list[target]["type"].as_str().unwrap_or_default().to_owned();
    if closes(&list[target]) || values_of(&kind) == 0 {
        return Err(format!(
            "Joint {joint} ({kind}) has no value to drive; drive a revolute, slider, cylindrical, planar or spherical joint."
        ));
    }
    for j in list.iter_mut() {
        let id = j["id"].as_str().unwrap_or_default();
        if id != joint && together.contains(id) && j.get("drive").is_some() {
            if let Some(o) = j.as_object_mut() {
                o.remove("drive");
            }
        }
    }
    list[target]["value"] = value.clone();
    list[target]["drive"] = json!(true);
    Ok(())
}

/// Writes the solved values of the followers back into a document's own
/// joints (top-level ones; a subassembly template's are solved on every
/// build from what it says), so the next solve starts where this one
/// ended and keeps the mechanism's branch. Returns the solver's report.
pub fn settle(document: &mut Value) -> Value {
    if document["kind"] != doc::ASSEMBLY {
        return Value::Null;
    }
    let flat = doc::as_assembly(document);
    // Frames named on faces are known only once the parts are built; the
    // build solves those from the values written.
    let mated = flat["joints"].as_array().into_iter().flatten().any(|j| {
        ["origin", "axis", "child_origin", "child_axis"]
            .iter()
            .any(|k| doc::mate(&j[*k]))
    });
    if mated {
        return Value::Null;
    }
    let solved = solve(&flat);
    if solved.report.is_null() || !solved.problems.is_empty() {
        return solved.report;
    }
    if let Some(list) = document["joints"].as_array_mut() {
        for j in list.iter_mut() {
            if let Some(s) = solved.joints.iter().find(|s| s["id"] == j["id"])
                && !s["value"].is_null()
            {
                j["value"] = s["value"].clone();
            }
        }
    }
    solved.report
}

/// Moves `joint` from `from` to `to` in `steps` steps, solving the loops at
/// each (from the step before, so the mechanism keeps its branch), and
/// says how far it got: ClassCAD's "move under constraints".
#[must_use]
pub fn sweep(assembly: &Value, joint: &str, from: f64, to: f64, steps: usize) -> Value {
    let mut current = assembly.clone();
    if let Some(list) = current["joints"].as_array_mut() {
        let together = loop_of(assembly, joint);
        for j in list.iter_mut() {
            let id = j["id"].as_str().unwrap_or_default().to_owned();
            if let Some(o) = j.as_object_mut() {
                if id == joint {
                    o.insert("drive".into(), json!(true));
                } else if together.contains(&id) {
                    o.remove("drive");
                }
            }
        }
    }
    let steps = steps.clamp(1, 720);
    let mut rows = Vec::new();
    let mut reached: Option<f64> = None;
    let mut stopped = Value::Null;
    for s in 0..=steps {
        let value = from + (to - from) * s as f64 / steps as f64;
        if let Some(j) = current["joints"]
            .as_array_mut()
            .and_then(|l| l.iter_mut().find(|j| j["id"] == joint))
        {
            let n = values_of(j["type"].as_str().unwrap_or_default());
            if n <= 1 {
                j["value"] = json!(value);
            } else {
                let mut v = doc::joint_values(j);
                v.resize(n, 0.0);
                v[0] = value;
                j["value"] = json!(v);
            }
        }
        let solved = solve(&current);
        let ok = solved.problems.is_empty();
        let mut values = serde_json::Map::new();
        for l in solved.report["loops"].as_array().into_iter().flatten() {
            for (k, v) in l["joints"].as_object().into_iter().flatten() {
                values.insert(k.clone(), v.clone());
            }
        }
        rows.push(json!({"value": value, "closed": ok, "residual": solved.report["residual"], "joints": values}));
        if !ok {
            stopped = json!({"at": value, "why": solved.problems.join(" ")});
            break;
        }
        reached = Some(value);
        current["joints"] = Value::Array(solved.joints);
    }
    json!({
        "joint": joint,
        "from": from,
        "to": to,
        "reached": reached,
        "stopped": stopped,
        "steps": rows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A four-bar linkage in the XY plane: ground pivots A (0, 0) and
    /// D (d, 0), crank AB of length a, coupler BC of length b, rocker DC of
    /// length c. Each link is an instance hanging from the one before; the
    /// coupler's far end closes on the rocker.
    fn four_bar(a: f64, b: f64, c: f64, d: f64, crank: f64) -> Value {
        json!({
            "kind": "assembly.v1", "id": "fourbar", "parts": {},
            "instances": [
                {"id": "ground"},
                {"id": "crank", "parent": "ground"},
                {"id": "coupler", "parent": "crank", "at": [a, 0, 0]},
                {"id": "rocker", "parent": "ground", "at": [d, 0, 0]}
            ],
            "joints": [
                {"id": "a", "type": "revolute", "parent": "ground", "child": "crank",
                 "origin": [0, 0, 0], "axis": [0, 0, 1], "value": crank, "drive": true},
                {"id": "b", "type": "revolute", "parent": "crank", "child": "coupler",
                 "origin": [a, 0, 0], "axis": [0, 0, 1], "value": 40},
                {"id": "d", "type": "revolute", "parent": "ground", "child": "rocker",
                 "origin": [d, 0, 0], "axis": [0, 0, 1], "value": 60},
                {"id": "c", "type": "revolute", "closes": true, "parent": "coupler", "child": "rocker",
                 "origin": [b, 0, 0], "axis": [0, 0, 1], "child_origin": [c, 0, 0]}
            ]
        })
    }

    fn value(solved: &Solved, id: &str) -> f64 {
        let j = solved.joints.iter().find(|j| j["id"] == id).unwrap();
        doc::joint_values(j)[0]
    }

    /// The rocker's angle for a crank angle, by the closed-form (Freudenstein)
    /// solution on the open branch.
    fn rocker_analytic(a: f64, b: f64, c: f64, d: f64, theta: f64) -> f64 {
        let (bx, by) = (a * theta.cos(), a * theta.sin());
        // C is where circles about B (radius b) and D (radius c) meet, above the line BD.
        let (dx, dy) = (d - bx, -by);
        let l = (dx * dx + dy * dy).sqrt();
        let along = (l * l + b * b - c * c) / (2.0 * l);
        let h = (b * b - along * along).sqrt();
        let (ux, uy) = (dx / l, dy / l);
        let (cx, cy) = (bx + along * ux - h * uy, by + along * uy + h * ux);
        (cy).atan2(cx - d)
    }

    #[test]
    fn a_four_bar_closes_and_traces_its_analytic_coupler_curve() {
        let (a, b, c, d) = (40.0, 120.0, 80.0, 100.0);
        let mut doc = four_bar(a, b, c, d, 0.0);
        for step in 0..=36 {
            let theta = f64::from(step) * 10.0;
            doc["joints"][0]["value"] = json!(theta);
            let solved = solve(&doc);
            assert!(solved.problems.is_empty(), "{:?}", solved.problems);
            let report = &solved.report;
            assert!(report["residual"].as_f64().unwrap() < 1e-9, "{report}");
            let l = &report["loops"][0];
            // A four-bar moves with one degree of freedom; driven, none left.
            assert_eq!(l["mobility"], 1, "{l}");
            assert_eq!(l["dof"], 0, "{l}");
            // Three revolutes in a plane, written in 3D: three equations repeat.
            assert_eq!(l["redundant"], 3, "{l}");
            let want = rocker_analytic(a, b, c, d, theta.to_radians()).to_degrees();
            let got = value(&solved, "d");
            let diff = (want - got + 540.0).rem_euclid(360.0) - 180.0;
            assert!(diff.abs() < 1e-6, "crank {theta}: rocker {got} vs {want}");
            // The coupler's far point (the coupler curve) where the analytic C is.
            let worlds = crate::build::placements(
                &json!({"instances": doc["instances"], "joints": solved.joints}),
            );
            let p = worlds["coupler"].apply([b, 0.0, 0.0]);
            let r = want.to_radians();
            let want_c = [d + c * r.cos(), c * r.sin()];
            assert!(
                (p[0] - want_c[0]).abs() < 1e-6 && (p[1] - want_c[1]).abs() < 1e-6,
                "{p:?} vs {want_c:?}"
            );
            doc["joints"] = Value::Array(solved.joints);
        }
    }

    #[test]
    fn a_slider_crank_follows_its_crank() {
        // Crank r about the origin, rod l from the crank pin to a block
        // sliding along X: x = r cos θ + sqrt(l² − r² sin² θ).
        let (r, l) = (30.0, 100.0);
        let mut doc = json!({
            "kind": "assembly.v1", "id": "engine", "parts": {},
            "instances": [
                {"id": "frame"},
                {"id": "crank", "parent": "frame"},
                {"id": "rod", "parent": "crank", "at": [r, 0, 0]},
                {"id": "block", "parent": "frame"}
            ],
            "joints": [
                {"id": "shaft", "type": "revolute", "parent": "frame", "child": "crank",
                 "origin": [0, 0, 0], "axis": [0, 0, 1], "value": 0, "drive": true},
                {"id": "pin", "type": "revolute", "parent": "crank", "child": "rod",
                 "origin": [r, 0, 0], "axis": [0, 0, 1], "value": 0},
                {"id": "bore", "type": "slider", "parent": "frame", "child": "block",
                 "origin": [0, 0, 0], "axis": [1, 0, 0], "value": 130, "limits": [[0, 200]]},
                {"id": "wrist", "type": "revolute", "closes": true, "parent": "rod", "child": "block",
                 "origin": [l, 0, 0], "axis": [0, 0, 1], "child_origin": [0, 0, 0]}
            ]
        });
        for step in 0..=24 {
            let theta = f64::from(step) * 15.0;
            doc["joints"][0]["value"] = json!(theta);
            let solved = solve(&doc);
            assert!(solved.problems.is_empty(), "{:?}", solved.problems);
            let t = theta.to_radians();
            let want = r * t.cos() + (l * l - r * r * t.sin() * t.sin()).sqrt();
            assert!((value(&solved, "bore") - want).abs() < 1e-6, "{theta}");
            doc["joints"] = Value::Array(solved.joints);
        }
        // The block's stroke limit stops it: a limit short of the crank's
        // reach is reported, never forced.
        doc["joints"][2]["limits"] = json!([[0, 120]]);
        doc["joints"][0]["value"] = json!(0);
        let solved = solve(&doc);
        assert!(
            solved
                .problems
                .join(" ")
                .contains("bore is at its limit (120)"),
            "{:?}",
            solved.problems
        );
    }

    #[test]
    fn over_constrained_loops_are_reported_not_forced() {
        let (a, b, c, d) = (40.0, 120.0, 80.0, 100.0);
        // Driving the rocker as well as the crank asks for two inputs of a
        // one-degree-of-freedom loop.
        let mut doc = four_bar(a, b, c, d, 30.0);
        doc["joints"][2]["drive"] = json!(true);
        doc["joints"][2]["value"] = json!(10);
        let solved = solve(&doc);
        let why = solved.problems.join(" ");
        assert!(why.contains("driven joints (a, d)"), "{why}");
        assert_eq!(solved.report["loops"][0]["closed"], false);
        // Links that can't reach: a coupler too short to span the gap.
        let doc = four_bar(10.0, 20.0, 20.0, 100.0, 0.0);
        let why = solve(&doc).problems.join(" ");
        assert!(why.contains("can't reach"), "{why}");
        // A gear coupling between the crank and a second crank on the
        // ground is held: the second turns at twice the crank.
        let mut doc = four_bar(a, b, c, d, 25.0);
        doc["instances"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id": "fly", "parent": "ground", "at": [0, -200, 0]}));
        let joints = doc["joints"].as_array_mut().unwrap();
        joints.push(
            json!({"id": "fly_axle", "type": "revolute", "parent": "ground", "child": "fly",
            "origin": [0, -200, 0], "axis": [0, 0, 1], "value": 0}),
        );
        joints.push(json!({"id": "belt", "type": "gear", "closes": true, "driver": "a", "driven": "fly_axle", "ratio": 2}));
        let solved = solve(&doc);
        assert!(solved.problems.is_empty(), "{:?}", solved.problems);
        assert!((value(&solved, "fly_axle") - 50.0).abs() < 1e-9);
        // Driving the flywheel too contradicts the belt: reported.
        doc["joints"][4]["drive"] = json!(true);
        doc["joints"][4]["value"] = json!(10);
        assert!(!solve(&doc).problems.is_empty());
    }

    #[test]
    fn a_sweep_moves_the_mechanism_until_a_limit_stops_it() {
        let mut doc = four_bar(40.0, 120.0, 80.0, 100.0, 0.0);
        doc["joints"][2]["limits"] = json!([[0, 120]]);
        let out = sweep(&doc, "a", 0.0, 360.0, 36);
        let reached = out["reached"].as_f64().unwrap();
        assert!(reached > 0.0 && reached < 360.0, "{out}");
        assert!(
            out["stopped"]["why"]
                .as_str()
                .unwrap()
                .contains("d is at its limit"),
            "{out}"
        );
        // Without the limit it turns all the way round (a crank-rocker).
        doc["joints"][2]["limits"] = Value::Null;
        let out = sweep(&doc, "a", 0.0, 360.0, 36);
        assert_eq!(out["reached"], 360.0, "{}", out["stopped"]);
    }
}
