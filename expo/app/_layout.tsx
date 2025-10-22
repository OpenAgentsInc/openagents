import { Stack } from 'expo-router';
import { ThemeProvider } from '@react-navigation/native';
import { StatusBar } from 'expo-status-bar';
import { useTypographySetup, applyTypographyGlobals } from '@/constants/typography';
import { Colors, NavigationTheme } from '@/constants/theme';

export const unstable_settings = {
  anchor: '(tabs)',
};

export default function RootLayout() {
  // Load fonts and apply global typography
  const fontsLoaded = useTypographySetup();

  if (!fontsLoaded) {
    return null;
  }

  // Ensure default fonts are applied before rendering any screens
  applyTypographyGlobals();

  return (
    <ThemeProvider value={NavigationTheme}>
      <StatusBar style="light" />
      <Stack screenOptions={{ contentStyle: { backgroundColor: Colors.background } }}>
        <Stack.Screen name="(tabs)" options={{ headerShown: false }} />
      </Stack>
    </ThemeProvider>
  );
}
