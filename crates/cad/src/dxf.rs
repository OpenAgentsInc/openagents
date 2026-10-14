//! Polylines and DXF.
//!
//! A **polyline** is a sketch drawing step of our own,
//! `{"op": "sketch.polyline", "params": {"points": [[x, y], [x, y, bulge], ...],
//! "closed": true}}`: points joined by straight segments, or by circular
//! arcs where a point carries a bulge (the tangent of a quarter of the arc's
//! included angle, positive turning counter-clockwise, as DXF's LWPOLYLINE
//! has it). It is how a 2D outline lives in a document as small, reviewable
//! JSON; the build turns it into the kernel's lines and arcs.
//!
//! **DXF in:** [`import`] reads an ASCII DXF drawing (lines, arcs, circles,
//! polylines with bulges, ellipses, splines, text; blocks expanded) into
//! sketch drawing steps, joining touching lines and arcs into polylines, so
//! DXF stays the authoring format and the document holds the JSON.
//!
//! **DXF out:** [`section`] cuts a built model by a plane into a DXF
//! drawing; a sketch's own DXF comes from the kernel
//! ([`crate::build::sketch_dxf`]).

use serde_json::{Value, json};
use solidcraft_engine::geom::Vec2;
use solidcraft_engine::io::{Geom2, read_dxf, write_dxf};

use crate::build::Built;

/// The most points one polyline may have.
pub const MAX_POINTS: usize = 20_000;

fn round(x: f64) -> f64 {
    (x * 1e6).round() / 1e6
}

/// The kernel drawing steps a polyline becomes: runs of straight segments
/// as `sketch.line`, each bulged segment as `sketch.arc.three_point`.
///
/// # Errors
/// Points that aren't `[x, y]` or `[x, y, bulge]`, or too few.
pub fn polyline_steps(params: &Value) -> Result<Vec<(String, Value)>, String> {
    let list = params["points"]
        .as_array()
        .ok_or("a polyline needs points: [[x, y], [x, y, bulge], ...]")?;
    if list.len() < 2 || list.len() > MAX_POINTS {
        return Err(format!(
            "a polyline needs 2 to {MAX_POINTS} points (got {})",
            list.len()
        ));
    }
    let mut points = Vec::with_capacity(list.len());
    for p in list {
        let a = p.as_array().filter(|a| (2..=3).contains(&a.len()));
        let coords: Option<Vec<f64>> = a
            .map(|a| a.iter().map(Value::as_f64).collect::<Option<Vec<_>>>())
            .flatten();
        match coords {
            Some(c) if c.iter().all(|x| x.is_finite()) => {
                points.push((c[0], c[1], c.get(2).copied().unwrap_or(0.0)));
            }
            _ => {
                return Err(format!(
                    "a polyline point is [x, y] or [x, y, bulge] (got {p})"
                ));
            }
        }
    }
    let closed = params["closed"] == true;
    let n = points.len();
    let segments = if closed { n } else { n - 1 };
    let mut steps = Vec::new();
    let mut run: Vec<[f64; 2]> = Vec::new();
    let flush = |run: &mut Vec<[f64; 2]>, steps: &mut Vec<(String, Value)>| {
        if run.len() >= 2 {
            steps.push((
                "sketch.line".to_owned(),
                json!({"points": run.clone(), "infer": false}),
            ));
        }
        run.clear();
    };
    for i in 0..segments {
        let (x0, y0, bulge) = points[i];
        let (x1, y1, _) = points[(i + 1) % n];
        if (x1 - x0).hypot(y1 - y0) < 1e-9 {
            continue;
        }
        if bulge.abs() < 1e-12 {
            if run.is_empty() {
                run.push([x0, y0]);
            }
            run.push([x1, y1]);
            continue;
        }
        flush(&mut run, &mut steps);
        // The arc's middle: off the chord's middle by the sagitta, to the
        // right of the chord for a counter-clockwise (positive) bulge.
        let (dx, dy) = (x1 - x0, y1 - y0);
        let half = dx.hypot(dy) / 2.0;
        let sagitta = bulge * half;
        let (rx, ry) = (dy / (2.0 * half), -dx / (2.0 * half));
        let through = [
            round((x0 + x1) / 2.0 + rx * sagitta),
            round((y0 + y1) / 2.0 + ry * sagitta),
        ];
        steps.push((
            "sketch.arc.three_point".to_owned(),
            json!({"start": [x0, y0], "end": [x1, y1], "through": through}),
        ));
    }
    flush(&mut run, &mut steps);
    if steps.is_empty() {
        return Err("a polyline needs two different points".into());
    }
    Ok(steps)
}

/// A segment of an outline: from, to, bulge.
#[derive(Clone, Copy)]
struct Seg {
    a: Vec2,
    b: Vec2,
    bulge: f64,
}

fn same(a: Vec2, b: Vec2, tol: f64) -> bool {
    a.dist(b) <= tol
}

