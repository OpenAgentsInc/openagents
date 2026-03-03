use std::collections::HashMap;

use crate::kernel_math::{Point3, Vec3};

use super::types::{HatchPattern, HatchRegion, Point2D, SectionCurve, SectionPlane, SectionView};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SectionOptions {
    pub hatch: Option<HatchPattern>,
}

const SECTION_TOLERANCE: f64 = 1e-6;

impl Default for SectionOptions {
    fn default() -> Self {
        Self {
            hatch: Some(HatchPattern {
                spacing: 2.0,
                angle_radians: std::f64::consts::FRAC_PI_4,
            }),
        }
    }
}

pub fn project_to_section_plane(point: Point3, plane: &SectionPlane) -> Point2D {
    let normal = plane.normal.normalized().unwrap_or(Vec3::z());
    let reference = if normal.z.abs() > 0.9 {
        Vec3::x()
    } else {
        Vec3::z()
    };
    let axis_x = normal.cross(reference).normalized().unwrap_or(Vec3::x());
    let axis_y = normal.cross(axis_x).normalized().unwrap_or(Vec3::y());
    let relative = Vec3::new(
        point.x - plane.origin.x,
        point.y - plane.origin.y,
        point.z - plane.origin.z,
    );
    Point2D::new(relative.dot(axis_x), relative.dot(axis_y))
}

pub fn intersect_mesh_with_plane(
    points: &[Point3],
    plane: &SectionPlane,
    tolerance: f64,
) -> Vec<(Point3, Point3)> {
    let mut segments = Vec::new();

    if points.len() >= 3 && points.len().is_multiple_of(3) {
        for triangle in points.chunks_exact(3) {
            let intersections = intersect_triangle_with_plane(
                triangle[0],
                triangle[1],
                triangle[2],
                plane,
                tolerance,
            );
            if intersections.len() == 2 {
                segments.push((intersections[0], intersections[1]));
            }
        }
        return segments;
    }

    for pair in points.windows(2) {
        let start = pair[0];
        let end = pair[1];
        let start_distance = signed_distance(start, plane);
        let end_distance = signed_distance(end, plane);
        if start_distance.abs() <= tolerance && end_distance.abs() <= tolerance {
            segments.push((start, end));
            continue;
        }
        if start_distance * end_distance < 0.0 {
            segments.push((start, end));
        }
    }
    segments
}

fn intersect_triangle_with_plane(
    v0: Point3,
    v1: Point3,
    v2: Point3,
    plane: &SectionPlane,
    tolerance: f64,
) -> Vec<Point3> {
    let d0 = signed_distance(v0, plane);
    let d1 = signed_distance(v1, plane);
    let d2 = signed_distance(v2, plane);

    let mut intersections = Vec::new();

    if d0.abs() <= tolerance {
        intersections.push(v0);
    }
    if d1.abs() <= tolerance && !contains_point(&intersections, v1, tolerance) {
        intersections.push(v1);
    }
    if d2.abs() <= tolerance && !contains_point(&intersections, v2, tolerance) {
        intersections.push(v2);
    }

    for (a, b, da, db) in [(v0, v1, d0, d1), (v1, v2, d1, d2), (v2, v0, d2, d0)] {
        if (da > tolerance && db < -tolerance) || (da < -tolerance && db > tolerance) {
            let t = da / (da - db);
            let intersection = lerp_point(a, b, t);
            if !contains_point(&intersections, intersection, tolerance) {
                intersections.push(intersection);
            }
        }
    }

    intersections.truncate(2);
    intersections
}

fn lerp_point(start: Point3, end: Point3, t: f64) -> Point3 {
    Point3::new(
        start.x + (end.x - start.x) * t,
        start.y + (end.y - start.y) * t,
        start.z + (end.z - start.z) * t,
    )
}

fn contains_point(points: &[Point3], candidate: Point3, tolerance: f64) -> bool {
    points
        .iter()
        .any(|point| point_distance(*point, candidate) <= tolerance)
}

pub fn chain_segments(segments: &[(Point3, Point3)], plane: &SectionPlane) -> Vec<SectionCurve> {
    if segments.is_empty() {
        return Vec::new();
    }

    let tolerance = SECTION_TOLERANCE;
    let mut adjacency: HashMap<(i64, i64, i64), Vec<(usize, bool)>> = HashMap::new();
    for (index, (start, end)) in segments.iter().enumerate() {
        adjacency
            .entry(point_key(*start, tolerance))
            .or_default()
            .push((index, false));
        adjacency
            .entry(point_key(*end, tolerance))
            .or_default()
            .push((index, true));
    }

    let mut used = vec![false; segments.len()];
    let mut curves = Vec::new();

    for start_index in 0..segments.len() {
        if used[start_index] {
            continue;
        }
        used[start_index] = true;

        let (start, end) = segments[start_index];
        let mut chain = vec![start, end];

        loop {
            let Some(current) = chain.last().copied() else {
                break;
            };
            let key = point_key(current, tolerance);
            let mut extended = false;
            if let Some(neighbors) = adjacency.get(&key) {
                for (segment_index, is_end) in neighbors {
                    if used[*segment_index] {
                        continue;
                    }
                    let (segment_start, segment_end) = segments[*segment_index];
                    let next = if *is_end { segment_start } else { segment_end };
                    chain.push(next);
                    used[*segment_index] = true;
                    extended = true;
                    break;
                }
            }
            if !extended {
                break;
            }
        }

        loop {
            let current = chain[0];
            let key = point_key(current, tolerance);
            let mut extended = false;
            if let Some(neighbors) = adjacency.get(&key) {
                for (segment_index, is_end) in neighbors {
                    if used[*segment_index] {
                        continue;
                    }
                    let (segment_start, segment_end) = segments[*segment_index];
                    let next = if *is_end { segment_start } else { segment_end };
                    chain.insert(0, next);
                    used[*segment_index] = true;
                    extended = true;
                    break;
                }
            }
            if !extended {
                break;
            }
        }

        let mut closed = chain.len() > 2
            && point_distance(chain[0], *chain.last().unwrap_or(&chain[0])) <= tolerance;
        if closed {
            chain.pop();
            closed = true;
        }

        curves.push(SectionCurve {
            points: chain
                .into_iter()
                .map(|point| project_to_section_plane(point, plane))
                .collect(),
            closed,
        });
    }

    curves
}

