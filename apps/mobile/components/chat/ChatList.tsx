import { useMemo, useCallback, useState } from "react"
import { View, ViewStyle, TextStyle, FlatList, Pressable, TextInput, TouchableOpacity, Modal } from "react-native"
import { useSafeAreaInsets } from "react-native-safe-area-context"

import { Text } from "../core/Text"
import type { ChatSession } from "../../types/chat"
import { ChatListItem } from "./ChatListItem"
import { IconPlus } from "../icons/IconPlus"
import { DARK_THEME } from "../../constants/colors"

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
  const [showRenameModal, setShowRenameModal] = useState(false)
  const [renameSession, setRenameSession] = useState<ChatSession | null>(null)
  const [renameValue, setRenameValue] = useState("")

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

  const handleSessionPress = useCallback((sessionId: string) => {
    onSessionSelect(sessionId)
  }, [onSessionSelect])

  const handleSessionDelete = useCallback((sessionId: string) => {
    onSessionDelete(sessionId)
  }, [onSessionDelete])

  const handleSessionStar = useCallback((sessionId: string) => {
    onSessionStar(sessionId)
  }, [onSessionStar])

  const handleSessionRename = useCallback((session: ChatSession) => {
    if (!onSessionRename) return
    setRenameSession(session)
    setRenameValue(session.title || "")
    setShowRenameModal(true)
  }, [onSessionRename])

  const handleRenameConfirm = useCallback(() => {
    if (renameSession && onSessionRename && renameValue.trim()) {
      onSessionRename(renameSession.id, renameValue.trim())
    }
    setShowRenameModal(false)
    setRenameSession(null)
    setRenameValue("")
  }, [renameSession, onSessionRename, renameValue])

  const handleRenameCancel = useCallback(() => {
    setShowRenameModal(false)
    setRenameSession(null)
    setRenameValue("")
  }, [])

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
          <Pressable 
            onPress={onNewChat} 
            style={$newChatButton}
            accessibilityRole="button"
            accessibilityLabel="Create new chat"
          >
            <IconPlus />
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
        <Pressable 
          onPress={onNewChat} 
          style={$newChatButton}
          accessibilityRole="button"
          accessibilityLabel="Create new chat"
        >
          <RNText style={{ fontSize: 20, color: "#f4f4f5", fontWeight: "bold" }}>+</RNText>
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

      {/* Rename Modal */}
      <Modal
        visible={showRenameModal}
        transparent
        animationType="fade"
        onRequestClose={handleRenameCancel}
      >
        <View style={$modalOverlay}>
          <View style={$modalContent}>
            <View style={$modalHeader}>
              <Text style={$modalTitle}>Rename Chat</Text>
              <TouchableOpacity 
                onPress={handleRenameCancel}
                accessibilityRole="button"
                accessibilityLabel="Close rename dialog"
              >
                <Text style={$modalClose}>×</Text>
              </TouchableOpacity>
            </View>
            
            <View style={$modalBody}>
              <Text style={$modalLabel}>Enter new name:</Text>
              <TextInput
                style={$modalInput}
                value={renameValue}
                onChangeText={setRenameValue}
                placeholder="Chat name"
                placeholderTextColor={DARK_THEME.textTertiary}
                autoFocus
                selectTextOnFocus
                accessibilityLabel="Chat name input"
              />
              
              <View style={$modalActions}>
                <TouchableOpacity 
                  style={[$modalButton, $modalCancelButton]} 
                  onPress={handleRenameCancel}
                  accessibilityRole="button"
                  accessibilityLabel="Cancel rename"
                >
                  <Text style={$modalCancelText}>Cancel</Text>
                </TouchableOpacity>
                <TouchableOpacity 
                  style={[$modalButton, $modalConfirmButton, !renameValue.trim() && $modalButtonDisabled]} 
                  onPress={handleRenameConfirm}
                  disabled={!renameValue.trim()}
                  accessibilityRole="button"
                  accessibilityLabel="Confirm rename"
                >
                  <Text style={$modalConfirmText}>Rename</Text>
                </TouchableOpacity>
              </View>
            </View>
          </View>
        </View>
      </Modal>
    </View>
  )
}

// Styles using our dark zinc color scheme
const $container: ViewStyle = {
  flex: 1,
  backgroundColor: DARK_THEME.background,
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
  color: DARK_THEME.text,
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
  color: DARK_THEME.textSecondary,
  textAlign: "center",
  marginBottom: 8, // xs spacing
}

const $emptyStateSubtext: TextStyle = {
  fontSize: 14,
  color: DARK_THEME.textTertiary,
  textAlign: "center",
}

// Modal styles
const $modalOverlay: ViewStyle = {
  flex: 1,
  backgroundColor: DARK_THEME.overlay,
  justifyContent: 'center',
  alignItems: 'center',
}

const $modalContent: ViewStyle = {
  backgroundColor: DARK_THEME.background,
  borderRadius: 12,
  width: '90%',
  maxWidth: 400,
  borderWidth: 1,
  borderColor: DARK_THEME.border,
}

const $modalHeader: ViewStyle = {
  flexDirection: 'row',
  justifyContent: 'space-between',
  alignItems: 'center',
  padding: 16,
  borderBottomWidth: 1,
  borderBottomColor: DARK_THEME.border,
}

const $modalTitle: TextStyle = {
  fontSize: 18,
  fontWeight: 'bold',
  color: DARK_THEME.text,
}

const $modalClose: TextStyle = {
  fontSize: 20,
  color: DARK_THEME.textSecondary,
  fontWeight: 'bold',
}

const $modalBody: ViewStyle = {
  padding: 16,
}

const $modalLabel: TextStyle = {
  color: DARK_THEME.text,
  fontSize: 14,
  fontWeight: 'bold',
  marginBottom: 8,
}

const $modalInput: TextStyle = {
  backgroundColor: DARK_THEME.backgroundSecondary,
  color: DARK_THEME.text,
  padding: 12,
  borderRadius: 8,
  borderWidth: 1,
  borderColor: DARK_THEME.border,
  fontSize: 14,
  marginBottom: 16,
}

const $modalActions: ViewStyle = {
  flexDirection: 'row',
  gap: 12,
}

const $modalButton: ViewStyle = {
  flex: 1,
  paddingVertical: 12,
  borderRadius: 8,
  alignItems: 'center',
}

const $modalCancelButton: ViewStyle = {
  backgroundColor: DARK_THEME.border,
}

const $modalConfirmButton: ViewStyle = {
  backgroundColor: DARK_THEME.primary,
}

const $modalButtonDisabled: ViewStyle = {
  backgroundColor: DARK_THEME.disabled,
}

const $modalCancelText: TextStyle = {
  color: DARK_THEME.text,
  fontSize: 14,
  fontWeight: 'bold',
}

const $modalConfirmText: TextStyle = {
  color: DARK_THEME.text,
  fontSize: 14,
  fontWeight: 'bold',
}