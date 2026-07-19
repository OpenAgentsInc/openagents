import { ColorThemeSchema } from "@effect-native/tokens";
import { Schema } from "effect";

const HexColorSchema = Schema.String.check(
  Schema.isPattern(/^#[0-9a-f]{6}(?:[0-9a-f]{2})?$/u),
).annotate({ identifier: "TokyoNightHexColor" });

const colorFields = <const Keys extends ReadonlyArray<string>>(keys: Keys) =>
  Object.fromEntries(keys.map((key) => [key, HexColorSchema])) as {
    readonly [Key in Keys[number]]: typeof HexColorSchema;
  };

export const DesktopThemeProjectionSchemaVersion = Schema.Literals([
  "openagents.desktop.tokyo-night-projection.v1",
  "openagents.desktop.khala-editor-projection.v1",
]);

/** Compatibility export retained for the admitted Tokyo Night fallback. */
export const TokyoNightThemeSchemaVersion = DesktopThemeProjectionSchemaVersion;

export const TokyoNightPaletteSchema = Schema.Struct({
  ...colorFields([
    "background",
    "surface",
    "surfaceRaised",
    "surfaceOverlay",
    "foreground",
    "foregroundMuted",
    "foregroundFaint",
    "foregroundDisabled",
    "blue",
    "blueHover",
    "blueActive",
    "cyan",
    "green",
    "orange",
    "red",
    "magenta",
    "yellow",
    "border",
    "borderSubtle",
    "borderStrong",
    "selection",
    "hover",
    "active",
    "scrim",
    "diffAdded",
    "diffAddedBackground",
    "diffRemoved",
    "diffRemovedBackground",
    "diffModified",
  ] as const),
}).annotate({ identifier: "TokyoNightPalette" });
export type TokyoNightPalette = typeof TokyoNightPaletteSchema.Type;

export const TokyoNightMonacoTokenRuleSchema = Schema.Struct({
  token: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(100)),
  foreground: HexColorSchema,
  fontStyle: Schema.Literals(["", "italic", "bold", "italic bold"]),
}).annotate({ identifier: "TokyoNightMonacoTokenRule" });

export const TokyoNightMonacoProjectionSchema = Schema.Struct({
  base: Schema.Literal("vs-dark"),
  inherit: Schema.Literal(true),
  colors: Schema.Struct({
    editorBackground: HexColorSchema,
    editorForeground: HexColorSchema,
    cursor: HexColorSchema,
    lineNumber: HexColorSchema,
    activeLineNumber: HexColorSchema,
    lineHighlight: HexColorSchema,
    selection: HexColorSchema,
    inactiveSelection: HexColorSchema,
    findMatch: HexColorSchema,
    findMatchBorder: HexColorSchema,
    focusBorder: HexColorSchema,
    gutterAdded: HexColorSchema,
    gutterModified: HexColorSchema,
    gutterDeleted: HexColorSchema,
    error: HexColorSchema,
    warning: HexColorSchema,
    info: HexColorSchema,
  }),
  rules: Schema.Array(TokyoNightMonacoTokenRuleSchema),
}).annotate({ identifier: "TokyoNightMonacoProjection" });
export type TokyoNightMonacoProjection = typeof TokyoNightMonacoProjectionSchema.Type;

export const TokyoNightPierreProjectionSchema = Schema.Struct({
  themeName: Schema.Literals(["openagents-tokyo-night", "openagents-khala-editor"]),
  cssVariables: Schema.Struct({
    background: HexColorSchema,
    foreground: HexColorSchema,
    numberForeground: HexColorSchema,
    contextBackground: HexColorSchema,
    contextGutterBackground: HexColorSchema,
    additionForeground: HexColorSchema,
    additionBackground: HexColorSchema,
    deletionForeground: HexColorSchema,
    deletionBackground: HexColorSchema,
    selectionBackground: HexColorSchema,
    separatorBackground: HexColorSchema,
    annotationBackground: HexColorSchema,
    conflictMarkerForeground: HexColorSchema,
  }),
  allowUnsafeCss: Schema.Literal(false),
  allowRemoteTheme: Schema.Literal(false),
}).annotate({ identifier: "TokyoNightPierreProjection" });
export type TokyoNightPierreProjection = typeof TokyoNightPierreProjectionSchema.Type;