fn point_key(point: Point3, tolerance: f64) -> (i64, i64, i64) {
    let inv = 1.0 / tolerance.max(1e-9);
    (
        (point.x * inv).round() as i64,
        (point.y * inv).round() as i64,
        (point.z * inv).round() as i64,
    )
}

fn point_distance(left: Point3, right: Point3) -> f64 {
    ((left.x - right.x).powi(2) + (left.y - right.y).powi(2) + (left.z - right.z).powi(2)).sqrt()
}

pub fn generate_hatch_lines(
    region: &HatchRegion,
    spacing: f64,
    angle_radians: f64,
) -> Vec<(Point2D, Point2D)> {
    let width = region.bounds.width().max(spacing);
    let height = region.bounds.height().max(spacing);
    let count = ((width + height) / spacing.max(1e-6)).ceil() as usize;
    let dx = angle_radians.cos() * width;
    let dy = angle_radians.sin() * height;

    let mut lines = Vec::with_capacity(count);
    for index in 0..count {
        let t = index as f64 * spacing;
        let start = Point2D::new(region.bounds.min_x + t, region.bounds.min_y);
        let end = Point2D::new(start.x + dx, start.y + dy);
        lines.push((start, end));
    }
    lines
}

pub fn section_mesh(
    points: &[Point3],
    plane: &SectionPlane,
    options: SectionOptions,
) -> SectionView {
    let segments = intersect_mesh_with_plane(points, plane, 1e-9);
    let curves = chain_segments(&segments, plane);

    let hatch_lines = if let Some(pattern) = options.hatch {
        let mut bounds = super::types::BoundingBox2D::empty();
        for curve in &curves {
            for point in &curve.points {
                bounds.include_point(*point);
            }
        }
        if bounds.is_valid() {
            generate_hatch_lines(
                &HatchRegion { bounds },
                pattern.spacing,
                pattern.angle_radians,
            )
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    SectionView {
        curves,
        hatch_lines,
    }
}

fn signed_distance(point: Point3, plane: &SectionPlane) -> f64 {
    let delta = Vec3::new(
        point.x - plane.origin.x,
        point.y - plane.origin.y,
        point.z - plane.origin.z,
    );
    delta.dot(plane.normal.normalized().unwrap_or(Vec3::z()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn section_generation_is_deterministic() {
        let points = vec![
            Point3::new(0.0, 0.0, -1.0),
            Point3::new(0.0, 0.0, 1.0),
            Point3::new(1.0, 0.0, 1.0),
        ];
        let plane = SectionPlane {
            origin: Point3::origin(),
            normal: Vec3::z(),
        };

        let first = section_mesh(&points, &plane, SectionOptions::default());
        let second = section_mesh(&points, &plane, SectionOptions::default());
        assert_eq!(first, second);
    }

    #[test]
    fn section_generation_produces_hatch_lines_when_bounds_exist() {
        let points = vec![Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 0.0, 0.0)];
        let plane = SectionPlane {
            origin: Point3::origin(),
            normal: Vec3::z(),
        };
        let view = section_mesh(&points, &plane, SectionOptions::default());
        assert!(!view.hatch_lines.is_empty());
    }

    #[test]
    fn triangle_mesh_intersection_emits_expected_segment_count() {
        let points = vec![
            Point3::new(-1.0, -1.0, -1.0),
            Point3::new(1.0, -1.0, 1.0),
            Point3::new(0.0, 1.0, 1.0),
        ];
        let plane = SectionPlane {
            origin: Point3::origin(),
            normal: Vec3::z(),
        };

        let segments = intersect_mesh_with_plane(&points, &plane, SECTION_TOLERANCE);
        assert_eq!(segments.len(), 1);
    }

    #[test]
    fn chained_segments_form_closed_curve() {
        let plane = SectionPlane {
            origin: Point3::origin(),
            normal: Vec3::z(),
        };
        let segments = vec![
            (Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 0.0, 0.0)),
            (Point3::new(1.0, 0.0, 0.0), Point3::new(1.0, 1.0, 0.0)),
            (Point3::new(1.0, 1.0, 0.0), Point3::new(0.0, 1.0, 0.0)),
            (Point3::new(0.0, 1.0, 0.0), Point3::new(0.0, 0.0, 0.0)),
        ];

        let curves = chain_segments(&segments, &plane);
        assert_eq!(curves.len(), 1);
        assert!(curves[0].closed);
        assert_eq!(curves[0].points.len(), 4);
    }
}
