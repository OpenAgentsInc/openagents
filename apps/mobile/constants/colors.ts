/**
 * Zinc color constants for the mobile app dark theme
 * Based on Tailwind CSS zinc color palette
 */
export const ZINC_COLORS = {
  50: '#fafafa',
  100: '#f4f4f5',
  200: '#e4e4e7',
  300: '#d4d4d8',
  400: '#a1a1aa',
  500: '#71717a',
  600: '#52525b',
  700: '#3f3f46',
  800: '#27272a',
  900: '#18181b',
  950: '#09090b',
} as const

/**
 * Dark theme color mapping using zinc palette
 */
export const DARK_THEME = {
  // Background colors
  background: '#000', // Pure black for main background
  backgroundSecondary: ZINC_COLORS[900], // #18181b
  backgroundTertiary: ZINC_COLORS[800], // #27272a
  
  // Text colors
  text: ZINC_COLORS[100], // #f4f4f5
  textSecondary: ZINC_COLORS[400], // #a1a1aa
  textTertiary: ZINC_COLORS[500], // #71717a
  
  // Border colors
  border: ZINC_COLORS[800], // #27272a
  borderSecondary: ZINC_COLORS[700], // #3f3f46
  
  // Overlay colors
  overlay: 'rgba(9, 9, 11, 0.8)', // zinc-950 with opacity
  
  // Status colors (keeping existing blue accent for buttons)
  primary: '#60a5fa',
  primaryDark: '#3b82f6',
  
  // Disabled states
  disabled: '#374151',
} as const

export type ZincColor = keyof typeof ZINC_COLORS
export type ThemeColor = keyof typeof DARK_THEME