//! The film engine's take: a scene recorded once as one stream of messages
//! (shapes and images uploaded under content keys, then frames of records
//! that draw them), which any renderer, native or browser, draws on demand.
//! The take is the seam between what a video shows and how it is drawn.
//!
//! The take's shape is carried over from ManimGX's engine (`rust/engine/src/take.rs`,
//! MIT, Academa Inc.), reimplemented here for Verse's own records; see
//! `docs/verse/film.md`.

pub mod take;

pub use take::{Error, Film, Frame, Kind, Record, Take, Upload, VERSION, View, read};
