import { StyleSheet, Text, View } from "react-native"

export default function SpawnScreen() {
  return (
    <View style={styles.container}>
      <Text style={styles.title}>Spawn</Text>
    </View>
  )
}

const styles = StyleSheet.create({
  container: {
    alignItems: "center",
    flex: 1,
    justifyContent: "center",
    padding: 24,
  },
  title: {
    fontSize: 24,
    fontWeight: "600",
  },
})
