//! The `layertree.events.v1` replay the Verse viewer uses
//! (`crates/verse/examples/layertree/`), compiled in from its files so the
//! desktop viewer, the film shots, and this page check a run the same way.
//! They need only `glam`, `serde`, and `serde_json`.

#[path = "../../verse/examples/layertree/escape.rs"]
pub mod escape;
#[path = "../../verse/examples/layertree/model.rs"]
pub mod model;
#[path = "../../verse/examples/layertree/watch.rs"]
pub mod watch;
