import { useFonts } from "expo-font";
import { ShareIntentProvider } from "expo-share-intent";
import { View } from "react-native";
import { SafeAreaProvider } from "react-native-safe-area-context";

import { AmbientProvider } from "./ambient/ambient-provider";
import { ControllerRoot } from "./controller/controller-root";
import { ControllerSessionProvider } from "./controller/session-provider";
import { MobileClientOutboxProvider } from "./outbox/client-outbox-provider";
import { colors, fontAssets } from "./ui/theme";

/**
 * The OpenAgents mobile app.
 *
 * The app is a bounded native controller for the Pro work projection. It reads
 * reactive state directly from Convex and sends every durable command through
 * the authenticated Pro capability broker and the device outbox.
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
    <ShareIntentProvider options={{ scheme: "openagents", resetOnBackground: false }}>
      <MobileClientOutboxProvider>
        <SafeAreaProvider>
          <ControllerSessionProvider>
            <AmbientProvider>
              <ControllerRoot />
            </AmbientProvider>
          </ControllerSessionProvider>
        </SafeAreaProvider>
      </MobileClientOutboxProvider>
    </ShareIntentProvider>
  );
};
