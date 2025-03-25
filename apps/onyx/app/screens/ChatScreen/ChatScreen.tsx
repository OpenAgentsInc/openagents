import React from "react"
import { View, Text, ScrollView, TextInput, Pressable } from "react-native"
import { useAppTheme } from "@/utils/useAppTheme"
import { typography, fontWeights } from "@/theme/typography"
import { Ionicons } from "@expo/vector-icons"
import CodeHighlighter from "react-native-code-highlighter"
import { xt256 as syntaxTheme } from "react-syntax-highlighter/dist/esm/styles/hljs"
import { BlurView } from "expo-blur"
import { styles } from "./ChatScreen.styles"

const DEMO_MESSAGES = [
  { id: 1, text: "How can I help you today?", isAgent: true },
  { id: 2, text: "I want to implement Lightning Network payments in my app", isUser: true },
  { id: 3, text: "Great choice! Lightning Network is perfect for fast, low-cost Bitcoin transactions. Are you looking to implement a Lightning node or connect to an existing one?", isAgent: true },
  { id: 4, text: "I want to use Breez SDK to handle the Lightning stuff", isUser: true },
  { id: 5, text: "Excellent! Breez SDK is a great choice for adding Lightning capabilities without running your own node. Let's start by setting up the SDK and configuring it with your credentials. Would you like to see some example code?", isAgent: true },
  { id: 6, text: "Yes - for React Native", isUser: true },
  { id: 7, text: "Here's a basic example of initializing Breez SDK in a React Native app:\n\n```typescript\nconst mnemonic = '<mnemonics words>'\n\n// Create the default config, providing your Breez API key\nconst config = await defaultConfig(\n  LiquidNetwork.MAINNET,\n  '<your-Breez-API-key>'\n)\n\n// By default in React Native the workingDir is set to:\n// `/<APPLICATION_SANDBOX_DIRECTORY>/breezSdkLiquid`\n// You can change this to another writable directory or a\n// subdirectory of the workingDir if managing multiple mnemonics.\nconsole.log(`Working directory: ${config.workingDir}`)\n// config.workingDir = \"path to writable directory\"\n\nawait connect({ mnemonic, config })\n```\n\nThis example shows the basic setup. Would you like me to explain each part or show how to handle specific payment scenarios?", isAgent: true },
]

export const ChatScreen = () => {
  const { theme } = useAppTheme()
  const [message, setMessage] = React.useState("")

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
        {DEMO_MESSAGES.map((message) => (
          <View
            key={message.id}
            style={[
              styles.messageRow,
              message.isUser ? styles.userMessageRow : styles.agentMessageRow,
            ]}
          >
            <View
              style={[
                styles.messageContainer,
                message.isUser ? styles.userMessage : styles.agentMessage,
              ]}
            >
              {renderMessageContent(message.text)}
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
        <Pressable style={styles.sendButton}>
          <Ionicons name="arrow-up" size={20} color={theme.colors.text} />
        </Pressable>
      </View>
    </View>
  )
}
