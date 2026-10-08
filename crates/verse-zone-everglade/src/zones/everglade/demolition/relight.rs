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
        let mut covered = 0.0;
        if let Some(boxes) = self.cells.get(&(at.x, at.y, at.z)) {
            for &index in boxes
                .iter()
                .filter(|&&i| Some(self.boxes[i].piece) != skip)
                .take(MAX_BOXES)
            {
                let b = &self.boxes[index];
                let local = b.inverse.transform_point3(point);
                let distance = (local.abs() - b.half).max(Vec3::ZERO).length();
                if distance >= REACH {
                    continue;
                }
                let toward = (b.center - point).normalize_or(normal);
                let hemisphere = normal.dot(toward).max(0.0);
                let solid = b.radius * b.radius
                    / (point.distance_squared(b.center) + b.radius * b.radius).max(0.01);
                covered += hemisphere * solid * (1.0 - distance / REACH) * 0.65;
            }
        }
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
}
