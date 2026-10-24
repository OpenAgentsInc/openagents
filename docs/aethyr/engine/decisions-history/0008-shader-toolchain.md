> Deprecated (owner, 2026-10-24): history only. Architecture decisions live in [`docs/adr/`](../../../adr/README.md).

# 0008: Author engine shaders in Slang, ship WGSL

Status: selected, 2026-10-24. Not implemented.

## Context

Browsers accept only WGSL. [Decision 0006](0006-cross-platform-engine-core.md)
makes native builds consume the same WGSL through Dawn. Hand-written WGSL has no
modules, generics, or interfaces, which makes a large GPU-driven shader library
([decision 0007](0007-gpu-driven-rendering.md)) hard to keep consistent across
material families, visibility passes, and capability variants.

## Decision

Author engine shaders in [Slang](https://shader-slang.org/) and compile them
offline to WGSL. The compiled WGSL is the release artifact for every platform,
identified by digest in the release manifest. The runtime never ships or runs a
Slang compiler.

Every generated module passes WGSL validation with both Tint (Dawn's compiler)
and Naga before release. Capability variants are separate compiled outputs
selected by the startup profile.

Creator material programs stay WGSL, inserted through the existing material
hook and compiled live in the browser. The hook's contract does not change.

## Alternatives and consequences

- **Hand-written WGSL only:** no new tool, but duplicated code across variants
  grows with every material family.
- **WGSL with an owned preprocessor or WESL:** smaller step, fewer language
  features. Acceptable fallback if Slang output fails the checks below.
- **HLSL or GLSL compiled to WGSL:** works through extra translation layers
  with weaker WGSL output control.

Consequences: a pinned Slang toolchain joins the build, and engineers debug
generated WGSL in browser tools. Generated source must stay readable enough to
debug.

## Revisit

Compare generated WGSL against hand-written versions of the same passes on the
pinned profiles. If generated shaders are measurably slower, fail validation on a
target browser, or exceed module size limits, switch that pass family to the
fallback.
