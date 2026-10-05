//! Numbers floating up from a hit, which the demolition yard and the Grove
//! both draw: a camera-facing label that rises and fades over [`FLOAT`]
//! seconds.

use crate::mesh::{Mesh, Vertex};
use glam::{Mat4, Vec3};

/// How long a number floats, s.
pub const FLOAT: f32 = 1.4;
/// Height of a floating number, m.
const NUMBER: f32 = 0.42;

/// A number floating up from a hit.
#[derive(Clone, Debug, PartialEq)]
pub struct Floater {
    pub at: Vec3,
    pub text: String,
    pub color: [f32; 3],
    pub start: f32,
}

/// Builds floating numbers seen from `eye`.
pub struct Painter {
    pub mesh: Mesh,
    eye: Vec3,
}

impl Painter {
    pub fn new(eye: Vec3) -> Self {
        Self {
            mesh: Mesh::default(),
            eye,
        }
    }

    /// A number rising and fading over [`FLOAT`] seconds.
    pub fn floater(&mut self, floater: &Floater, now: f32) {
        paint(&mut self.mesh, self.eye, floater, now);
    }
}

/// Draws `floater` at `now` into `mesh`, facing `eye`.
pub fn paint(mesh: &mut Mesh, eye: Vec3, floater: &Floater, now: f32) {
    let k = ((now - floater.start) / FLOAT).clamp(0.0, 1.0);
    let fade = 1.0 - k * k;
    let color = floater.color.map(|c| c * (0.35 + 0.65 * fade));
    text(
        mesh,
        eye,
        floater.at + Vec3::Y * (1.2 * k),
        &floater.text,
        NUMBER,
        color,
    );
}

/// The turn that makes text at `at` face a camera at `eye`.
pub fn facing(eye: Vec3, at: Vec3) -> Mat4 {
    let d = eye - at;
    Mat4::from_translation(at) * Mat4::from_rotation_y((-d.x).atan2(-d.z))
}

/// Text centered over `at`, facing a camera at `eye`.
pub fn text(mesh: &mut Mesh, eye: Vec3, at: Vec3, text: &str, height: f32, color: [f32; 3]) {
    let mut glyphs = Mesh::default();
    crate::doors::scene_label(
        &mut glyphs,
        text,
        Vec3::ZERO,
        height,
        coder_ui::theme::Intensity::Full,
    );
    let m = facing(eye, at);
    mesh.faces.extend(glyphs.faces.into_iter().map(|v| {
        Vertex {
            pos: m
                .transform_point3(Vec3::from(v.pos) - Vec3::Z * 0.02)
                .to_array(),
            color,
            ..v
        }
    }));
}
