import { khalaTheme } from "@effect-native/tokens";

import {
  DesktopThemeProjectionSchema,
  TokyoNightPaletteSchema,
  monacoThemeDataForProjection,
  pierreCssVariablesForProjection,
  tokyoNightDesktopThemeProjection,
} from "./tokyo-night-theme.ts";

/**
 * Khala's blue-black product identity with Tokyo Night's editor legibility.
 *
 * Chrome, surfaces, focus, selection, and terminal framing come from the
 * repository-owned Khala theme. Syntax hues intentionally retain Tokyo
 * Night's proven green/magenta/orange/cyan separation so the default changes
 * product identity without flattening code readability.
 */
export const khalaEditorPalette = TokyoNightPaletteSchema.make({
  background: khalaTheme.color.background,
  surface: khalaTheme.color.surface,
  surfaceRaised: khalaTheme.color.surfaceRaised,
  surfaceOverlay: khalaTheme.color.surfaceOverlay,
  foreground: khalaTheme.color.textPrimary,
  foregroundMuted: tokyoNightDesktopThemeProjection.palette.foregroundMuted,
  foregroundFaint: tokyoNightDesktopThemeProjection.palette.foregroundFaint,
  foregroundDisabled: tokyoNightDesktopThemeProjection.palette.foregroundDisabled,
  blue: khalaTheme.color.accent,
  blueHover: khalaTheme.color.accentHover,
  blueActive: khalaTheme.color.accentActive,
  cyan: tokyoNightDesktopThemeProjection.palette.cyan,
  green: tokyoNightDesktopThemeProjection.palette.green,
  orange: tokyoNightDesktopThemeProjection.palette.orange,
  red: tokyoNightDesktopThemeProjection.palette.red,
  magenta: tokyoNightDesktopThemeProjection.palette.magenta,
  yellow: tokyoNightDesktopThemeProjection.palette.yellow,
  border: khalaTheme.color.border,
  borderSubtle: khalaTheme.color.borderSubtle,
  borderStrong: khalaTheme.color.borderStrong,
  selection: "#3b82f64d",
  hover: khalaTheme.color.stateHover,
  active: khalaTheme.color.stateActive,
  scrim: khalaTheme.color.scrim,
  diffAdded: tokyoNightDesktopThemeProjection.palette.diffAdded,
  diffAddedBackground: "#22c55e20",
  diffRemoved: tokyoNightDesktopThemeProjection.palette.diffRemoved,
  diffRemovedBackground: "#f8717122",
  diffModified: khalaTheme.color.accent,
});

const surfaceProjection = {
  background: khalaEditorPalette.surface,
  foreground: khalaEditorPalette.foreground,
  border: khalaEditorPalette.border,
  accent: khalaEditorPalette.blue,
  danger: khalaEditorPalette.red,
  warning: khalaEditorPalette.yellow,
  success: khalaEditorPalette.green,
  info: khalaEditorPalette.cyan,
} as const;

