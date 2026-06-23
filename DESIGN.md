# Design

The OpenAgents house style. StarCraft-Protoss energy: a dark void with
luminous blue energy, precise high-craft, technical typography. This is the
**base design language for all OpenAgents surfaces** — public explainer pages,
the Khala API surface, and (over time) the in-app product chrome. New surfaces
inherit this; they do not invent a new palette.

The canonical token source for the existing app is
`@openagentsinc/design-tokens` (projected into `apps/openagents.com/apps/web/
src/styles.css`). This file documents the Protoss *brand* layer that sits on
top of those neutrals: the glow palette, the energy motif, and how to apply
them. Khala-specific glow tokens are added under `@theme` in `styles.css`.

## Theme

- **Mood:** the void of space lit by Protoss psionic energy. Near-black,
  blue-tinted background; luminous blue energy as the only saturated color;
  crisp white primary text; cool desaturated blue-grays for secondary text.
- **Mode:** dark, always. The physical scene: a developer at night reading an
  engineering pitch on a screen that glows blue. The product runs over a
  persistent 3D pylon scene (HDR-emissive blue cores blooming through an
  `UnrealBloomPass`) dimmed by a 75%-black scrim; surfaces float over that void.
- **Color strategy:** *Restrained.* Tinted near-black neutrals + one committed
  blue accent (≤ ~10% of the surface), carried by glow rather than fills.
  Never drenched, never gradient-washed.

## Color

OKLCH where authoring new tokens; the legacy app tokens are hex/rgba and are
preserved as-is.

### Energy accents (the Protoss blue)

Sourced from the live 3D scene (`src/scene/landingSquares.ts`) so the UI and the
canvas behind it share one palette.

- **Pylon core blue** `#3a7bff` — the primary energy accent. The scene drives
  it above 1.0 (HDR) so it blooms. In flat UI use it for eyebrows, active
  dividers, focus rings, key links, and button borders/text.
- **Energy cyan** `#4fd0ff` — the brighter highlight at the hot end of the
  energy ramp. Use sparingly for the peak of a glow, hover lift, and the
  brightest text in a glowing label. Do not use it for body text.
- **Soft blue ink** `#8fb6ff` — desaturated blue for eyebrow text and link
  text on dark surfaces. Meets AA on the near-black panel.
- **Faint blue glow** `rgba(58,123,255, .12–.35)` — for `box-shadow` glow
  halos, divider gradients, and hairline rings. The glow is the motif; reach
  for it instead of solid fills.

### Surfaces (tinted blacks — never pure #000)

- **Void** `#000` is the scene canvas only (behind the scrim). UI surfaces are
  tinted near-blacks:
  - **Panel** `#0c0f13` at ~92% opacity — the long-form content panel, so a
    breath of the 3D scene bleeds through.
  - **Surface raised** `#11161d` — inset blocks (model cards, steps).
  - **Surface sunken** `#0a0d12` — code blocks (darkest, so glow reads on it).
  - **Hairlines** `#1d2530` (calm) → `rgba(58,123,255,.25)` (energized).

### Text

- **Primary** `#ffffff` — headings, key terms.
- **Body** `#c9d2dd` — cool blue-white, AA on the panel. The default reading
  color. NOT gray.
- **Secondary** `#aeb9c6` — notes, captions.
- **Faint** `#7e8a98` — footnotes, the least-load text.

### Contrast rules

- Body ≥ 4.5:1; large/bold headings ≥ 3:1; placeholder text ≥ 4.5:1.
- Gray-on-blue is banned. Secondary text over a blue tint uses a lighter blue,
  not gray.

## Typography

- **Family:** `Berkeley Mono` (the shipped technical font; `--font-family-mono`).
  It is the brand voice — earned, not costume — because OpenAgents is genuinely
  an engineering product. Headings, eyebrows, code, and key labels are mono.
  Long-form prose may fall back to the sans stack for reading comfort, but the
  Khala surface elevates mono throughout for a terminal-grade, precise feel.
- **Display / h1:** mono, tight tracking (`tracking-tight`, ≥ -0.04em floor),
  fluid `clamp()` with a ceiling ≤ 6rem. White, `text-wrap: balance`.
- **Section headings:** mono, white, with a glowing-blue eyebrow above the
  *first* hero only — section rhythm is carried by glowing dividers + numbered
  index, NOT an eyebrow on every section (avoid the AI eyebrow grammar).
- **Eyebrow:** mono, uppercase, wide tracking (`0.22em`), `#8fb6ff`. One on the
  hero; section headings use a small glowing index marker instead.
- **Body:** 1.6–1.7 line-height (light type on dark needs the room), capped at
  65–75ch.
- **Code:** mono, `#d7e2f0` on the sunken surface.

## Spacing & Layout

- Centered single column, `min(100%, 880px)`, generous side gutter so the 3D
  scene breathes around it.
- Rhythm by variation: large section separations (`mt-16`-class) with tight
  groupings inside. Vary, don't uniform-pad.
- Dividers are energized hairlines: a thin line that fades from
  `rgba(58,123,255,.4)` to transparent — a glowing seam, not a flat gray rule.
- Cards only where they are the right affordance (the two model ids, the
  numbered key steps). Never card-in-card.
- Responsive grids without breakpoints: `repeat(auto-fit, minmax(280px, 1fr))`.

## The glow / bloom motif

The signature move. Energy is conveyed by light, not by fills or gradients:

- **Glow halo:** `box-shadow: 0 0 24px -6px rgba(58,123,255,.45)` on
  energized borders, the hero rule, and the back-button on hover.
- **Energized hairline:** a 1px line with a horizontal blue→transparent
  gradient, optionally with a faint outer glow. Used as section dividers and
  the hero underline.
- **Index marker:** small mono section index in a glowing-blue pill (border +
  faint inner glow), replacing the per-section eyebrow.
- **Focus ring:** `ring-2 ring-[#3a7bff]/70` + faint glow — visible on dark.
- Bloom proper lives in the 3D scene (`UnrealBloomPass`, HDR cores). The flat
  UI *echoes* it with shadow-glow; it does not try to re-implement bloom in CSS.

## Motion

- Restrained and intentional. Ease-out (quart/quint/expo) only — **no bounce,
  no elastic** (Protoss is precise, not springy).
- A single tasteful hero entrance and hover-glow lifts; not fade-on-scroll for
  every section.
- Energized borders may have a very slow, low-amplitude glow pulse (echoing the
  scene's pulsing cores) — subtle, never strobing.
- `prefers-reduced-motion: reduce`: pulses become static glow; transitions
  become instant or a crossfade. Content is visible by default and never gated
  behind a class-triggered reveal (headless/hidden-tab safe).

## Z-index scale

Behind the scene canvas (z-0) → scrim (z-5) → page overlay (z-10) → sticky
back-button (z-20) → any modal/toast above. No arbitrary `9999`.

## Bans (house)

- No gradient text, no `background-clip: text`.
- No side-stripe (`border-left`) accents — full borders / glow / index markers.
- No violet/purple SaaS gradients; the only saturated hue is the energy blue.
- No pure-`#000` UI surfaces (tint the blacks); `#000` is the scene only.
- No card-in-card. No eyebrow on every section. No bounce easing.
