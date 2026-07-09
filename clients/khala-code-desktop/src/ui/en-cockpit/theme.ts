import { defineTheme } from "@effect-native/tokens"

// Khala Protoss-blue theme for the Effect Native fleet cockpit (MH-7 / EN-5).
//
// NOTE: the shared `khalaTheme` export now EXISTS in the vendored
// `@effect-native/tokens` snapshot (catalog v19, commit 3c1645e). This
// route-local theme is kept as working code for now, but it can and should be
// replaced by importing the canonical `khalaTheme` from `@effect-native/tokens`
// in a follow-up so the cockpit shares exactly one Protoss-blue palette.
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
    surfaceRaised: "#0f1d3a",
    textPrimary: "#e8f0ff",
    textMuted: "#8fa6cc",
    accent: "#3a7bff",
    danger: "#ff5470",
    border: "#17315f",
    focus: "#4fd0ff",
    info: "#38bdf8",
    success: "#22c55e",
    warning: "#f59e0b",
    codeBackground: "#0a0f1c",
    diffAdd: "#4ade80",
    diffRemove: "#f87171",
    syntaxKeyword: "#60a5fa",
    syntaxString: "#4ade80",
    syntaxComment: "#5b6b8c",
    syntaxFunction: "#c084fc",
    syntaxNumber: "#fbbf24",
    syntaxOperator: "#93a4c3",
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
