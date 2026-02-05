/**
 * Color presets for DotsGridBackground and full-page gradient (arwes-style).
 * Each preset has a base background, radial gradient, and dots/grid line colors.
 */

export interface DotsGridColorPreset {
  /** Base background color (e.g. dark fill) */
  backgroundColor: string;
  /** CSS background-image for radial gradient overlay */
  backgroundImage: string;
  /** Color for dots (e.g. hsla with low alpha) */
  dotsColor: string;
  /** Color for grid lines */
  lineColor: string;
  /** Distance between dots/grid lines in px (same as arwes demo app uses for Dots: 40) */
  distance: number;
}

/** Green/teal preset (arwes default) */
export const greenPreset: DotsGridColorPreset = Object.freeze({
  backgroundColor: '#000906',
  backgroundImage:
    'radial-gradient(85% 85% at 50% 50%, hsla(185, 100%, 25%, 0.25) 0%, hsla(185, 100%, 25%, 0.12) 50%, hsla(185, 100%, 25%, 0) 100%)',
  dotsColor: 'hsla(180, 100%, 75%, 0.05)',
  lineColor: 'hsla(180, 100%, 75%, 0.05)',
  distance: 30,
});

/** Purple preset (dark, a bit lighter than black). Dots/grid distance 40 matches arwes.dev demo. */
export const purplePreset: DotsGridColorPreset = Object.freeze({
  backgroundColor: '#070510',
  backgroundImage:
    'radial-gradient(85% 85% at 50% 50%, hsla(280, 100%, 20%, 0.18) 0%, hsla(280, 100%, 14%, 0.08) 50%, transparent 100%)',
  dotsColor: 'hsla(280, 100%, 75%, 0.04)',
  lineColor: 'hsla(280, 100%, 75%, 0.04)',
  distance: 40,
});

export const colorPresets = Object.freeze({
  green: greenPreset,
  purple: purplePreset,
} as const);

export type ColorPresetName = keyof typeof colorPresets;
