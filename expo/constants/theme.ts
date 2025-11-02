import { DarkTheme as NavigationDarkTheme } from "@react-navigation/native"
import type { Theme } from "@react-navigation/native"
import { Colors as SharedColors } from "@openagents/theme/colors";

export const Colors = SharedColors;

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
