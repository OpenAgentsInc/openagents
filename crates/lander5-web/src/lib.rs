//! `/lander5`'s procedural background: the Grid at twilight, drawn by two
//! GLSL passes through raw WebGL2.
//!
//! - [`shaders`] holds the GLSL (`shaders/`): the Shader Skills functions
//!   the passes call (`library.glsl`, MIT, gokayfem/shader-skills), the
//!   shared scene constants, the sky-view table pass, and the grid pass,
//!   plus the minifier the site runs before inlining them.
//! - The runner (wasm32 only) reads the two fragment sources from the page,
//!   fills the sky table once, then draws the grid every frame, with a
//!   half-resolution first frame, dynamic resolution, and one still frame
//!   for people who ask for reduced motion.
//!
//! The site (`openagents-web`, `src/pages/lander5.rs`) inlines the shaders,
//! the wasm-bindgen glue, and the module into the one HTML response.

pub mod shaders;

#[cfg(target_arch = "wasm32")]
mod bump;
#[cfg(target_arch = "wasm32")]
mod web;
