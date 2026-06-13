import { StyleSheet, Text, View } from "react-native"

export default function SessionDetailScreen() {
  return (
    <View style={styles.container}>
      <Text style={styles.title}>Session Detail</Text>
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
