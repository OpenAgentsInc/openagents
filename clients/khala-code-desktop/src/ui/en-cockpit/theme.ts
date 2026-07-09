import { defineTheme } from "@effect-native/tokens"

// Khala Protoss-blue theme for the Effect Native fleet cockpit (MH-7 / EN-5).
//
// The upstream `@effect-native/tokens` snapshot EN-1 vendored (commit
// 6dda1d4) ships only `defaultTheme` / `defineTheme` — the shared
// `khalaTheme` export is not in that snapshot. So, exactly like EN-1's
// `/stage1` route defined a route-local theme via `defineTheme`, this proof
// defines the Protoss-blue palette locally rather than inventing a parallel
// styling system. When the shared `khalaTheme` lands in a re-vendored
// effect-native snapshot, this local theme should be replaced by it.
export const khalaCockpitTheme = defineTheme({
  spacing: {
    "0": 0,
    "0.5": 2,
    "1": 4,
    "1.5": 6,
    "2": 8,
    "2.5": 10,
    "3": 12,
    "3.5": 14,
    "4": 16,
    "5": 20,
    "6": 24,
    "8": 32,
    "10": 40,
    "12": 48,
    "16": 64,
    "20": 80,
    "24": 96,
    "32": 128,
    "40": 160,
    "48": 192,
    "56": 224,
    "64": 256,
  },
  color: {
    // Protoss-blue on near-black, matching the desktop shell's uniform theme.
    background: "#02040a",
    surface: "#081226",
    textPrimary: "#e8f0ff",
    textMuted: "#8fa6cc",
    accent: "#3a7bff",
    danger: "#ff5470",
    border: "#17315f",
    focus: "#4fd0ff",
  },
  radius: {
    none: 0,
    sm: 2,
    md: 4,
    lg: 8,
    xl: 12,
    full: 9999,
  },
  typeScale: {
    caption: { fontSize: 12, lineHeight: 16, fontWeight: 400 },
    body: { fontSize: 15, lineHeight: 22, fontWeight: 400 },
    label: { fontSize: 13, lineHeight: 18, fontWeight: 500 },
    title: { fontSize: 20, lineHeight: 28, fontWeight: 600 },
    heading: { fontSize: 32, lineHeight: 38, fontWeight: 600 },
  },
  breakpoint: {
    sm: 640,
    md: 768,
    lg: 1024,
    xl: 1280,
  },
  dimension: {
    xs: 160,
    sm: 220,
    md: 320,
    lg: 480,
    xl: 640,
    full: 9999,
  },
})
