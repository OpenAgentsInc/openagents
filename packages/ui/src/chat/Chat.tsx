import { View, TextInput, StyleSheet, SafeAreaView, KeyboardAvoidingView, Platform } from 'react-native'
import { MessageList } from './MessageList'
import { dummyMessages } from './dummyData'

export const Chat = () => (
  <SafeAreaView style={styles.container}>
    <KeyboardAvoidingView
      behavior={Platform.OS === 'ios' ? 'padding' : undefined}
      style={styles.keyboardAvoidingView}
    >
      <View style={styles.inner}>
        <View style={styles.topBorder} />
        <MessageList messages={dummyMessages} />
        <View style={styles.inputWrapper}>
          <View style={styles.inputBorder} />
          <View style={styles.inputContainer}>
            <TextInput
              autoFocus
              style={styles.input}
              placeholder="Message Coder"
              placeholderTextColor="#666"
            />
          </View>
        </View>
      </View>
    </KeyboardAvoidingView>
  </SafeAreaView>
)

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: '#000',
  },
  keyboardAvoidingView: {
    flex: 1,
  },
  inner: {
    flex: 1,
    position: 'relative',
  },
  topBorder: {
    height: 1,
    backgroundColor: '#333',
    width: '100%',
  },
  inputWrapper: {
    position: 'absolute',
    bottom: 0,
    left: 0,
    right: 0,
    backgroundColor: '#000',
  },
  inputBorder: {
    height: 1,
    backgroundColor: '#333',
    width: '100%',
  },
  inputContainer: {
    paddingVertical: 20,
    paddingHorizontal: '5%',
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
