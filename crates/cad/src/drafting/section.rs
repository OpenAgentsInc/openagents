use crate::kernel_math::{Point3, Vec3};

use super::types::{HatchPattern, HatchRegion, Point2D, SectionCurve, SectionPlane, SectionView};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SectionOptions {
    pub hatch: Option<HatchPattern>,
}

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

pub fn chain_segments(segments: &[(Point3, Point3)], plane: &SectionPlane) -> Vec<SectionCurve> {
    segments
        .iter()
        .map(|(start, end)| SectionCurve {
            points: vec![
                project_to_section_plane(*start, plane),
                project_to_section_plane(*end, plane),
            ],
            closed: false,
        })
        .collect()
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
}
