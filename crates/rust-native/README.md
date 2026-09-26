# Rust Native

Rust Native is OpenAgents' experimental Rust foundation for shared UI
components, typed interactions, and styles. Applications describe what a screen
means; platform adapters render it with terminal facilities, semantic web
elements, or native controls. The planned iOS adapter uses SwiftUI.

This crate lives in the OpenAgents workspace. It has no stable API, standalone
release, JavaScript runtime, or native renderer yet. It depends on `serde` and
`serde_json`, with no network, executor, or platform framework dependency.

## What works now

| Area | Implemented foundation |
| --- | --- |
| Views | Serializable `View<I>` and keyed `Node<I>` trees with `Stack`, `Text`, and `Button`. Validation checks identities, labels, schema, and resource limits. |
| Interactions | An `Activation` names a surface instance, revision, and node. Only a current enabled button resolves to the application's typed intent. The application still checks authority and performs effects. |
| Styles | Named `StyleSheet` declarations, ordered composition of canonical leaf properties, and explicit `Unset`, `Set`, and `Reset`. Resolved values are semantic tokens. |
| Theme | The existing four amber intensities and background colors, moved from the public terminal code without changing their values. |
| Adoption | `coder-terminal` re-exports the same `Intensity` type and backgrounds. Existing terminal consumers, including Verse's palette, receive the shared definitions through that compatibility path. |

Stateful application hosts, renderer implementations, native text input,
accessibility mappings, virtualization, and a shared Markdown document model
are planned. A valid tree does not demonstrate native rendering or grant
permission to execute a task.

## Describe a screen

Applications supply their own closed serializable intent enum. For example,
this creates an inert button view:

```rust
use rust_native::{Element, Node, View};
use rust_native::style::Style;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
enum Intent {
    InspectTask { task: String },
}

let screen = View::new("task-panel:mount-1", 1, Node {
    key: "inspect".into(),
    style: Style::default(),
    element: Element::Button {
        label: "Inspect task".into(),
        enabled: true,
        intent: Intent::InspectTask { task: "task-1".into() },
    },
}).validate()?;
let bytes = screen.to_json()?;
```

The [task status example](examples/task_status.rs) adds styles and a vertical
stack. It emits data only and never starts an agent or contacts a service.
The [view contract](docs/spec.md#view-and-interaction-contract) explains
revision lifetimes and the difference between resolving an intent and
authorizing an action.

## Design and migration

Start with the [documentation index](docs/README.md):

- [Specification](docs/spec.md): ownership, views, adapters, native input,
  effects, and trust boundaries.
- [Build order](docs/build-order.md): the smallest useful migrations and their
  completion criteria.
- [Existing-code adoption map](docs/adoption.md): specific files and consumers.
- [Stylesheet design](docs/styling.md): StyleX lessons and Rust semantics.
- [Source review](docs/references.md): Effect Native and React Native findings
  and pinned sources.

[Issue #9693](https://github.com/OpenAgentsInc/openagents/issues/9693) tracks
this foundation. The broader [suite tracker](../../docs/coder/migration-status.md)
and [master roadmap](../../docs/roadmap.md) track the applications it will serve.

## Development checks

Use the repository's pinned toolchain and a Cargo target directory dedicated
to your worktree:

```sh
cargo test -p rust-native -p coder-terminal --lib
cargo clippy -p rust-native -p coder-terminal --all-targets -- -D warnings
cargo fmt -p rust-native -p coder-terminal --check
```

These checks cover the core and its first consumer. They do not claim a native
application works. Platform acceptance belongs to the adapter that adds that
behavior; the full workspace gate remains release-only.
