import { useFonts } from "expo-font"
import { StatusBar } from "expo-status-bar"
import { LogBox, StyleSheet, View } from "react-native"
import { ConvexProvider, ConvexReactClient } from "convex/react"
import { GestureHandlerRootView } from "react-native-gesture-handler"
import { SafeAreaProvider } from "react-native-safe-area-context"
import { NavigationContainer } from "@react-navigation/native"
import { ClaudeCodeMobile } from "./components/ClaudeCodeMobile"
import { SimpleConfectAuthProvider, useConfectAuth } from "./contexts/SimpleConfectAuthContext"
import { SimpleOnboardingScreen } from "./components/onboarding/SimpleOnboardingScreen"
import { DARK_THEME } from "./constants/colors"
import { ConvexProviderWithAuth } from "./contexts/ConvexProviderWithAuth"

// Disable all development warnings
LogBox.ignoreAllLogs(true)

function MainApp() {
  const { 
    isAuthenticated, 
    isLoading, 
    needsOnboarding, 
    hasCompletedInitialSetup,
  } = useConfectAuth();

  // Show loading state while checking authentication
  if (isLoading) {
    return (
      <View style={styles.loadingContainer}>
        <View style={styles.loadingContent}>
          {/* Loading indicator will be handled by the hook */}
        </View>
      </View>
    );
  }

  // TEMPORARILY SKIP ONBOARDING - just show main app
  return (
    <ConvexProviderWithAuth>
      <NavigationContainer>
        <ClaudeCodeMobile />
      </NavigationContainer>
    </ConvexProviderWithAuth>
  );
}

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
        <SimpleConfectAuthProvider>
          <MainApp />
        </SimpleConfectAuthProvider>
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
  loadingContainer: {
    flex: 1,
    backgroundColor: DARK_THEME.background,
    justifyContent: 'center',
    alignItems: 'center',
  },
  loadingContent: {
    padding: 40,
  },
});
