# Structure and Navigation

## The functional layer

- Liquid Glass creates a functional layer that floats above content to add structure and clarity without stealing focus.
- It clarifies relationships between surfaces and keeps interactions feeling spatial yet grounded.

## Anchor interactions to their source

- Example: Action Sheet now springs from the originating control rather than the screen bottom.
- Define clear roles so the system understands the relationship; apply materials to the control itself, not its inner subviews.

## Bars and hierarchy (clean up customizations)

- With the new appearance, avoid extra backgrounds/borders that previously added weight; emphasis should come from layout and grouping, not decoration.
- Group bar items using the platform APIs so items share background and preserve spatial relationships automatically.
- If crowded, remove non‑essential items or move secondary actions into a “More” menu.
- Group by function and frequency; don’t group symbols with text (to avoid the appearance of a single button).
- Primary action stands apart and is tinted (e.g., blue checkmark on iOS/iPadOS; prominent text button on macOS).

## Tab bar and Search

- Tab bars provide persistent, app‑wide structure; organize clearly to aid wayfinding.
- iOS includes a dedicated bottom Search tab for fast, reachable access when content isn’t visible up front.
- Use accessory views in tab bars for persistent features (e.g., media controls); avoid screen‑specific actions (e.g., Checkout) that belong with contextual content.

## Scroll edge effects

- Controls sit on a system material above content (not directly on content) to preserve legibility.
- Scroll edge effects replace hard dividers, clarifying where UI meets content.
- They are not overlays; they don’t block or darken, and should only appear where floating UI exists.
- Two styles:
  - Soft (default; most iOS/iPadOS cases) — subtle transition for interactive elements on Liquid Glass.
  - Hard (primarily macOS) — stronger boundary for interactive text, un‑backgrounded controls, pinned table headers.
- Guidance:
  - Apply one scroll edge effect per view; avoid mixing/stacking styles.
  - In Split View, each pane can have its own edge effect; keep heights consistent to align visually.

## Sidebars and background extension

- Sidebars can extend to the edge and are built with Liquid Glass so content flows behind for immersion.
- Background extension effects let content expand behind sidebars (full width) while keeping visuals centered.
- Use extension effects for expansive surfaces (hero images, tinted backgrounds); ensure text/controls are layered above to avoid distortion.
- Scroll views extend beneath sidebars by default (e.g., carousels glide through), supporting discovery without interruption.
- Apply background extension effects per view as needed to compose richer layouts.

