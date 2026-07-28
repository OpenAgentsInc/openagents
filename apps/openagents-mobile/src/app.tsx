import { SafeAreaProvider } from "react-native-safe-area-context";

import { OmegaHomeScreen } from "./screens/omega-home-screen";

/**
 * The OpenAgents mobile app.
 *
 * Owner direction 2026-07-27: one surface, the desktop mirror, built from the
 * arcade component patterns in plain React Native. Effect Native is gone from
 * this app. The only Effect left is inside the device-bridge client, which is
 * transport code rather than a user-interface framework.
 */
export const App = () => (
  <SafeAreaProvider>
    <OmegaHomeScreen />
  </SafeAreaProvider>
);
