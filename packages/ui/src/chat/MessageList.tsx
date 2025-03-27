import { View, Text, StyleSheet } from 'react-native'
import { UIMessage } from './types'
import { ToolCall } from './ToolCall'

interface MessageListProps {
  messages: UIMessage[]
}

export const MessageList = ({ messages }: MessageListProps) => {
  const visibleMessages = messages.filter(message => message.role !== 'system')

  return (
    <View style={styles.container}>
      {visibleMessages.map((message) => (
        <View
          key={message.id}
          style={[
            styles.messageContainer,
            message.role === 'user' && styles.userMessageContainer,
          ]}
        >
          {message.parts.map((part, index) => {
            if (part.type === 'text') {
              return (
                <Text
                  key={`${message.id}-${index}`}
                  style={[
                    styles.messageText,
                    message.role === 'user' && styles.userMessageText,
                  ]}
                >
                  {part.text}
                </Text>
              )
            }
            if (part.type === 'tool-invocation') {
              return (
                <ToolCall
                  key={`${message.id}-${index}`}
                  toolInvocation={part.toolInvocation}
                />
              )
            }
            // Add handling for other part types as needed
            return null
          })}
        </View>
      ))}
    </View>
  )
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    paddingHorizontal: 20,
    paddingVertical: 10,
  },
  messageContainer: {
    marginVertical: 8,
    maxWidth: '80%',
    padding: 12,
  },
  userMessageContainer: {
    alignSelf: 'flex-end',
    backgroundColor: 'transparent',
    borderColor: '#fff',
    borderWidth: 1,
  },
  messageText: {
    color: '#fff',
    fontSize: 14,
    fontFamily: 'Berkeley Mono',
  },
  userMessageText: {
    textAlign: 'right',
  },
})
