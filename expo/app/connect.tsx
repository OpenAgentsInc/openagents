import React from 'react'
import { View, ActivityIndicator, Text } from 'react-native'
import { useLocalSearchParams, useRouter } from 'expo-router'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { parseBridgeCode, normalizeBridgeCodeInput } from '@/lib/pairing'
import { useBridge } from '@/providers/ws'
import { useSettings } from '@/lib/settings-store'

export default function ConnectRoute() {
  const params = useLocalSearchParams<{ j?: string; data?: string }>()
  const router = useRouter()
  const { setBridgeHost, connect } = useBridge()
  const setBridgeCode = useSettings((s) => s.setBridgeCode)
  const setBridgeToken = useSettings((s) => s.setBridgeToken)

  React.useEffect(() => {
    const raw = typeof params?.j === 'string' && params.j ? params.j : (typeof params?.data === 'string' ? params.data : '')
    const display = normalizeBridgeCodeInput(raw)
    if (display) {
      try { setBridgeCode(display) } catch {}
      const parsed = parseBridgeCode(display)
      if (parsed?.bridgeHost) try { setBridgeHost(parsed.bridgeHost) } catch {}
      if (parsed?.token) try { setBridgeToken(parsed.token || '') } catch {}
      try { connect() } catch {}
      try { router.replace('/thread/new' as any) } catch {}
    } else {
      // No param found — go to onboarding
      try { router.replace('/onboarding' as any) } catch {}
    }
  }, [params?.j, params?.data])

  return (
    <View style={{ flex: 1, backgroundColor: Colors.background, alignItems: 'center', justifyContent: 'center' }}>
      <ActivityIndicator color={Colors.foreground} />
      <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12, marginTop: 8 }}>Connecting…</Text>
    </View>
  )
}
