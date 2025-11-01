import * as Haptics from "expo-haptics"
import { router, usePathname } from "expo-router"
import React from "react"
import { Pressable, Text, View, type LayoutChangeEvent } from "react-native"
import { useSafeAreaInsets } from "react-native-safe-area-context"
import { Colors } from "@/constants/theme"
import { Typography } from "@/constants/typography"
import { useHeaderStore } from "@/lib/header-store"
import { clearLogs as clearLogsStore } from "@/lib/log-store"
import { useDrawer } from "@/providers/drawer"
import { useBridge } from "@/providers/ws"
import { Ionicons, MaterialCommunityIcons } from "@expo/vector-icons"

export function AppHeader() {
  const insets = useSafeAreaInsets()
  const title = useHeaderStore((s) => s.title)
  const subtitle = useHeaderStore((s) => s.subtitle)
  const setHeight = useHeaderStore((s) => s.setHeight)
  const { toggle } = useDrawer()
  const { connected } = useBridge()
  const pathname = usePathname()
  const showBack = React.useMemo(() => {
    const p = String(pathname || '')
    // Library routes removed; no special back behavior needed
    return false
  }, [pathname])

  const onLayout = React.useCallback((e: LayoutChangeEvent) => {
    const h = e?.nativeEvent?.layout?.height ?? 0
    try {
      const prev = useHeaderStore.getState().height
      if (h > 0 && h !== prev) setHeight(h)
    } catch {
      if (h > 0) setHeight(h)
    }
  }, [setHeight])

  const onNewChat = React.useCallback(async () => {
    try { if (process.env.EXPO_OS === 'ios') { await Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light) } } catch {}
    // Start a new local thread view
    try { router.push('/thread/new') } catch {}
  }, [])

  return (
    <View onLayout={onLayout} style={{ paddingTop: insets.top, backgroundColor: Colors.background, borderBottomColor: Colors.border, borderBottomWidth: 1 }}>
      <View style={{ height: 48, flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between' }}>
        <View style={{ flexDirection: 'row', alignItems: 'center' }}>
          <Pressable
            onPress={() => { try { if (process.env.EXPO_OS === 'ios') { Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light) } } catch {}; showBack ? router.back() : toggle() }}
            accessibilityRole="button"
            testID="header-menu-button"
            // Expand the tappable area without changing visuals
            hitSlop={{ top: 32, bottom: 32, left: 20, right: 20 }}
            pressRetentionOffset={{ top: 20, bottom: 20, left: 20, right: 20 }}
            style={{ marginTop: -10, height: '100%', paddingHorizontal: 12, paddingVertical: 0, justifyContent: 'center', alignItems: 'center', position: 'relative' }}
          >
            <Ionicons name={showBack ? 'chevron-back' : 'menu'} size={22} color={Colors.foreground} />
            <View
              pointerEvents="none"
              accessibilityLabel="Connection status"
              testID="header-connection-indicator"
              style={{ position: 'absolute', right: 9, top: 12, width: 9, height: 9, borderRadius: 4.5, backgroundColor: connected ? Colors.success : Colors.danger, borderWidth: 1, borderColor: Colors.black }}
            />
          </Pressable>
          <View style={{ marginLeft: 6, maxWidth: '75%' }}>
            {!!title && (
              <Text numberOfLines={1} ellipsizeMode="tail" style={{ color: Colors.foreground, fontFamily: Typography.primary, fontSize: 14 }}>
                {title}
              </Text>
            )}
            {!!subtitle && (
              <View style={{ flexDirection: 'row', alignItems: 'center', marginTop: 1 }}>
                <Ionicons name="folder-outline" size={12} color={Colors.tertiary} />
                <Text style={{ color: Colors.tertiary, fontFamily: Typography.primary, fontSize: 12, marginLeft: 4 }}>{subtitle}</Text>
              </View>
            )}
          </View>
        </View>
        <View style={{ flexDirection: 'row', alignItems: 'center', marginRight: 12 }}>
          <Pressable onPress={onNewChat} accessibilityRole="button" accessibilityLabel="New chat" testID="header-new-chat" hitSlop={{ top: 8, bottom: 8, left: 8, right: 8 }} style={{ paddingHorizontal: 6, paddingVertical: 6 }}>
            <MaterialCommunityIcons name="comment-plus-outline" size={22} color={Colors.foreground} />
          </Pressable>
        </View>
      </View>
    </View>
  )
}

