import { DarkTheme as NavigationDarkTheme, type Theme } from '@react-navigation/native';

export const Colors = {
  // Core palette
  background: '#000000',
  card: '#0A0A0A',
  border: '#222222',
  textPrimary: '#FFFFFF',
  textSecondary: '#B3B3B3',
  tint: '#FFFFFF', // active accents (no blue)

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
