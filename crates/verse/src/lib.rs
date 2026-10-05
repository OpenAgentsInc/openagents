//! Verse's shared world simulation, renderer, and Nostr session.
//!
//! Desktop and native surfaces use the same city, player controller, camera,
//! follower, and wgpu pipelines. The default `desktop` feature adds the winit
//! window and the host's model, XP, replay-file, and capture adapters. A build
//! without default features retains the shared world and network APIs without
//! the desktop agent or benchmark harness dependencies. The application palette
//! belongs to `coder_ui::theme`. Read `docs/verse/README.md`.

pub mod agent;
#[cfg(feature = "desktop")]
pub mod app;
#[cfg(feature = "native-audio")]
pub mod audio_native;
pub mod avatar;
pub mod ball;
pub mod blocks;
pub use verse_net::blocklist;
#[cfg(feature = "model-host")]
pub mod brain;
pub use verse_gfx::camera;
pub use verse_net::chat;
pub mod controller;
pub mod crowd;
pub mod doors;
#[cfg(all(target_os = "macos", feature = "desktop"))]
pub mod edr;
pub use verse_gfx::gles;
pub use verse_net::feed;
pub mod fx;
#[cfg(test)]
mod gles_tests;
pub use verse_gfx::gpu_lifecycle;
#[cfg(feature = "imported-surface")]
pub mod grid_engine;
pub mod grid_frame;
pub mod grid_pack;
#[cfg(not(target_arch = "wasm32"))]
pub mod gym;
pub mod gym_evals;
pub mod gym_hall;
pub mod gym_notes;
pub mod gym_replay;
#[cfg(not(target_arch = "wasm32"))]
pub mod gym_results;
#[cfg(feature = "hosted-social")]
pub mod hosted;
pub mod hud;
pub use verse_net::identity;
pub mod imported;
#[cfg(not(target_arch = "wasm32"))]
pub mod loopback;
pub use verse_pbr::mesh;
pub mod minimap;
pub use verse_net::mv;
pub mod nav;
pub use verse_gfx::overlay;
pub use verse_gfx::palette;
pub use verse_net::net;
#[cfg(feature = "panels")]
pub mod panels;
pub use verse_pbr::pbr;
pub mod pillar;
pub use verse_gfx::profiling;
pub mod render;
pub mod replay;
pub mod ritual;
pub mod runtime;
pub mod session;
pub mod shared;
pub mod spectator;
#[cfg(not(target_arch = "wasm32"))]
pub use verse_pbr::streaming;
#[cfg(all(feature = "terminal", not(target_arch = "wasm32")))]
pub mod terminal;
#[cfg(unix)]
pub mod terminal_control;
pub mod tooltip;
pub use verse_gfx::ui;
pub mod world;
#[cfg(feature = "xp-host")]
pub use verse_net::xp;
pub mod zones;
