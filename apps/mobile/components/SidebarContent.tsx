import { useEffect } from "react"
import { View, ViewStyle } from "react-native"

import { ChatList } from "./chat/ChatList"
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
  } = props

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

  return (
    <View style={[$container, $styleOverride]}>
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
  )
}

const $container: ViewStyle = {
  backgroundColor: '#000', // Pure black background
  flex: 1,
  borderRightWidth: 1,
  borderRightColor: '#27272a', // Zinc-800 border
}