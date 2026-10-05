//! Portable admission shared by render clients, Rust tools, and dedicated hosts.
#[cfg(feature = "authoring")]
pub mod authoring;
pub mod collision;
#[cfg(feature = "compiler")]
pub mod compiler;
pub mod remote_content;
pub fn basis() -> glam::Mat4 {
    glam::Mat4::from_cols_array(&[
        0.0, 0.0, -0.9144, 0.0, -0.9144, 0.0, 0.0, 0.0, 0.0, 0.9144, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ])
}
