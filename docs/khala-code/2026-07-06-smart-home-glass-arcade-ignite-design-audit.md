# Khala Code Smart Home Glass + Arcade/Ignite Design Audit

Date: 2026-07-06

Scope:

- Figma reference: `Smart Home App Design (Community)`, file key `EF6SpLYfXoV3AhHGo3beaM`.
- Product surfaces inspected: `clients/khala-code-desktop/`, `clients/khala-mobile/`, `apps/openagents.com/apps/web/src/page/code.ts`, and shared UI tokens in `packages/ui/`.
- Prior art base: Arcade/Ignite harvest already landed in `clients/khala-mobile` and documented in `docs/design/2026-07-05-arcade-ui-harvest-audit.md`.

## 1. What The Figma Effect Actually Is

The useful part of the Smart Home file is not the smart-home domain. It is a reusable dark iOS control language:

- a near-black navy base (`#080b1c`) with one or two oversized offscreen blue glows;
- glass cards with translucent blue radial fills, white/cyan hairline borders, and soft black depth shadows;
- circular 44 px icon buttons with border, blur, and inset highlight;
- pill CTAs that use a vertical dark-blue-to-cyan gradient with a top neon lip;
- segmented controls that make the active segment look charged, not merely selected;
- switch toggles with cyan gradient knobs, dark tracks, and real measured knob motion;
- a large circular control that layers rings, tick marks, glow, progress arc, center disk, and a cyan knob.

For Khala Code, the thermostat dial maps cleanly to a "capacity / active turn / verification" control. The dashboard cards map to fleet accounts, running turns, token flow, plan/status, and local readiness. The Smart Home tab bar maps less directly; Khala Code already has stronger sidebars/drawer patterns, so we should not import the floating iOS tab bar wholesale.

## 2. What Already Exists

### Desktop

`clients/khala-code-desktop/src/ui/styles.css` already has the right base vocabulary:

- dark Khala tokens (`--oa-color-khala-void`, surface, text, energy-blue, energy-cyan);
- grid/scanning backgrounds and a full-shell scene layer;
- dense, operator-facing panels for fleet, Gym, settings, transcript, review, and terminal surfaces;
- shared `@openagentsinc/ui` AI Elements for prompt input, response, reasoning, task, tool, diff, and code blocks.

The desktop shell is operational and dense, which is good. It should not be turned into a soft mobile-card toy. The Figma effect should show up as activation/state chrome on important controls, not as a total layout rewrite.

### Mobile

`clients/khala-mobile` already has the stronger Arcade/Ignite base:

- `components/frame/` ports Arcade's Skia frame, animated border, corner squares, and scaler primitives;
- `components/background-gradient/` ports Arcade's breathing Skia sweep-gradient surface;
- `components/toggle/toggle.tsx` ports the measured switch/knob animation;
- `components/drawer-icon-button.tsx` ports the hamburger-to-X morph;
- `theme/motion.ts` gives shared timing tokens instead of repeating Arcade's hardcoded durations;
- `theme/tokens.ts` recolors native tokens to Khala's accent/surface palette.

This means React Native should not recreate the Smart Home effect with ad hoc `View` stacks alone. The high-fidelity path is already known: Skia for luminous frames/glows/dials, Reanimated for state transitions, NativeWind tokens for layout and color routing.

### Web Landing

`apps/openagents.com/apps/web/src/page/code.ts` already has the honest Khala Code landing model: a representative coding conversation built from shared AI Elements over the persistent landing scene. That is the correct proof-oriented center. The weak point was the first impression: it led with chat content before giving the visitor a concrete, branded control surface.

## 3. Proposed Combination

Use the Smart Home file as a compositional reference, not a literal UI kit.

### Product Shell

Keep the Khala Code desktop shell dense and rectangular. Add the Figma/Arcade effect at component boundaries:

- active turn panels: glass card + breathing `BackgroundGradient` only while work is in progress;
- fleet worker cards: Arcade `Frame` around selected/active/blocked rows;
- primary run controls: Arwes-style framed buttons rather than generic filled buttons;
- toggles/settings: mobile `Toggle` switch for binary modes such as local-only, evidence posting, or notification preferences;
- transient state: Reanimated stagger/fade for new transcript parts and worker rows.

### Mobile React Native

For React Native, build a small "Khala Glass" component family on top of the existing Arcade ports:

1. `GlassPanel`: NativeWind `View` wrapper with tokenized dark fill, border, shadow, and optional Skia `BackgroundGradient` behind it.
2. `GlyphButton`: Arcade `Frame` + press highlight + centered icon/text, for compact actions.
3. `EnergySwitch`: the existing measured `Toggle` switch with Smart Home gradient colors.
4. `CapacityDial`: Skia circular dial inspired by the thermostat controller, backed by real capacity/turn state.
5. `StatusMetricGrid`: two-by-two stat grid from the thermostat bottom panel, for readiness, credit, local queue, and verification counts.

This keeps the Ignite/Arcade base intact while letting the Smart Home aesthetic provide the "glass energy" layer.

### Web Landing

The landing page should preview this direction without pretending the web surface is the native app. A web-only mock device/control panel is acceptable because it is illustrative and copy-safe. The first viewport should:

- keep "Khala Code" as the literal product signal;
- show own-capacity / Codex requirement honestly;
- present a glass control surface that visually matches where mobile/desktop are going;
- leave the existing AI Elements conversation below as proof of the coding-agent anatomy.

## 4. Implementation Boundaries

Do not:

- claim a public signed DMG exists until the release receipts exist;
- claim paid/free plan economics are live;
- imply Khala Code works without Codex on the current desktop path;
- add ad hoc icon SVGs to `apps/openagents.com`; use existing text/shape CSS there or the generated icon catalog when a real icon is needed;
- introduce a parallel RN animation system. Use Reanimated, Skia, and existing NativeWind tokens.

Do:

- route new visual constants through `packages/ui` / `clients/khala-mobile/src/theme/tokens.ts`;
- keep motion on `MOTION_FAST`, `MOTION_MEDIUM`, and `MOTION_AMBIENT`;
- reserve breathing/ambient loops for live work, active runs, or thinking states;
- use testable pure helpers for geometry where possible, especially for dial/progress math.

## 5. Recommended Build Order

1. Web landing preview: ship a copy-safe glass control mockup on `/code` so the public surface immediately communicates the direction.
2. RN `GlassPanel` and `GlyphButton`: compose existing `Frame` and `BackgroundGradient` rather than starting from scratch.
3. RN `CapacityDial`: implement the Smart Home thermostat idea as a Skia component with real props (`value`, `min`, `max`, `state`, `label`).
4. Desktop active state pass: apply the same tokens to fleet worker cards, turn-status panels, and primary controls.
5. Motion polish: add staggered transcript/worker row entrance and measured switch motion where real toggles exist.

## 6. Acceptance Checks

- `/code` still renders the shared AI Elements conversation and does not loosen the Khala Code promise gate.
- `/code/download` remains the exact install-truth path and keeps Codex visible.
- React Native additions reuse the existing Arcade ports and do not introduce a second animation vocabulary.
- No public UI copy claims downloadable desktop availability, outside-user installs, live paid plans, or Codex-free operation.
