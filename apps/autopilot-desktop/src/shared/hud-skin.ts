import type { HudStatusTone } from "./hud-status-projection.js"
import type { ManagedPane, PaneLayer } from "../ui/pane-manager.js"

// HUD H4 (#5502): the PURE skin vocabulary + small mapping logic for the
// simple white-on-black HUD skin applied to the HTML chrome (shell bar, hotbar,
// pane windows, full UI). The *aesthetic* lives in CSS (styles.css + index.html
// `:root` tokens); this module owns the bits of skin LOGIC that are worth a unit
// test, framework-free and DOM-free so the view and the tests agree:
//
//   - the canonical HUD palette as CSS-ready hex strings, mirroring
//     `HUD_STATUS_COLORS` in `@openagentsinc/three-effect/core` so the CSS
//     tokens and the three-effect H2 kit never drift (single source of the
//     WGPUI white-on-black + white-primary look — see
//     `docs/launch/2026-06-19-previous-hud-systems-audit.md` §4.5);
//   - which open pane is FOCUSED (top of the z-stack) so the view can give only
//     that window the bright rectangular frame + glow (the rest read dim);
//   - the status-tone → accent CSS class map, so a focused pane window's frame
//     can echo the live node status tone the H7 status overlay already projects
//     (one tone vocabulary across the HUD), degrading any unknown tone to the
//     neutral accent rather than throwing.
//
// Nothing here imports Three.js or the DOM; it is the same discipline as
// `hud-status-projection.ts`.

// The white/WGPUI HUD palette as CSS hex strings. Keys + values mirror
// `HUD_STATUS_COLORS` (hex *numbers*) in the H2 three-effect kit
// (`@openagentsinc/three-effect/core/hudPrimitives.ts`). Kept as a local copy of
// the strings so this pure module has no Three.js import, exactly like
// `hud-status-projection.ts` keeps its own copy of the tone keys. The
// `desktop-style-palette.test.ts` guards these against the old accent/frame
// tokens returning to the desktop source.
export const HUD_SKIN_COLORS = {
  primary: "#ffffff",
  secondary: "#f2f4f8",
  success: "#2bd576",
  info: "#ffffff",
  warning: "#f5c542",
  error: "#ff4d4d",
  neutral: "#9aa6b2",
  line: "#e6e9ef",
  background: "#0b0d12",
} as const

export type HudSkinColorKey = keyof typeof HUD_SKIN_COLORS

// The CSS custom-property names the skin exposes on `:root`. The view never
// hardcodes a hex; the (generated) stylesheet reads these tokens. Centralized
// here so the token names are a single source the CSS and any future consumer
// share.
export const HUD_SKIN_CSS_VARS: Readonly<Record<HudSkinColorKey, string>> = {
  primary: "--hud-primary",
  secondary: "--hud-secondary",
  success: "--hud-success",
  info: "--hud-info",
  warning: "--hud-warning",
  error: "--hud-error",
  neutral: "--hud-neutral",
  line: "--hud-line",
  background: "--hud-bg",
}

// Render the `:root` token block the skin needs, e.g.
// `--hud-primary:#ffffff;--hud-secondary:#f2f4f8;…`. The CSS file embeds the
// equivalent literally (Tailwind does not run TS), but exposing it keeps the
// values authoritative here and lets the test assert the CSS file matches.
export const hudSkinCssVarDeclarations = (): string =>
  (Object.keys(HUD_SKIN_COLORS) as Array<HudSkinColorKey>)
    .map((key) => `${HUD_SKIN_CSS_VARS[key]}:${HUD_SKIN_COLORS[key]};`)
    .join("")

// The focused pane is the one with the highest z (most recently opened/focused —
// `reducePaneLayer` bumps z on open + focus). Returns null for an empty layer.
// Ties (which `reducePaneLayer` never produces — z is monotonic) resolve to the
// last pane in array order, deterministically.
export const focusedPaneId = (layer: PaneLayer): string | null => {
  let top: ManagedPane | null = null
  for (const pane of layer.panes) {
    if (top === null || pane.z >= top.z) top = pane
  }
  return top?.id ?? null
}

// The set of status tones the accent map recognizes (mirrors `HudStatusTone`).
const KNOWN_TONES: ReadonlyArray<HudStatusTone> = [
  "success",
  "info",
  "warning",
  "error",
  "neutral",
]

// Normalize a live node status tone to a known accent tone. An unrecognized or
// missing tone degrades to "neutral" — never throws, never invents a tone.
export const hudAccentTone = (
  tone: HudStatusTone | string | null | undefined,
): HudStatusTone =>
  typeof tone === "string" &&
  (KNOWN_TONES as ReadonlyArray<string>).includes(tone)
    ? (tone as HudStatusTone)
    : "neutral"

// The CSS class for the focused pane window's accent frame, so its bright
// rectangular frame echoes the H7 status light's tone (one HUD vocabulary).
// Consumed in CSS as `.pane-window-accent-${tone}`.
export const paneWindowAccentClass = (
  tone: HudStatusTone | string | null | undefined,
): string => `pane-window-accent-${hudAccentTone(tone)}`
