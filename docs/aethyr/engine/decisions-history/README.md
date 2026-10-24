> Deprecated (owner, 2026-10-24): history only. Architecture decisions live in [`docs/adr/`](../../../adr/README.md).

# Architecture decisions

Status: selected design decisions, not implementation results. Revisit decisions
using recorded requirements, alternatives, experiments, and consequences.

| Record | Decision |
| --- | --- |
| [0001](0001-hybrid-engine.md) | React/TypeScript and Effect 4 editor, Rust/Wasm CPU core, direct WebGPU browser graphics; graphics placement revised by 0006 |
| [0002](0002-incremental-world.md) | Semantic source, incremental derivation, atomic complete generation activation |
| [0003](0003-authority-and-publishing.md) | Serialized structural transactions, scoped CRDT text, immutable independent releases |
| [0004](0004-safe-behaviors.md) | Familiar TypeScript subset compiled to owned metered bytecode, not ambient JavaScript |
| [0005](0005-react-effect-frontend.md) | React DOM components, Effect Atom UI state, Effect 4 services; engine frames independent of UI rendering |
| [0006](0006-cross-platform-engine-core.md) | Web first, then mobile and desktop; one Rust engine core and renderer over `webgpu.h`, browser WebGPU on the web and Dawn natively |
| [0007](0007-gpu-driven-rendering.md) | GPU-resident scene, compute visibility, visibility buffer, temporal anti-aliasing, cluster LOD, and crowd tiers |
| [0008](0008-shader-toolchain.md) | Engine shaders authored in Slang and shipped as validated WGSL; creator materials stay WGSL |

An ADR changes only through a new/revised record with affected contracts and
qualification requirements. “Unlimited budget” does not exempt a decision from
browser support, security, resource accounting, or measurable falsification.
