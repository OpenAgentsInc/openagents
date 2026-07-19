import { khalaEditorDesktopThemeProjection } from "./khala-editor-theme.ts";
import {
  tokyoNightDesktopThemeProjection,
  type DesktopThemeProjection,
} from "./tokyo-night-theme.ts";

export const defaultDesktopEditorThemeId = "khala-editor" as const;
export const fallbackDesktopEditorThemeId = "tokyo-night" as const;

export const desktopEditorThemeRegistry = {
  [defaultDesktopEditorThemeId]: khalaEditorDesktopThemeProjection,
  [fallbackDesktopEditorThemeId]: tokyoNightDesktopThemeProjection,
} as const satisfies Readonly<Record<string, DesktopThemeProjection>>;

export type DesktopEditorThemeId = keyof typeof desktopEditorThemeRegistry;

export const defaultDesktopEditorThemeProjection =
  desktopEditorThemeRegistry[defaultDesktopEditorThemeId];

export const fallbackDesktopEditorThemeProjection =
  desktopEditorThemeRegistry[fallbackDesktopEditorThemeId];
