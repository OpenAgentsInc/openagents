//! LayerTree in a browser.
//!
//! The page's `<canvas id="layertree-canvas">` shows a shared Bitcoin output's
//! Merkle-sum tree as a Verse scene: thousands of members joining, paying
//! each other, splitting, and leaving; the signer and its checks; light
//! clients following each block; and, once a run, the signer switched off
//! and every member paid back by the timed unroll. The run is a seeded,
//! simulated `layertree.events.v1` stream ([`demo`]), or the real signet run
//! with `?run=signet`. Read `README.md` for the build and the page contract.
//!
//! A native build holds the generator, the replay, the playback stage, the
//! layout, and the words, for their tests.

pub mod demo;
pub mod layertree;
pub mod layout;
pub mod stage;
pub mod words;

#[cfg(target_arch = "wasm32")]
mod scene;
#[cfg(target_arch = "wasm32")]
mod web;
