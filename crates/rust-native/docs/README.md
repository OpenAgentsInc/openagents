# Rust Native design

Rust Native carries the useful Effect Native idea into this repository's Rust
architecture: describe semantic components and typed interactions once, then
render them through platform adapters. Rust owns application state and domain
logic. Native controls retain their platform behavior.

The foundation is implemented; the full UI framework is a plan. Refer to the
[crate README](../README.md) for the current API and delivered subset.

| Document | Question it answers |
| --- | --- |
| [Specification](spec.md) | What belongs in the shared core, application host, and each renderer? |
| [Build order](build-order.md) | What should ship first, and how can existing development adopt it immediately? |
| [Adoption map](adoption.md) | Which public files and existing clients should change, and which behavior must remain? |
| [Styling](styling.md) | How should a Rust stylesheet work, and which StyleX ideas apply? |
| [Source review](references.md) | What do Effect Native and React Native actually implement, and what should Rust Native borrow? |

This plan extends the [Coder suite migration](../../../docs/coder/migration-status.md).
It does not restart stopped benchmark or platform acceptance runs, replace the
task runtime, or require a complete framework before independent issues proceed.
The existing [mobile feasibility evidence](../../../docs/coder/design/rust-mobile-feasibility.md)
describes UIKit and Android prototypes, not a delivered SwiftUI adapter.
