import { Stack } from 'expo-router';
import { ThemeProvider } from '@react-navigation/native';
import { StatusBar } from 'expo-status-bar';
import { useTypographySetup, applyTypographyGlobals, Typography } from '@/constants/typography';
import { Colors, NavigationTheme } from '@/constants/theme';
import { useAutoUpdate } from '@/app/hooks/useAutoUpdate';

export default function RootLayout() {
  const fontsLoaded = useTypographySetup();
  useAutoUpdate();
  if (!fontsLoaded) return null;
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
