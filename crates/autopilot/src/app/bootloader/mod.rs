//! Bootloader Module - DSPy-style boot sequence for OpenAgents.
//!
//! This module implements a progressive boot sequence that discovers system
//! capabilities and shows real-time updates in the HUD.
//!
//! # Design
//!
//! The bootloader follows DSPy primitives:
//! - **Signatures**: Typed I/O contracts for each boot stage
//! - **Module**: BootloaderModule orchestrating the sequence
//! - **Tools**: Hardware/compute/network probes wrapping adjutant discovery
//!
//! # Usage
//!
//! ```rust,ignore
//! use tokio::sync::mpsc;
//! use crate::app::bootloader::{BootloaderModule, BootEvent};
//!
//! let (tx, mut rx) = mpsc::unbounded_channel();
//! let bootloader = BootloaderModule::new().with_events(tx);
//!
//! // Spawn boot task
//! tokio::spawn(async move {
//!     let manifest = bootloader.run().await?;
//!     println!("Boot complete in {:?}", manifest.boot_duration);
//! });
//!
//! // Consume events for UI updates
//! while let Some(event) = rx.recv().await {
//!     display::render_event(&event);
//! }
//! ```

pub mod display;
pub mod events;
pub mod module;
pub mod probes;
pub mod signatures;

pub use display::render_event;
pub use events::{BootEvent, BootStage, StageDetails};
pub use module::{BootloaderConfig, BootloaderModule};
pub use signatures::BootManifest;
