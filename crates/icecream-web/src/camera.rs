//! The Roblox-style camera: third person behind the avatar, orbited by a
//! right-drag, zoomed with the wheel (first person all the way in), Shift
//! Lock over the right shoulder with a crosshair, and V to switch between
//! first person and third person (`docs/verse/games/the-ice-cream-man.md`,
//! Camera).
//!
//! Third person is a boom from the avatar's head. Indoors the boom tilts
//! down so the camera stays under the ceiling, and a sphere swept from the
//! head to the camera against the walls, furniture, lintels, floor and
//! ceiling pulls it in short of anything solid. [`Boom`] smooths the length:
//! it snaps in at once (never through a wall) and eases back out slowly.

use glam::{Mat4, Vec3};
use icecream_rules::geom::{V2, v};
use icecream_rules::layout::{DOOR, DOOR_HEIGHT, EYE_HEIGHT, House, WALL, WALL_HEIGHT};

/// Closest and farthest zoom; under [`FIRST_PERSON`] the camera is in the
/// avatar's head.
pub const MIN_ZOOM: f32 = 0.5;
pub const MAX_ZOOM: f32 = 8.0;
pub const FIRST_PERSON: f32 = 0.9;
/// The avatar's eyes: first person looks from here and third person looks
/// at here.
pub const HEAD: f32 = EYE_HEIGHT;
/// The camera's eye stays this far under the ceiling.
pub const HEADROOM: f32 = 0.3;
/// The highest the camera goes in a room.
pub const CEILING: f32 = WALL_HEIGHT - HEADROOM;
/// The lowest it goes.
pub const FLOOR: f32 = 0.35;
/// The swept sphere's radius and the gap left in front of a hit.
pub const RADIUS: f32 = 0.2;
pub const MARGIN: f32 = 0.08;
/// Under this boom the avatar would fill the view, so it isn't drawn.
pub const CLOSE: f32 = 0.6;
/// How fast a pulled-in boom eases back out, per second.
pub const EASE_OUT: f32 = 3.0;
/// Up and down limits, in radians below the horizon.
pub const MIN_PITCH: f32 = -0.6;
pub const MAX_PITCH: f32 = 1.2;
/// Shift Lock's shoulder offset, to the right.
pub const SHOULDER: f32 = 0.7;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// Turn about the vertical; 0 looks toward +z.
    pub yaw: f32,
    /// Radians below the horizon.
    pub pitch: f32,
    pub zoom: f32,
    pub shift_lock: bool,
    /// The third-person zoom to go back to from first person.
    pub third: f32,
}

/// The third-person zoom a run starts with.
pub const THIRD_PERSON: f32 = 4.5;

impl Default for Camera {
    fn default() -> Self {
        Self {
            yaw: std::f32::consts::PI,
            pitch: 0.35,
            zoom: THIRD_PERSON,
            shift_lock: false,
            third: THIRD_PERSON,
        }
    }
}

/// Where the camera is this frame and what it looks at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub eye: Vec3,
    pub target: Vec3,
    pub first_person: bool,
}

/// A solid box in the room, for the camera to stop short of.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Box3 {
    pub min: Vec3,
    pub max: Vec3,
}

impl Box3 {
    /// Where along `a` to `b` (0 to 1) the segment enters the box grown by
    /// `r`; `None` if it misses or starts inside.
    #[must_use]
    pub fn sweep(&self, a: Vec3, b: Vec3, r: f32) -> Option<f32> {
        let (min, max) = (self.min - Vec3::splat(r), self.max + Vec3::splat(r));
        if a.cmpge(min).all() && a.cmple(max).all() {
            return None;
        }
        let d = b - a;
        let (mut low, mut high) = (0.0_f32, 1.0_f32);
        for axis in 0..3 {
            if d[axis].abs() < 1e-9 {
                if a[axis] < min[axis] || a[axis] > max[axis] {
                    return None;
                }
            } else {
                let (mut t0, mut t1) = (
                    (min[axis] - a[axis]) / d[axis],
                    (max[axis] - a[axis]) / d[axis],
                );
                if t0 > t1 {
                    std::mem::swap(&mut t0, &mut t1);
                }
                low = low.max(t0);
                high = high.min(t1);
                if low > high {
                    return None;
                }
            }
        }
        Some(low)
    }
}

