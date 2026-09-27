//! A shared, bounded map overlay. Hosts supply viewport insets and route state.
//! Map clicks select local walking destinations; they grant no service access.
#[cfg(test)]
use crate::world;
use coder_ui::theme::Intensity;
use serde::Serialize;

use crate::{
    controller::Footprint,
    ui::{self, Atlas, UiBatch},
};

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Landmark {
    pub id: &'static str,
    pub label: &'static str,
    pub x: f32,
    pub z: f32,
}

pub const LANDMARKS: [Landmark; 10] = [
    Landmark {
        id: "computer",
        label: "Computer",
        x: 0.0,
        z: -8.0,
    },
    Landmark {
        id: "gym",
        label: "Gym",
        x: 33.0,
        z: 0.0,
    },
    Landmark {
        id: "oracle",
        label: "Oracle",
        x: 27.0,
        z: 25.0,
    },
    Landmark {
        id: "library",
        label: "Library",
        x: -26.0,
        z: 24.0,
    },
    Landmark {
        id: "proving",
        label: "Proving ground",
        x: 0.0,
        z: 41.0,
    },
    Landmark {
        id: "plaza",
        label: "Plaza",
        x: -5.0,
        z: -10.0,
    },
    Landmark {
        id: "spark",
        label: "Spark door",
        x: -12.0,
        z: -9.0,
    },
    Landmark {
        id: "halo",
        label: "Halo door",
        x: 12.0,
        z: -9.0,
    },
    Landmark {
        id: "ruins",
        label: "Ruins portal",
        x: -12.0,
        z: 9.0,
    },
    Landmark {
        id: "lagrange1",
        label: "L1 portal",
        x: 12.0,
        z: 9.0,
    },
];

