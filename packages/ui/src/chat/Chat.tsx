import { View, TextInput, StyleSheet } from 'react-native'

export const Chat = () => (
  <View style={{ flex: 1, backgroundColor: "#0a0a0a" }}>
    {/* text input */}
    <View style={styles.inputContainer}>
      <TextInput
        style={styles.input}
        placeholder="Type a message..."
        placeholderTextColor="#666"
      />
    </View>
  </View>
)

const styles = StyleSheet.create({
  inputContainer: {
    position: 'absolute',
    bottom: 20,
    left: '5%',
    right: '5%',
    width: '90%',
  },
  input: {
    backgroundColor: 'transparent',
    borderWidth: 1,
    borderColor: '#fff',
    borderRadius: 8,
    padding: 12,
    color: '#fff',
    fontSize: 16,
  },
})
