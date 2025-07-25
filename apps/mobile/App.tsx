import { useFonts } from "expo-font"
import { StatusBar } from "expo-status-bar"
import { LogBox, Platform, SafeAreaView, StyleSheet, Text, View } from "react-native"
import { ConvexProvider, ConvexReactClient } from "convex/react"
import { ConvexMobileDemo } from "./components/ConvexMobileDemo"

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
    <SafeAreaView style={styles.container}>
      <Text style={styles.text}>OpenAgents</Text>
      <View style={styles.demoContainer}>
        <ConvexMobileDemo />
      </View>
      <StatusBar style="light" />
    </SafeAreaView>
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
    backgroundColor: '#000',
    alignItems: 'center',
    justifyContent: 'center',
    padding: 20,
  },
  text: {
    color: '#fff',
    fontSize: 22,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
    marginBottom: 20,
  },
  demoContainer: {
    flex: 1,
    width: '100%',
    maxWidth: 400,
  }
});
