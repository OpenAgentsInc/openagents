import { useEffect } from 'react';
import { Text, TextInput } from 'react-native';
import { useFonts } from 'expo-font';
import * as SplashScreen from 'expo-splash-screen';

export const Typography = {
  primary: 'BerkeleyMono',
  // You can add semantic roles here later (e.g., title, label),
  // but default is to use `primary` for all text.
};

// Load fonts and set global defaults so all <Text/> uses the primary font.
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
    // Set default font on core text components so styles don't have to repeat it.
    if ((Text as any).defaultProps == null) (Text as any).defaultProps = {};
    if ((TextInput as any).defaultProps == null) (TextInput as any).defaultProps = {};

    const baseTextStyle = { fontFamily: Typography.primary } as const;
    (Text as any).defaultProps.style = [
      (Text as any).defaultProps.style,
      baseTextStyle,
    ];
    (TextInput as any).defaultProps.style = [
      (TextInput as any).defaultProps.style,
      baseTextStyle,
    ];

    // Hide splash once applied
    SplashScreen.hideAsync().catch(() => {});
  }, [fontsLoaded]);

  return fontsLoaded;
}

