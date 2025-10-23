import { DarkTheme as NavigationDarkTheme, type Theme } from '@react-navigation/native';

export const Colors = {
  // Core palette
  background: '#000000',
  card: '#0A0A0A',
  border: '#222222',
  textPrimary: '#FFFFFF',
  textSecondary: '#B3B3B3',
  tint: '#FFFFFF', // active accents (no blue)

  // Text variants
  textMuted: '#D4D4D8',
  textOnBright: '#000000',

  // Status & feedback
  statusSuccess: '#22C55E',
  statusError: '#EF4444',
  statusWarn: '#F59E0B',
  statusFailText: '#FCA5A5',
  statusSuccessText: '#86EFAC',

  // Surfaces
  surfaceAlt: '#12141C',
  surfaceMuted: '#0F1217',
  codeBg: '#0F1217',
  buttonBg: '#3F3F46',
  buttonActiveBorder: '#4B5563',
  muted: '#A3A3A3',

  // Overlays / tints
  overlayHigh: 'rgba(255,255,255,0.06)',
  overlayLow: 'rgba(255,255,255,0.02)',

  // Specials
  transparent: 'transparent',

  // Components
  tabBarBackground: '#000000',
  tabBarActive: '#FFFFFF',
  tabBarInactive: '#808080',
} as const;

export const NavigationTheme: Theme = {
  ...NavigationDarkTheme,
  dark: true,
  colors: {
    ...NavigationDarkTheme.colors,
    primary: Colors.tint,
    background: Colors.background,
    card: Colors.card,
    text: Colors.textPrimary,
    border: Colors.border,
    notification: Colors.textSecondary,
  },
};
