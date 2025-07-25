import { StatusBar } from "expo-status-bar"
import { StyleSheet, Text, View, LogBox } from "react-native"
import { useFonts } from 'expo-font'

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
    <View style={styles.container}>
      <Text style={styles.text}>OpenAgents</Text>
      <StatusBar style="light" />
    </View>
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
    fontFamily: 'Berkeley Mono'
  }
});
