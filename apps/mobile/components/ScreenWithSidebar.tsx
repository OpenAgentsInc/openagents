import { FC, useCallback, useRef, useState } from "react"
import { Platform, Pressable, View, ViewStyle, TextStyle } from "react-native"
import { useFocusEffect } from "@react-navigation/native"
import { PenSquare } from "lucide-react-native"
import { DrawerLayout, DrawerState } from "react-native-gesture-handler"
import { useSharedValue, withTiming } from "react-native-reanimated"

import { Header } from "./Header"
import { Screen } from "./Screen"
import { SidebarContent } from "./SidebarContent"
import { DrawerIconButton } from "./DrawerIconButton"
import type { ChatSession } from "../types/chat"

interface ScreenWithSidebarProps {
  title: string | React.ReactNode
  children: React.ReactNode
  onNewChat?: () => void
  disableKeyboardAvoidance?: boolean
  onSessionSelect?: (sessionId: string) => void
  currentSessionId?: string | null
  sessions?: ChatSession[]
  onSessionDelete?: (sessionId: string) => void
  onSessionStar?: (sessionId: string) => void
  onSessionRename?: (sessionId: string, newTitle: string) => void
}

export const ScreenWithSidebar: FC<ScreenWithSidebarProps> = ({
  title,
  children,
  onNewChat,
  disableKeyboardAvoidance = false,
  onSessionSelect,
  currentSessionId,
  sessions,
  onSessionDelete,
  onSessionStar,
  onSessionRename,
}) => {
  const [open, setOpen] = useState(false)

  const drawerRef = useRef<DrawerLayout>(null)
  const progress = useSharedValue(0)

  const toggleDrawer = () => {
    if (!open) {
      setOpen(true)
      drawerRef.current?.openDrawer({ speed: 2 })
    } else {
      setOpen(false)
      drawerRef.current?.closeDrawer({ speed: 2 })
    }
  }

  useFocusEffect(useCallback(() => () => drawerRef.current?.closeDrawer({ speed: 2 }), []))

  const handleSessionSelect = (sessionId: string) => {
    onSessionSelect?.(sessionId)
    // Close drawer after selection
    setOpen(false)
    drawerRef.current?.closeDrawer({ speed: 2 })
  }

  const handleNewChatFromSidebar = () => {
    onNewChat?.()
    // Close drawer after creating new chat
    setOpen(false)
    drawerRef.current?.closeDrawer({ speed: 2 })
  }

  return (
    <DrawerLayout
      ref={drawerRef}
      drawerWidth={Platform.select({ default: 300 })}
      drawerType={"slide"}
      drawerPosition={"left"}
      overlayColor={open ? "rgba(0, 0, 0, 0.5)" : "transparent"}
      onDrawerSlide={(drawerProgress) => {
        progress.value = open ? 1 - drawerProgress : drawerProgress
      }}
      onDrawerStateChanged={(newState: DrawerState, drawerWillShow: boolean) => {
        if (newState === "Settling") {
          progress.value = withTiming(drawerWillShow ? 1 : 0, {
            duration: 250,
          })
          setOpen(drawerWillShow)
        }
      }}
      renderNavigationView={() => (
        <SidebarContent
          onSessionSelect={handleSessionSelect}
          onNewChat={handleNewChatFromSidebar}
          currentSessionId={currentSessionId}
          isVisible={open}
          sessions={sessions}
          onSessionDelete={onSessionDelete}
          onSessionStar={onSessionStar}
          onSessionRename={onSessionRename}
        />
      )}
    >
      <Screen
        preset="fixed"
        safeAreaEdges={[]}
        contentContainerStyle={$screenContainer}
        KeyboardAvoidingViewProps={disableKeyboardAvoidance ? { enabled: false } : undefined}
      >
        <View style={$headerContainer}>
          <Header
            title={typeof title === "string" ? title : ""}
            TitleActionComponent={typeof title === "string" ? undefined : () => title}
            titleMode={typeof title === "string" ? "center" : "flex"}
            LeftActionComponent={
              <DrawerIconButton onPress={toggleDrawer} {...{ open, progress }} />
            }
            titleStyle={$headerTitle}
            RightActionComponent={
              <View style={$headerRightActions}>
                <Pressable
                  onPress={() => onNewChat?.()}
                  hitSlop={{ top: 10, bottom: 10, left: 10, right: 10 }}
                >
                  <PenSquare size={20} color="#f4f4f5" />
                </Pressable>
              </View>
            }
            safeAreaEdges={["top"]}
            style={$header}
          />
        </View>
        {children}
      </Screen>
    </DrawerLayout>
  )
}

// Styles using our dark zinc color scheme
const $headerContainer: ViewStyle = {
  borderBottomWidth: 1,
  borderBottomColor: '#27272a', // Zinc-800 border
}

const $header: ViewStyle = {
  backgroundColor: "transparent",
}

const $headerRightActions: ViewStyle = {
  flexDirection: "row",
  gap: 16, // md spacing
  paddingRight: 16, // md spacing
  paddingVertical: 8, // sm spacing
}

const $headerTitle: TextStyle = {
  color: '#f4f4f5', // Zinc-100 text
  fontSize: 16,
  lineHeight: 22,
}

const $screenContainer: ViewStyle = {
  flex: 1,
}