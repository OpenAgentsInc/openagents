# ADR 0012 — Adopt Liquid Glass for Apple Platforms

- Date: 2025-11-04
- Status: Accepted — Standardization

## Context

Liquid Glass is Apple’s new first‑party UI material that brings a cohesive, adaptive visual system across iOS, iPadOS, and macOS. It provides dynamic translucency, refraction, tint, and morphing behaviors and is exposed via platform APIs (SwiftUI and UIKit) starting iOS 26 / iPadOS 26 / macOS 15 (Sequoia).

This ADR builds on ADR‑0011 (Swift Cross‑Platform App Experiment) by standardizing how we adopt Liquid Glass in our Apple‑native surfaces while keeping our existing Expo mobile app and Rust bridge contracts unchanged. It also aligns with our design documentation under `docs/liquid-glass/` (visual design, structure, layout, continuity, and APIs).

## Decision

Adopt Liquid Glass as the standard material for structural UI on Apple platforms (bars, sidebars, sheets/popovers, toolbars, tab bars’ accessory areas, cards/panels), with first‑party APIs and clear fallbacks.

- SwiftUI
  - Use `glassEffect(_:in:)` to apply the material to shapes.
  - Wrap multi‑glass layouts in `GlassEffectContainer`; tag elements with `.glassEffectID(_:)` when animating between states.
- UIKit
  - Use `UIGlassEffect` with `UIVisualEffectView` for glass surfaces on iOS 26+.
  - Fallback to `UIBlurEffect(style: .systemMaterial)` on earlier OS versions.
- Structure and hierarchy
  - Treat Liquid Glass as a functional layer floating above content. Do not place controls directly on content.
  - Use one scroll edge effect per view to clarify content↔UI boundaries (soft on iOS/iPadOS by default, hard on macOS when needed for text/pinned headers).
  - Use background extension effects to let content extend beneath sidebars/inspectors.
- Shapes and concentricity
  - Standardize on three shape types: fixed radius, capsule, and concentric.
  - macOS uses rounded rectangles for Mini/Small/Medium controls; Large/XL adopt capsule for emphasis.
- Accessibility and system behavior
  - Respect Reduce Transparency/Increase Contrast and the iOS 26.1 “Tinted” vs “Clear” system toggle.
  - Ensure text/icons meet contrast over glass; add subtle overlays or solid backgrounds where necessary.
- Cross‑surface policy
  - Swift app (ADR‑0011): adopt Liquid Glass immediately for Apple‑native views.
  - Expo app: align visual language and spacing; approximate with platform‑compatible materials (e.g., `.ultraThinMaterial` lookalikes / blur) where appropriate. Do not block on first‑party APIs in RN; keep implementations optional and non‑invasive.
  - Rust bridge: no change.

## Rationale

- Consistency: Liquid Glass creates a harmonized, Apple‑native look and depth model that maps to HIG guidance.
- Native performance and accessibility: first‑party APIs manage tint/contrast/motion and respond to system toggles.
- Clarity and hierarchy: scroll edge effects and background extension improve separation without heavy chrome.
- Future‑proofing: aligns our Swift app direction (ADR‑0011) with current Apple design language.

## Alternatives Considered

1) Keep using legacy `systemMaterial`/custom blur only
- Pros: broad OS support, known behavior.
- Cons: misses morphing and shape integration; less cohesive with current HIG.

2) Postpone adoption until RN has wrappers
- Pros: fewer surfaces diverging from Expo look.
- Cons: blocks Apple‑native polish; RN may lag or not expose full behaviors.

## Consequences

- Minimum OS dependency for full effect (iOS/iPadOS 26, macOS 15); we must ship graceful fallbacks on older versions.
- Additional QA across Light/Dark/Increased Contrast, Reduce Transparency/Motion, and the iOS 26.1 “Tinted” toggle.
- Performance considerations for large translucency regions; we’ll limit and group via `GlassEffectContainer`.
- Design system updates (shape tokens, spacing) to maintain concentric geometry and edge breathing on phone near device edges.

## Implementation Plan

Phase A — Design tokens and foundations
- Add shape semantics (fixed/capsule/concentric) and spacing rules matching HIG concentricity.
- Document edge behaviors (phone extra margin vs window‑aligned concentric on iPad/Mac).

Phase B — Swift app (ADR‑0011)
- Apply `glassEffect(_:in:)` and `GlassEffectContainer` to bars, sheets/popovers, cards/panels, and sidebars.
- Add scroll edge effects (one per view); prefer soft on iOS/iPadOS, hard on macOS when needed.
- Implement background extension under sidebars/inspectors.
- Add availability guards and pre‑26 fallbacks (`.ultraThinMaterial` in SwiftUI; `UIBlurEffect(systemMaterial)` in UIKit).

Phase C — Expo alignment (non‑blocking)
- Align spacing, grouping, and hierarchy with Liquid Glass guidance.
- Optional visual approximations via RN blur/backdrop where safe, behind a feature flag; do not introduce performance regressions.

Phase D — Accessibility and QA
- Verify contrast on both Clear and Tinted system modes; validate Reduce Transparency and Increased Contrast.
- Test with varied wallpapers/backgrounds and text scales (Dynamic Type).

Phase E — Developer guidance
- Maintain `docs/liquid-glass/` as the source for design/implementation guidance and examples.

## Acceptance

- Apple‑native app adopts Liquid Glass across primary structural surfaces with correct edge effects and background extension.
- All text/icons over glass pass contrast and remain legible under system toggles and accessibility settings.
- No stacked/mixed edge effects within a single view; one container per scene manages glass morphing.
- Pre‑26 OS behavior uses specified fallbacks without functional regressions.
- No changes to ACP/Tinyvex or bridge contracts.

## Open Questions

- Should we expose a user‑facing toggle inside the app mirroring the iOS 26.1 Clear/Tinted setting, or defer fully to the system?
- For complex nested hierarchies, do we scope one `GlassEffectContainer` per window/scene or per major region for best perf?
- Do we want a minimal RN native module to expose Apple glass on iOS 26+ for parity, or keep Expo purely approximated?

## References

- ADR‑0011 — Swift Cross‑Platform App (macOS + iOS) Experiment (`docs/adr/0011-swift-cross-platform-app-experiment.md`)
- Liquid Glass docs (internal): `docs/liquid-glass/` (visual design, structure, layout, continuity, APIs)
- Apple Developer — Applying Liquid Glass to custom views (SwiftUI)
- Apple Developer — GlassEffectContainer
- Apple Developer — Adopting Liquid Glass (Technology Overview)

