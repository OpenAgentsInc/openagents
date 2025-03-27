import { View, TextInput, StyleSheet } from 'react-native'
import { MessageList } from './MessageList'
import { dummyMessages } from './dummyData'

export const Chat = () => (
  <View style={styles.container}>
    <MessageList messages={dummyMessages} />
    <View style={styles.inputContainer}>
      <TextInput
        autoFocus
        style={styles.input}
        placeholder="Message Coder"
        placeholderTextColor="#666"
      />
    </View>
  </View>
)

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: '#000',
  },
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
    fontSize: 14,
    fontFamily: "Berkeley Mono"
  },
})
