# Rust Native

Rust Native is an experimental reusable Rust UI foundation. Applications
produce bounded semantic views with typed intents and generic styles. Platform
adapters render those views with native controls, terminal facilities, or web
elements. Applications retain their state, effects, permissions, and palettes.

The crate implements data contracts and validation. It does not yet include a
native renderer, mounting runtime, text editor, or stable API. It depends only
on `serde` and `serde_json`; it has no application, network, or executor
dependency. Its manifest and Apache-2.0 license are self-contained so the
library can be reused outside its containing workspace. No package release is
implied, and publication is disabled while the API is experimental.

## Current contract

| Area | Implemented |
| --- | --- |
| Views | Serializable `View<I>` and keyed `Node<I>` trees with stacks, bounded lists, text, and buttons. Validation checks schema, identities, labels, and resource limits. |
| Intents | An `Activation` names a surface instance, revision, and node. Only a current enabled button resolves to the application's stored typed intent. Resolving an intent neither authenticates a caller nor performs an effect. |
| Styles | Named declarations with ordered leaf-property composition and explicit `Unset`, `Set`, and `Reset`. Colors are generic sRGB RGBA values; spacing and text properties use typed values. |
| Examples | A product-neutral [settings view](examples/settings.rs) emits JSON only. |

Applications supply their own closed serializable intent type:

```rust
use rust_native::{Element, Node, View};
use rust_native::style::Style;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
enum Intent {
    OpenPreferences,
}

let screen = View::new("settings:mount-1", 1, Node {
    key: "preferences".into(),
    style: Style::default(),
    element: Element::Button {
        label: "Open preferences".into(),
        enabled: true,
        intent: Intent::OpenPreferences,
    },
}).validate()?;
let bytes = screen.to_json()?;
# Ok::<(), rust_native::ViewError>(())
```

Read the [view and adapter contract](docs/spec.md) and
[style semantics](docs/styling.md). These distinguish implemented validation
from the renderer and lifecycle behavior an adapter must supply.

## Development checks

Use a compatible pinned Rust toolchain and an isolated Cargo target directory:

```sh
cargo test -p rust-native --lib
cargo clippy -p rust-native --all-targets -- -D warnings
cargo fmt -p rust-native --check
```

Core tests and serialized examples do not establish platform rendering,
accessibility, text composition, or lifecycle support. Verify each adapter's
supported subset separately when implementing it.
