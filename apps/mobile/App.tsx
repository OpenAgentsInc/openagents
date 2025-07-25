import { useFonts } from "expo-font"
import { StatusBar } from "expo-status-bar"
import { LogBox, Platform, SafeAreaView, StyleSheet, Text } from "react-native"

// Disable all development warnings
LogBox.ignoreAllLogs(true)

export default function App() {
  const [fontsLoaded] = useFonts({
    'Berkeley Mono': require('./assets/fonts/BerkeleyMono-Regular.ttf'),
  })

  if (!fontsLoaded) {
    return null
  }

  return (
    <SafeAreaView style={styles.container}>
      <Text style={styles.text}>OpenAgents</Text>
      <StatusBar style="light" />
    </SafeAreaView>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: '#000',
    alignItems: 'center',
    justifyContent: 'center',
  },
  text: {
    color: '#fff',
    fontSize: 22,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    })
  }
});
