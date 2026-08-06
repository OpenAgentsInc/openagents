import { khalaTheme, transparentColor } from "./index.ts";
import type { Theme } from "./index.ts";

/**
 * Renderer-neutral React Native projection. Applications may add ergonomic
 * aliases, but every rendered value must resolve through this object instead
 * of introducing an app-local color, spacing, radius, or motion authority.
 */
export const projectNativeTheme = (theme: Theme) => ({
  color: theme.color,
  colorMatrix: theme.colorMatrix,
  transparent: transparentColor,
  spacing: theme.spacing,
  radius: theme.radius,
  typeScale: theme.typeScale,
  control: theme.control,
  motion: {
    standard: theme.motion,
    reduced: {
      ...theme.motion,
      durationFastMs: 0,
      durationEnterMs: 0,
      durationExitMs: 0,
      durationLoopMs: 0,
    },
  },
});

export const khalaNativeTheme = projectNativeTheme(khalaTheme);
