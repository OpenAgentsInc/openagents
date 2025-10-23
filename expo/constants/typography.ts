import { useEffect } from 'react';
import { Text, TextInput } from 'react-native';
import { useFonts } from 'expo-font';
import * as SplashScreen from 'expo-splash-screen';
import { Colors } from '@/constants/theme';

export const Typography = {
  primary: 'BerkeleyMono',
  bold: 'BerkeleyMono-Bold',
  italic: 'BerkeleyMono-Italic',
  boldItalic: 'BerkeleyMono-BoldItalic',
  // You can add semantic roles here later (e.g., title, label),
  // but default is to use `primary` for all text.
};

// Load fonts and set global defaults so all <Text/> uses the primary font.
let appliedTypographyGlobals = false;

export function applyTypographyGlobals() {
  if (appliedTypographyGlobals) return;
  if ((Text as any).defaultProps == null) (Text as any).defaultProps = {};
  if ((TextInput as any).defaultProps == null) (TextInput as any).defaultProps = {};
  const baseTextStyle = { fontFamily: Typography.primary, color: Colors.foreground } as const;
  (Text as any).defaultProps.style = [
    (Text as any).defaultProps.style,
    baseTextStyle,
  ];
  (TextInput as any).defaultProps.style = [
    (TextInput as any).defaultProps.style,
    baseTextStyle,
  ];
  appliedTypographyGlobals = true;
}

export function useTypographySetup() {
  const [fontsLoaded] = useFonts({
    // Family names used in `fontFamily` must match these keys
    BerkeleyMono: require('../assets/fonts/BerkeleyMono-Regular.ttf'),
    'BerkeleyMono-Bold': require('../assets/fonts/BerkeleyMono-Bold.ttf'),
    'BerkeleyMono-Italic': require('../assets/fonts/BerkeleyMono-Italic.ttf'),
    'BerkeleyMono-BoldItalic': require('../assets/fonts/BerkeleyMono-BoldItalic.ttf'),
  });

  useEffect(() => {
    // Keep splash screen up until fonts are ready
    SplashScreen.preventAutoHideAsync().catch(() => {});
  }, []);

  useEffect(() => {
    if (!fontsLoaded) return;
    // Hide splash once fonts are ready
    SplashScreen.hideAsync().catch(() => {});
  }, [fontsLoaded]);

  return fontsLoaded;
}
