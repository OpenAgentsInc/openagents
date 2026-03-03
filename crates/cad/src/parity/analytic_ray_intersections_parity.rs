use std::f64::consts::PI;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::hash::stable_hex_digest;
use crate::parity::scorecard::ParityScorecard;
use crate::{CadError, CadResult};

pub const PARITY_ANALYTIC_RAY_INTERSECTIONS_ISSUE_ID: &str = "VCAD-PARITY-098";
pub const ANALYTIC_RAY_INTERSECTIONS_REFERENCE_FIXTURE_PATH: &str =
    "crates/cad/parity/fixtures/analytic_ray_intersections_vcad_reference.json";
const ANALYTIC_RAY_INTERSECTIONS_REFERENCE_FIXTURE_JSON: &str =
    include_str!("../../parity/fixtures/analytic_ray_intersections_vcad_reference.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnalyticRayIntersectionsParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_fixture_path: String,
    pub reference_fixture_sha256: String,
    pub reference_source: String,
    pub reference_commit_match: bool,
    pub sample_set_match: bool,
    pub surface_coverage_match: bool,
    pub hit_ordering_match: bool,
    pub positive_t_filter_match: bool,
    pub deterministic_replay_match: bool,
    pub samples: Vec<AnalyticIntersectionSample>,
    pub covered_surfaces: Vec<String>,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct AnalyticRayIntersectionsReferenceFixture {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    expected_samples: Vec<AnalyticIntersectionSample>,
    expected_surfaces: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct AnalyticRayIntersectionsSnapshot {
    samples: Vec<AnalyticIntersectionSample>,
    covered_surfaces: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnalyticIntersectionSample {
    pub case_id: String,
    pub surface: String,
    pub hit_count: usize,
    pub t_values: Vec<f64>,
    pub uv_values: Vec<[f64; 2]>,
}

#[derive(Debug, Clone, Copy)]
struct Ray3 {
    origin: [f64; 3],
    direction: [f64; 3],
}

#[derive(Debug, Clone, Copy)]
struct Hit {
    t: f64,
    uv: [f64; 2],
}

pub fn build_analytic_ray_intersections_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<AnalyticRayIntersectionsParityManifest> {
    let reference: AnalyticRayIntersectionsReferenceFixture = serde_json::from_str(
        ANALYTIC_RAY_INTERSECTIONS_REFERENCE_FIXTURE_JSON,
    )
    .map_err(|error| CadError::ParseFailed {
        reason: format!("failed parsing analytic ray intersections reference fixture: {error}"),
    })?;

    let reference_fixture_sha256 =
        sha256_hex(ANALYTIC_RAY_INTERSECTIONS_REFERENCE_FIXTURE_JSON.as_bytes());
    let reference_commit_match = reference.vcad_commit == scorecard.vcad_commit;

    let snapshot = collect_snapshot();
    let replay_snapshot = collect_snapshot();
    let deterministic_replay_match = snapshot == replay_snapshot;

    let sample_set_match = sample_sets_match(
        snapshot.samples.clone(),
        reference.expected_samples.clone(),
        1e-6,
    );
    let surface_coverage_match = sorted_strings(snapshot.covered_surfaces.clone())
        == sorted_strings(reference.expected_surfaces.clone());
    let hit_ordering_match = snapshot.samples.iter().all(|sample| {
        sample
            .t_values
            .windows(2)
            .all(|pair| pair[0] <= pair[1] + 1e-9)
    });
    let positive_t_filter_match = snapshot
        .samples
        .iter()
        .flat_map(|sample| sample.t_values.iter())
        .all(|t| *t >= -1e-9);

    let deterministic_signature = parity_signature(
        &snapshot,
        reference_commit_match,
        sample_set_match,
        surface_coverage_match,
        hit_ordering_match,
        positive_t_filter_match,
        deterministic_replay_match,
        &reference_fixture_sha256,
    );

    Ok(AnalyticRayIntersectionsParityManifest {
        manifest_version: 1,
        issue_id: PARITY_ANALYTIC_RAY_INTERSECTIONS_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_fixture_path: ANALYTIC_RAY_INTERSECTIONS_REFERENCE_FIXTURE_PATH.to_string(),
        reference_fixture_sha256,
        reference_source: reference.source,
        reference_commit_match,
        sample_set_match,
        surface_coverage_match,
        hit_ordering_match,
        positive_t_filter_match,
        deterministic_replay_match,
        samples: snapshot.samples,
        covered_surfaces: snapshot.covered_surfaces,
        deterministic_signature,
        parity_contracts: vec![
            "plane intersections return no hit for parallel/behind rays and project UV on hit"
                .to_string(),
            "cylinder and sphere quadratic solvers return positive-t hits sorted in ascending order"
                .to_string(),
            "cone intersections filter opposite nappe via v>=0 and keep linear/quadratic edge behavior"
                .to_string(),
            "torus quartic solver returns deterministic sorted roots for analytic hit points"
                .to_string(),
            "analytic surface coverage remains locked to plane/cylinder/sphere/cone/torus"
                .to_string(),
        ],
    })
}

fn collect_snapshot() -> AnalyticRayIntersectionsSnapshot {
    let mut samples = vec![
        sample_from_hits(
            "plane_perpendicular_hit",
            "plane",
            vec![
                intersect_plane(
                    Ray3::new([0.0, 0.0, 5.0], [0.0, 0.0, -1.0]),
                    [0.0, 0.0, 0.0],
                    [0.0, 0.0, 1.0],
                )
                .expect("plane_perpendicular_hit should hit"),
            ],
        ),
        sample_from_hits(
            "plane_parallel_miss",
            "plane",
            intersect_plane(
                Ray3::new([0.0, 0.0, 5.0], [1.0, 0.0, 0.0]),
                [0.0, 0.0, 0.0],
                [0.0, 0.0, 1.0],
            )
            .into_iter()
            .collect(),
        ),
        sample_from_hits(
            "plane_behind_miss",
            "plane",
            intersect_plane(
                Ray3::new([0.0, 0.0, -5.0], [0.0, 0.0, -1.0]),
                [0.0, 0.0, 0.0],
                [0.0, 0.0, 1.0],
            )
            .into_iter()
            .collect(),
        ),
        sample_from_hits(
            "cylinder_through_center",
            "cylinder",
            intersect_cylinder(Ray3::new([-10.0, 0.0, 0.0], [1.0, 0.0, 0.0]), 5.0),
        ),
        sample_from_hits(
            "cylinder_tangent",
            "cylinder",
            intersect_cylinder(Ray3::new([5.0, -10.0, 0.0], [0.0, 1.0, 0.0]), 5.0),
        ),
        sample_from_hits(
            "cylinder_parallel_axis",
            "cylinder",
            intersect_cylinder(Ray3::new([2.0, 0.0, -10.0], [0.0, 0.0, 1.0]), 5.0),
        ),
        sample_from_hits(
            "sphere_through_center",
            "sphere",
            intersect_sphere(Ray3::new([-10.0, 0.0, 0.0], [1.0, 0.0, 0.0]), 5.0),
        ),
        sample_from_hits(
            "sphere_from_inside",
            "sphere",
            intersect_sphere(Ray3::new([0.0, 0.0, 0.0], [1.0, 0.0, 0.0]), 5.0),
        ),
        sample_from_hits(
            "sphere_tangent",
            "sphere",
            intersect_sphere(Ray3::new([5.0, -10.0, 0.0], [0.0, 1.0, 0.0]), 5.0),
        ),
        sample_from_hits(
            "cone_through_axis",
            "cone",
            intersect_cone(Ray3::new([-20.0, 0.0, 5.0], [1.0, 0.0, 0.0]), PI / 4.0),
        ),
        sample_from_hits(
            "cone_wrong_nappe",
            "cone",
            intersect_cone(Ray3::new([-20.0, 0.0, -5.0], [1.0, 0.0, 0.0]), PI / 4.0),
        ),
        sample_from_hits(
            "torus_through_center",
            "torus",
            intersect_torus(Ray3::new([-20.0, 0.0, 0.0], [1.0, 0.0, 0.0]), 10.0, 3.0),
        ),
        sample_from_hits(
            "torus_miss",
            "torus",
            intersect_torus(Ray3::new([-20.0, 0.0, 10.0], [1.0, 0.0, 0.0]), 10.0, 3.0),
        ),
    ];

    samples.sort_by(|left, right| left.case_id.cmp(&right.case_id));

    let mut covered_surfaces: Vec<String> = samples
        .iter()
        .map(|sample| sample.surface.clone())
        .collect();
    covered_surfaces.sort();
    covered_surfaces.dedup();

    AnalyticRayIntersectionsSnapshot {
        samples,
        covered_surfaces,
    }
}

fn sample_from_hits(
    case_id: &str,
    surface: &str,
    mut hits: Vec<Hit>,
) -> AnalyticIntersectionSample {
    hits.sort_by(|left, right| {
        left.t
            .partial_cmp(&right.t)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let t_values: Vec<f64> = hits.iter().map(|hit| canonical_f64(hit.t)).collect();
    let uv_values: Vec<[f64; 2]> = hits
        .iter()
        .map(|hit| [canonical_f64(hit.uv[0]), canonical_f64(hit.uv[1])])
        .collect();

    AnalyticIntersectionSample {
        case_id: case_id.to_string(),
        surface: surface.to_string(),
        hit_count: hits.len(),
        t_values,
        uv_values,
    }
}

impl Ray3 {
    fn new(origin: [f64; 3], direction: [f64; 3]) -> Self {
        Self {
            origin,
            direction: normalize(direction),
        }
    }

    fn at(&self, t: f64) -> [f64; 3] {
        add(self.origin, scale(self.direction, t))
    }
}

fn intersect_plane(ray: Ray3, plane_origin: [f64; 3], plane_normal: [f64; 3]) -> Option<Hit> {
    let denom = dot(ray.direction, plane_normal);
    if denom.abs() < 1e-12 {
        return None;
    }

    let t = dot(sub(plane_origin, ray.origin), plane_normal) / denom;
    if t < 0.0 {
        return None;
    }

    let point = ray.at(t);
    Some(Hit {
        t,
        uv: [point[0], point[1]],
    })
}

fn intersect_cylinder(ray: Ray3, radius: f64) -> Vec<Hit> {
    let axis = [0.0, 0.0, 1.0];
    let ref_dir = [1.0, 0.0, 0.0];

    let d = ray.direction;
    let oc = ray.origin;

    let d_perp = sub(d, scale(axis, dot(d, axis)));
    let oc_perp = sub(oc, scale(axis, dot(oc, axis)));

    let a = dot(d_perp, d_perp);
    let b = 2.0 * dot(oc_perp, d_perp);
    let c = dot(oc_perp, oc_perp) - radius * radius;

    if a.abs() < 1e-12 {
        return Vec::new();
    }

    let discriminant = b * b - 4.0 * a * c;
    if discriminant < 0.0 {
        return Vec::new();
    }

    let sqrt_disc = discriminant.sqrt();
    let t1 = (-b - sqrt_disc) / (2.0 * a);
    let t2 = (-b + sqrt_disc) / (2.0 * a);

    let mut hits = Vec::new();
    for t in [t1, t2] {
        if t < 0.0 {
            continue;
        }
        let point = ray.at(t);
        hits.push(Hit {
            t,
            uv: compute_cylinder_uv(point, axis, ref_dir),
        });
    }

    hits.sort_by(|left, right| {
        left.t
            .partial_cmp(&right.t)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    hits
}

fn compute_cylinder_uv(point: [f64; 3], axis: [f64; 3], ref_dir: [f64; 3]) -> [f64; 2] {
    let y_dir = cross(axis, ref_dir);
    let v = dot(point, axis);
    let proj = sub(point, scale(axis, v));
    let x = dot(proj, ref_dir);
    let y = dot(proj, y_dir);
    [normalize_angle(y.atan2(x)), v]
}

fn intersect_sphere(ray: Ray3, radius: f64) -> Vec<Hit> {
    let axis = [0.0, 0.0, 1.0];
    let ref_dir = [1.0, 0.0, 0.0];

    let oc = ray.origin;
    let d = ray.direction;

    let a = dot(d, d);
    let b = 2.0 * dot(oc, d);
    let c = dot(oc, oc) - radius * radius;

    let discriminant = b * b - 4.0 * a * c;
    if discriminant < 0.0 {
        return Vec::new();
    }

    let sqrt_disc = discriminant.sqrt();
    let t1 = (-b - sqrt_disc) / (2.0 * a);
    let t2 = (-b + sqrt_disc) / (2.0 * a);

    let mut hits = Vec::new();
    for t in [t1, t2] {
        if t < 0.0 {
            continue;
        }
        let point = ray.at(t);
        hits.push(Hit {
            t,
            uv: compute_sphere_uv(point, radius, axis, ref_dir),
        });
    }

    hits.sort_by(|left, right| {
        left.t
            .partial_cmp(&right.t)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    hits
}

fn compute_sphere_uv(point: [f64; 3], radius: f64, axis: [f64; 3], ref_dir: [f64; 3]) -> [f64; 2] {
    let y_dir = cross(axis, ref_dir);

    let to_point = scale(point, 1.0 / radius);
    let z = dot(to_point, axis).clamp(-1.0, 1.0);
    let v = z.asin();

    let proj = sub(to_point, scale(axis, z));
    let proj_len = norm(proj);
    if proj_len < 1e-12 {
        return [0.0, v];
    }

    let x = dot(proj, ref_dir) / proj_len;
    let y = dot(proj, y_dir) / proj_len;
    [normalize_angle(y.atan2(x)), v]
}

fn intersect_cone(ray: Ray3, half_angle: f64) -> Vec<Hit> {
    let axis = [0.0, 0.0, 1.0];
    let ref_dir = [1.0, 0.0, 0.0];

    let d = ray.direction;
    let co = ray.origin;

    let cos_a = half_angle.cos();
    let cos2 = cos_a * cos_a;

    let d_dot_a = dot(d, axis);
    let co_dot_a = dot(co, axis);

    let a = d_dot_a * d_dot_a - cos2;
    let b = 2.0 * (d_dot_a * co_dot_a - cos2 * dot(d, co));
    let c = co_dot_a * co_dot_a - cos2 * dot(co, co);

    let mut hits = Vec::new();

    if a.abs() < 1e-12 {
        if b.abs() > 1e-12 {
            let t = -c / b;
            if t >= 0.0 {
                let point = ray.at(t);
                let v = dot(point, axis) / cos_a;
                if v >= 0.0 {
                    hits.push(Hit {
                        t,
                        uv: compute_cone_uv(point, half_angle, axis, ref_dir),
                    });
                }
            }
        }
    } else {
        let discriminant = b * b - 4.0 * a * c;
        if discriminant >= 0.0 {
            let sqrt_disc = discriminant.sqrt();
            let t1 = (-b - sqrt_disc) / (2.0 * a);
            let t2 = (-b + sqrt_disc) / (2.0 * a);

            for t in [t1, t2] {
                if t < 0.0 {
                    continue;
                }
                let point = ray.at(t);
                let height_along_axis = dot(point, axis);
                let v = height_along_axis / cos_a;
                if v >= 0.0 {
                    hits.push(Hit {
                        t,
                        uv: compute_cone_uv(point, half_angle, axis, ref_dir),
                    });
                }
            }
        }
    }

    hits.sort_by(|left, right| {
        left.t
            .partial_cmp(&right.t)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    hits
}

fn compute_cone_uv(
    point: [f64; 3],
    half_angle: f64,
    axis: [f64; 3],
    ref_dir: [f64; 3],
) -> [f64; 2] {
    let y_dir = cross(axis, ref_dir);
    let cos_a = half_angle.cos();

    let height = dot(point, axis);
    let v = height / cos_a;

    let proj = sub(point, scale(axis, height));
    let proj_len = norm(proj);
    if proj_len < 1e-12 {
        return [0.0, v];
    }

    let x = dot(proj, ref_dir) / proj_len;
    let y = dot(proj, y_dir) / proj_len;
    [normalize_angle(y.atan2(x)), v]
}

fn intersect_torus(ray: Ray3, major_radius: f64, minor_radius: f64) -> Vec<Hit> {
    let axis = [0.0, 0.0, 1.0];
    let ref_dir = [1.0, 0.0, 0.0];

    let r2 = major_radius * major_radius;
    let a2 = minor_radius * minor_radius;

    let d = ray.direction;
    let o = ray.origin;

    let od = dot(o, d);
    let oo = dot(o, o);
    let dd = dot(d, d);

    let oa = dot(o, axis);
    let da = dot(d, axis);

    let sum_r2_a2 = r2 + a2;
    let k = oo - sum_r2_a2;

    let c4 = dd * dd;
    let c3 = 4.0 * dd * od;
    let c2 = 2.0 * dd * k + 4.0 * od * od + 4.0 * r2 * da * da;
    let c1 = 4.0 * k * od + 8.0 * r2 * oa * da;
    let c0 = k * k - 4.0 * r2 * (a2 - oa * oa);

    let roots = solve_quartic(c4, c3, c2, c1, c0);

    let mut hits: Vec<Hit> = roots
        .into_iter()
        .filter(|t| *t >= 0.0)
        .map(|t| Hit {
            t,
            uv: compute_torus_uv(ray.at(t), major_radius, axis, ref_dir),
        })
        .collect();

    hits.sort_by(|left, right| {
        left.t
            .partial_cmp(&right.t)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    hits
}

fn compute_torus_uv(
    point: [f64; 3],
    major_radius: f64,
    axis: [f64; 3],
    ref_dir: [f64; 3],
) -> [f64; 2] {
    let y_dir = cross(axis, ref_dir);

    let h = dot(point, axis);
    let proj = sub(point, scale(axis, h));
    let proj_len = norm(proj);

    let x = dot(proj, ref_dir);
    let y = dot(proj, y_dir);

    let u = if proj_len > 1e-12 {
        normalize_angle(y.atan2(x))
    } else {
        0.0
    };

    let tube_center_dist = proj_len - major_radius;
    let v = normalize_angle(h.atan2(tube_center_dist));

    [u, v]
}

fn solve_quartic(a: f64, b: f64, c: f64, d: f64, e: f64) -> Vec<f64> {
    if a.abs() < 1e-12 {
        return solve_cubic(b, c, d, e);
    }

    let p = b / a;
    let q = c / a;
    let r = d / a;
    let s = e / a;

    let p2 = p * p;
    let p3 = p2 * p;
    let p4 = p2 * p2;

    let a2 = q - 3.0 * p2 / 8.0;
    let a1 = r - p * q / 2.0 + p3 / 8.0;
    let a0 = s - p * r / 4.0 + p2 * q / 16.0 - 3.0 * p4 / 256.0;

    let cubic_roots = solve_cubic(8.0, 8.0 * a2, 2.0 * a2 * a2 - 8.0 * a0, -a1 * a1);
    let u = cubic_roots.into_iter().find(|u| *u > 1e-12).unwrap_or(0.0);

    let sqrt_2u = (2.0 * u).max(0.0).sqrt();
    let mut roots = Vec::new();

    if sqrt_2u.abs() > 1e-12 {
        let alpha = a2 + 2.0 * u;
        let beta = a1 / sqrt_2u;

        let disc1 = sqrt_2u * sqrt_2u - 2.0 * (alpha + beta);
        if disc1 >= 0.0 {
            let sqrt_disc1 = disc1.sqrt();
            roots.push((-sqrt_2u + sqrt_disc1) / 2.0 - p / 4.0);
            roots.push((-sqrt_2u - sqrt_disc1) / 2.0 - p / 4.0);
        }

        let disc2 = sqrt_2u * sqrt_2u - 2.0 * (alpha - beta);
        if disc2 >= 0.0 {
            let sqrt_disc2 = disc2.sqrt();
            roots.push((sqrt_2u + sqrt_disc2) / 2.0 - p / 4.0);
            roots.push((sqrt_2u - sqrt_disc2) / 2.0 - p / 4.0);
        }
    } else {
        let disc = a2 * a2 - 4.0 * a0;
        if disc >= 0.0 {
            let sqrt_disc = disc.sqrt();
            let y2_1 = (-a2 + sqrt_disc) / 2.0;
            let y2_2 = (-a2 - sqrt_disc) / 2.0;

            if y2_1 >= 0.0 {
                let y = y2_1.sqrt();
                roots.push(y - p / 4.0);
                roots.push(-y - p / 4.0);
            }
            if y2_2 >= 0.0 {
                let y = y2_2.sqrt();
                roots.push(y - p / 4.0);
                roots.push(-y - p / 4.0);
            }
        }
    }

    roots.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
    roots.dedup_by(|left, right| (*left - *right).abs() < 1e-10);
    roots
}

fn solve_cubic(a: f64, b: f64, c: f64, d: f64) -> Vec<f64> {
    if a.abs() < 1e-12 {
        return solve_quadratic(b, c, d);
    }

    let p = b / a;
    let q = c / a;
    let r = d / a;

    let p2 = p * p;
    let aa = q - p2 / 3.0;
    let bb = r - p * q / 3.0 + 2.0 * p2 * p / 27.0;
    let delta = bb * bb / 4.0 + aa * aa * aa / 27.0;

    let shift = p / 3.0;
    let mut roots = Vec::new();

    if delta > 1e-12 {
        let sqrt_delta = delta.sqrt();
        let u = cbrt(-bb / 2.0 + sqrt_delta);
        let v = cbrt(-bb / 2.0 - sqrt_delta);
        roots.push(u + v - shift);
    } else if delta.abs() <= 1e-12 {
        if aa.abs() < 1e-12 && bb.abs() < 1e-12 {
            roots.push(-shift);
        } else {
            let u = cbrt(-bb / 2.0);
            roots.push(2.0 * u - shift);
            roots.push(-u - shift);
        }
    } else {
        let m = 2.0 * (-aa / 3.0).sqrt();
        let theta = (3.0 * bb / (aa * m)).acos() / 3.0;
        roots.push(m * theta.cos() - shift);
        roots.push(m * (theta - 2.0 * PI / 3.0).cos() - shift);
        roots.push(m * (theta + 2.0 * PI / 3.0).cos() - shift);
    }

    roots
}

fn solve_quadratic(a: f64, b: f64, c: f64) -> Vec<f64> {
    if a.abs() < 1e-12 {
        if b.abs() > 1e-12 {
            return vec![-c / b];
        }
        return Vec::new();
    }

    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 {
        return Vec::new();
    }

    let sqrt_disc = disc.sqrt();
    vec![(-b - sqrt_disc) / (2.0 * a), (-b + sqrt_disc) / (2.0 * a)]
}

fn cbrt(value: f64) -> f64 {
    if value >= 0.0 {
        value.powf(1.0 / 3.0)
    } else {
        -(-value).powf(1.0 / 3.0)
    }
}

fn sample_sets_match(
    mut actual: Vec<AnalyticIntersectionSample>,
    mut expected: Vec<AnalyticIntersectionSample>,
    epsilon: f64,
) -> bool {
    actual.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    expected.sort_by(|left, right| left.case_id.cmp(&right.case_id));

    if actual.len() != expected.len() {
        return false;
    }

    actual
        .iter()
        .zip(expected.iter())
        .all(|(left, right)| sample_approx_eq(left, right, epsilon))
}

fn sample_approx_eq(
    left: &AnalyticIntersectionSample,
    right: &AnalyticIntersectionSample,
    epsilon: f64,
) -> bool {
    if left.case_id != right.case_id
        || left.surface != right.surface
        || left.hit_count != right.hit_count
        || left.t_values.len() != right.t_values.len()
        || left.uv_values.len() != right.uv_values.len()
    {
        return false;
    }

    let t_match = left
        .t_values
        .iter()
        .zip(right.t_values.iter())
        .all(|(a, b)| approx_eq(*a, *b, epsilon));
    if !t_match {
        return false;
    }

    left.uv_values
        .iter()
        .zip(right.uv_values.iter())
        .all(|(a, b)| approx_eq(a[0], b[0], epsilon) && approx_eq(a[1], b[1], epsilon))
}

fn canonical_f64(value: f64) -> f64 {
    let rounded = (value * 1_000_000_000.0).round() / 1_000_000_000.0;
    if rounded.abs() < 1e-12 { 0.0 } else { rounded }
}

fn approx_eq(left: f64, right: f64, epsilon: f64) -> bool {
    (left - right).abs() <= epsilon
}

fn parity_signature(
    snapshot: &AnalyticRayIntersectionsSnapshot,
    reference_commit_match: bool,
    sample_set_match: bool,
    surface_coverage_match: bool,
    hit_ordering_match: bool,
    positive_t_filter_match: bool,
    deterministic_replay_match: bool,
    reference_fixture_sha256: &str,
) -> String {
    let payload = serde_json::to_vec(&(
        snapshot,
        reference_commit_match,
        sample_set_match,
        surface_coverage_match,
        hit_ordering_match,
        positive_t_filter_match,
        deterministic_replay_match,
        reference_fixture_sha256,
    ))
    .expect("serialize analytic ray intersections parity signature payload");
    stable_hex_digest(&payload)
}

fn sorted_strings(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn add(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [left[0] + right[0], left[1] + right[1], left[2] + right[2]]
}

fn sub(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn scale(vector: [f64; 3], factor: f64) -> [f64; 3] {
    [vector[0] * factor, vector[1] * factor, vector[2] * factor]
}

fn dot(left: [f64; 3], right: [f64; 3]) -> f64 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

fn cross(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

fn norm(vector: [f64; 3]) -> f64 {
    dot(vector, vector).sqrt()
}

fn normalize(vector: [f64; 3]) -> [f64; 3] {
    let len = norm(vector);
    if len <= 1e-12 {
        [1.0, 0.0, 0.0]
    } else {
        scale(vector, 1.0 / len)
    }
}

fn normalize_angle(angle: f64) -> f64 {
    if angle < 0.0 { angle + 2.0 * PI } else { angle }
}
