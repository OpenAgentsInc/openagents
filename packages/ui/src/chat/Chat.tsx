import { View, StyleSheet, SafeAreaView, KeyboardAvoidingView, Platform } from 'react-native'
import { MessageList } from './MessageList'
import { MessageInput } from './MessageInput'
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
        <MessageInput maxRows={5} />
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
})
