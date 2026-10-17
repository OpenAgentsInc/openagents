//! openagents-doc — session & workspace Loro doc schemas and the typed mirror layer.
//!
//! Port of openagents's `packages/session-doc`. The schema SHAPE (container names, part maps with
//! LoroText bodies, command entries) is kept identical to the TS implementation so the edge's
//! tail materializer and any TS peer remain compatible.
//!
//! Load-bearing invariant (measured in openagents, `oplog-shape.test.ts`): message parts are a
//! LoroList of part maps whose text bodies live in **LoroText** — streaming appends RLE-merge at
//! ~1.03x oplog overhead, whereas rewriting whole part values costs ~125x.

pub mod commands;
pub mod continuation;
pub mod native_fork;
pub mod constants;
pub mod parts;
pub mod queue;
pub mod rebuild;
pub mod registry;
pub mod schema;
pub mod transcript_delta;
pub mod voice;
pub mod wait_target;
pub mod workspace;

pub use commands::*;
pub use constants::*;
pub use parts::*;
pub use queue::*;
pub use rebuild::*;
pub use registry::*;
pub use schema::*;
pub use transcript_delta::*;
pub use workspace::*;
