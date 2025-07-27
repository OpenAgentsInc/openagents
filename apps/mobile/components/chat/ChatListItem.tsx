import { useState } from "react"
import { View, ViewStyle, TextStyle, Pressable, Alert } from "react-native"
import { Star, Trash2, Edit2 } from "lucide-react-native"
import { Gesture, GestureDetector } from "react-native-gesture-handler"
import Animated, {
  useSharedValue,
  useAnimatedStyle,
  withTiming,
  runOnJS,
} from "react-native-reanimated"

import { Text } from "../core/Text"
import type { ChatSession } from "../../types/chat"

export interface ChatListItemProps {
  session: ChatSession
  isActive?: boolean
  onPress: () => void
  onDelete: () => void
  onToggleStar: () => void
  onRename?: () => void
}

export function ChatListItem({
  session,
  isActive = false,
  onPress,
  onDelete,
  onToggleStar,
  onRename,
}: ChatListItemProps) {
  const [showActions, setShowActions] = useState(false)

  const translateX = useSharedValue(0)
  const scale = useSharedValue(1)

  // Swipe gesture for actions
  const panGesture = Gesture.Pan()
    .onUpdate((event) => {
      // Only allow left swipe (negative translation)
      if (event.translationX < 0) {
        translateX.value = Math.max(event.translationX, -100)
      }
    })
    .onEnd((event) => {
      if (event.translationX < -50) {
        // Show actions
        translateX.value = withTiming(-80)
        runOnJS(setShowActions)(true)
      } else {
        // Hide actions
        translateX.value = withTiming(0)
        runOnJS(setShowActions)(false)
      }
    })

  const animatedStyle = useAnimatedStyle(() => ({
    transform: [{ translateX: translateX.value }, { scale: scale.value }],
  }))

  const handlePress = () => {
    scale.value = withTiming(0.95, { duration: 100 }, () => {
      scale.value = withTiming(1, { duration: 100 })
    })

    if (showActions) {
      // Hide actions if they're showing
      translateX.value = withTiming(0)
      setShowActions(false)
    } else {
      onPress()
    }
  }

  const handleDelete = () => {
    Alert.alert(
      "Delete Chat",
      `Are you sure you want to delete "${session.title}"? This action cannot be undone.`,
      [
        { text: "Cancel", style: "cancel" },
        {
          text: "Delete",
          style: "destructive",
          onPress: () => {
            translateX.value = withTiming(0)
            setShowActions(false)
            onDelete()
          },
        },
      ],
    )
  }

  const handleToggleStar = () => {
    onToggleStar()
    // Keep actions visible after starring
  }

  return (
    <View style={$container}>
      <GestureDetector gesture={panGesture}>
        <Animated.View style={animatedStyle}>
          <Pressable
            onPress={handlePress}
            style={[
              $itemContainer,
              isActive && $activeContainer
            ]}
          >
            <View style={$content}>
              <View style={$header}>
                <Text style={[
                  $title,
                  isActive && $activeTitle
                ]} numberOfLines={1}>
                  {session.title || `Session - ${session.projectPath.split('/').pop()}`}
                </Text>
                {session.isStarred && (
                  <Star
                    size={14}
                    color="#f59e0b"
                    fill="#f59e0b"
                  />
                )}
              </View>
            </View>
          </Pressable>
        </Animated.View>
      </GestureDetector>

      {/* Action buttons (shown when swiped) */}
      {showActions && (
        <View style={$actions}>
          <Pressable
            onPress={handleToggleStar}
            style={[$actionButton, $starButton]}
          >
            <Star
              size={16}
              color="#f59e0b"
              fill={session.isStarred ? "#f59e0b" : "transparent"}
            />
          </Pressable>

          {onRename && (
            <Pressable onPress={onRename} style={[$actionButton, $editButton]}>
              <Edit2 size={16} color="#60a5fa" />
            </Pressable>
          )}

          <Pressable onPress={handleDelete} style={[$actionButton, $deleteButton]}>
            <Trash2 size={16} color="#ef4444" />
          </Pressable>
        </View>
      )}
    </View>
  )
}

// Styles using our dark zinc color scheme
const $container: ViewStyle = {
  position: "relative",
}

const $itemContainer: ViewStyle = {
  backgroundColor: '#000', // Pure black background
  paddingHorizontal: 16, // md spacing
  paddingVertical: 10, // xs + 2 for compact look
}

const $activeContainer: ViewStyle = {
  backgroundColor: '#27272a', // Zinc-800 for active
  borderLeftWidth: 3,
  borderLeftColor: '#f4f4f5', // Zinc-100 accent
}

const $content: ViewStyle = {
  flex: 1,
}

const $header: ViewStyle = {
  flexDirection: "row",
  alignItems: "center",
  justifyContent: "space-between",
}

const $title: TextStyle = {
  fontSize: 15,
  fontWeight: "500",
  color: '#f4f4f5', // Zinc-100 text
  flex: 1,
  marginRight: 8, // xs spacing
}

const $activeTitle: TextStyle = {
  color: '#f4f4f5', // Zinc-100 for active
}

const $actions: ViewStyle = {
  position: "absolute",
  right: 0,
  top: 0,
  bottom: 0,
  flexDirection: "row",
  alignItems: "center",
  backgroundColor: '#000', // Pure black background
  paddingRight: 12, // sm spacing
}

const $actionButton: ViewStyle = {
  width: 24,
  height: 24,
  borderRadius: 12,
  alignItems: "center",
  justifyContent: "center",
  marginLeft: 8, // xs spacing
}

const $starButton: ViewStyle = {
  backgroundColor: '#71717a', // Zinc-500 for star
}

const $editButton: ViewStyle = {
  backgroundColor: '#52525b', // Zinc-600 for edit
}

const $deleteButton: ViewStyle = {
  backgroundColor: '#52525b', // Zinc-600 for delete
}