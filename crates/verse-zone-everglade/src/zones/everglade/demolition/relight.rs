//! Destruction's local ambient fallback: current standing boxes, rather
//! than the pristine town's baked irradiance.

use super::site::{Role, Side, Site, Status};
use glam::{IVec3, Mat4, Vec3};
use std::collections::BTreeMap;

/// Reach of the local ambient occlusion estimate, m.
pub const REACH: f32 = 4.0;
/// The darkest local fallback; direct sun and lamp light still use the
/// renderer's current shadows.
pub const MIN_OPEN: f32 = 0.35;
const MAX_BOXES: usize = 32;

struct BoxSample {
    piece: usize,
    inverse: Mat4,
    center: Vec3,
    half: Vec3,
    radius: f32,
}

/// A bounded ambient estimate from the pieces that still stand. Moving
/// rubble uses the current shadow passes; it leaves no frozen local shadow
/// in this field. Build once for a pose, then sample each affected chunk.
pub struct LocalOcclusion {
    boxes: Vec<BoxSample>,
    cells: BTreeMap<(i32, i32, i32), Vec<usize>>,
}

impl LocalOcclusion {
    #[must_use]
    pub fn new(site: &Site) -> Self {
        let mut field = Self {
            boxes: Vec::new(),
            cells: BTreeMap::new(),
        };
        for (piece, (spec, state)) in site.specs().iter().zip(site.pieces()).enumerate() {
            if state.status != Status::Standing {
                continue;
            }
            let pose = site.piece_pose(piece);
            for collider in &spec.colliders {
                let frame = pose * collider.frame();
                let center = frame.w_axis.truncate();
                let half = collider.half.as_vec3();
                let extent = frame.x_axis.truncate().abs() * half.x
                    + frame.y_axis.truncate().abs() * half.y
                    + frame.z_axis.truncate().abs() * half.z;
                let index = field.boxes.len();
                field.boxes.push(BoxSample {
                    piece,
                    inverse: frame.inverse(),
                    center,
                    half,
                    radius: half.length(),
                });
                let lo = cell(center - extent - Vec3::splat(REACH));
                let hi = cell(center + extent + Vec3::splat(REACH));
                for x in lo.x..=hi.x {
                    for y in lo.y..=hi.y {
                        for z in lo.z..=hi.z {
                            field.cells.entry((x, y, z)).or_default().push(index);
                        }
                    }
                }
            }
        }
        field
    }

    /// Ambient openness at a surface. Nearby boxes approximate covered
    /// solid angle, weighted toward the surface normal, with a fixed floor.
    /// `skip` excludes the surface's own piece.
    #[must_use]
    pub fn sample(&self, point: Vec3, normal: Vec3, skip: Option<usize>) -> f32 {
        if !point.is_finite() || !normal.is_finite() {
            return 1.0;
        }
        let normal = normal.normalize_or(Vec3::Y);
        let at = cell(point);
        let mut strongest = [0.0; MAX_BOXES];
        if let Some(boxes) = self.cells.get(&(at.x, at.y, at.z)) {
            for &index in boxes.iter().filter(|&&i| Some(self.boxes[i].piece) != skip) {
                let b = &self.boxes[index];
                let delta = b.center - point;
                let center_distance = delta.length_squared();
                if center_distance >= (b.radius + REACH).powi(2) {
                    continue;
                }
                let hemisphere = normal.dot(delta.normalize_or(normal)).max(0.0);
                let solid = b.radius * b.radius / (center_distance + b.radius * b.radius).max(0.01);
                let upper = hemisphere * solid * 0.65;
                if upper <= strongest[MAX_BOXES - 1] {
                    continue;
                }
                let local = b.inverse.transform_point3(point);
                let distance = (local.abs() - b.half).max(Vec3::ZERO).length();
                if distance >= REACH {
                    continue;
                }
                let contribution = upper * (1.0 - distance / REACH);
                if contribution <= strongest[MAX_BOXES - 1] {
                    continue;
                }
                // Retain the strongest local cover in a fixed-size array.
                // Sorted accumulation also makes piece order irrelevant.
                let position = strongest.partition_point(|&v| v > contribution);
                strongest.copy_within(position..MAX_BOXES - 1, position + 1);
                strongest[position] = contribution;
            }
        }
        let mut covered: f32 = strongest.iter().sum();
        // Downward faces near the actual ground receive less sky light.
        covered += (-normal.y).max(0.0) * (1.0 - point.y.max(0.0) / REACH).clamp(0.0, 1.0) * 0.35;
        (1.0 - covered).clamp(MIN_OPEN, 1.0)
    }
}

fn cell(point: Vec3) -> IVec3 {
    (point / REACH).floor().as_ivec3()
}

