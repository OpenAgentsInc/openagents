# CUT-24 desktop accessibility audit + diagnostics/recovery receipt

- Issue: OpenAgentsInc/openagents#8704 (CUT-24)
- Surface: `apps/openagents-desktop` (Effect Native on Electron)
- Date: 2026-07-11
- Executable oracle: `apps/openagents-desktop/tests/accessibility.test.ts`
  (contrast + reduced-motion), `apps/openagents-desktop/src/renderer/diagnostics.test.ts`
  (accessible names)

This document is the written half of the CUT-24 accessibility audit; the
numbers below are locked as a test so a regression is red.

## Scope

DESKTOP core coding flows: composer/transcript, fleet, settings, command
palette, and the new diagnostics/preferences operability surfaces. Mobile
accessibility for core coding flows is a **separate app** (`apps/openagents-mobile`)
and is **out of scope** here — it is the named residual on #8704.

## Contrast (WCAG 2.1, computed from `@effect-native/tokens` khalaTheme)

Ratios are relative-luminance contrast of each text/UI role against each
surface. Thresholds: normal text ≥ 4.5:1 (1.4.3), large text / non-text UI
≥ 3:1 (1.4.11), disabled text EXEMPT (1.4.3 note).

| Role | background `#05070d` | surface `#0b1220` | surfaceRaised `#141f36` | surfaceOverlay `#182640` | verdict |
| --- | --- | --- | --- | --- | --- |
| textPrimary `#eef3ff` | 18.13 | 16.85 | 14.78 | 13.60 | AA (all) |
| textMuted `#93a4c3` | 8.00 | 7.43 | 6.52 | 6.00 | AA (all) |
| textFaint `#6b7ca1` | 4.82 | 4.48 | 3.93 | 3.61 | AA-large (≥3) |
| accent `#3b82f6` | 5.48 | 5.09 | 4.46 | 4.11 | AA-large / UI (≥3) |
| danger `#f87171` | 7.28 | 6.77 | 5.94 | 5.46 | AA (all) |
| success `#22c55e` | 8.84 | 8.22 | 7.21 | 6.63 | AA (all) |
| warning `#f59e0b` | 9.38 | 8.72 | 7.65 | 7.03 | AA (all) |
| info `#38bdf8` | 9.40 | 8.74 | 7.66 | 7.05 | AA (all) |
| textDisabled `#55648a` | 3.43 | 3.19 | 2.80 | 2.57 | EXEMPT (disabled) |
| focus ring `#60a5fa` | 7.92 | — | — | — | AA + (2.4.7 / 1.4.11) |

Findings:
- Primary and secondary body text (`textPrimary`, `textMuted`) pass AA on every
  surface with wide margin.
- Status text (danger/success/warning/info) passes AA on every surface.
- `textFaint` (uppercase section labels / captions — large-text class) and
  `accent` (links / primary controls — UI class) meet the applicable 3:1 floor
  on every surface; on the two darkest surfaces they sit just under the 4.5
  normal-text bar, which is why they are used for large/label/UI roles, not
  normal body copy.
- `textDisabled` is below AA on the raised surfaces; this is the WCAG 1.4.3
  disabled-control exemption and is recorded, not treated as a defect.
- The focus ring (`--en-color-focus`, a 2px outline drawn by the render-dom
  base stylesheet) is high-contrast (7.92:1), satisfying 2.4.7 (focus visible)
  and 1.4.11 (non-text contrast).

No token change was made: the single Protoss-blue identity already clears AA
for the text roles that carry normal copy. Changing a shared token is a color
identity decision (owner-gated "uniform blue") and was not required.

## Keyboard, focus, screen reader, target size

- **Keyboard / focus order:** the shell nav items, settings controls, command
  palette, and the new diagnostics controls are standard focusable `Button`
  / `TextField` / `Toggle` nodes in DOM order; the render-dom base stylesheet
  draws a `:focus-visible` outline on `button,a,input,textarea,[tabindex]`.
- **Screen-reader labels:** every interactive control in the diagnostics panel
  carries a non-empty accessible name (Button `label`); each health row is an
  `a11y.role: "group"` region labelled `"<domain>, status <level>"`. The
  existing composer/transcript/settings/fleet/command-palette nav already carry
  `accessibilityLabel`s (e.g. the settings toggle announces Open/Close).
  Asserted by `diagnostics.test.ts` (a11y: every interactive control has a
  non-empty accessible name).
- **Target size:** interactive controls render through the shared `control`
  token sizes (sm 24 / md 28 / lg 32 / xl 40px height); the density preference
  scales them proportionally and never below a 1px floor.
- **Reduced motion:** honored two ways — the OS `prefers-reduced-motion: reduce`
  media query zeroes transitions/animations, and an explicit in-app override
  (`data-en-reduce-motion="true"|"false"`) wins over the OS setting either way.
  Asserted by `accessibility.test.ts` + `desktop-preferences.test.ts`.
- **Dynamic type:** the font-scale preference scales the type-scale tokens
  (small → x-large) through the shared theme, resizing the whole app via the
  token pipeline. Asserted by `desktop-preferences.test.ts`.

## Residual

- **Mobile accessibility for core coding flows** (`apps/openagents-mobile`) is
  NOT met by this change and gates the #8704 close. Mobile has partial a11y
  (some `accessibilityLabel`s on the coding composer) but no reduced-motion
  handling, no contrast audit, and no comprehensive keyboard/focus/target-size
  acceptance for core coding flows.
- **PTY health** is honestly reported "unavailable" until the CUT-20 (#8700)
  PTY host merges.

## Diagnostics / recovery receipt (public-safe)

The diagnostics/watchdog panel was exercised in the built-Electron smoke
(`diagnostics-and-preferences` step). Public-safe result:

```
rowsRendered: true
levels: [provider Degraded, runtimeGateway Degraded, sync Unknown,
         workspace OK, pty Unavailable, extensions OK]   (smoke-fixture health)
secretLike: false          (no rendered diagnostics text is path/url/token-like)
noticeSafe: true           (redacted export notice carries no saved path)
prefRoundTrip: true        (preferences IPC: update compact → read compact → reset comfortable)
```

- Every export is redacted before it touches disk; a secret-pattern scrubber
  runs even if an upstream builder regresses (`diagnostics.test.ts` redaction).
- Recovery actions map only to safe typed paths (provider re-probe + fresh
  re-gathers); `restart_runtime` / `reconnect_sync` honestly report "no recovery
  action available" until a safe typed restart exists.
