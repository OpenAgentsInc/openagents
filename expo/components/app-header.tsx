import * as Haptics from "expo-haptics"
import { router, usePathname } from "expo-router"
import React from "react"
import { Pressable, Text, View } from "react-native"
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
  const { connected, clearLog, setResumeNextId } = useBridge()
  const pathname = usePathname()
  const showBack = React.useMemo(() => pathname?.startsWith('/message/') ?? false, [pathname])

  const onLayout = React.useCallback((e: any) => {
    const h = e?.nativeEvent?.layout?.height ?? 0
    if (h > 0) setHeight(h)
  }, [setHeight])

  const onNewChat = React.useCallback(async () => {
    try { if (process.env.EXPO_OS === 'ios') { await Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light) } } catch {}
    try { setResumeNextId('new') } catch {}
    // Persistently clear logs even if the session screen is not mounted yet
    try { await clearLogsStore() } catch {}
    clearLog()
    router.push('/thread?focus=1&new=1')
  }, [clearLog])

  return (
    <View onLayout={onLayout} style={{ paddingTop: insets.top, backgroundColor: Colors.background, borderBottomColor: Colors.border, borderBottomWidth: 1 }}>
      <View style={{ height: 48, flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between' }}>
        <View style={{ flexDirection: 'row', alignItems: 'center' }}>
          <Pressable
            onPressIn={() => { try { if (process.env.EXPO_OS === 'ios') { Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light) } } catch {} }}
            onPress={() => { showBack ? router.back() : toggle() }}
            accessibilityRole="button"
            hitSlop={{ top: 8, bottom: 8, left: 8, right: 8 }}
            style={{ paddingHorizontal: 6, paddingVertical: 6, position: 'relative' }}
          >
            <Ionicons name={showBack ? 'chevron-back' : 'menu'} size={22} color={Colors.foreground} />
            <View
              pointerEvents="none"
              style={{ position: 'absolute', right: 4, top: 7, width: 9, height: 9, borderRadius: 4.5, backgroundColor: connected ? Colors.success : Colors.danger, borderWidth: 1, borderColor: Colors.black }}
            />
          </Pressable>
          <View style={{ marginLeft: 6 }}>
            {!!title && (
              <Text style={{ color: Colors.foreground, fontFamily: Typography.primary, fontSize: 14 }}>{title}</Text>
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
          <Pressable onPress={onNewChat} accessibilityRole="button" accessibilityLabel="New chat" hitSlop={{ top: 8, bottom: 8, left: 8, right: 8 }} style={{ paddingHorizontal: 6, paddingVertical: 6 }}>
            <MaterialCommunityIcons name="comment-plus-outline" size={22} color={Colors.foreground} />
          </Pressable>
        </View>
      </View>
    </View>
  )
}
