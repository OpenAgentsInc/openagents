/**
 * CUT-24 criterion 2 (#8704): desktop accessibility acceptance for core coding
 * flows — contrast (WCAG AA), a visible focus ring, and reduced-motion honoring.
 *
 * This suite is the executable half of the accessibility audit (the written
 * summary lives in docs/desktop/2026-07-11-cut24-accessibility-audit.md). It
 * locks the theme's contrast ratios against WCAG 2.1 thresholds and pins the
 * reduced-motion CSS so a regression is red.
 *
 * WCAG 2.1 thresholds used:
 *  - 1.4.3 normal text        ≥ 4.5:1
 *  - 1.4.3 large text         ≥ 3:1  (≥ 18.66px bold or 24px)
 *  - 1.4.11 non-text/UI        ≥ 3:1
 *  - disabled/inactive text   EXEMPT (1.4.3 note)
 */
import { describe, expect, test } from "vite-plus/test"
import { readFileSync } from "node:fs"
import path from "node:path"

import { tokyoNightDesktopThemeProjection } from "../src/ide/tokyo-night-theme.ts"

// --- WCAG relative-luminance contrast ---------------------------------------
const hexToRgb = (hex: string): [number, number, number] => {
  const clean = hex.replace("#", "")
  const base = clean.length === 8 ? clean.slice(0, 6) : clean
  return [parseInt(base.slice(0, 2), 16), parseInt(base.slice(2, 4), 16), parseInt(base.slice(4, 6), 16)]
}
const channel = (value: number): number => {
  const c = value / 255
  return c <= 0.03928 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4)
}
const luminance = (hex: string): number => {
  const [r, g, b] = hexToRgb(hex)
  return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
}
export const contrastRatio = (a: string, b: string): number => {
  const la = luminance(a)
  const lb = luminance(b)
  const hi = Math.max(la, lb)
  const lo = Math.min(la, lb)
  return (hi + 0.05) / (lo + 0.05)
}

const color = tokyoNightDesktopThemeProjection.effectNative
const surfaces: ReadonlyArray<[string, string]> = [
  ["background", color.background],
  ["surface", color.surface],
  ["surfaceRaised", color.surfaceRaised],
  ["surfaceOverlay", color.surfaceOverlay],
]

describe("theme contrast meets WCAG AA for core coding text", () => {
  test("primary body text is ≥ 4.5:1 on every surface", () => {
    for (const [name, bg] of surfaces) {
      const ratio = contrastRatio(color.textPrimary, bg)
      expect(ratio, `textPrimary on ${name}`).toBeGreaterThanOrEqual(4.5)
    }
  })

  test("secondary (muted) body text is ≥ 4.5:1 on every surface", () => {
    for (const [name, bg] of surfaces) {
      const ratio = contrastRatio(color.textMuted, bg)
      expect(ratio, `textMuted on ${name}`).toBeGreaterThanOrEqual(4.5)
    }
  })

  test("status text (danger/success/warning/info) is ≥ 4.5:1 on the primary surfaces", () => {
    for (const role of [color.danger, color.success, color.warning, color.info]) {
      for (const bg of [color.background, color.surface]) {
        expect(contrastRatio(role, bg)).toBeGreaterThanOrEqual(4.5)
      }
    }
  })

  test("faint/caption text meets the AA-large / non-text 3:1 floor on every surface", () => {
    // textFaint drives uppercase section labels/captions (large-text class); AA-large is the applicable bar.
    for (const [name, bg] of surfaces) {
      expect(contrastRatio(color.textFaint, bg), `textFaint on ${name}`).toBeGreaterThanOrEqual(3)
    }
  })

  test("the accent (links / primary controls) meets the 3:1 UI-component floor on every surface", () => {
    for (const [name, bg] of surfaces) {
      expect(contrastRatio(color.accent, bg), `accent on ${name}`).toBeGreaterThanOrEqual(3)
    }
  })

  test("the focus ring is a high-contrast, clearly-visible indicator (WCAG 2.4.7 / 1.4.11)", () => {
    // The render-dom base stylesheet draws a 2px outline in `--en-color-focus`.
    expect(contrastRatio(color.focus, color.background)).toBeGreaterThanOrEqual(3)
    // In practice it is well above the floor (documented audit evidence).
    expect(contrastRatio(color.focus, color.background)).toBeGreaterThan(6)
  })

  test("disabled text is exempt from 1.4.3 but still legible-ish (documented)", () => {
    // WCAG 1.4.3 explicitly exempts inactive/disabled controls; we assert a sane
    // floor rather than AA so the audit records the exemption honestly.
    for (const [, bg] of surfaces) {
      expect(contrastRatio(color.textDisabled, bg)).toBeGreaterThanOrEqual(2.2)
    }
  })
})

describe("reduced-motion is honored (WCAG 2.3.3 / user preference)", () => {
  const css = readFileSync(path.join(import.meta.dirname, "..", "src", "renderer", "app.css"), "utf8")

  test("the OS prefers-reduced-motion media query zeroes transitions", () => {
    expect(css).toContain("@media (prefers-reduced-motion: reduce)")
    expect(css).toMatch(/transition-duration:\s*0\.01ms\s*!important/)
  })

  test("an explicit in-app reduced-motion override is honored regardless of the OS setting", () => {
    // The preference resolves to data-en-reduce-motion (see desktop-preferences-effects).
    expect(css).toContain('[data-en-reduce-motion="true"]')
  })
})
