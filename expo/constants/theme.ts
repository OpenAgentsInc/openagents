import {
  DarkTheme as NavigationDarkTheme, Theme, type
} from "@react-navigation/native"

// Adapted from the provided CSS variables to React Native theme tokens.
export const Colors = {
  // Palette Colors - Rainbow Order
  // Reds
  flRed: '#ff0000',
  activeRed: '#ff4136',
  brightRed: '#e7040f',
  maroon: '#660000',

  // Oranges
  flOrange: '#ff6600',
  orange: '#e25600',

  // Yellows
  flYellow: '#ffff00',
  activeYellow: '#ffd700',
  gold: '#ffb700',
  yellow: '#FEBF00',
  olive: '#666600',

  // Greens
  flGreen: '#00ff00',
  green: '#04A545',

  // Cyans
  flCyan: '#00ffff',
  cyan: '#006566',

  // Blues
  flBlue: '#0000ff',
  blue: '#000066',

  // Purples & Magentas
  purple: '#7643b9',
  activePurple: '#7643b9',
  magenta: '#660065',
  flMagenta: '#ff00ff',
  activeMagenta: '#cc2197',
  hotPink: '#ff41b4',

  // Neutrals
  black: '#000000',
  gray: '#999999',
  white: '#ffffff',

  // Core Design System Colors
  background: '#08090a',
  border: '#23252a',
  primary: '#f7f8f8',
  secondary: '#d0d6e0',
  tertiary: '#8a8f98',
  quaternary: '#62666d',

  // System Colors - Using Design System
  foreground: '#f7f8f8', // var(--primary)
  card: '#08090a', // var(--background)
  cardForeground: '#f7f8f8',
  popover: '#08090a',
  popoverForeground: '#f7f8f8',
  primaryForeground: '#08090a',
  secondaryForeground: '#08090a',
  muted: '#08090a',
  mutedForeground: '#d0d6e0',
  accent: '#08090a',
  accentForeground: '#f7f8f8',
  destructive: '#e7040f', // bright red
  destructiveForeground: '#f7f8f8',
  input: '#08090a',
  ring: '#f7f8f8',
  radius: 0,

  // Status/State colors from palette
  success: '#04A545',
  successForeground: '#f7f8f8',
  warning: '#FEBF00',
  warningForeground: '#08090a',
  danger: '#e7040f',
  dangerForeground: '#f7f8f8',

  // Components
  tabBarBackground: '#08090a',
  tabBarActive: '#f7f8f8',
  tabBarInactive: '#8a8f98',
  sidebarBackground: '#23252a',

  // Specials
  transparent: 'transparent',
} as const;

export const NavigationTheme: Theme = {
  ...NavigationDarkTheme,
  dark: true,
  colors: {
    ...NavigationDarkTheme.colors,
    primary: Colors.foreground,
    background: Colors.background,
    card: Colors.card,
    text: Colors.foreground,
    border: Colors.border,
    notification: Colors.secondary,
  },
};