export const TokyoNightTerminalProjectionSchema = Schema.Struct({
  background: HexColorSchema,
  foreground: HexColorSchema,
  cursor: HexColorSchema,
  cursorAccent: HexColorSchema,
  selectionBackground: HexColorSchema,
  ansi: Schema.Tuple([
    HexColorSchema,
    HexColorSchema,
    HexColorSchema,
    HexColorSchema,
    HexColorSchema,
    HexColorSchema,
    HexColorSchema,
    HexColorSchema,
    HexColorSchema,
    HexColorSchema,
    HexColorSchema,
    HexColorSchema,
    HexColorSchema,
    HexColorSchema,
    HexColorSchema,
    HexColorSchema,
  ]),
}).annotate({ identifier: "TokyoNightTerminalProjection" });
export type TokyoNightTerminalProjection = typeof TokyoNightTerminalProjectionSchema.Type;

const semanticSurfaceSchema = Schema.Struct({
  background: HexColorSchema,
  foreground: HexColorSchema,
  border: HexColorSchema,
  accent: HexColorSchema,
  danger: HexColorSchema,
  warning: HexColorSchema,
  success: HexColorSchema,
  info: HexColorSchema,
});

export const DesktopThemeProjectionSchema = Schema.Struct({
  schemaVersion: DesktopThemeProjectionSchemaVersion,
  id: Schema.Literals(["tokyo-night", "khala-editor"]),
  kind: Schema.Literal("owned_static_data"),
  initializedBeforeEditorPaint: Schema.Literal(true),
  recreatesModelsOrSessions: Schema.Literal(false),
  provenance: Schema.Struct({
    upstream: Schema.Literals([
      "https://github.com/tokyo-night/tokyo-night-vscode-theme",
      "repository://@effect-native/tokens/khalaTheme",
    ]),
    sourceCommit: Schema.Literals([
      "7c0f11eaef322f293621ca7befe462214b7ea468",
      "467bde0760d052ee2f4a8fa678bb2f1f6bf200d8",
    ]),
    sourceFile: Schema.Literals([
      "themes/tokyo-night-color-theme.json",
      "apps/openagents.com/packages/effect-native-tokens/src/index.ts",
    ]),
    license: Schema.Literal("MIT"),
    palettePolicy: Schema.Literals([
      "data-only-curated-semantic-projection",
      "repository-owned-khala-semantic-projection",
    ]),
  }),
  accessibility: Schema.Struct({
    adjustedForegroundFaint: Schema.Literal(true),
    focusWidthPixels: Schema.Literal(2),
    focusOffsetPixels: Schema.Literal(2),
    focusStyle: Schema.Literal("solid-outline-plus-shape"),
    minimumNormalTextContrast: Schema.Literal(4.5),
    minimumLargeTextContrast: Schema.Literal(3),
    reliesOnColorAlone: Schema.Literal(false),
  }),
  palette: TokyoNightPaletteSchema,
  effectNative: ColorThemeSchema,
  monaco: TokyoNightMonacoProjectionSchema,
  pierre: TokyoNightPierreProjectionSchema,
  terminal: TokyoNightTerminalProjectionSchema,
  surfaces: Schema.Struct({
    problems: semanticSurfaceSchema,
    output: semanticSurfaceSchema,
    debug: semanticSurfaceSchema,
    review: semanticSurfaceSchema,
    proposal: semanticSurfaceSchema,
    browser: semanticSurfaceSchema,
    status: semanticSurfaceSchema,
  }),
}).annotate({ identifier: "DesktopThemeProjection" });
export type DesktopThemeProjection = typeof DesktopThemeProjectionSchema.Type;

/**
 * Curated from Enkia Tokyo Night at the pinned commit above. foregroundFaint
 * is lifted from upstream #787c99 to #8990ad so ordinary 12px metadata clears
 * WCAG AA on the editor background; this is the only palette adjustment.
 */
export const tokyoNightPalette = TokyoNightPaletteSchema.make({
  background: "#1a1b26",
  surface: "#16161e",
  surfaceRaised: "#202330",
  surfaceOverlay: "#24283b",
  foreground: "#c0caf5",
  foregroundMuted: "#a9b1d6",
  foregroundFaint: "#8990ad",
  foregroundDisabled: "#737aa2",
  blue: "#7aa2f7",
  blueHover: "#89b4fa",
  blueActive: "#6183bb",
  cyan: "#7dcfff",
  green: "#9ece6a",
  orange: "#ff9e64",
  red: "#f7768e",
  magenta: "#bb9af7",
  yellow: "#e0af68",
  border: "#292e42",
  borderSubtle: "#232433",
  borderStrong: "#3b4261",
  selection: "#515c7e4d",
  hover: "#c0caf514",
  active: "#c0caf521",
  scrim: "#0d0f17db",
  diffAdded: "#9ece6a",
  diffAddedBackground: "#41a6b520",
  diffRemoved: "#f7768e",
  diffRemovedBackground: "#db4b4b22",
  diffModified: "#7aa2f7",
});