/// What the camera can bump: every wall, piece of furniture and doorway
/// lintel as a box, and the floor and ceiling.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Space {
    pub boxes: Vec<Box3>,
    pub ceiling: f32,
}

impl Space {
    /// The house's solids.
    #[must_use]
    pub fn of(house: &House) -> Self {
        let mut boxes: Vec<Box3> = house
            .blocks
            .iter()
            .map(|b| Box3 {
                min: Vec3::new(b.min.x, 0.0, b.min.z),
                max: Vec3::new(b.max.x, b.height, b.max.z),
            })
            .collect();
        for (mid, along_x) in house.doorways() {
            let (hx, hz) = if along_x {
                (DOOR / 2.0, WALL / 2.0)
            } else {
                (WALL / 2.0, DOOR / 2.0)
            };
            boxes.push(Box3 {
                min: Vec3::new(mid.x - hx, DOOR_HEIGHT, mid.z - hz),
                max: Vec3::new(mid.x + hx, WALL_HEIGHT, mid.z + hz),
            });
        }
        Self {
            boxes,
            ceiling: WALL_HEIGHT,
        }
    }

    /// An empty room with the house's ceiling.
    #[must_use]
    pub fn open() -> Self {
        Self {
            boxes: Vec::new(),
            ceiling: WALL_HEIGHT,
        }
    }

    /// Where along `a` to `b` (0 to 1) a sphere of radius `r` first
    /// touches a box, the ceiling or the floor; 1 if nothing.
    #[must_use]
    pub fn cast(&self, a: Vec3, b: Vec3, r: f32) -> f32 {
        let mut t = 1.0_f32;
        let d = b - a;
        let top = self.ceiling - r;
        if b.y > top && a.y < top && d.y.abs() > 1e-9 {
            t = t.min((top - a.y) / d.y);
        }
        if b.y < r && a.y > r && d.y.abs() > 1e-9 {
            t = t.min((r - a.y) / d.y);
        }
        for block in &self.boxes {
            if let Some(hit) = block.sweep(a, b, r) {
                t = t.min(hit);
            }
        }
        t.max(0.0)
    }
}

/// This frame's boom from the avatar's head: which way it points for a
/// given length, and how long it can safely be. The shorter the boom, the
/// closer it tilts to the pitch asked for (a long one tilts down to stay
/// under the ceiling).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rig<'a> {
    space: &'a Space,
    /// The avatar's head (or Shift Lock's shoulder spot).
    pub pivot: Vec3,
    /// Level and away from where the camera faces.
    back: Vec3,
    /// The pitch asked for, as a sine.
    rise: f32,
    /// Where the camera looks in first person.
    look: Vec3,
    /// The boom's length the zoom asks for.
    pub want: f32,
    /// The longest boom that keeps the camera clear of everything.
    pub safe: f32,
    pub first_person: bool,
}

impl Rig<'_> {
    /// The unit direction from the pivot to the camera for a boom
    /// `length` long: the pitch asked for, lowered until the camera stays
    /// [`HEADROOM`] under the ceiling (and raised to stay over the floor).
    #[must_use]
    pub fn dir(&self, length: f32) -> Vec3 {
        let length = length.max(1e-3);
        let top = (self.space.ceiling - HEADROOM).max(FLOOR);
        let rise = (length * self.rise)
            .min(top - self.pivot.y)
            .max(FLOOR - self.pivot.y)
            .clamp(-length * 0.95, length * 0.95);
        let flat = (length * length - rise * rise).sqrt();
        (self.back * flat + Vec3::Y * rise) / length
    }

    /// How far a boom pointing as one `length` long goes before the swept
    /// sphere touches anything (`length` if nothing).
    #[must_use]
    pub fn reach(&self, length: f32) -> f32 {
        let end = self.pivot + self.dir(length) * length;
        self.space.cast(self.pivot, end, RADIUS) * length
    }

    /// `length`, or short of whatever its line touches by [`MARGIN`].
    #[must_use]
    pub fn clear(&self, length: f32) -> f32 {
        let reach = self.reach(length);
        if reach >= length - 1e-4 {
            length
        } else {
            (reach - MARGIN).max(0.0)
        }
    }

    /// The view with the boom `boom` long, never past what's clear.
    #[must_use]
    pub fn view(&self, boom: f32) -> View {
        if self.first_person {
            return View {
                eye: self.pivot,
                target: self.pivot + self.look,
                first_person: true,
            };
        }
        let length = self.clear(boom.clamp(0.0, self.safe));
        let dir = self.dir(length);
        let eye = self.pivot + dir * length;
        View {
            eye,
            target: eye - dir,
            first_person: false,
        }
    }
}

