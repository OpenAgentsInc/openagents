import React from "react"
import { View, Text, StyleSheet, ScrollView } from "react-native"
import { useAppTheme } from "@/utils/useAppTheme"
import { typography, fontWeights } from "@/theme/typography"

const DEMO_MESSAGES = [
  { id: 1, text: "Hi! How can I help you with Bitcoin development today?", isAgent: true },
  { id: 2, text: "I want to implement Lightning Network payments in my app", isUser: true },
  { id: 3, text: "Great choice! Lightning Network is perfect for fast, low-cost Bitcoin transactions. Are you looking to implement a Lightning node or connect to an existing one?", isAgent: true },
  { id: 4, text: "I want to use Breez SDK to handle the Lightning stuff", isUser: true },
  { id: 5, text: "Excellent! Breez SDK is a great choice for adding Lightning capabilities without running your own node. Let's start by setting up the SDK and configuring it with your credentials. Would you like to see some example code?", isAgent: true },
]

export const ChatScreen = () => {
  const { theme } = useAppTheme()

  return (
    <View style={[styles.container, { backgroundColor: theme.colors.background }]}>
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
              <Text
                style={[
                  styles.messageText,
                  { color: theme.colors.text },
                  message.isUser && styles.userMessageText,
                ]}
              >
                {message.text}
              </Text>
            </View>
          </View>
        ))}
      </ScrollView>
    </View>
  )
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
  },
  scrollView: {
    flex: 1,
  },
  scrollContent: {
    paddingTop: 60,
    paddingBottom: 20,
    paddingHorizontal: 8,
  },
  messageRow: {
    flexDirection: "row",
    marginBottom: 16,
  },
  userMessageRow: {
    justifyContent: "flex-end",
  },
  agentMessageRow: {
    justifyContent: "flex-start",
    paddingRight: 32,
  },
  messageContainer: {
    paddingVertical: 8,
    paddingHorizontal: 12,
  },
  userMessage: {
    backgroundColor: "#000000",
    borderWidth: 1,
    borderColor: "#FFFFFF",
    maxWidth: "80%",
  },
  agentMessage: {
    borderRadius: 12,
    flex: 1,
  },
  messageText: {
    fontFamily: typography.primary.normal,
    fontWeight: fontWeights.normal,
    fontSize: 13,
    lineHeight: 18,
  },
  userMessageText: {
    color: "#FFFFFF",
  },
})
