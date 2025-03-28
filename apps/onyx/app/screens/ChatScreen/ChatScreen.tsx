import React, { useCallback } from "react"
import { View, Text, ScrollView, TextInput, Pressable } from "react-native"
import { useAppTheme } from "@/utils/useAppTheme"
import { Ionicons } from "@expo/vector-icons"
import CodeHighlighter from "react-native-code-highlighter"
import { xt256 as syntaxTheme } from "react-syntax-highlighter/dist/esm/styles/hljs"
import { BlurView } from "expo-blur"
import { styles } from "./ChatScreen.styles"
import { fetch as expoFetch } from 'expo/fetch';
import { useChat } from "@ai-sdk/react"
import { ToastProvider } from "@openagents/ui"

// Wrapper component that uses the toast context
const ChatScreenContent = () => {
  const { theme } = useAppTheme()
  const { messages, append } = useChat({
    fetch: expoFetch as unknown as typeof globalThis.fetch,
    api: "https://chat.openagents.com",
    onError: error => console.error(error, 'ERROR')
  })

  const [message, setMessage] = React.useState("")

  const onSubmit = useCallback(() => {
    if (!message.trim()) return

    append({
      content: message,
      role: 'user'
    })
    setMessage("")
  }, [message])

  const renderMessageContent = (text: string) => {
    if (!text.includes('```')) {
      return (
        <Text
          style={[
            styles.messageText,
            { color: theme.colors.text },
          ]}
        >
          {text}
        </Text>
      )
    }

    const parts = text.split('```')
    const [beforeCode, codeBlock, afterCode] = parts
    const [lang, ...codeLines] = codeBlock.split('\n')
    const code = codeLines.join('\n').trim()

    return (
      <>
        {beforeCode && (
          <Text style={[styles.messageText, { color: theme.colors.text }]}>
            {beforeCode}
          </Text>
        )}
        <View style={styles.codeBlock}>
          <CodeHighlighter
            hljsStyle={syntaxTheme}
            textStyle={styles.codeText}
            language={lang.trim()}
            scrollViewProps={{
              contentContainerStyle: { padding: 12 },
              style: { backgroundColor: '#000' }
            }}
          >
            {code}
          </CodeHighlighter>
        </View>
        {afterCode && (
          <Text style={[styles.messageText, { color: theme.colors.text }]}>
            {afterCode}
          </Text>
        )}
      </>
    )
  }

  return (
    <View style={[styles.container, { backgroundColor: theme.colors.background }]}>
      <BlurView intensity={20} tint="dark" style={styles.statusBarOverlay} />
      <ScrollView
        style={styles.scrollView}
        contentContainerStyle={styles.scrollContent}
      >
        {messages.map((message) => (
          <View
            key={message.id}
            style={[
              styles.messageRow,
              message.role === 'user' ? styles.userMessageRow : styles.agentMessageRow,
            ]}
          >
            <View
              style={[
                styles.messageContainer,
                message.role === 'user' ? styles.userMessage : styles.agentMessage,
              ]}
            >
              {renderMessageContent(message.content)}
            </View>
          </View>
        ))}
      </ScrollView>
      <View style={styles.inputContainer}>
        <TextInput
          style={[styles.input, { color: theme.colors.text }]}
          placeholder="Message OpenAgents"
          placeholderTextColor={theme.colors.text + '80'}
          value={message}
          onChangeText={setMessage}
          multiline
        />
        <Pressable style={styles.sendButton} onPress={onSubmit}>
          <Ionicons name="arrow-up" size={20} color={theme.colors.text} />
        </Pressable>
      </View>
    </View>
  )
}

// Export wrapped with ToastProvider
export const ChatScreen = () => {
  return (
    <ToastProvider>
      <ChatScreenContent />
    </ToastProvider>
  )
}
