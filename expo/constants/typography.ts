import { useEffect } from 'react';
import { Text, TextInput, Platform } from 'react-native';
import { useFonts } from 'expo-font';
import * as SplashScreen from 'expo-splash-screen';
import { Colors } from '@/constants/theme';
import { Typography as SharedTypography } from '@openagentsinc/theme';

export const Typography = SharedTypography;

// Load fonts and set global defaults so all <Text/> uses the primary font.
let appliedTypographyGlobals = false;

export function applyTypographyGlobals() {
  if (appliedTypographyGlobals) return;
  const TextAny = Text as unknown as { defaultProps?: { style?: unknown } };
  const TextInputAny = TextInput as unknown as { defaultProps?: { style?: unknown } };
  if (TextAny.defaultProps == null) TextAny.defaultProps = {};
  if (TextInputAny.defaultProps == null) TextInputAny.defaultProps = {};
  const baseTextStyle = { fontFamily: Typography.primary, color: Colors.foreground } as const;
  TextAny.defaultProps!.style = [
    TextAny.defaultProps!.style as any,
    baseTextStyle,
  ];
  TextInputAny.defaultProps!.style = [
    TextInputAny.defaultProps!.style as any,
    baseTextStyle,
  ];
  appliedTypographyGlobals = true;
}

export function useTypographySetup() {
  // On web (Expo web/Metro), skip expo-font to avoid fontfaceobserver 6s timeout
  if (Platform.OS === 'web') {
    // No splash management on web; return true to continue rendering
    return true as const;
  }

  const [fontsLoaded] = useFonts({
    BerkeleyMono: require('../assets/fonts/BerkeleyMono-Regular.ttf'),
    'BerkeleyMono-Bold': require('../assets/fonts/BerkeleyMono-Bold.ttf'),
    'BerkeleyMono-Italic': require('../assets/fonts/BerkeleyMono-Italic.ttf'),
    'BerkeleyMono-BoldItalic': require('../assets/fonts/BerkeleyMono-BoldItalic.ttf'),
  });

  useEffect(() => {
    SplashScreen.preventAutoHideAsync().catch(() => {});
  }, []);

  useEffect(() => {
    if (!fontsLoaded) return;
    SplashScreen.hideAsync().catch(() => {});
  }, [fontsLoaded]);

  return fontsLoaded;
}
