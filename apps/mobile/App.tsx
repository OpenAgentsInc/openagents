import { useFonts } from "expo-font"
import { StatusBar } from "expo-status-bar"
import { LogBox, StyleSheet, View } from "react-native"
import { ConvexProvider, ConvexReactClient } from "convex/react"
import { GestureHandlerRootView } from "react-native-gesture-handler"
import { SafeAreaProvider } from "react-native-safe-area-context"
import { NavigationContainer } from "@react-navigation/native"
import { ClaudeCodeMobile } from "./components/ClaudeCodeMobile"
import { AuthProvider } from "./contexts/AuthContext"
import { DARK_THEME } from "./constants/colors"
import { ConvexProviderWithAuth } from "./contexts/ConvexProviderWithAuth"

// Disable all development warnings
LogBox.ignoreAllLogs(true)

function AppContent() {
  const [fontsLoaded] = useFonts({
    'Berkeley Mono': require('./assets/fonts/BerkeleyMono-Regular.ttf'),
  })

  if (!fontsLoaded) {
    return null
  }

  return (
    <SafeAreaProvider>
      <GestureHandlerRootView style={styles.container}>
        <AuthProvider>
          <ConvexProviderWithAuth>
            <NavigationContainer>
              <ClaudeCodeMobile />
            </NavigationContainer>
          </ConvexProviderWithAuth>
        </AuthProvider>
        <StatusBar style="light" />
      </GestureHandlerRootView>
    </SafeAreaProvider>
  );
}

export default function App() {
  return <AppContent />;
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: DARK_THEME.background,
  },
});
