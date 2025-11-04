# Layout Guidelines (Liquid Glass)

A concise, device‑agnostic summary of Apple’s HIG layout guidance with Liquid Glass. Focuses on principles, hierarchy, adaptability, and developer hooks — without device spec tables.

## Principles

- Consistent and adaptive: Layout should feel familiar, scale gracefully, and keep content primary.
- Content first: Controls support interaction and remain unobtrusive until needed.
- System harmony: Controls appear on Liquid Glass materials above content; boundaries are clarified with scroll edge effects, not hard dividers.

## Best Practices

- Group related items
  - Use negative space, background shapes, colors, materials, or separators to signal relationships.
  - Keep content and controls visually distinct.
- Prioritize essentials
  - Give the most important information space and prominence; move secondary details to secondary areas or views.
- Extend content to the edges
  - Full‑bleed backgrounds and artwork should reach the screen/window edges.
  - Ensure scrollable layouts extend to the bottom and sides; account for controls appearing above content.
- Use background extension when content doesn’t span the window
  - Create the appearance of content behind the control layer at the sides (e.g., beneath a sidebar/inspector).
  - Developer hooks: `backgroundExtensionEffect()` (SwiftUI) and `UIBackgroundExtensionView` (UIKit).

## Visual Hierarchy

- Differentiate controls from content
  - Place controls on Liquid Glass materials; use scroll edge effects to transition between content and control areas (see “Scroll views”).
- Place by importance and reading order
  - Put high‑priority items near the top and leading side; account for right‑to‑left languages.
- Align to communicate structure
  - Align components and use indentation to clarify organization; improve scanability during scroll.
- Apply progressive disclosure
  - Hint at additional content (e.g., partial items, disclosure controls) when not all items can be shown at once.
- Space and grouping for controls
  - Provide touch/pointer‑friendly spacing; group related controls into logical sections (see “Toolbars”).

## Adaptability

Design for context changes while remaining recognizably consistent.

- Handle trait variations
  - Screen sizes, orientations, color spaces, window resizing, external displays, Dynamic Island/camera housing.
  - Text‑size changes via Dynamic Type; locale variations (RTL, formats, text length).
- Respect safe areas, margins, and guides
  - Use system safe areas and layout guides to avoid interactive/display features.
- Prefer adaptive frameworks
  - SwiftUI or Auto Layout for dynamic adaptation; otherwise implement equivalent behavior.
- Artwork scaling
  - Maintain aspect ratio; scale to keep important content visible when aspect changes would crop/letterbox.
- Preview broadly
  - Test extremes first (largest/smallest layouts); then expand to orientations, localizations, and text sizes. Use Simulator to catch clipping and layout issues.

## Guides and Safe Areas

- Layout guides
  - Use predefined guides for standard margins and optimal text widths; define custom guides as needed.
  - APIs: `UILayoutGuide`, `NSLayoutGuide`.
- Safe areas
  - Keep key content within safe areas to avoid bars and device features.
  - APIs: `SafeAreaRegions` (SwiftUI), “Positioning content relative to the safe area”.

## Implementation Notes (Liquid Glass)

- Controls on material, not content
  - Place controls atop a system material to preserve legibility over varied backgrounds.
- Scroll edge effects
  - Use edge effects to clarify where UI meets content; don’t stack or use decoratively.
  - Choose soft (most iOS/iPadOS) vs hard (often macOS for text/pinned headers) as appropriate.
- Background extension
  - Extend content behind sidebars/inspectors to maintain immersion while keeping text/controls above to avoid distortion.

## Review Checklist

- Content priority is clear; controls don’t obscure key information.
- Grouping and alignment communicate hierarchy; spacing supports comfortable interaction.
- Scroll edge effects and background extension are used correctly and not stacked.
- Safe areas respected; no clipping near sensors or system bars; margins feel consistent.
- Layout adapts across size, orientation, text size, and locale changes without losing familiarity.

## Related

- `docs/liquid-glass/structure-and-navigation.md` — Functional layer, bars, tabs, sidebars, edge effects.
- `docs/liquid-glass/visual-design.md` — Color, type, shapes, concentricity.
- `docs/liquid-glass/continuity-and-components.md` — Cross‑device continuity and component anatomy.
