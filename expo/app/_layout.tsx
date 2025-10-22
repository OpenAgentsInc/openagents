import { Stack } from 'expo-router';
import { useTypographySetup } from '@/constants/typography';

export const unstable_settings = {
  anchor: '(tabs)',
};

export default function RootLayout() {
  // Load fonts and apply global typography
  const fontsLoaded = useTypographySetup();

  if (!fontsLoaded) {
    return null;
  }

  return (
    <Stack>
      <Stack.Screen name="(tabs)" options={{ headerShown: false }} />
    </Stack>
  );
}
