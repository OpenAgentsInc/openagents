# Adoption Checklist (Liquid Glass)

Use this as a practical guide while updating designs and implementations. Derived from the “Get to know the new design system” session transcript.

## Design

- Align visual language
  - Use fixed, capsule, and concentric shapes; avoid pinched/flared corners.
  - Harmonize control shapes with hardware curvature; align radii/margins concentrically.
  - Phone edges: add extra margin for capsule buttons; iPad/Mac: align concentric shapes to window edges.
- Colors and type
  - Validate across Light, Dark, Increased Contrast; ensure hue differentiation and legibility over materials.
  - Apply refined type: stronger/bolder, left‑aligned in key contexts (alerts, onboarding).
- Hierarchy and structure
  - Treat Liquid Glass as a functional layer above content; keep UI unobtrusive until needed.
  - Anchor transient UI (e.g., action sheets) to their source control.
  - Remove decorative bars/backgrounds; express hierarchy via layout, grouping, and spacing.
- Navigation elements
  - Group bar items using platform APIs; separate primary action (tinted).
  - Keep tab bars persistent; add a bottom Search tab on iOS when content isn’t upfront.
  - Use accessory views for persistent features; avoid screen‑specific actions in the tab bar.
- Effects and surfaces
  - Use scroll edge effects (one per view): Soft by default (iOS/iPadOS), Hard for macOS where stronger separation is needed.
  - Don’t mix/stack edge styles; ensure consistent heights in split views.
  - Build sidebars with Liquid Glass; let content flow behind using background extension effects as appropriate.

## Implementation

- Materials and layering
  - Apply system material directly to the control (not inner subviews) to preserve intended tints/contrast.
  - Place controls on material, not directly on content, to maintain legibility.
- Shapes and APIs
  - Prefer system shape semantics (capsule, rounded rect, concentric where available) to match platform behavior.
  - For dual‑mode components, use a concentric shape with a fallback radius (nested vs standalone).
- Bars and menus
  - Use grouping APIs so items share backgrounds and spacing automatically.
  - When menus adopt symbols, avoid duplicating/tweaking icons for related actions — one symbol introduces the group; labels disambiguate.
- Cross‑device continuity
  - Define shared component anatomy (slots for icon, label, accessory, selection indicator).
  - Ensure consistent core behaviors and feedback (selection, navigation, state) across platforms.

## Review and QA

- Test with varied wallpapers/backgrounds to vet contrast and legibility over materials.
- Verify dynamic appearances: Light/Dark/Increased Contrast; typography changes in key contexts.
- Exercise edge cases: split views, pinned headers, overlapping controls triggering edge effects.
- Validate navigation focus changes (e.g., dragging sheets): Liquid Glass should adjust opacity/size subtly to signal depth.