/// Touching lines and arcs joined into polylines (closed where they meet
/// their start).
fn chains(segs: Vec<Seg>, tol: f64) -> Vec<Value> {
    let mut used = vec![false; segs.len()];
    let mut out = Vec::new();
    for start in 0..segs.len() {
        if used[start] {
            continue;
        }
        used[start] = true;
        let mut chain = vec![segs[start]];
        // Grow at the end, then at the start.
        loop {
            let end = chain.last().map_or(Vec2::ZERO, |s| s.b);
            let next = (0..segs.len())
                .find(|&j| !used[j] && (same(segs[j].a, end, tol) || same(segs[j].b, end, tol)));
            let Some(j) = next else { break };
            used[j] = true;
            let s = segs[j];
            chain.push(if same(s.a, end, tol) {
                s
            } else {
                Seg {
                    a: s.b,
                    b: s.a,
                    bulge: -s.bulge,
                }
            });
        }
        loop {
            let begin = chain.first().map_or(Vec2::ZERO, |s| s.a);
            let prev = (0..segs.len()).find(|&j| {
                !used[j] && (same(segs[j].a, begin, tol) || same(segs[j].b, begin, tol))
            });
            let Some(j) = prev else { break };
            used[j] = true;
            let s = segs[j];
            chain.insert(
                0,
                if same(s.b, begin, tol) {
                    s
                } else {
                    Seg {
                        a: s.b,
                        b: s.a,
                        bulge: -s.bulge,
                    }
                },
            );
        }
        let closed =
            chain.len() > 1 && same(chain.last().map_or(Vec2::ZERO, |s| s.b), chain[0].a, tol);
        let mut points: Vec<Value> = chain
            .iter()
            .map(|s| {
                if s.bulge.abs() < 1e-12 {
                    json!([round(s.a.x), round(s.a.y)])
                } else {
                    json!([round(s.a.x), round(s.a.y), round(s.bulge)])
                }
            })
            .collect();
        if !closed {
            let last = chain.last().map_or(Vec2::ZERO, |s| s.b);
            points.push(json!([round(last.x), round(last.y)]));
        }
        let mut params = json!({"points": points});
        if closed {
            params["closed"] = json!(true);
        }
        out.push(json!({"op": "sketch.polyline", "params": params}));
    }
    out
}

/// An ASCII DXF drawing as sketch drawing steps (millimetres; `scale`
/// multiplies, `at` moves). Lines and arcs that touch become polylines.
///
/// # Errors
/// A drawing the reader can't read, or one with nothing to draw.
pub fn import(text: &str, scale: f64, at: [f64; 2]) -> Result<Vec<Value>, String> {
    if !(scale.is_finite() && scale > 0.0) {
        return Err("scale must be a positive number".into());
    }
    let geometry = read_dxf(text).map_err(|e| format!("the DXF can't be read: {e}"))?;
    let place = |p: Vec2| Vec2 {
        x: p.x * scale + at[0],
        y: p.y * scale + at[1],
    };
    let xy = |p: Vec2| {
        let p = place(p);
        json!([round(p.x), round(p.y)])
    };
    let mut segs = Vec::new();
    let mut steps = Vec::new();
    let mut size = 0.0f64;
    for g in &geometry {
        match g {
            Geom2::Line(a, b) => segs.push(Seg {
                a: place(*a),
                b: place(*b),
                bulge: 0.0,
            }),
            Geom2::Arc { c, a, b } => {
                let mut sweep = (*b - *c).angle() - (*a - *c).angle();
                while sweep <= 0.0 {
                    sweep += std::f64::consts::TAU;
                }
                segs.push(Seg {
                    a: place(*a),
                    b: place(*b),
                    bulge: (sweep / 4.0).tan(),
                });
            }
            Geom2::Circle(c, r) => steps.push(json!({"op": "sketch.circle.center",
                "params": {"center": xy(*c), "radius": round(r * scale)}})),
            Geom2::Ellipse { c, major, minor } => {
                let axis = *major;
                let len = axis.len().max(1e-12);
                let perp = Vec2 {
                    x: -axis.y / len * minor,
                    y: axis.x / len * minor,
                };
                steps.push(json!({"op": "sketch.ellipse", "params": {
                    "center": xy(*c), "major": xy(*c + axis), "minor": xy(*c + perp)}}));
            }
            Geom2::Spline { pts, control, .. } => {
                let points: Vec<Value> = pts.iter().map(|p| xy(*p)).collect();
                let op = if *control {
                    "sketch.spline.control_point"
                } else {
                    "sketch.spline.fit_point"
                };
                steps.push(json!({"op": op, "params": {"points": points}}));
            }
            Geom2::Conic { a, apex, b, rho } => steps.push(json!({"op": "sketch.conic",
                "params": {"start": xy(*a), "apex": xy(*apex), "end": xy(*b), "rho": rho}})),
            Geom2::Text {
                text,
                at: p,
                height,
                ..
            } => steps.push(json!({"op": "sketch.text",
                "params": {"text": text, "at": xy(*p), "height": round(height * scale)}})),
            Geom2::Point(_) => {}
        }
    }
    for s in &segs {
        size = size.max(s.a.x.abs()).max(s.a.y.abs());
    }
    let mut out = chains(segs, (size * 1e-9).max(1e-6));
    out.extend(steps);
    if out.is_empty() {
        return Err(
            "the DXF has nothing to draw (lines, arcs, circles, polylines, splines or text)".into(),
        );
    }
    Ok(out)
}

