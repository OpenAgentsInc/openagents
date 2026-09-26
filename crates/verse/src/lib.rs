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
pub mod avatar;
#[cfg(feature = "model-host")]
pub mod brain;
pub mod camera;
pub mod chat;
pub mod controller;
pub mod crowd;
pub mod feed;
pub mod hud;
pub mod identity;
pub mod mesh;
pub mod mv;
pub mod net;
pub mod palette;
pub mod render;
pub mod replay;
pub mod runtime;
pub mod session;
pub mod ui;
pub mod world;
#[cfg(feature = "xp-host")]
pub mod xp;
