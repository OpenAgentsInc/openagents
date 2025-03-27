import { ScrollView, View, Text, StyleSheet } from 'react-native'
import { UIMessage } from './types'
import { ToolCall } from './ToolCall'
import Markdown from 'react-native-markdown-display'

interface MessageListProps {
  messages: UIMessage[]
}

export const MessageList = ({ messages }: MessageListProps) => {
  const visibleMessages = messages.filter(message => message.role !== 'system')

  return (
    <ScrollView
      style={styles.container}
      contentContainerStyle={styles.contentContainer}
    >
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
              return message.role === 'user' ? (
                <Text
                  key={`${message.id}-${index}`}
                  style={[
                    styles.messageText,
                    styles.userMessageText,
                  ]}
                >
                  {part.text}
                </Text>
              ) : (
                <Markdown
                  key={`${message.id}-${index}`}
                  style={markdownStyles}
                >
                  {part.text}
                </Markdown>
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
    </ScrollView>
  )
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
  },
  contentContainer: {
    padding: 20,
    paddingBottom: 80, // Height that matches input container + padding
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

const markdownStyles = {
  body: {
    color: '#fff',
    fontSize: 14,
    fontFamily: 'Berkeley Mono',
  },
  code_inline: {
    backgroundColor: '#333',
    color: '#fff',
    fontFamily: 'Berkeley Mono',
    padding: 4,
    borderRadius: 4,
  },
  code_block: {
    backgroundColor: '#333',
    padding: 8,
    borderRadius: 4,
    fontFamily: 'Berkeley Mono',
  },
  fence: {
    backgroundColor: '#333',
    padding: 8,
    borderRadius: 4,
    fontFamily: 'Berkeley Mono',
  },
  link: {
    color: '#58a6ff',
    textDecorationLine: 'underline',
  },
  list_item: {
    marginTop: 4,
    marginBottom: 4,
  },
  bullet_list: {
    marginTop: 8,
    marginBottom: 8,
  },
  ordered_list: {
    marginTop: 8,
    marginBottom: 8,
  },
  heading1: {
    fontSize: 24,
    fontWeight: 'bold',
    marginTop: 16,
    marginBottom: 8,
    color: '#fff',
  },
  heading2: {
    fontSize: 20,
    fontWeight: 'bold',
    marginTop: 16,
    marginBottom: 8,
    color: '#fff',
  },
  heading3: {
    fontSize: 18,
    fontWeight: 'bold',
    marginTop: 16,
    marginBottom: 8,
    color: '#fff',
  }
}
