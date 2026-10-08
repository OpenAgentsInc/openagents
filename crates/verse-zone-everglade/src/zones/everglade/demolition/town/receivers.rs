//! Static ground receiver membership for each building's former sun shadow.

use super::super::relight::REACH;
use super::{Building, TexturedVertex};
use glam::Vec3;
use std::collections::BTreeMap;

const CELL: f32 = 16.0;

#[derive(Default)]
pub(super) struct Index {
    buildings: Vec<Vec<u32>>,
}

struct Bounds {
    lo: Vec3,
    hi: Vec3,
    ceiling: f32,
}

impl Bounds {
    fn new(building: &Building, sun: Vec3) -> Self {
        let ([x, z], [hx, hz]) = building.rect;
        let shadow = -sun * ((building.top - building.base).max(0.0) / sun.y.max(0.05));
        Self {
            lo: Vec3::new(
                x - hx + shadow.x.min(0.0) - REACH,
                building.base,
                z - hz + shadow.z.min(0.0) - REACH,
            ),
            hi: Vec3::new(
                x + hx + shadow.x.max(0.0) + REACH,
                building.base,
                z + hz + shadow.z.max(0.0) + REACH,
            ),
            ceiling: building.base + 0.25,
        }
    }

    fn contains(&self, point: Vec3) -> bool {
        point.y <= self.ceiling
            && point.x >= self.lo.x
            && point.x <= self.hi.x
            && point.z >= self.lo.z
            && point.z <= self.hi.z
    }
}

fn cell(value: f32) -> i32 {
    (value / CELL).floor() as i32
}

impl Index {
    pub fn new(vertices: &[TexturedVertex], buildings: &[Building], sun: Vec3) -> Self {
        Self::with_bounds(
            vertices,
            buildings.iter().map(|building| Bounds::new(building, sun)),
        )
    }

    /// Encloses every normalized sun direction under the shadow's 0.05
    /// elevation clamp. B3 retains this membership throughout the day.
    pub fn all_directions(vertices: &[TexturedVertex], buildings: &[Building]) -> Self {
        Self::with_bounds(
            vertices,
            buildings.iter().map(|building| {
                let ([x, z], [hx, hz]) = building.rect;
                let reach = (building.top - building.base).max(0.0) / 0.05;
                Bounds {
                    lo: Vec3::new(
                        x - hx - reach - REACH,
                        building.base,
                        z - hz - reach - REACH,
                    ),
                    hi: Vec3::new(
                        x + hx + reach + REACH,
                        building.base,
                        z + hz + reach + REACH,
                    ),
                    ceiling: building.base + 0.25,
                }
            }),
        )
    }

    fn with_bounds(vertices: &[TexturedVertex], bounds: impl IntoIterator<Item = Bounds>) -> Self {
        let mut grid: BTreeMap<(i32, i32), Vec<u32>> = BTreeMap::new();
        for (index, vertex) in vertices.iter().enumerate() {
            grid.entry((cell(vertex.pos[0]), cell(vertex.pos[2])))
                .or_default()
                .push(index as u32);
        }
        Self {
            buildings: bounds
                .into_iter()
                .map(|bounds| {
                    let lo = (cell(bounds.lo.x), cell(bounds.lo.z));
                    let hi = (cell(bounds.hi.x), cell(bounds.hi.z));
                    let mut indices = Vec::new();
                    for (&(_, z), candidates) in grid.range((lo.0, i32::MIN)..=(hi.0, i32::MAX)) {
                        if z < lo.1 || z > hi.1 {
                            continue;
                        }
                        indices.extend(candidates.iter().copied().filter(|&index| {
                            bounds.contains(Vec3::from(vertices[index as usize].pos))
                        }));
                    }
                    indices
                })
                .collect(),
        }
    }