/// Whether a kit role directly depends on another role in the same
/// building. This mirrors the yard's support relationships, without
/// following the graph beyond one edge.
pub(super) fn depends(role: Role, other: Role, top: u8) -> bool {
    let end = |index: u8, count: u8, side: Side| {
        (index == 0 && matches!(side, Side::West | Side::South))
            || (index + 1 == count && matches!(side, Side::East | Side::North))
    };
    match role {
        Role::Wall {
            side,
            index,
            count,
            story,
        } => match other {
            Role::Wall {
                side: s,
                index: i,
                story: t,
                ..
            } => {
                s == side
                    && ((t == story && index.abs_diff(i) == 1) || (i == index && t + 1 == story))
            }
            Role::Post { a, b, story: t } => {
                t == story
                    && ((a == side && end(index, count, b)) || (b == side && end(index, count, a)))
            }
            _ => false,
        },
        Role::Post { a, b, story } => match other {
            Role::Wall {
                side,
                index,
                count,
                story: t,
            } => {
                t == story
                    && ((side == a && end(index, count, b)) || (side == b && end(index, count, a)))
            }
            Role::Post {
                a: pa,
                b: pb,
                story: t,
            } => pa == a && pb == b && t + 1 == story,
            _ => false,
        },
        Role::Roof { span, spans } => match other {
            Role::Wall {
                side, index, story, ..
            } if story == top => {
                let index = i32::from(index);
                let boundary =
                    |b: u8| (i32::from(b) * 4 - 2..=i32::from(b) * 4 + 1).contains(&index);
                (span == 0 && side == Side::West)
                    || (span + 1 == spans && side == Side::East)
                    || (matches!(side, Side::South | Side::North)
                        && ((span > 0 && boundary(span))
                            || (span + 1 < spans && boundary(span + 1))))
            }
            _ => false,
        },
        Role::Gable { side, span } => match other {
            Role::Roof { span: s, .. } => s == span,
            Role::Wall {
                side: s,
                index,
                story,
                ..
            } => s == side && story == top && index / 4 == span,
            _ => false,
        },
        Role::Chimney { span } => matches!(other, Role::Roof { span: s, .. } if s == span),
        Role::Block { .. } => false,
    }
}

#[cfg(test)]
mod tests {
    use super::super::cottage;
    use super::super::site::{Cuboid, Link};
    use super::*;
    use glam::{DQuat, DVec3};

    #[test]
    fn breaking_a_block_invalidates_immediate_support_neighbors_and_reset_restores_them() {
        let template = cottage::specs_without_meshes().remove(0);
        let specs = (0..5)
            .map(|i| {
                let mut spec = template.clone();
                spec.building = usize::from(i == 4);
                spec.role = Role::Block { level: 0 };
                spec.link = Link {
                    footing: true,
                    under: Vec::new(),
                    beside: if i == 0 || i == 4 {
                        Vec::new()
                    } else {
                        vec![i - 1]
                    },
                };
                spec.center = DVec3::new(f64::from(i) * 3.0, 1.0, 0.0);
                spec
            })
            .collect();
        let mut site = Site::new(specs, 1);
        assert_eq!(site.support_neighbors(1), vec![0, 2]);
        let point = site.specs()[1].center;
        assert!(!site.damage(1, 1, point, DVec3::ZERO));
        assert!(site.pieces().iter().all(|p| !p.relight));
        assert!(site.damage(1, 10000, point, DVec3::ZERO));
        assert_eq!(
            site.pieces().iter().map(|p| p.relight).collect::<Vec<_>>(),
            vec![true, true, true, false, false]
        );
        assert_eq!(site.pieces()[2].status, Status::Standing);
        site.retain(|s| s.building == 0);
        assert!(site.relight(2), "retaining bodies preserves invalidation");
        site.reset();
        assert!(site.pieces().iter().all(|p| !p.relight));
    }

    #[test]
    fn kit_neighbors_follow_vertical_walls_roof_spans_and_corner_posts() {
        let wall = |story, index| Role::Wall {
            side: Side::South,
            story,
            index,
            count: 8,
        };
        assert!(depends(wall(1, 2), wall(0, 2), 1));
        assert!(depends(wall(0, 2), wall(0, 3), 1));
        assert!(!depends(wall(0, 2), wall(0, 4), 1));
        let post = Role::Post {
            a: Side::South,
            b: Side::West,
            story: 0,
        };
        assert!(depends(wall(0, 0), post, 1));
        assert!(depends(post, wall(0, 0), 1));
        let roof = Role::Roof { span: 0, spans: 2 };
        assert!(depends(roof, wall(1, 3), 1));
        assert!(!depends(roof, wall(0, 3), 1));
        assert!(depends(Role::Chimney { span: 0 }, roof, 1));
    }

