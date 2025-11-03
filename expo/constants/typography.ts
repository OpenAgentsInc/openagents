import { useEffect } from 'react';
import { Text, TextInput, Platform } from 'react-native';
import { Asset } from 'expo-asset';
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
    // Inject @font-face rules for Berkeley Mono using Expo asset URLs.
    // Avoid expo-font on web (it can time out in WebView); rely on CSS + swap.
    try {
      const reg = Asset.fromModule(require('../assets/fonts/BerkeleyMono-Regular.ttf')).uri;
      const bold = Asset.fromModule(require('../assets/fonts/BerkeleyMono-Bold.ttf')).uri;
      const italic = Asset.fromModule(require('../assets/fonts/BerkeleyMono-Italic.ttf')).uri;
      const boldItalic = Asset.fromModule(require('../assets/fonts/BerkeleyMono-BoldItalic.ttf')).uri;
      const css = `
@font-face { font-family: BerkeleyMono; src: url('${reg}') format('truetype'); font-display: swap; }
@font-face { font-family: 'BerkeleyMono-Bold'; src: url('${bold}') format('truetype'); font-display: swap; }
@font-face { font-family: 'BerkeleyMono-Italic'; src: url('${italic}') format('truetype'); font-display: swap; }
@font-face { font-family: 'BerkeleyMono-BoldItalic'; src: url('${boldItalic}') format('truetype'); font-display: swap; }
`;
      const id = '__oa-web-fonts';
      if (typeof document !== 'undefined' && !document.getElementById(id)) {
        const el = document.createElement('style');
        el.id = id;
        el.type = 'text/css';
        el.appendChild(document.createTextNode(css));
        document.head.appendChild(el);
      }
    } catch {}
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