export const khalaEditorDesktopThemeProjection = DesktopThemeProjectionSchema.make({
  schemaVersion: "openagents.desktop.khala-editor-projection.v1",
  id: "khala-editor",
  kind: "owned_static_data",
  initializedBeforeEditorPaint: true,
  recreatesModelsOrSessions: false,
  provenance: {
    upstream: "repository://@effect-native/tokens/khalaTheme",
    sourceCommit: "467bde0760d052ee2f4a8fa678bb2f1f6bf200d8",
    sourceFile: "apps/openagents.com/packages/effect-native-tokens/src/index.ts",
    license: "MIT",
    palettePolicy: "repository-owned-khala-semantic-projection",
  },
  accessibility: {
    adjustedForegroundFaint: true,
    focusWidthPixels: 2,
    focusOffsetPixels: 2,
    focusStyle: "solid-outline-plus-shape",
    minimumNormalTextContrast: 4.5,
    minimumLargeTextContrast: 3,
    reliesOnColorAlone: false,
  },
  palette: khalaEditorPalette,
  effectNative: {
    ...khalaTheme.color,
    textMuted: khalaEditorPalette.foregroundMuted,
    textFaint: khalaEditorPalette.foregroundFaint,
    textDisabled: khalaEditorPalette.foregroundDisabled,
    codeBackground: khalaEditorPalette.background,
    diffAdd: khalaEditorPalette.diffAdded,
    diffRemove: khalaEditorPalette.diffRemoved,
    syntaxKeyword: khalaEditorPalette.magenta,
    syntaxString: khalaEditorPalette.green,
    syntaxComment: khalaEditorPalette.foregroundFaint,
    syntaxFunction: khalaEditorPalette.blue,
    syntaxNumber: khalaEditorPalette.orange,
    syntaxOperator: khalaEditorPalette.cyan,
  },
  monaco: {
    base: "vs-dark",
    inherit: true,
    colors: {
      editorBackground: khalaEditorPalette.background,
      editorForeground: khalaEditorPalette.foregroundMuted,
      cursor: khalaEditorPalette.foreground,
      lineNumber: khalaEditorPalette.borderStrong,
      activeLineNumber: khalaEditorPalette.foregroundFaint,
      lineHighlight: khalaEditorPalette.surfaceRaised,
      selection: khalaEditorPalette.selection,
      inactiveSelection: "#3b82f625",
      findMatch: "#3b82f666",
      findMatchBorder: khalaEditorPalette.yellow,
      focusBorder: khalaEditorPalette.blueHover,
      gutterAdded: "#164846",
      gutterModified: "#294d80",
      gutterDeleted: "#823c41",
      error: khalaEditorPalette.red,
      warning: khalaEditorPalette.yellow,
      info: khalaEditorPalette.cyan,
    },
    rules: tokyoNightDesktopThemeProjection.monaco.rules.map((rule) => ({
      ...rule,
      foreground: rule.token === "function" ? khalaEditorPalette.blue : rule.foreground,
    })),
  },
  pierre: {
    themeName: "openagents-khala-editor",
    cssVariables: {
      background: khalaEditorPalette.background,
      foreground: khalaEditorPalette.foreground,
      numberForeground: khalaEditorPalette.foregroundFaint,
      contextBackground: khalaEditorPalette.background,
      contextGutterBackground: khalaEditorPalette.surface,
      additionForeground: khalaEditorPalette.diffAdded,
      additionBackground: khalaEditorPalette.diffAddedBackground,
      deletionForeground: khalaEditorPalette.diffRemoved,
      deletionBackground: khalaEditorPalette.diffRemovedBackground,
      selectionBackground: khalaEditorPalette.selection,
      separatorBackground: khalaEditorPalette.surfaceRaised,
      annotationBackground: khalaEditorPalette.surfaceOverlay,
      conflictMarkerForeground: khalaEditorPalette.yellow,
    },
    allowUnsafeCss: false,
    allowRemoteTheme: false,
  },
  terminal: {
    background: khalaEditorPalette.background,
    foreground: khalaEditorPalette.foreground,
    cursor: khalaEditorPalette.foreground,
    cursorAccent: khalaEditorPalette.background,
    selectionBackground: khalaEditorPalette.selection,
    ansi: [
      khalaEditorPalette.borderStrong,
      khalaEditorPalette.red,
      khalaEditorPalette.green,
      khalaEditorPalette.yellow,
      khalaEditorPalette.blue,
      khalaEditorPalette.magenta,
      khalaEditorPalette.cyan,
      khalaEditorPalette.foreground,
      khalaEditorPalette.foregroundDisabled,
      khalaEditorPalette.red,
      khalaEditorPalette.green,
      khalaEditorPalette.yellow,
      khalaEditorPalette.blueHover,
      khalaEditorPalette.magenta,
      khalaEditorPalette.cyan,
      "#ffffff",
    ],
  },
  surfaces: {
    problems: surfaceProjection,
    output: surfaceProjection,
    debug: surfaceProjection,
    review: surfaceProjection,
    proposal: surfaceProjection,
    browser: surfaceProjection,
    status: surfaceProjection,
  },
});

export const khalaEditorMonacoThemeData = () =>
  monacoThemeDataForProjection(khalaEditorDesktopThemeProjection);

export const khalaEditorPierreCssVariables = () =>
  pierreCssVariablesForProjection(khalaEditorDesktopThemeProjection);
