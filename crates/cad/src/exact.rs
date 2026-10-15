//! Exact mass properties, as a cross-check of ours.
//!
//! Our mass properties integrate over the solids' triangles ([`crate::mass`]),
//! exact for the triangles but off the true curved faces by the display
//! tolerance. For a part flagged `"exact_mass": true` (or `{"tolerance":
//! 0.002}`), every edit also has SMLib (the open NURBS kernel, Apache-2.0)
//! integrate over the exact surfaces of the part's STEP model, and refuses
//! the edit when the two disagree by more than the tolerance (1% unless the
//! part says): the volume, the centre of mass (against the part's size) and
//! the inertia tensor (against its largest moment).
//!
//! SMLib runs through its Python binding, `smlibpy` (MIT), as a separate
//! process with a small script of ours; nothing of it is linked or
//! vendored. Point `CAD_SMLIB_PYTHON` at a Python (3.10 or newer) with
//! `pip install smlibpy` done, or have `python3` be one. Without it, flagged
//! parts say plainly that the exact check didn't run, and the edit isn't
//! refused for that.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use serde_json::{Value, json};

use crate::build::Built;
use crate::mass::Sum;
use crate::xf::Xf;

/// Our script: each solid of a STEP file's exact volume, centre and
/// inertia tensor (unit density, mm), one JSON line.
const SCRIPT: &str = r#"
import json, sys
import smlibpy as s
shapes = s.import_step(sys.argv[1])
items = list(shapes) if isinstance(shapes, s.ShapeList) else [shapes]
out = []
for p in items:
    if not hasattr(p, "mass_properties"):
        continue
    m = p.mass_properties(1.0, float(sys.argv[2]))
    out.append({"volume": float(m.volume), "center": [float(x) for x in m.center],
                "inertia": [[float(x) for x in row] for row in m.inertia]})
print(json.dumps(out))
"#;

/// The Python with SMLib this machine has: `CAD_SMLIB_PYTHON`, else
/// `python3` if it has `smlibpy`. Looked for once.
#[must_use]
pub fn python() -> Option<PathBuf> {
    static FOUND: OnceLock<Option<PathBuf>> = OnceLock::new();
    FOUND
        .get_or_init(|| {
            let candidate = std::env::var_os("CAD_SMLIB_PYTHON")
                .map_or_else(|| PathBuf::from("python3"), PathBuf::from);
            available(&candidate).then_some(candidate)
        })
        .clone()
}

