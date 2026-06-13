import { StyleSheet, Text, View } from "react-native"

export default function DecisionsScreen() {
  return (
    <View style={styles.container}>
      <Text style={styles.title}>Decisions</Text>
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
