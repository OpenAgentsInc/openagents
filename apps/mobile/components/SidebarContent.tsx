import { useEffect } from "react"
import { View, ViewStyle, TextStyle, TouchableOpacity, Platform } from "react-native"

import { ChatList } from "./chat/ChatList"
import { Text } from "./index"
import { useConfectAuth } from "../contexts/SimpleConfectAuthContext"
import type { ChatSession } from "../types/chat"

export interface SidebarContentProps {
  /**
   * Optional style override for the container.
   */
  style?: ViewStyle
  /**
   * Callback when a chat session is selected
   */
  onSessionSelect?: (sessionId: string) => void
  /**
   * Callback when new chat is requested
   */
  onNewChat?: () => void
  /**
   * Current active session ID
   */
  currentSessionId?: string | null
  /**
   * Whether the sidebar is visible (drawer is open)
   */
  isVisible?: boolean
  /**
   * Sessions data - for now we'll need to pass this from parent
   * until we implement the persistence hook
   */
  sessions?: ChatSession[]
  /**
   * Session management callbacks
   */
  onSessionDelete?: (sessionId: string) => void
  onSessionStar?: (sessionId: string) => void
  onSessionRename?: (sessionId: string, newTitle: string) => void
  /**
   * Callback when sidebar should be collapsed
   */
  onCollapseSidebar?: () => void
}

/**
 * Sidebar content component for the drawer navigation.
 * Contains chat list.
 */
export function SidebarContent(props: SidebarContentProps) {
  const { 
    style: $styleOverride, 
    onSessionSelect, 
    onNewChat, 
    currentSessionId, 
    isVisible,
    sessions = [], // Default to empty array for now
    onSessionDelete,
    onSessionStar,
    onSessionRename,
    onCollapseSidebar,
  } = props

  const { isAuthenticated, user, logout } = useConfectAuth()

  // TODO: Replace with actual chat persistence hook
  // For now we'll use the sessions passed as props

  const handleSessionSelect = (sessionId: string) => {
    onSessionSelect?.(sessionId)
  }

  const handleSessionDelete = (sessionId: string) => {
    onSessionDelete?.(sessionId)
  }

  const handleSessionStar = (sessionId: string) => {
    onSessionStar?.(sessionId)
  }

  const handleNewChat = () => {
    onNewChat?.()
  }

  const handleSessionRename = (sessionId: string, newTitle: string) => {
    onSessionRename?.(sessionId, newTitle)
  }

  const handleLogout = () => {
    logout()
    onCollapseSidebar?.()
  }

  return (
    <View style={[$container, $styleOverride]}>
      <View style={$chatListContainer}>
        <ChatList
          sessions={sessions}
          currentSessionId={currentSessionId}
          onSessionSelect={handleSessionSelect}
          onSessionDelete={handleSessionDelete}
          onSessionStar={handleSessionStar}
          onNewChat={handleNewChat}
          onSessionRename={handleSessionRename}
        />
      </View>
      
      {isAuthenticated && user && (
        <View style={$logoutContainer}>
          <TouchableOpacity style={$logoutButton} onPress={handleLogout}>
            <Text style={$logoutText}>
              Logout ({user.githubUsername})
            </Text>
          </TouchableOpacity>
        </View>
      )}
    </View>
  )
}

const $container: ViewStyle = {
  backgroundColor: '#000', // Pure black background
  flex: 1,
  borderRightWidth: 1,
  borderRightColor: '#27272a', // Zinc-800 border
}

const $chatListContainer: ViewStyle = {
  flex: 1,
}

const $logoutContainer: ViewStyle = {
  borderTopWidth: 1,
  borderTopColor: '#27272a', // Zinc-800 border
  padding: 16,
}

const $logoutButton: ViewStyle = {
  backgroundColor: '#000000', // Match login button style
  borderWidth: 1,
  borderColor: '#ffffff',
  paddingHorizontal: 16,
  paddingVertical: 12,
  borderRadius: 8,
  alignItems: 'center',
  justifyContent: 'center',
}

const $logoutText: TextStyle = {
  color: '#ffffff',
  fontSize: 14,
  fontWeight: 'bold',
  fontFamily: Platform.select({
    ios: 'Berkeley Mono',
    android: 'Berkeley Mono',
    default: 'monospace',
  }),
}