/// Whether `python` can import SMLib's binding.
#[must_use]
pub fn available(python: &Path) -> bool {
    Command::new(python)
        .args(["-I", "-c", "import smlibpy"])
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Exact mass properties at unit density: volume (mm³), centre (mm) and
/// the inertia tensor about the centre (mm⁵, the part's axes).
#[derive(Clone, Copy, Debug)]
pub struct Exact {
    pub volume: f64,
    pub center: [f64; 3],
    pub inertia: [[f64; 3]; 3],
}

/// SMLib's exact mass properties of every solid in a STEP file, summed.
///
/// # Errors
/// Python or SMLib failed, or the file has no solid.
pub fn of_step(python: &Path, step: &Path) -> Result<Exact, String> {
    let out = Command::new(python)
        .args(["-I", "-c", SCRIPT])
        .arg(step)
        .arg("1e-7")
        .output()
        .map_err(|e| format!("SMLib couldn't start: {e}"))?;
    if !out.status.success() {
        let why = String::from_utf8_lossy(&out.stderr);
        return Err(format!(
            "SMLib couldn't measure it: {}",
            why.lines().last().unwrap_or("no reason given")
        ));
    }
    let bodies: Vec<Value> = serde_json::from_slice(
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .last()
            .unwrap_or("[]")
            .as_bytes(),
    )
    .map_err(|e| format!("SMLib's answer wasn't readable: {e}"))?;
    // Summed about the origin, then moved to the joint centre.
    let mut volume = 0.0;
    let mut first = [0.0; 3];
    let mut about_origin = [[0.0; 3]; 3];
    for b in &bodies {
        let v = b["volume"].as_f64().unwrap_or(0.0);
        let c: [f64; 3] = [0, 1, 2].map(|k| b["center"][k].as_f64().unwrap_or(0.0));
        let i: [[f64; 3]; 3] =
            [0, 1, 2].map(|r| [0, 1, 2].map(|k| b["inertia"][r][k].as_f64().unwrap_or(0.0)));
        volume += v;
        for k in 0..3 {
            first[k] += v * c[k];
        }
        let shifted = parallel_axis(&i, v, c);
        for r in 0..3 {
            for k in 0..3 {
                about_origin[r][k] += shifted[r][k];
            }
        }
    }
    if volume <= 0.0 {
        return Err("SMLib found no solid in it.".into());
    }
    let center = first.map(|x| x / volume);
    let back = parallel_axis(&about_origin, -volume, center);
    Ok(Exact {
        volume,
        center,
        inertia: back,
    })
}

/// An inertia tensor about a body's centre `c` moved to the origin (or,
/// with a negative mass, back).
fn parallel_axis(i: &[[f64; 3]; 3], m: f64, c: [f64; 3]) -> [[f64; 3]; 3] {
    let d2 = c[0] * c[0] + c[1] * c[1] + c[2] * c[2];
    let mut out = *i;
    for r in 0..3 {
        for k in 0..3 {
            let delta = if r == k { d2 } else { 0.0 };
            out[r][k] += m * (delta - c[r] * c[k]);
        }
    }
    out
}

/// A part's tolerance when it asks for the exact check.
fn tolerance(part: &Value) -> Option<f64> {
    match &part["exact_mass"] {
        Value::Bool(true) => Some(0.01),
        Value::Object(o) => Some(
            o.get("tolerance")
                .and_then(Value::as_f64)
                .filter(|t| *t > 0.0)
                .unwrap_or(0.01),
        ),
        _ => None,
    }
}

fn rotate_tensor(x: &Xf, i: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let mut out = [[0.0; 3]; 3];
    for (a, row) in out.iter_mut().enumerate() {
        for (b, cell) in row.iter_mut().enumerate() {
            *cell = (0..3)
                .flat_map(|p| (0..3).map(move |q| (p, q)))
                .map(|(p, q)| x.r[a][p] * i[p][q] * x.r[b][q])
                .sum();
        }
    }
    out
}

/// The exact cross-check of every flagged part: what each measured and
/// whether they agree. Returns the entries for the report and the problems
/// that refuse the edit.
#[must_use]
pub fn cross_check(built: &Built, python: Option<&Path>) -> (Vec<Value>, Vec<String>) {
    let mut entries = Vec::new();
    let mut problems = Vec::new();
    let parts = built.assembly["parts"].as_object();
    for (key, part) in parts.into_iter().flatten() {
        let Some(tol) = tolerance(part) else { continue };
        let Some(instance) = built.assembly["instances"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|i| i["part"] == key.as_str())
            .and_then(|i| i["id"].as_str())
        else {
            continue;
        };
        let Some(python) = python else {
            entries.push(json!({"part": key, "checked": false,
                "why": "SMLib isn't installed here (pip install smlibpy, Python 3.10+; or set CAD_SMLIB_PYTHON)"}));
            continue;
        };
        let Some(step) = built.part_steps.get(key) else {
            continue;
        };
        // Cached beside the part's STEP: the same part measures the same.
        let cached = step.with_extension("exact.json");
        let exact = match std::fs::read_to_string(&cached)
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        {
            Some(v) => Exact {
                volume: v["volume"].as_f64().unwrap_or(0.0),
                center: [0, 1, 2].map(|k| v["center"][k].as_f64().unwrap_or(0.0)),
                inertia: [0, 1, 2]
                    .map(|r| [0, 1, 2].map(|k| v["inertia"][r][k].as_f64().unwrap_or(0.0))),
            },
            None => match of_step(python, step) {
                Ok(e) => {
                    let _ = std::fs::write(
                        &cached,
                        json!({"volume": e.volume, "center": e.center, "inertia": e.inertia})
                            .to_string(),
                    );
                    e
                }
                Err(why) => {
                    problems.push(format!("Part {key}'s exact mass check failed: {why}"));
                    continue;
                }
            },
        };
        // Ours, of the same instance, at unit density (1 g/cm³).
        let mut sum = Sum::default();
        for s in built.solids.iter().filter(|s| s.instance == instance) {
            sum.add(&s.mesh, 1.0);
        }
        let ours = sum.mass();
        let place = built
            .placements
            .get(instance)
            .copied()
            .unwrap_or(Xf::IDENTITY);
        let center = place.apply(exact.center);
        let inertia = rotate_tensor(&place, &exact.inertia).map(|r| r.map(|x| x * 1e-3));
        let mut lo = [f64::MAX; 3];
        let mut hi = [f64::MIN; 3];
        for s in built.solids.iter().filter(|s| s.instance == instance) {
            for p in &s.mesh.positions {
                for (k, x) in [p.x, p.y, p.z].into_iter().enumerate() {
                    lo[k] = lo[k].min(x);
                    hi[k] = hi[k].max(x);
                }
            }
        }
        let size = (0..3)
            .map(|k| (hi[k] - lo[k]).powi(2))
            .sum::<f64>()
            .sqrt()
            .max(1e-9);
        let volume_off = (ours.volume_mm3 - exact.volume).abs() / exact.volume;
        let center_off = (0..3)
            .map(|k| (ours.com[k] - center[k]).powi(2))
            .sum::<f64>()
            .sqrt();
        let largest = (0..3)
            .map(|k| inertia[k][k].abs())
            .fold(0.0, f64::max)
            .max(1e-12);
        let inertia_off = (0..3)
            .flat_map(|r| (0..3).map(move |k| (r, k)))
            .map(|(r, k)| (ours.inertia[r][k] - inertia[r][k]).abs())
            .fold(0.0, f64::max)
            / largest;
        let agrees = volume_off <= tol && center_off <= tol * size && inertia_off <= tol;
        let pct = |x: f64| (x * 1e6).round() / 1e4;
        entries.push(json!({
            "part": key, "checked": true, "agrees": agrees, "tolerance": tol,
            "exact": {"volume_cm3": exact.volume / 1000.0, "center_of_mass_mm": center.map(|x| (x * 100.0).round() / 100.0)},
            "difference": {"volume_pct": pct(volume_off), "center_mm": (center_off * 1000.0).round() / 1000.0,
                           "inertia_pct": pct(inertia_off)},
        }));
        if !agrees {
            problems.push(format!(
                "Part {key}'s mass properties from its triangles and SMLib's exact ones disagree beyond {}%: volume by {}%, centre of mass by {:.3} mm, inertia by {}%.",
                tol * 100.0,
                pct(volume_off),
                center_off,
                pct(inertia_off)
            ));
        }
    }
    (entries, problems)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_parallel_axis_moves_there_and_back() {
        let i = [[2.0, 0.1, 0.0], [0.1, 3.0, 0.2], [0.0, 0.2, 4.0]];
        let there = parallel_axis(&i, 5.0, [1.0, -2.0, 3.0]);
        let back = parallel_axis(&there, -5.0, [1.0, -2.0, 3.0]);
        for r in 0..3 {
            for k in 0..3 {
                assert!((back[r][k] - i[r][k]).abs() < 1e-12);
            }
        }
        assert!((there[0][0] - (2.0 + 5.0 * 13.0)).abs() < 1e-12);
    }
}
