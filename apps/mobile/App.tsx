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

// Disable all development warnings
LogBox.ignoreAllLogs(true)

const convexUrl = process.env.EXPO_PUBLIC_CONVEX_URL;
const convex = new ConvexReactClient(convexUrl!, {
  // Disable for React Native compatibility
  unsavedChangesWarning: false,
});

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
          <NavigationContainer>
            <ClaudeCodeMobile />
          </NavigationContainer>
        </AuthProvider>
        <StatusBar style="light" />
      </GestureHandlerRootView>
    </SafeAreaProvider>
  );
}

export default function App() {
  return (
    <ConvexProvider client={convex}>
      <AppContent />
    </ConvexProvider>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: DARK_THEME.background,
  },
});
