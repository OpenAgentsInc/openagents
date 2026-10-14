//! The Code Graph (#11622): a deterministic map of a repository's Rust
//! that agents query before they open source files.
//!
//! - [`build`] lists the code files of a commit (or a working tree), parses
//!   each with tree-sitter, and resolves names into one [`model::Graph`]:
//!   nodes for modules, functions, methods, structs, enums, unions, traits,
//!   impls, type aliases, consts, statics and macros, each with `file:line`;
//!   edges for calls, `use` imports, `impl` of a trait, `impl` for a type,
//!   and type references. No model runs in the build, and the same input
//!   gives byte-identical output: everything is sorted, and the builder,
//!   parser and grammar versions are recorded in the header.
//! - [`query`] answers the typed questions Coder's `graph_*` tools ask:
//!   callers, callees, impact, a path between two symbols, a symbol's
//!   definition, and a module's public surface. Answers are short
//!   `file:line` lists, never source.
//! - [`store`] finds the graph for a checkout: the published graph of the
//!   `code-graph` branch when it matches `HEAD` and the tree is clean,
//!   else a fast local build, cached under the repository's Git directory.
//! - [`tools`] holds the tool definitions and their execution.
//!
//! Names are resolved syntactically (scope, `use` imports, `Self`, and
//! unique names), not by the compiler: an edge the builder could not pin to
//! one target is kept only when it has a few candidates, and is marked
//! inexact. Calls inside macro arguments are found by a token pattern.

pub mod build;
pub mod extract;
pub mod model;
pub mod query;
pub mod store;
pub mod tools;

pub use model::Graph;
pub use query::Index;