const surfaceProjection = {
  background: tokyoNightPalette.surface,
  foreground: tokyoNightPalette.foreground,
  border: tokyoNightPalette.border,
  accent: tokyoNightPalette.blue,
  danger: tokyoNightPalette.red,
  warning: tokyoNightPalette.yellow,
  success: tokyoNightPalette.green,
  info: tokyoNightPalette.cyan,
} as const;

export const tokyoNightDesktopThemeProjection = DesktopThemeProjectionSchema.make({
  schemaVersion: "openagents.desktop.tokyo-night-projection.v1",
  id: "tokyo-night",
  kind: "owned_static_data",
  initializedBeforeEditorPaint: true,
  recreatesModelsOrSessions: false,
  provenance: {
    upstream: "https://github.com/tokyo-night/tokyo-night-vscode-theme",
    sourceCommit: "7c0f11eaef322f293621ca7befe462214b7ea468",
    sourceFile: "themes/tokyo-night-color-theme.json",
    license: "MIT",
    palettePolicy: "data-only-curated-semantic-projection",
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
  palette: tokyoNightPalette,
  effectNative: {
    background: tokyoNightPalette.background,
    surface: tokyoNightPalette.surface,
    surfaceRaised: tokyoNightPalette.surfaceRaised,
    surfaceOverlay: tokyoNightPalette.surfaceOverlay,
    textPrimary: tokyoNightPalette.foreground,
    textMuted: tokyoNightPalette.foregroundMuted,
    textFaint: tokyoNightPalette.foregroundFaint,
    textInverse: tokyoNightPalette.background,
    textDisabled: tokyoNightPalette.foregroundDisabled,
    accent: tokyoNightPalette.blue,
    accentHover: tokyoNightPalette.blueHover,
    accentActive: tokyoNightPalette.blueActive,
    danger: tokyoNightPalette.red,
    border: tokyoNightPalette.border,
    borderSubtle: tokyoNightPalette.borderSubtle,
    borderStrong: tokyoNightPalette.borderStrong,
    focus: tokyoNightPalette.blue,
    info: tokyoNightPalette.cyan,
    success: tokyoNightPalette.green,
    warning: tokyoNightPalette.yellow,
    stateHover: tokyoNightPalette.hover,
    stateActive: tokyoNightPalette.active,
    stateSelected: "#7aa2f729",
    scrim: tokyoNightPalette.scrim,
    codeBackground: tokyoNightPalette.background,
    diffAdd: tokyoNightPalette.diffAdded,
    diffRemove: tokyoNightPalette.diffRemoved,
    syntaxKeyword: tokyoNightPalette.magenta,
    syntaxString: tokyoNightPalette.green,
    syntaxComment: tokyoNightPalette.foregroundFaint,
    syntaxFunction: tokyoNightPalette.blue,
    syntaxNumber: tokyoNightPalette.orange,
    syntaxOperator: tokyoNightPalette.cyan,
  },
  monaco: {
    base: "vs-dark",
    inherit: true,
    colors: {
      editorBackground: tokyoNightPalette.background,
      editorForeground: tokyoNightPalette.foregroundMuted,
      cursor: tokyoNightPalette.foreground,
      lineNumber: tokyoNightPalette.borderStrong,
      activeLineNumber: tokyoNightPalette.foregroundFaint,
      lineHighlight: tokyoNightPalette.surfaceRaised,
      selection: tokyoNightPalette.selection,
      inactiveSelection: "#515c7e25",
      findMatch: "#3d59a166",
      findMatchBorder: tokyoNightPalette.yellow,
      focusBorder: tokyoNightPalette.blue,
      gutterAdded: "#164846",
      gutterModified: "#394b70",
      gutterDeleted: "#823c41",
      error: tokyoNightPalette.red,
      warning: tokyoNightPalette.yellow,
      info: tokyoNightPalette.cyan,
    },
    rules: [
      { token: "comment", foreground: tokyoNightPalette.foregroundFaint, fontStyle: "italic" },
      { token: "keyword", foreground: tokyoNightPalette.magenta, fontStyle: "" },
      { token: "string", foreground: tokyoNightPalette.green, fontStyle: "" },
      { token: "number", foreground: tokyoNightPalette.orange, fontStyle: "" },
      { token: "type", foreground: tokyoNightPalette.cyan, fontStyle: "" },
      { token: "function", foreground: tokyoNightPalette.blue, fontStyle: "" },
      { token: "operator", foreground: tokyoNightPalette.cyan, fontStyle: "" },
    ],
  },
  pierre: {
    themeName: "openagents-tokyo-night",
    cssVariables: {
      background: tokyoNightPalette.background,
      foreground: tokyoNightPalette.foreground,
      numberForeground: tokyoNightPalette.foregroundFaint,
      contextBackground: tokyoNightPalette.background,
      contextGutterBackground: tokyoNightPalette.surface,
      additionForeground: tokyoNightPalette.diffAdded,
      additionBackground: tokyoNightPalette.diffAddedBackground,
      deletionForeground: tokyoNightPalette.diffRemoved,
      deletionBackground: tokyoNightPalette.diffRemovedBackground,
      selectionBackground: tokyoNightPalette.selection,
      separatorBackground: tokyoNightPalette.surfaceRaised,
      annotationBackground: tokyoNightPalette.surfaceOverlay,
      conflictMarkerForeground: tokyoNightPalette.yellow,
    },
    allowUnsafeCss: false,
    allowRemoteTheme: false,
  },
  terminal: {
    background: tokyoNightPalette.background,
    foreground: tokyoNightPalette.foreground,
    cursor: tokyoNightPalette.foreground,
    cursorAccent: tokyoNightPalette.background,
    selectionBackground: tokyoNightPalette.selection,
    ansi: [
      "#414868",
      tokyoNightPalette.red,
      tokyoNightPalette.green,
      tokyoNightPalette.yellow,
      tokyoNightPalette.blue,
      tokyoNightPalette.magenta,
      tokyoNightPalette.cyan,
      tokyoNightPalette.foreground,
      "#565f89",
      tokyoNightPalette.red,
      tokyoNightPalette.green,
      tokyoNightPalette.yellow,
      tokyoNightPalette.blue,
      tokyoNightPalette.magenta,
      tokyoNightPalette.cyan,
      "#d5d6db",
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

/** Data-only Monaco projection; importing this file never imports Monaco. */
export const monacoThemeDataForProjection = (projection: DesktopThemeProjection) => ({
  base: projection.monaco.base,
  inherit: projection.monaco.inherit,
  rules: projection.monaco.rules.map((rule) => ({
    token: rule.token,
    foreground: rule.foreground.slice(1),
    fontStyle: rule.fontStyle,
  })),
  colors: {
    "editor.background": projection.monaco.colors.editorBackground,
    "editor.foreground": projection.monaco.colors.editorForeground,
    "editorCursor.foreground": projection.monaco.colors.cursor,
    "editorLineNumber.foreground": projection.monaco.colors.lineNumber,
    "editorLineNumber.activeForeground": projection.monaco.colors.activeLineNumber,
    "editor.lineHighlightBackground": projection.monaco.colors.lineHighlight,
    "editor.selectionBackground": projection.monaco.colors.selection,
    "editor.inactiveSelectionBackground": projection.monaco.colors.inactiveSelection,
    "editor.findMatchBackground": projection.monaco.colors.findMatch,
    "editor.findMatchBorder": projection.monaco.colors.findMatchBorder,
    focusBorder: projection.monaco.colors.focusBorder,
    "editorGutter.addedBackground": projection.monaco.colors.gutterAdded,
    "editorGutter.modifiedBackground": projection.monaco.colors.gutterModified,
    "editorGutter.deletedBackground": projection.monaco.colors.gutterDeleted,
    "editorError.foreground": projection.monaco.colors.error,
    "editorWarning.foreground": projection.monaco.colors.warning,
    "editorInfo.foreground": projection.monaco.colors.info,
  },
});

export const pierreCssVariablesForProjection = (projection: DesktopThemeProjection) => {
  const colors = projection.pierre.cssVariables;
  return {
    "--diffs-bg": colors.background,
    "--diffs-fg": colors.foreground,
    "--diffs-fg-number-override": colors.numberForeground,
    "--diffs-bg-context-override": colors.contextBackground,
    "--diffs-bg-context-gutter-override": colors.contextGutterBackground,
    "--diffs-addition-color-override": colors.additionForeground,
    "--diffs-bg-addition-override": colors.additionBackground,
    "--diffs-deletion-color-override": colors.deletionForeground,
    "--diffs-bg-deletion-override": colors.deletionBackground,
    "--diffs-bg-selection-override": colors.selectionBackground,
    "--diffs-bg-separator-override": colors.separatorBackground,
    "--diffs-annotation-bg": colors.annotationBackground,
    "--diffs-fg-conflict-marker-override": colors.conflictMarkerForeground,
  } as const;
};

export const tokyoNightMonacoThemeData = () =>
  monacoThemeDataForProjection(tokyoNightDesktopThemeProjection);

export const tokyoNightPierreCssVariables = () =>
  pierreCssVariablesForProjection(tokyoNightDesktopThemeProjection);

export const decodeDesktopThemeProjection = Schema.decodeUnknownSync(DesktopThemeProjectionSchema);
