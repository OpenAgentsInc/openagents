import React from "react"
import { useFonts } from "expo-font"
import { StatusBar } from "expo-status-bar"
import { LogBox, StyleSheet, View } from "react-native"
import { ConvexProvider, ConvexReactClient } from "convex/react"
import { GestureHandlerRootView } from "react-native-gesture-handler"
import { SafeAreaProvider } from "react-native-safe-area-context"
import { NavigationContainer } from "@react-navigation/native"
import { ClaudeCodeMobile } from "./components/ClaudeCodeMobile"
import { SimpleConfectAuthProvider, useConfectAuth } from "./contexts/SimpleConfectAuthContext"
import { useUserSync } from "./hooks/useUserSync"
import { OnboardingScreen } from "./components/onboarding/OnboardingScreen"
import { DARK_THEME } from "./constants/colors"
import { ConvexProviderWithAuth } from "./contexts/ConvexProviderWithAuth"
import { ErrorBoundary } from "./components/ErrorBoundary"

// Disable all development warnings
LogBox.ignoreAllLogs(true)

// Component that handles user sync (must be inside ConvexProvider)
function UserSyncWrapper({ children }: { children: React.ReactNode }) {
  const { isAuthenticated } = useConfectAuth();
  
  // Sync user data to Convex when authenticated
  const { isSynced } = useUserSync();
  console.log('📱 [USER_SYNC_WRAPPER] User sync status:', { isAuthenticated, isSynced });

  // Show loading state while syncing user
  if (isAuthenticated && !isSynced) {
    return (
      <View style={styles.loadingContainer}>
        <View style={styles.loadingContent}>
          {/* Loading indicator while syncing user data */}
        </View>
      </View>
    );
  }

  return <>{children}</>;
}

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

  // Show onboarding if needed, otherwise show main app
  return (
    <ErrorBoundary
      onError={(error, errorInfo) => {
        console.error('🚨 [APP_LEVEL] Critical app error:', {
          error: error.message,
          stack: error.stack,
          componentStack: errorInfo.componentStack,
          timestamp: new Date().toISOString(),
        });
      }}
    >
      <ConvexProviderWithAuth>
        <UserSyncWrapper>
          {needsOnboarding ? (
            <ErrorBoundary
              onError={(error, errorInfo) => {
                console.error('🚨 [ONBOARDING] Onboarding error:', {
                  error: error.message,
                  componentStack: errorInfo.componentStack,
                });
              }}
            >
              <OnboardingScreen 
                onComplete={() => {
                  // Handle onboarding completion
                  console.log('📱 [APP] Onboarding completed');
                }} 
              />
            </ErrorBoundary>
          ) : (
            <ErrorBoundary
              onError={(error, errorInfo) => {
                console.error('🚨 [MAIN_APP] Main app error:', {
                  error: error.message,
                  componentStack: errorInfo.componentStack,
                });
              }}
            >
              <NavigationContainer>
                <ClaudeCodeMobile />
              </NavigationContainer>
            </ErrorBoundary>
          )}
        </UserSyncWrapper>
      </ConvexProviderWithAuth>
    </ErrorBoundary>
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
    <ErrorBoundary
      onError={(error, errorInfo) => {
        console.error('🚨 [ROOT_LEVEL] Root app error:', {
          error: error.message,
          stack: error.stack,
          componentStack: errorInfo.componentStack,
          timestamp: new Date().toISOString(),
        });
      }}
    >
      <SafeAreaProvider>
        <GestureHandlerRootView style={styles.container}>
          <ErrorBoundary
            onError={(error, errorInfo) => {
              console.error('🚨 [AUTH_PROVIDER] Auth provider error:', {
                error: error.message,
                componentStack: errorInfo.componentStack,
              });
            }}
          >
            <SimpleConfectAuthProvider>
              <MainApp />
            </SimpleConfectAuthProvider>
          </ErrorBoundary>
          <StatusBar style="light" />
        </GestureHandlerRootView>
      </SafeAreaProvider>
    </ErrorBoundary>
  );
}

export default function App() {
  return (
    <ErrorBoundary
      onError={(error, errorInfo) => {
        console.error('🚨 [TOP_LEVEL] Top-level app error:', {
          error: error.message,
          stack: error.stack,
          componentStack: errorInfo.componentStack,
          timestamp: new Date().toISOString(),
        });
      }}
    >
      <AppContent />
    </ErrorBoundary>
  );
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
