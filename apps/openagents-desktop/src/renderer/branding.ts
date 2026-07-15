/**
 * Launch-stage branding, adapted from T3 Code's renderer fallback.
 * Vite owns this distinction: the dev server sets DEV, while packaged
 * renderer assets are production builds and therefore present as Alpha.
 */
export const resolveDesktopStageLabel = (isDevelopment: boolean): "Dev" | "Alpha" =>
  isDevelopment ? "Dev" : "Alpha"

export const DESKTOP_STAGE_LABEL = resolveDesktopStageLabel(import.meta.env.DEV)