    pub fn affected(&self, buildings: impl IntoIterator<Item = usize>) -> Vec<u32> {
        let mut indices: Vec<_> = buildings
            .into_iter()
            .filter_map(|building| self.buildings.get(building))
            .flat_map(|indices| indices.iter().copied())
            .collect();
        indices.sort_unstable();
        indices.dedup();
        indices
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn building(rect: ([f32; 2], [f32; 2]), base: f32, top: f32) -> Building {
        Building {
            rect,
            walls: None,
            base,
            top,
            stories: 1,
            valid: true,
            carved: Vec::new(),
            ready: true,
            pieces: Vec::new(),
            blocks: Vec::new(),
            roofs: Vec::new(),
        }
    }

    fn exhaustive(
        vertices: &[TexturedVertex],
        buildings: &[Building],
        sun: Vec3,
        mask: usize,
    ) -> Vec<u32> {
        vertices
            .iter()
            .enumerate()
            .filter_map(|(index, vertex)| {
                let point = Vec3::from(vertex.pos);
                buildings
                    .iter()
                    .enumerate()
                    .any(|(i, building)| {
                        if mask & (1 << i) == 0 {
                            return false;
                        }
                        let ([x, z], [hx, hz]) = building.rect;
                        let shadow =
                            -sun * ((building.top - building.base).max(0.0) / sun.y.max(0.05));
                        let lo = Vec3::new(
                            x - hx + shadow.x.min(0.0),
                            building.base,
                            z - hz + shadow.z.min(0.0),
                        );
                        let hi = Vec3::new(
                            x + hx + shadow.x.max(0.0),
                            building.base,
                            z + hz + shadow.z.max(0.0),
                        );
                        point.y <= building.base + 0.25
                            && point.x >= lo.x - REACH
                            && point.x <= hi.x + REACH
                            && point.z >= lo.z - REACH
                            && point.z <= hi.z + REACH
                    })
                    .then_some(index as u32)
            })
            .collect()
    }

    #[test]
    fn spatial_membership_matches_exhaustive_shadow_receivers_for_every_affected_subset() {
        let buildings = [
            building(([-16.0, -16.0], [3.0, 5.0]), 2.0, 14.0),
            building(([-10.0, -18.0], [4.0, 4.0]), -1.0, 20.0),
            building(([64.0, 48.0], [16.0, 2.0]), 7.0, 6.0),
        ];
        for sun in [
            Vec3::Y,
            Vec3::new(1.0, 0.1, -1.0),
            Vec3::new(-1.0, -0.5, 1.0),
        ] {
            let sun = sun.normalize();
            let mut vertices = Vec::new();
            for z in -32..32 {
                for x in -32..32 {
                    let point = Vec3::new(x as f32 * 4.0, ((x + z) % 10) as f32, z as f32 * 4.0);
                    vertices.push(TexturedVertex::new(
                        point,
                        Vec3::new(0.1, 0.9, 0.2),
                        [0.0; 2],
                    ));
                }
            }
            // Include exact and adjacent boundaries at both signs of grid cells.
            for building in &buildings {
                let bounds = Bounds::new(building, sun);
                for x in [
                    bounds.lo.x.next_down(),
                    bounds.lo.x,
                    bounds.hi.x,
                    bounds.hi.x.next_up(),
                ] {
                    for z in [
                        bounds.lo.z.next_down(),
                        bounds.lo.z,
                        bounds.hi.z,
                        bounds.hi.z.next_up(),
                    ] {
                        for y in [bounds.ceiling, bounds.ceiling.next_up(), -100.0] {
                            vertices.push(TexturedVertex::new(
                                Vec3::new(x, y, z),
                                Vec3::Z,
                                [0.0; 2],
                            ));
                        }
                    }
                }
            }
            let index = Index::new(&vertices, &buildings, sun);
            for mask in 0..1 << buildings.len() {
                let affected = (0..buildings.len()).filter(|i| mask & (1 << i) != 0);
                assert_eq!(
                    index.affected(affected),
                    exhaustive(&vertices, &buildings, sun, mask)
                );
            }
            assert_eq!(
                index.affected([1, 0, 1]),
                exhaustive(&vertices, &buildings, sun, 3)
            );
        }
    }
    #[test]
    fn all_direction_membership_contains_fixed_sun_receivers_at_shadow_boundaries() {
        let buildings = [
            building(([1.0, -2.0], [2.0, 3.0]), 1.0, 3.0),
            building(([-3.0, 4.0], [1.0, 2.0]), -1.0, 1.0),
        ];
        let mut vertices = Vec::new();
        for building in &buildings {
            let ([x, z], [hx, hz]) = building.rect;
            let reach = (building.top - building.base).max(0.0) / 0.05;
            for px in [x - hx - reach - REACH, x, x + hx + reach + REACH] {
                for pz in [z - hz - reach - REACH, z, z + hz + reach + REACH] {
                    for py in [
                        building.base,
                        building.base + 0.25,
                        f32::from_bits((building.base + 0.25).to_bits() + 1),
                    ] {
                        vertices.push(TexturedVertex::new(
                            Vec3::new(px, py, pz),
                            Vec3::Y,
                            [0.0; 2],
                        ));
                    }
                }
            }
        }
        let index = Index::all_directions(&vertices, &buildings);
        for mask in 0..1 << buildings.len() {
            let affected = || (0..buildings.len()).filter(|i| mask & (1 << i) != 0);
            let all = index.affected(affected());
            let expected: Vec<_> = vertices
                .iter()
                .enumerate()
                .filter_map(|(i, vertex)| {
                    affected()
                        .any(|b| {
                            let building = &buildings[b];
                            let ([x, z], [hx, hz]) = building.rect;
                            let reach = (building.top - building.base).max(0.0) / 0.05;
                            let p = Vec3::from(vertex.pos);
                            p.y <= building.base + 0.25
                                && p.x >= x - hx - reach - REACH
                                && p.x <= x + hx + reach + REACH
                                && p.z >= z - hz - reach - REACH
                                && p.z <= z + hz + reach + REACH
                        })
                        .then_some(i as u32)
                })
                .collect();
            assert_eq!(all, expected);
            for sun in [
                Vec3::X,
                -Vec3::X,
                Vec3::Z,
                -Vec3::Z,
                Vec3::Y,
                Vec3::new(1.0, 0.01, 0.7).normalize(),
                Vec3::new(-0.6, -0.2, 1.0).normalize(),
            ] {
                for fixed in Index::new(&vertices, &buildings, sun).affected(affected()) {
                    assert!(
                        all.binary_search(&fixed).is_ok(),
                        "fixed sun receiver remains a day-cycle target"
                    );
                }
            }
        }
    }
}