    #[test]
    fn an_exposed_interior_uses_current_cover_and_no_shadow_of_a_removed_roof() {
        let mut spec = cottage::specs_without_meshes().remove(0);
        spec.center = DVec3::new(0.0, 3.0, 0.0);
        spec.orientation = DQuat::IDENTITY;
        spec.role = Role::Block { level: 0 };
        spec.link = Link {
            footing: true,
            ..Link::default()
        };
        spec.size = DVec3::new(4.0, 1.0, 4.0);
        let box_shape = Cuboid::between(-spec.size * 0.5, spec.size * 0.5);
        spec.colliders = vec![box_shape];
        spec.chunks = vec![box_shape];
        let mut site = Site::new(vec![spec], 1);
        let point = Vec3::new(0.0, 1.0, 0.0);
        let covered = LocalOcclusion::new(&site).sample(point, Vec3::Y, None);
        assert!((MIN_OPEN..0.9).contains(&covered), "{covered}");
        assert_eq!(
            LocalOcclusion::new(&site).sample(point, Vec3::Y, Some(0)),
            1.0
        );
        site.crumble(0, DVec3::Y, 0.0);
        let field = LocalOcclusion::new(&site);
        assert_eq!(field.sample(point, Vec3::Y, None), 1.0);
        assert!(field.sample(point, Vec3::NEG_Y, None) >= MIN_OPEN);
        assert_eq!(field.sample(Vec3::NAN, Vec3::Y, None), 1.0);
        site.reset();
        assert_eq!(
            LocalOcclusion::new(&site).sample(point, Vec3::Y, None),
            covered
        );
    }

    #[test]
    fn irrelevant_cell_boxes_cannot_hide_an_intact_roof_in_either_piece_order() {
        let template = cottage::specs_without_meshes().remove(0);
        let make = |center: DVec3, size: DVec3| {
            let mut spec = template.clone();
            spec.center = center;
            spec.orientation = DQuat::IDENTITY;
            spec.role = Role::Block { level: 0 };
            spec.link = Link {
                footing: true,
                ..Link::default()
            };
            spec.size = size;
            let shape = Cuboid::between(-size * 0.5, size * 0.5);
            spec.colliders = vec![shape];
            spec.chunks = vec![shape];
            spec
        };
        let roof = make(DVec3::Y, DVec3::new(4.0, 0.2, 4.0));
        let sample =
            |specs| LocalOcclusion::new(&Site::new(specs, 1)).sample(Vec3::ZERO, Vec3::Y, None);
        let expected = sample(vec![roof.clone()]);
        assert!((MIN_OPEN..0.9).contains(&expected));
        let mut specs = vec![make(DVec3::new(7.9, 1.0, 0.0), DVec3::splat(0.2)); MAX_BOXES];
        specs.extend(vec![make(-DVec3::Y, DVec3::splat(0.2)); MAX_BOXES]);
        specs.push(roof);
        let field = LocalOcclusion::new(&Site::new(specs.clone(), 1));
        assert!(field.cells[&(0, 0, 0)].len() > MAX_BOXES);
        assert_eq!(field.sample(Vec3::ZERO, Vec3::Y, None), expected);
        specs.reverse();
        assert_eq!(sample(specs), expected);
    }

    #[test]
    fn dense_cover_retains_the_strongest_boxes_independent_of_piece_order() {
        let mut tiny = cottage::specs_without_meshes().remove(0);
        tiny.center = DVec3::Y * 2.0;
        tiny.orientation = DQuat::IDENTITY;
        tiny.role = Role::Block { level: 0 };
        tiny.link = Link {
            footing: true,
            ..Link::default()
        };
        tiny.size = DVec3::splat(0.04);
        let shape = Cuboid::between(-tiny.size * 0.5, tiny.size * 0.5);
        tiny.colliders = vec![shape];
        tiny.chunks = vec![shape];
        let mut roof = tiny.clone();
        roof.center = DVec3::Y;
        roof.size = DVec3::new(4.0, 0.2, 4.0);
        let shape = Cuboid::between(-roof.size * 0.5, roof.size * 0.5);
        roof.colliders = vec![shape];
        roof.chunks = vec![shape];
        let sample =
            |specs| LocalOcclusion::new(&Site::new(specs, 1)).sample(Vec3::ZERO, Vec3::Y, None);
        let mut expected = vec![tiny.clone(); MAX_BOXES - 1];
        expected.push(roof.clone());
        let expected = sample(expected);
        let mut dense = vec![tiny; MAX_BOXES + 8];
        dense.push(roof);
        assert_eq!(sample(dense.clone()), expected);
        dense.reverse();
        assert_eq!(sample(dense), expected);
    }
}
