//! Unified trace event taxonomy

mod event;
mod span;

pub use event::{TraceEvent, Venue};
pub use span::{Span, SpanKind};