/// The boom's length over time: in at once, out slowly, so walls and
/// doorways never let the camera through and it never bounces.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Boom {
    pub length: Option<f32>,
}

impl Boom {
    /// Follows `rig` for `dt` seconds; the length to draw with.
    pub fn follow(&mut self, rig: &Rig, dt: f32) -> f32 {
        let next = if rig.first_person {
            0.0
        } else {
            match self.length {
                None => rig.safe,
                Some(l) if rig.safe <= l => rig.safe,
                Some(l) => l + (rig.safe - l) * (1.0 - (-EASE_OUT * dt).exp()),
            }
        };
        self.length = Some(next);
        next
    }

    /// Whether the camera is so close the avatar would fill the view.
    #[must_use]
    pub fn close(&self) -> bool {
        self.length.is_some_and(|l| l < CLOSE)
    }
}

impl Camera {
    /// The floor direction the camera faces.
    #[must_use]
    pub fn forward(&self) -> V2 {
        v(self.yaw.sin(), self.yaw.cos())
    }

    /// The floor direction to the camera's right.
    #[must_use]
    pub fn right(&self) -> V2 {
        let f = self.forward();
        v(-f.z, f.x)
    }

    /// A walk direction for keys held: forward and back, right and left.
    #[must_use]
    pub fn walk(&self, ahead: f32, side: f32) -> V2 {
        (self.forward() * ahead + self.right() * side).clamped(1.0)
    }

    /// Turns by a mouse or finger drag of `dx`, `dy` pixels.
    pub fn orbit(&mut self, dx: f32, dy: f32) {
        self.yaw -= dx * 0.006;
        self.pitch = (self.pitch + dy * 0.005).clamp(MIN_PITCH, MAX_PITCH);
    }

    /// Zooms by a wheel's `delta` (positive zooms out).
    pub fn wheel(&mut self, delta: f32) {
        self.zoom = (self.zoom * 1.1_f32.powf(delta / 100.0)).clamp(MIN_ZOOM, MAX_ZOOM);
    }

    #[must_use]
    pub fn first_person(&self) -> bool {
        self.zoom < FIRST_PERSON
    }

    /// V: into the avatar's head, or back out to where third person was.
    /// Returns whether it's first person now.
    pub fn toggle_view(&mut self) -> bool {
        if self.first_person() {
            self.zoom = self.third.max(FIRST_PERSON + 1.0);
            if self.pitch < 0.1 {
                self.pitch = 0.3;
            }
        } else {
            self.third = self.zoom;
            self.zoom = MIN_ZOOM;
            self.pitch = self.pitch.min(0.15);
        }
        self.first_person()
    }

    /// Whether the crosshair shows and tools aim where the camera looks.
    #[must_use]
    pub fn aims(&self) -> bool {
        self.shift_lock || self.first_person()
    }

    /// Which way the camera looks before any room gets in the way.
    #[must_use]
    pub fn look(&self) -> Vec3 {
        Vec3::new(
            self.pitch.cos() * self.yaw.sin(),
            -self.pitch.sin(),
            self.pitch.cos() * self.yaw.cos(),
        )
    }

    /// The boom for an avatar standing at `at`, `feet` above the floor, in
    /// `space`.
    #[must_use]
    pub fn rig<'a>(&self, at: V2, feet: f32, space: &'a Space) -> Rig<'a> {
        let head = Vec3::new(at.x, feet + HEAD, at.z);
        let look = self.look();
        let back = -self.forward();
        let mut rig = Rig {
            space,
            pivot: head,
            back: Vec3::new(back.x, 0.0, back.z),
            rise: self.pitch.sin(),
            look,
            want: 0.0,
            safe: 0.0,
            first_person: self.first_person(),
        };
        if rig.first_person {
            return rig;
        }
        if self.shift_lock {
            let r = self.right();
            let shoulder = Vec3::new(r.x, 0.0, r.z) * SHOULDER * 0.6 + Vec3::Y * 0.1;
            let length = shoulder.length();
            let t = space.cast(head, head + shoulder, RADIUS);
            rig.pivot += shoulder * ((t * length - MARGIN).max(0.0) / length);
        }
        rig.want = self.zoom.clamp(FIRST_PERSON, MAX_ZOOM);
        // Pull in until the boom's own (steeper, shorter) line is clear.
        let mut length = rig.want;
        for _ in 0..6 {
            let clear = rig.clear(length);
            if clear >= length {
                break;
            }
            length = clear;
        }
        rig.safe = rig.clear(length);
        rig
    }

    /// The camera with the boom as long as it can be, unsmoothed.
    #[must_use]
    pub fn view(&self, at: V2, feet: f32, space: &Space) -> View {
        let rig = self.rig(at, feet, space);
        rig.view(rig.safe)
    }

    /// The view and projection matrix for `view` at an aspect ratio.
    #[must_use]
    pub fn matrix(view: &View, aspect: f32) -> Mat4 {
        let fov = if aspect < 1.0 { 70.0_f32 } else { 60.0 };
        Mat4::perspective_rh_gl(fov.to_radians(), aspect, 0.05, 120.0)
            * Mat4::look_at_rh(view.eye, view.target, Vec3::Y)
    }
}

