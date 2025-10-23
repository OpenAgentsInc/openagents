import React from 'react'
import { View, Text, Pressable, StyleSheet } from 'react-native'
import { useSafeAreaInsets } from 'react-native-safe-area-context'
import { Ionicons } from '@expo/vector-icons'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { useHeaderStore } from '@/lib/header-store'
import { useDrawer } from '@/providers/drawer'
import { useWs } from '@/providers/ws'
import * as Haptics from 'expo-haptics'
import { router, usePathname } from 'expo-router'

export function AppHeader() {
  const insets = useSafeAreaInsets()
  const title = useHeaderStore((s) => s.title)
  const setHeight = useHeaderStore((s) => s.setHeight)
  const { toggle } = useDrawer()
  const { connected, clearLog } = useWs()
  const pathname = usePathname()
  const showBack = React.useMemo(() => pathname?.startsWith('/message/') ?? false, [pathname])

  const onLayout = React.useCallback((e: any) => {
    const h = e?.nativeEvent?.layout?.height ?? 0
    if (h > 0) setHeight(h)
  }, [setHeight])

  const onNewChat = React.useCallback(async () => {
    try { if (process.env.EXPO_OS === 'ios') { await Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light) } } catch {}
    clearLog()
    router.push('/(tabs)/session')
  }, [clearLog])

  return (
    <View onLayout={onLayout} style={{ paddingTop: insets.top, backgroundColor: Colors.background, borderBottomColor: Colors.border, borderBottomWidth: StyleSheet.hairlineWidth }}>
      <View style={{ height: 48, flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between' }}>
        <View style={{ flexDirection: 'row', alignItems: 'center' }}>
          <Pressable onPress={() => { showBack ? router.back() : toggle() }} accessibilityRole="button" hitSlop={{ top: 8, bottom: 8, left: 8, right: 8 }} style={{ paddingHorizontal: 6, paddingVertical: 6 }}>
            <Ionicons name={showBack ? 'chevron-back' : 'menu'} size={22} color={Colors.foreground} />
          </Pressable>
          <View style={{ marginLeft: 8 }}>
            <View style={{ width: 8, height: 8, borderRadius: 4, backgroundColor: connected ? Colors.success : Colors.danger }} />
          </View>
          {!!title && (
            <Text style={{ color: Colors.foreground, fontFamily: Typography.bold, fontSize: 16, marginLeft: 6 }}>{title}</Text>
          )}
        </View>
        <View style={{ flexDirection: 'row', alignItems: 'center', marginRight: 12 }}>
          <Pressable onPress={onNewChat} accessibilityRole="button" accessibilityLabel="New chat" hitSlop={{ top: 8, bottom: 8, left: 8, right: 8 }} style={{ paddingHorizontal: 6, paddingVertical: 6 }}>
            <Ionicons name="add" size={22} color={Colors.foreground} />
          </Pressable>
        </View>
      </View>
    </View>
  )
}
