import { useFonts } from "expo-font";
import { View } from "react-native";
import { SafeAreaProvider } from "react-native-safe-area-context";

import { OmegaHomeScreen } from "./screens/omega-home-screen";
import { colors, fontAssets } from "./ui/theme";

/**
 * The OpenAgents mobile app.
 *
 * Owner direction 2026-07-27: one surface, the desktop mirror, built from the
 * arcade component patterns in plain React Native. Effect Native is gone from
 * this app. The only Effect left is inside the device-bridge client, which is
 * transport code rather than a user-interface framework.
 *
 * The desktop's own faces load before the first frame. Rendering through a
 * pending load would show the system font and then reflow the whole transcript
 * once the real face arrives, so the app holds one empty screen in the page
 * colour instead. A failed load falls through to the system font rather than
 * holding the app hostage to a font.
 */
export const App = () => {
  const [loaded, error] = useFonts(fontAssets);
  if (!loaded && error === null) {
    return <View style={{ flex: 1, backgroundColor: colors.background }} />;
  }
  return (
    <SafeAreaProvider>
      <OmegaHomeScreen />
    </SafeAreaProvider>
  );
};
