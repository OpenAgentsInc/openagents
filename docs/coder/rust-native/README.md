# Rust Native adoption in Coder

The [Coder iOS reader](../guides/mobile-readonly.md) now exercises this adoption
path: Rust-owned external-harness history and cache, generic Rust Native lists
and text, and a thin SwiftUI renderer. [Verification](../verification/2026-09-26-mobile-reader.md)
separates targeted code checks, native simulator behavior, and distribution.
The plans below retain the wider roadmap; their original foundation-only
inventory is not the latest reader status.

Rust Native carries the useful Effect Native idea into this repository's Rust
architecture: describe semantic components and typed interactions once, then
render them through platform adapters. Rust owns application state and domain
logic. Native controls retain their platform behavior.

The foundation is implemented; the full UI framework is a plan. Refer to the
[framework README](../../../crates/rust-native/README.md) for the
current API and delivered subset.

| Document | Question it answers |
| --- | --- |
| [Specification](architecture.md) | What belongs in the shared core, application host, and each renderer? |
| [Build order](build-order.md) | What should ship first, and how can existing development adopt it immediately? |
| [Adoption map](adoption.md) | Which public files and existing clients should change, and which behavior must remain? |
| [Styling](styling-design.md) | How should a Rust stylesheet work, and which StyleX ideas apply? |
| [Source review](references.md) | What do Effect Native and React Native actually implement, and what should Rust Native borrow? |

This plan extends the [Coder suite migration](../migration-status.md).
It does not restart stopped benchmark or platform acceptance runs, replace the
task runtime, or require a complete framework before independent issues proceed.
The existing [mobile feasibility evidence](../design/rust-mobile-feasibility.md)
describes UIKit and Android prototypes, not a delivered SwiftUI adapter.

Product-specific architecture, roadmap, palette, source reviews, and adoption
plans live here. The reusable framework keeps only generic API documentation
in [`crates/rust-native`](../../../crates/rust-native/README.md);
[`coder-ui`](../../../crates/coder-ui/src/lib.rs) owns Coder presentation values.