/// The plane a section cuts along: `XY` at height `at` (drawing x, y),
/// `XZ` at `y = at` (drawing x, z) or `YZ` at `x = at` (drawing y, z).
fn axes(plane: &str) -> Option<(usize, usize, usize)> {
    match plane {
        "XY" => Some((0, 1, 2)),
        "XZ" => Some((0, 2, 1)),
        "YZ" => Some((1, 2, 0)),
        _ => None,
    }
}

/// A built model cut by a plane, as a DXF drawing of the cut's outline
/// (the solids' triangles where they cross the plane, so curved faces are
/// as fine as the display tolerance). Returns the DXF and its line count.
///
/// # Errors
/// An unknown plane, or a plane that misses every solid.
pub fn section(built: &Built, plane: &str, at: f64) -> Result<(String, usize), String> {
    let (u, v, n) = axes(plane).ok_or("the section plane is XY, XZ or YZ")?;
    let mut lines = Vec::new();
    for solid in &built.solids {
        let m = &solid.mesh;
        for t in &m.triangles {
            let p = t.map(|i| {
                let q = m.positions[i as usize];
                [q.x, q.y, q.z]
            });
            let d: Vec<f64> = p.iter().map(|q| q[n] - at).collect();
            let mut cut = Vec::new();
            for (i, j) in [(0, 1), (1, 2), (2, 0)] {
                if (d[i] > 0.0) != (d[j] > 0.0) {
                    let s = d[i] / (d[i] - d[j]);
                    cut.push(Vec2 {
                        x: p[i][u] + (p[j][u] - p[i][u]) * s,
                        y: p[i][v] + (p[j][v] - p[i][v]) * s,
                    });
                }
            }
            if let [a, b] = cut[..]
                && a.dist(b) > 1e-9
            {
                lines.push(Geom2::Line(a, b));
            }
        }
    }
    if lines.is_empty() {
        return Err(format!(
            "the {plane} plane at {at} mm doesn't cut any solid"
        ));
    }
    let count = lines.len();
    Ok((write_dxf(&lines), count))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bulged_polyline_becomes_lines_and_arcs() {
        // A slot: two straight sides and two half circles (bulge 1).
        let steps = polyline_steps(&json!({"closed": true,
            "points": [[0, 0], [40, 0, 1], [40, 10], [0, 10, 1]]}))
        .unwrap();
        let ops: Vec<&str> = steps.iter().map(|(op, _)| op.as_str()).collect();
        assert_eq!(
            ops,
            [
                "sketch.line",
                "sketch.arc.three_point",
                "sketch.line",
                "sketch.arc.three_point"
            ]
        );
        // The right end bulges out to x = 45.
        assert_eq!(steps[1].1["through"], json!([45.0, 5.0]));
        assert_eq!(steps[3].1["through"], json!([-5.0, 5.0]));
        assert!(polyline_steps(&json!({"points": [[0, 0]]})).is_err());
    }

    #[test]
    fn a_dxf_outline_becomes_one_closed_polyline() {
        // The same slot as LINE and ARC entities, in no particular order.
        let dxf = "0\nSECTION\n2\nENTITIES\n\
            0\nARC\n8\n0\n10\n40\n20\n5\n40\n5\n50\n-90\n51\n90\n\
            0\nLINE\n8\n0\n10\n0\n20\n0\n11\n40\n21\n0\n\
            0\nLINE\n8\n0\n10\n40\n20\n10\n11\n0\n21\n10\n\
            0\nARC\n8\n0\n10\n0\n20\n5\n40\n5\n50\n90\n51\n270\n\
            0\nCIRCLE\n8\n0\n10\n20\n20\n5\n40\n2\n\
            0\nENDSEC\n0\nEOF\n";
        let steps = import(dxf, 1.0, [0.0, 0.0]).unwrap();
        assert_eq!(steps.len(), 2, "{steps:?}");
        assert_eq!(steps[0]["op"], "sketch.polyline");
        assert_eq!(steps[0]["params"]["closed"], true, "{}", steps[0]);
        assert_eq!(steps[0]["params"]["points"].as_array().unwrap().len(), 4);
        assert_eq!(steps[1]["op"], "sketch.circle.center");
        // And back out: the kernel's DXF reader reads what we write.
        let lines = write_dxf(&[Geom2::Line(
            Vec2 { x: 0.0, y: 0.0 },
            Vec2 { x: 3.0, y: 4.0 },
        )]);
        assert_eq!(read_dxf(&lines).unwrap().len(), 1);
    }
}