/// Where a ray from `eye` along `dir` meets the plane `y = height`, if it
/// goes that way.
#[must_use]
pub fn ray_to_height(eye: Vec3, dir: Vec3, height: f32) -> Option<V2> {
    if dir.y.abs() < 1e-5 {
        return None;
    }
    let t = (height - eye.y) / dir.y;
    (t > 0.0).then(|| {
        let p = eye + dir * t;
        v(p.x, p.z)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wall_behind() -> Space {
        Space {
            boxes: vec![Box3 {
                min: Vec3::new(-10.0, 0.0, -3.15),
                max: Vec3::new(10.0, WALL_HEIGHT, -2.85),
            }],
            ceiling: WALL_HEIGHT,
        }
    }

    /// The camera's eye keeps `RADIUS` minus a hair from every box.
    fn clear_of(space: &Space, eye: Vec3) -> bool {
        space.boxes.iter().all(|b| {
            let c = eye.clamp(b.min, b.max);
            c.distance(eye) >= RADIUS - 1e-3
        }) && eye.y <= space.ceiling - RADIUS + 1e-3
            && eye.y >= RADIUS - 1e-3
    }

    #[test]
    fn keys_walk_where_the_camera_faces() {
        let camera = Camera {
            yaw: 0.0,
            ..Camera::default()
        };
        let ahead = camera.walk(1.0, 0.0);
        assert!((ahead.z - 1.0).abs() < 1e-5 && ahead.x.abs() < 1e-5);
        let right = camera.walk(0.0, 1.0);
        assert!((right.x + 1.0).abs() < 1e-5, "{right:?}");
        let both = camera.walk(1.0, 1.0);
        assert!(
            (both.length() - 1.0).abs() < 1e-5,
            "diagonals aren't faster"
        );
    }

    #[test]
    fn the_camera_sits_behind_and_above_and_pulls_in_at_walls() {
        let camera = Camera {
            yaw: 0.0,
            pitch: 0.2,
            zoom: 4.5,
            shift_lock: false,
            third: 4.5,
        };
        let open = camera.view(V2::ZERO, 0.0, &Space::open());
        assert!(open.eye.z < -3.5, "behind: {:?}", open.eye);
        assert!(open.eye.y > HEAD && open.eye.y <= CEILING + 1e-4);
        let space = wall_behind();
        let walled = camera.view(V2::ZERO, 0.0, &space);
        assert!(
            walled.eye.z > -2.85 + RADIUS - 1e-3,
            "pulled in: {:?}",
            walled.eye
        );
        assert!(clear_of(&space, walled.eye));
    }

    #[test]
    fn indoors_the_pitch_lowers_so_the_camera_never_bumps_the_ceiling() {
        let empty = Space::open();
        for pitch in [0.0, 0.3, 0.6, 0.9, MAX_PITCH] {
            for zoom in [1.0, 2.0, 4.5, MAX_ZOOM] {
                for feet in [0.0, 0.5, 0.9] {
                    let camera = Camera {
                        pitch,
                        zoom,
                        ..Camera::default()
                    };
                    let rig = camera.rig(V2::ZERO, feet, &empty);
                    let view = rig.view(rig.safe);
                    assert!(
                        view.eye.y <= CEILING + 1e-4,
                        "{pitch} {zoom} {feet}: {view:?}"
                    );
                    // A flat-out room doesn't shorten the boom: only tilts it.
                    assert!((rig.safe - zoom).abs() < 1e-3, "{rig:?}");
                    assert!((rig.dir(rig.safe).length() - 1.0).abs() < 1e-4);
                }
            }
        }
        // Looking up from below stays over the floor.
        let camera = Camera {
            pitch: MIN_PITCH,
            zoom: MAX_ZOOM,
            ..Camera::default()
        };
        assert!(camera.view(V2::ZERO, 0.0, &Space::open()).eye.y >= FLOOR - 1e-4);
    }

    #[test]
    fn a_lintel_stops_the_boom_through_a_low_doorway() {
        // The avatar stands just past a doorway in a wall along x at z = 0,
        // the camera behind it on the far side, tilted up through the
        // opening: the lintel over the doorway pulls it in.
        let half = WALL / 2.0;
        let space = Space {
            boxes: vec![
                Box3 {
                    min: Vec3::new(-5.0, 0.0, -half),
                    max: Vec3::new(-DOOR / 2.0, WALL_HEIGHT, half),
                },
                Box3 {
                    min: Vec3::new(DOOR / 2.0, 0.0, -half),
                    max: Vec3::new(5.0, WALL_HEIGHT, half),
                },
                Box3 {
                    min: Vec3::new(-DOOR / 2.0, DOOR_HEIGHT, -half),
                    max: Vec3::new(DOOR / 2.0, WALL_HEIGHT, half),
                },
            ],
            ceiling: WALL_HEIGHT,
        };
        let camera = Camera {
            yaw: 0.0,
            pitch: 0.9,
            zoom: 4.5,
            ..Camera::default()
        };
        let rig = camera.rig(v(0.0, 3.0), 0.0, &space);
        assert!(rig.safe < rig.want - 1.0, "pulled in: {rig:?}");
        let view = rig.view(rig.safe);
        assert!(clear_of(&space, view.eye), "{view:?}");
        assert!(
            view.eye.z > WALL / 2.0 + RADIUS - 1e-3,
            "this side: {view:?}"
        );
    }

    #[test]
    fn a_narrow_hallway_keeps_the_camera_between_its_walls() {
        // A 1.6 m hallway along z; looking across it from the middle.
        let space = Space {
            boxes: vec![
                Box3 {
                    min: Vec3::new(-1.1, 0.0, -20.0),
                    max: Vec3::new(-0.8, WALL_HEIGHT, 20.0),
                },
                Box3 {
                    min: Vec3::new(0.8, 0.0, -20.0),
                    max: Vec3::new(1.1, WALL_HEIGHT, 20.0),
                },
            ],
            ceiling: WALL_HEIGHT,
        };
        let mut yaw = 0.0_f32;
        while yaw < std::f32::consts::TAU {
            let camera = Camera {
                yaw,
                ..Camera::default()
            };
            let view = camera.view(V2::ZERO, 0.0, &space);
            assert!(clear_of(&space, view.eye), "{yaw}: {view:?}");
            assert!(view.eye.y <= CEILING + 1e-4);
            yaw += 0.1;
        }
    }

    #[test]
    fn the_boom_snaps_in_and_eases_out_without_bouncing() {
        let camera = Camera {
            yaw: 0.0,
            ..Camera::default()
        };
        let space = wall_behind();
        let mut boom = Boom::default();
        let empty = Space::open();
        let open = camera.rig(V2::ZERO, 0.0, &empty);
        let walled = camera.rig(V2::ZERO, 0.0, &space);
        assert!(walled.safe < open.safe - 1.0);
        let long = boom.follow(&open, 1.0 / 60.0);
        assert!((long - open.safe).abs() < 1e-5, "starts where it can");
        // A wall swings in: the very next frame it's in front of it.
        assert_eq!(boom.follow(&walled, 1.0 / 60.0), walled.safe);
        // The wall goes: it eases back out, never overshooting.
        let mut last = walled.safe;
        let mut frames = 0;
        while open.safe - last > 0.01 {
            let next = boom.follow(&open, 1.0 / 60.0);
            assert!(next > last && next <= open.safe, "{last} -> {next}");
            assert!(next - last < 0.2, "eases, doesn't jump");
            last = next;
            frames += 1;
        }
        assert!(frames > 30, "slowly: {frames} frames");
        // Back to first person, then third person grows out of the head.
        let mut fp = camera;
        fp.toggle_view();
        assert_eq!(boom.follow(&fp.rig(V2::ZERO, 0.0, &empty), 0.1), 0.0);
        assert!(boom.close());
        assert!(boom.follow(&open, 1.0 / 60.0) < 0.5);
    }

    #[test]
    fn the_sweep_finds_boxes_but_not_the_one_it_starts_in() {
        let b = Box3 {
            min: Vec3::ZERO,
            max: Vec3::ONE,
        };
        let t = b
            .sweep(Vec3::new(-1.0, 0.5, 0.5), Vec3::new(1.0, 0.5, 0.5), 0.0)
            .unwrap();
        assert!((t - 0.5).abs() < 1e-5);
        assert!(
            b.sweep(Vec3::new(-1.0, 2.0, 0.5), Vec3::new(1.0, 2.0, 0.5), 0.0)
                .is_none()
        );
        assert!(
            b.sweep(Vec3::new(-1.0, 1.1, 0.5), Vec3::new(1.0, 1.1, 0.5), 0.2)
                .is_some()
        );
        assert!(
            b.sweep(Vec3::splat(0.5), Vec3::new(3.0, 0.5, 0.5), 0.0)
                .is_none()
        );
    }

    #[test]
    fn the_house_has_lintels_over_its_doorways() {
        let house = House::new(7);
        let space = Space::of(&house);
        let lintels = space
            .boxes
            .iter()
            .filter(|b| (b.min.y - DOOR_HEIGHT).abs() < 1e-5)
            .count();
        assert_eq!(lintels, house.doorways().len());
        assert_eq!(space.boxes.len(), house.blocks.len() + lintels);
    }

    #[test]
    fn zooming_all_the_way_in_is_first_person() {
        let mut camera = Camera::default();
        for _ in 0..100 {
            camera.wheel(-100.0);
        }
        assert_eq!(camera.zoom, MIN_ZOOM);
        assert!(camera.first_person() && camera.aims());
        let view = camera.view(v(2.0, 3.0), 0.0, &Space::open());
        assert!(view.first_person);
        assert_eq!(view.eye, Vec3::new(2.0, HEAD, 3.0));
        for _ in 0..100 {
            camera.wheel(100.0);
        }
        assert_eq!(camera.zoom, MAX_ZOOM);
    }

    #[test]
    fn v_switches_between_first_person_and_third_person() {
        let mut camera = Camera::default();
        camera.wheel(200.0);
        let zoom = camera.zoom;
        assert!(!camera.first_person());
        assert!(camera.toggle_view());
        assert!(camera.first_person() && camera.aims());
        let view = camera.view(v(1.0, 1.0), 0.0, &wall_behind());
        assert!(view.first_person);
        assert!(!camera.toggle_view());
        assert_eq!(camera.zoom, zoom, "back to the same third-person zoom");
        let view = camera.view(v(1.0, 1.0), 0.0, &Space::open());
        assert!(!view.first_person);
        assert!(view.eye.distance(Vec3::new(1.0, HEAD, 1.0)) > 4.0);
    }

    #[test]
    fn shift_lock_sits_over_the_right_shoulder() {
        let camera = Camera {
            yaw: 0.0,
            shift_lock: true,
            ..Camera::default()
        };
        assert!(camera.aims());
        let empty = Space::open();
        let rig = camera.rig(V2::ZERO, 0.0, &empty);
        assert!(rig.pivot.x < -0.3, "to the right, which is -x here");
    }

    #[test]
    fn orbiting_turns_and_tilts_within_limits() {
        let mut camera = Camera::default();
        let yaw = camera.yaw;
        camera.orbit(100.0, 0.0);
        assert!(camera.yaw < yaw);
        camera.orbit(0.0, 10_000.0);
        assert_eq!(camera.pitch, MAX_PITCH);
        camera.orbit(0.0, -10_000.0);
        assert_eq!(camera.pitch, MIN_PITCH);
    }

    #[test]
    fn a_ray_meets_the_aim_height() {
        let hit = ray_to_height(Vec3::new(0.0, 3.0, 0.0), Vec3::new(0.0, -1.0, 1.0), 1.0).unwrap();
        assert!((hit.z - 2.0).abs() < 1e-5);
        assert!(ray_to_height(Vec3::new(0.0, 3.0, 0.0), Vec3::new(0.0, 1.0, 1.0), 1.0).is_none());
    }
}
