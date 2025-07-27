import { useMemo } from "react"
import { View, ViewStyle, TextStyle, FlatList, Alert, Pressable } from "react-native"
import { Plus } from "lucide-react-native"
import { useSafeAreaInsets } from "react-native-safe-area-context"

import { Text } from "../core/Text"
import type { ChatSession } from "../../types/chat"
import { ChatListItem } from "./ChatListItem"

export interface ChatListProps {
  sessions: ChatSession[]
  currentSessionId?: string | null
  onSessionSelect: (sessionId: string) => void
  onSessionDelete: (sessionId: string) => void
  onSessionStar: (sessionId: string) => void
  onNewChat: () => void
  onSessionRename?: (sessionId: string, newTitle: string) => void
}

export function ChatList({
  sessions,
  currentSessionId,
  onSessionSelect,
  onSessionDelete,
  onSessionStar,
  onNewChat,
  onSessionRename,
}: ChatListProps) {
  const { top } = useSafeAreaInsets()

  // Sort sessions by last updated, with starred first
  const sortedSessions = useMemo(() => {
    return [...sessions].sort((a, b) => {
      // Starred items first
      if (a.isStarred && !b.isStarred) return -1
      if (!a.isStarred && b.isStarred) return 1
      // Then by date
      return b.updatedAt.getTime() - a.updatedAt.getTime()
    })
  }, [sessions])

  const handleSessionPress = (sessionId: string) => {
    onSessionSelect(sessionId)
  }

  const handleSessionDelete = (sessionId: string) => {
    onSessionDelete(sessionId)
  }

  const handleSessionStar = (sessionId: string) => {
    onSessionStar(sessionId)
  }

  const handleSessionRename = (session: ChatSession) => {
    if (!onSessionRename) return

    Alert.prompt(
      "Rename Chat",
      "Enter a new name for this chat:",
      [
        { text: "Cancel", style: "cancel" },
        {
          text: "Rename",
          onPress: (newTitle) => {
            if (newTitle && newTitle.trim()) {
              onSessionRename(session.id, newTitle.trim())
            }
          },
        },
      ],
      "plain-text",
      session.title,
    )
  }

  const renderSessionItem = ({ item }: { item: ChatSession }) => (
    <ChatListItem
      session={item}
      isActive={item.id === currentSessionId}
      onPress={() => handleSessionPress(item.id)}
      onDelete={() => handleSessionDelete(item.id)}
      onToggleStar={() => handleSessionStar(item.id)}
      onRename={onSessionRename ? () => handleSessionRename(item) : undefined}
    />
  )

  const renderEmptyState = () => (
    <View style={$emptyState}>
      <Text style={$emptyStateText}>No chats yet</Text>
      <Text style={$emptyStateSubtext}>Start a new conversation to begin</Text>
    </View>
  )

  if (sortedSessions.length === 0) {
    return (
      <View style={$container}>
        {/* Header */}
        <View style={[$header, { paddingTop: top + 16 }]}>
          <Text style={$headerTitle}>Chats</Text>
          <Pressable onPress={onNewChat} style={$newChatButton}>
            <Plus size={20} color="#fff" />
          </Pressable>
        </View>

        {renderEmptyState()}
      </View>
    )
  }

  return (
    <View style={$container}>
      {/* Header */}
      <View style={[$header, { paddingTop: top + 16 }]}>
        <Text style={$headerTitle}>Chats</Text>
        <Pressable onPress={onNewChat} style={$newChatButton}>
          <Plus size={20} color="#fff" />
        </Pressable>
      </View>

      {/* Chat List */}
      <FlatList<ChatSession>
        data={sortedSessions}
        renderItem={renderSessionItem}
        keyExtractor={(item) => item.id}
        showsVerticalScrollIndicator={false}
        contentContainerStyle={$listContent}
      />
    </View>
  )
}

// Styles using our black/gray color scheme
const $container: ViewStyle = {
  flex: 1,
  backgroundColor: '#1a1a1a', // Our black background
}

const $header: ViewStyle = {
  flexDirection: "row",
  alignItems: "center",
  justifyContent: "space-between",
  paddingHorizontal: 16, // md spacing
  paddingVertical: 16, // md spacing
}

const $headerTitle: TextStyle = {
  fontSize: 20,
  fontWeight: "600",
  color: '#fff', // White text
}

const $newChatButton: ViewStyle = {
  width: 32,
  height: 32,
  alignItems: "center",
  justifyContent: "center",
  borderRadius: 16,
}

const $listContent: ViewStyle = {
  paddingBottom: 24, // lg spacing
}

const $emptyState: ViewStyle = {
  flex: 1,
  alignItems: "center",
  justifyContent: "center",
  paddingHorizontal: 24, // lg spacing
  paddingVertical: 32, // xl spacing
}

const $emptyStateText: TextStyle = {
  fontSize: 16,
  color: '#999', // Dim text
  textAlign: "center",
  marginBottom: 8, // xs spacing
}

const $emptyStateSubtext: TextStyle = {
  fontSize: 14,
  color: '#666', // Even dimmer text
  textAlign: "center",
}