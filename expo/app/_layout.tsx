import { Stack } from 'expo-router';
import { ThemeProvider } from '@react-navigation/native';
import { StatusBar } from 'expo-status-bar';
import { useTypographySetup, applyTypographyGlobals, Typography } from '@/constants/typography';
import { Colors, NavigationTheme } from '@/constants/theme';

export default function RootLayout() {
  const fontsLoaded = useTypographySetup();
  if (!fontsLoaded) return null;
  applyTypographyGlobals();

  return (
    <ThemeProvider value={NavigationTheme}>
      <StatusBar style="light" />
      <Stack screenOptions={{ contentStyle: { backgroundColor: Colors.background } }}>
        <Stack.Screen
          name="index"
          options={{
            title: 'Home',
            headerTitleStyle: { fontFamily: Typography.bold },
            headerBackVisible: false,
          }}
        />
      </Stack>
    </ThemeProvider>
  );
}