#[derive(Clone, Debug, Serialize)]
pub struct Snapshot {
    pub visible: bool,
    pub expanded: bool,
    pub state: String,
    pub destination: Option<[f32; 2]>,
    pub captured_pointers: Vec<u64>,
    pub frame: [f32; 4],
    pub plot: [f32; 4],
    pub center: [f32; 2],
    pub half_extent: f32,
    pub landmarks: Vec<Landmark>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MapAction {
    Toggle,
    Cancel,
    Walk([f32; 2]),
}
#[derive(Clone, Copy, Debug)]
struct Contact {
    id: u64,
    origin: [f32; 2],
    valid: bool,
    action: Option<MapAction>,
}

#[derive(Default)]
pub struct MapHud {
    pub expanded: bool,
    insets: [f32; 4],
    contacts: Vec<Contact>,
    elapsed: f32,
}

impl MapHud {
    pub fn set_insets(&mut self, values: [f32; 4]) -> Result<(), String> {
        if values
            .iter()
            .any(|x| !x.is_finite() || !(0.0..=256.0).contains(x))
        {
            return Err("Map safe area exceeds its bounds".into());
        }
        self.insets = values;
        Ok(())
    }
    pub fn tick(&mut self, dt: f32) {
        if dt.is_finite() {
            self.elapsed = (self.elapsed + dt.clamp(0.0, 0.05)) % 1000.0;
        }
    }
    pub fn clear_contacts(&mut self) {
        self.contacts.clear();
    }
    pub fn captured(&self, id: u64) -> bool {
        self.contacts.iter().any(|c| c.id == id)
    }
    pub fn down(&mut self, id: u64, at: [f32; 2], map: &Snapshot) -> bool {
        if !map.visible || !inside(map.frame, at) {
            return false;
        }
        if self.contacts.len() < 8 && !self.captured(id) {
            self.contacts.push(Contact {
                id,
                origin: at,
                valid: self.contacts.is_empty(),
                action: hit(map, at),
            });
            if self.contacts.len() > 1 {
                for c in &mut self.contacts {
                    c.valid = false;
                }
            }
        }
        true
    }
    pub fn moved(&mut self, id: u64, at: [f32; 2]) {
        if let Some(c) = self.contacts.iter_mut().find(|c| c.id == id) {
            c.valid &= (at[0] - c.origin[0]).hypot(at[1] - c.origin[1]) <= 12.0;
        }
    }
    pub fn up(&mut self, id: u64, at: [f32; 2], cancelled: bool) -> Option<MapAction> {
        self.moved(id, at);
        let index = self.contacts.iter().position(|c| c.id == id)?;
        let c = self.contacts.remove(index);
        if !cancelled && c.valid {
            c.action
        } else {
            None
        }
    }
    pub fn snapshot(
        &self,
        size: [f32; 2],
        player: [f32; 2],
        visible: bool,
        state: &str,
        destination: Option<[f32; 2]>,
    ) -> Snapshot {
        self.snapshot_for_zone(
            size,
            player,
            visible,
            state,
            destination,
            crate::zones::ZoneId::Plaza,
        )
    }
    pub fn snapshot_for_zone(
        &self,
        size: [f32; 2],
        player: [f32; 2],
        visible: bool,
        state: &str,
        destination: Option<[f32; 2]>,
        zone: crate::zones::ZoneId,
    ) -> Snapshot {
        let landmarks = match zone {
            crate::zones::ZoneId::Plaza => LANDMARKS.to_vec(),
            crate::zones::ZoneId::Lagrange1 => vec![
                Landmark {
                    id: "return",
                    label: "Plaza portal",
                    x: -5.0,
                    z: 20.0,
                },
                Landmark {
                    id: "airlock",
                    label: "Airlock",
                    x: 0.0,
                    z: 19.0,
                },
                Landmark {
                    id: "depot",
                    label: "Parts depot",
                    x: -11.0,
                    z: 1.0,
                },
                Landmark {
                    id: "jig",
                    label: "Keel jig",
                    x: 0.0,
                    z: -2.0,
                },
            ],
            crate::zones::ZoneId::Ruins => vec![
                Landmark {
                    id: "return",
                    label: "Plaza portal",
                    x: 0.0,
                    z: 15.0,
                },
                Landmark {
                    id: "glade",
                    label: "Glade",
                    x: 0.0,
                    z: 2.0,
                },
                Landmark {
                    id: "grove",
                    label: "Grove",
                    x: 18.0,
                    z: 0.0,
                },
            ],
        };
        let expanded_extra = 76.0 + landmarks.len().div_ceil(3) as f32 * 28.0;
        let [top, right, bottom, left] = self.insets;
        let available_w = (size[0] - left - right - 24.0).max(1.0);
        let available_h = (size[1] - top - bottom - 24.0).max(1.0);
        let minimum_height = if self.expanded {
            expanded_extra + 28.0
        } else {
            160.0
        };
        let visible = visible && available_w >= 100.0 && available_h >= minimum_height;
        let side = if self.expanded {
            available_w
                .min(available_h - expanded_extra)
                .clamp(1.0, 440.0)
        } else {
            116.0_f32
                .min(available_w)
                .min((available_h - 26.0).max(1.0))
        };
        let height = side + if self.expanded { expanded_extra } else { 26.0 };
        let x = if self.expanded {
            left + (available_w - side) * 0.5 + 12.0
        } else {
            size[0] - right - side - 12.0
        };
        let y = top + 12.0;
        Snapshot {
            visible,
            expanded: self.expanded,
            state: state.chars().take(100).collect(),
            destination,
            captured_pointers: self.contacts.iter().map(|c| c.id).collect(),
            frame: [x, y, side, height],
            plot: [x, y + 26.0, side, side],
            center: if self.expanded { [0.0, 0.0] } else { player },
            half_extent: if self.expanded {
                zone.half_extent()
            } else {
                58.0_f32.min(zone.half_extent())
            },
            landmarks,
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        atlas: &Atlas,
        map: &Snapshot,
        blockers: &[Footprint],
        player: [f32; 2],
        yaw: f32,
        route: &[[f32; 2]],
        scale: f32,
    ) -> UiBatch {
        let mut ui = UiBatch::default();
        if !map.visible {
            return ui;
        }
        let [x, y, w, h] = map.frame;
        let [px, py, pw, ph] = map.plot;
        let full = ui::amber(Intensity::Full, 1.0);
        let half = ui::amber(Intensity::Half, 1.0);
        let faint = ui::amber(Intensity::Quarter, 0.65);
        ui.rect(atlas, x, y, w, h, ui::field(0.985));
        ui.frame(atlas, x, y, w, h, 1.0, half);
        ui.text(
            atlas,
            x + 8.0,
            y + 5.0,
            if map.expanded { "WORLD MAP" } else { "MAP" },
            full,
        );
        ui.text(
            atlas,
            x + w - 18.0,
            y + 5.0,
            if map.expanded { "X" } else { "+" },
            full,
        );
        for i in 1..8 {
            let t = i as f32 / 8.0;
            ui.rect(atlas, px + pw * t, py, 0.5, ph, faint);
            ui.rect(atlas, px, py + ph * t, pw, 0.5, faint);
        }
        // A quiet scanning line and breathing destination ring make state visible.
        let scan = (self.elapsed * 0.10).fract();
        ui.rect(
            atlas,
            px,
            py + ph * scan,
            pw,
            0.8,
            ui::amber(Intensity::Half, 0.45),
        );
        for b in blockers {
            let a = project(map, [b.min[0], b.max[1]]);
            let c = project(map, [b.max[0], b.min[1]]);
            let l = a[0].max(px);
            let t = a[1].max(py);
            let r = c[0].min(px + pw);
            let bottom = c[1].min(py + ph);
            if r > l && bottom > t {
                ui.rect(
                    atlas,
                    l,
                    t,
                    r - l,
                    bottom - t,
                    ui::amber(Intensity::Quarter, 0.8),
                );
            }
        }
        if map.expanded && map.half_extent > 100.0 {
            for (name, u, v) in [
                ("NORTH WARD", 0.34, 0.08),
                ("SOUTH WARD", 0.34, 0.86),
                ("EAST", 0.06, 0.46),
                ("WEST", 0.80, 0.46),
                ("PLAZA", 0.43, 0.57),
            ] {
                ui.text(atlas, px + pw * u, py + ph * v, name, half);
            }
        }
        for landmark in &map.landmarks {
            let p = project(map, [landmark.x, landmark.z]);
            if inside(map.plot, p) {
                ui.frame(atlas, p[0] - 2.0, p[1] - 2.0, 4.0, 4.0, 1.0, half);
            }
        }
        let mut previous = player;
        for &next in route {
            let a = project(map, previous);
            let b = project(map, next);
            if let Some((a, b)) = clip_segment(map.plot, a, b) {
                line(&mut ui, atlas, a, b, 1.4, full);
            }
            previous = next;
        }
        if let Some(target) = map.destination {
            let p = project(map, target);
            if inside(map.plot, p) {
                let radius = 4.0 + 2.0 * (self.elapsed * 3.0).sin().abs();
                for i in 0..16 {
                    let angle = i as f32 * std::f32::consts::TAU / 16.0;
                    ui.rect(
                        atlas,
                        p[0] + angle.cos() * radius - 0.7,
                        p[1] + angle.sin() * radius - 0.7,
                        1.4,
                        1.4,
                        full,
                    );
                }
            }
        }
        let p = project(map, player);
        if inside(map.plot, p) {
            let tip = [p[0] + yaw.sin() * 7.0, p[1] - yaw.cos() * 7.0];
            line(&mut ui, atlas, p, tip, 2.0, full);
            ui.rect(atlas, p[0] - 2.0, p[1] - 2.0, 4.0, 4.0, full);
        }
        if map.expanded {
            let status = if map.state.is_empty() {
                "Choose a place to walk"
            } else {
                &map.state
            };
            let max = (w / atlas.advance).floor().max(1.0) as usize;
            ui.text(
                atlas,
                x + 6.0,
                py + ph + 5.0,
                &status
                    .chars()
                    .take(max.saturating_sub(2))
                    .collect::<String>(),
                full,
            );
            for (i, l) in map.landmarks.iter().enumerate() {
                let r = landmark_rect(map, i);
                ui.frame(
                    atlas,
                    r[0] + 2.0,
                    r[1] + 2.0,
                    r[2] - 4.0,
                    r[3] - 4.0,
                    0.8,
                    half,
                );
                let limit = ((r[2] - 10.0) / atlas.advance).max(1.0) as usize;
                ui.text(
                    atlas,
                    r[0] + 6.0,
                    r[1] + 8.0,
                    &l.label.chars().take(limit).collect::<String>(),
                    full,
                );
            }
            ui.text(atlas, x + 8.0, y + h - 20.0, "STOP WALK", half);
        }
        for v in &mut ui.vertices {
            v.pos[0] *= scale;
            v.pos[1] *= scale;
        }
        ui
    }
}

fn inside(rect: [f32; 4], point: [f32; 2]) -> bool {
    point.iter().all(|x| x.is_finite())
        && point[0] >= rect[0]
        && point[0] <= rect[0] + rect[2]
        && point[1] >= rect[1]
        && point[1] <= rect[1] + rect[3]
}
fn landmark_rect(map: &Snapshot, i: usize) -> [f32; 4] {
    [
        map.frame[0] + (i % 3) as f32 * map.frame[2] / 3.0,
        map.plot[1] + map.plot[3] + 24.0 + (i / 3) as f32 * 28.0,
        map.frame[2] / 3.0,
        28.0,
    ]
}
fn hit(map: &Snapshot, at: [f32; 2]) -> Option<MapAction> {
    if !map.expanded || at[1] < map.plot[1] {
        return Some(MapAction::Toggle);
    }
    if inside(map.plot, at) {
        return Some(MapAction::Walk([
            map.center[0] + ((at[0] - map.plot[0]) / map.plot[2] * 2.0 - 1.0) * map.half_extent,
            map.center[1] + (1.0 - (at[1] - map.plot[1]) / map.plot[3] * 2.0) * map.half_extent,
        ]));
    }
    for (i, l) in map.landmarks.iter().enumerate() {
        if inside(landmark_rect(map, i), at) {
            return Some(MapAction::Walk([l.x, l.z]));
        }
    }
    if at[1] >= map.frame[1] + map.frame[3] - 26.0 {
        Some(MapAction::Cancel)
    } else {
        None
    }
}
fn project(map: &Snapshot, p: [f32; 2]) -> [f32; 2] {
    [
        map.plot[0] + ((p[0] - map.center[0]) / map.half_extent + 1.0) * map.plot[2] * 0.5,
        map.plot[1] + (1.0 - (p[1] - map.center[1]) / map.half_extent) * map.plot[3] * 0.5,
    ]
}

/// Keep the visible part of a route, including segments with both ends offscreen.
fn clip_segment(rect: [f32; 4], a: [f32; 2], b: [f32; 2]) -> Option<([f32; 2], [f32; 2])> {
    let mut enter = 0.0_f32;
    let mut leave = 1.0_f32;
    for axis in 0..2 {
        let delta = b[axis] - a[axis];
        let lo = rect[axis];
        let hi = lo + rect[axis + 2];
        if delta.abs() < f32::EPSILON {
            if a[axis] < lo || a[axis] > hi {
                return None;
            }
        } else {
            let t0 = (lo - a[axis]) / delta;
            let t1 = (hi - a[axis]) / delta;
            enter = enter.max(t0.min(t1));
            leave = leave.min(t0.max(t1));
            if enter > leave {
                return None;
            }
        }
    }
    let at = |t: f32| [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
    Some((at(enter), at(leave)))
}

fn line(ui: &mut UiBatch, atlas: &Atlas, a: [f32; 2], b: [f32; 2], width: f32, color: [f32; 4]) {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let length = dx.hypot(dy);
    if length < 0.001 {
        return;
    }
    let start = ui.vertices.len();
    ui.rect(atlas, 0.0, -width * 0.5, length, width, color);
    for v in &mut ui.vertices[start..] {
        let [x, y] = v.pos;
        v.pos = [
            a[0] + x * dx / length - y * dy / length,
            a[1] + x * dy / length + y * dx / length,
        ];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn map_selects_exact_positions_and_cancelled_drags_do_nothing() {
        let mut hud = MapHud {
            expanded: true,
            ..MapHud::default()
        };
        let map = hud.snapshot([393.0, 852.0], [0.0, -10.0], true, "", None);
        let center = [
            map.plot[0] + map.plot[2] * 0.5,
            map.plot[1] + map.plot[3] * 0.5,
        ];
        assert!(hud.down(1, center, &map));
        assert_eq!(hud.up(1, center, false), Some(MapAction::Walk([0.0, 0.0])));
        hud.down(2, center, &map);
        hud.moved(2, [center[0] + 20.0, center[1]]);
        assert_eq!(hud.up(2, center, false), None);
        hud.down(3, center, &map);
        assert_eq!(hud.up(3, center, true), None);
    }

    #[test]
    fn every_landmark_has_a_separate_footer_target() {
        let mut hud = MapHud {
            expanded: true,
            ..MapHud::default()
        };
        let map = hud.snapshot([393.0, 852.0], [0.0, -10.0], true, "", None);
        for (index, landmark) in LANDMARKS.iter().enumerate() {
            let [x, y, w, h] = landmark_rect(&map, index);
            assert!(y + h <= map.frame[1] + map.frame[3] - 26.0);
            let point = [x + w * 0.5, y + h * 0.5];
            assert!(hud.down(index as u64, point, &map));
            assert_eq!(
                hud.up(index as u64, point, false),
                Some(MapAction::Walk([landmark.x, landmark.z]))
            );
        }
    }
    #[test]
    fn insets_and_visibility_bound_capture() {
        let mut hud = MapHud::default();
        hud.set_insets([62.0, 0.0, 34.0, 0.0]).unwrap();
        let map = hud.snapshot([393.0, 852.0], [0.0, 0.0], true, "", None);
        assert_eq!(map.frame[1], 74.0);
        assert!(!hud.down(1, [5.0, 5.0], &map));
        assert!(hud.set_insets([f32::NAN, 0.0, 0.0, 0.0]).is_err());
        let hidden = hud.snapshot([393.0, 852.0], [0.0, 0.0], false, "", None);
        assert!(!hud.down(1, [hidden.frame[0] + 2.0, hidden.frame[1] + 2.0], &hidden));
    }

    #[test]
    fn small_viewports_hide_both_map_sizes_and_refuse_capture() {
        for expanded in [false, true] {
            let mut hud = MapHud {
                expanded,
                ..MapHud::default()
            };
            hud.set_insets([20.0, 10.0, 20.0, 10.0]).unwrap();
            for size in [[143.0, 400.0], [400.0, 223.0]] {
                let map = hud.snapshot(size, [0.0, 0.0], true, "", None);
                assert!(!map.visible);
                let at = [map.frame[0] + 0.5, map.frame[1] + 0.5];
                assert!(!hud.down(1, at, &map));
                assert!(!hud.captured(1));
            }
            let boundary = hud.snapshot([144.0, 280.0], [0.0, 0.0], true, "", None);
            assert!(boundary.visible);
        }
    }

    #[test]
    fn route_segments_clip_at_every_edge_and_can_cross_the_entire_plot() {
        let rect = [10.0, 20.0, 100.0, 100.0];
        assert_eq!(
            clip_segment(rect, [60.0, 70.0], [160.0, 70.0]),
            Some(([60.0, 70.0], [110.0, 70.0]))
        );
        assert_eq!(
            clip_segment(rect, [-40.0, 70.0], [160.0, 70.0]),
            Some(([10.0, 70.0], [110.0, 70.0]))
        );
        assert_eq!(
            clip_segment(rect, [60.0, 170.0], [60.0, -30.0]),
            Some(([60.0, 120.0], [60.0, 20.0]))
        );
        assert_eq!(clip_segment(rect, [0.0, 0.0], [0.0, 150.0]), None);
        assert_eq!(clip_segment(rect, [-40.0, 70.0], [60.0, -130.0]), None);
    }

    #[test]
    fn compact_map_draws_a_route_to_an_offscreen_destination() {
        let hud = MapHud::default();
        let atlas = Atlas::new(12.0);
        let map = hud.snapshot([393.0, 852.0], [0.0, 0.0], true, "Walking", None);
        let before = hud.draw(&atlas, &map, &[], [0.0, 0.0], 0.0, &[], 1.0);
        let route = hud.draw(&atlas, &map, &[], [0.0, 0.0], 0.0, &[[200.0, 0.0]], 1.0);
        assert_eq!(route.vertices.len(), before.vertices.len() + 6);
    }

    #[test]
    fn seeded_world_and_far_route_fit_the_renderer_hud_budget() {
        let world = world::build();
        let start = [world::SPAWN.x, world::SPAWN.z];
        let route = crate::nav::plan(start, [252.0, 252.0], &world.blockers, world::HALF)
            .expect("the far street intersection is reachable");
        let atlas = Atlas::new(12.0);
        for expanded in [false, true] {
            let hud = MapHud {
                expanded,
                ..MapHud::default()
            };
            let map = hud.snapshot(
                [393.0, 852.0],
                start,
                true,
                "Walking",
                Some(route.destination),
            );
            let ui = hud.draw(
                &atlas,
                &map,
                &world.blockers,
                start,
                0.0,
                &route.waypoints,
                3.0,
            );
            let bytes = ui.vertices.len() * std::mem::size_of::<ui::UiVertex>();
            assert!(bytes < 2 * 1024 * 1024, "map uses {bytes} HUD bytes");
            assert!(
                ui.vertices
                    .iter()
                    .all(|v| v.pos.iter().all(|p| p.is_finite()))
            );
        }
    }
}
