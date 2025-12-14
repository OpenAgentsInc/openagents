//! # coder_ui_runtime - Reactive UI Runtime
//!
//! A fine-grained reactive runtime for Coder's UI, inspired by Solid.js.
//! Provides signals, effects, memos, and a frame scheduler for building
//! high-performance reactive interfaces.
//!
//! ## Core Concepts
//!
//! - **Signal<T>**: Reactive state container that notifies subscribers on change
//! - **Memo<T>**: Cached derived value that recomputes when dependencies change
//! - **Effect**: Side effect that re-runs when its dependencies change
//! - **Scope**: Manages cleanup of effects and nested reactive computations
//!
//! ## Example
//!
//! ```rust,ignore
//! use coder_ui_runtime::{create_signal, create_effect, create_memo};
//!
//! // Create reactive state
//! let count = create_signal(0);
//!
//! // Create derived value
//! let doubled = create_memo(move || count.get() * 2);
//!
//! // Create side effect
//! create_effect(move || {
//!     println!("Count is now: {}", count.get());
//! });
//!
//! // Update state - effect runs automatically
//! count.set(5);
//! assert_eq!(doubled.get(), 10);
//! ```
//!
//! ## Architecture
//!
//! The runtime uses a push-pull reactive model:
//! - Signals push notifications to dependent effects/memos
//! - Effects and memos pull values when they need to recompute
//!
//! All reactive primitives are tied to a Scope for automatic cleanup.

pub mod command;
pub mod effect;
pub mod memo;
pub mod runtime;
pub mod scheduler;
pub mod scope;
pub mod signal;

// Re-exports
pub use command::{Command, CommandBus, CommandHandler};
pub use effect::{create_effect, Effect, EffectHandle};
pub use memo::{create_memo, Memo};
pub use runtime::Runtime;
pub use scheduler::{FramePhase, Scheduler};
pub use scope::{create_scope, Scope, ScopeId};
pub use signal::{create_signal, ReadSignal, Signal, WriteSignal};
