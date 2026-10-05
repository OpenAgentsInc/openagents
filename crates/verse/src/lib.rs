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
#[cfg(feature = "model-host")]
pub mod brain;
pub mod camera;
pub mod chat;
pub mod controller;
pub mod crowd;
pub mod doors;
#[cfg(all(target_os = "macos", feature = "desktop"))]
pub mod edr;
pub mod feed;
pub(crate) mod gles;
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
pub mod identity;
pub mod imported;
#[cfg(not(target_arch = "wasm32"))]
pub mod loopback;
pub mod mesh;
pub mod minimap;
pub mod mv;
pub mod nav;
pub mod net;
pub mod overlay;
pub mod palette;
#[cfg(feature = "panels")]
pub mod panels;
pub mod pbr;
pub mod pillar;
pub mod render;
pub mod replay;
pub mod runtime;
pub mod session;
pub mod shared;
pub mod spectator;
pub mod ui;
pub mod world;
#[cfg(feature = "xp-host")]
pub mod xp;
pub mod zones;
