import React from 'react'
import { ActivityIndicator, Text, View } from 'react-native'
import { useRouter } from 'expo-router'
import { useBridge } from '@/providers/ws'
import { useArchiveStore } from '@openagentsinc/core'
import { useTinyvexThreads } from 'tinyvex/react'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { useSettings } from '@/lib/settings-store'
import { devBridgeHost, devBridgeToken, isDevEnv } from '@/lib/env'

export default function Index() {
  const router = useRouter()
  const { connected, connecting, connect } = useBridge()
  const setBridgeHost = useSettings((s) => s.setBridgeHost)
  const setBridgeToken = useSettings((s) => s.setBridgeToken)
  const { threads } = useTinyvexThreads(50)
  const isArchived = useArchiveStore((s) => s.isArchived)

  React.useEffect(() => {
    if (!connected) return
    if (!Array.isArray(threads) || threads.length === 0) return
    try {
      const pick = threads
        .filter((r) => {
          const tid = String(r.id || '')
          // skip ephemeral/transient threads and archived
          return tid && !tid.startsWith('ephemeral_') && !isArchived(tid)
        })
        .sort((a, b) => Number(b.updated_at ?? 0) - Number(a.updated_at ?? 0))[0]
      const tid = pick?.id ? String(pick.id) : ''
      if (tid) router.replace(`/thread/${encodeURIComponent(tid)}` as any)
    } catch {}
  }, [connected, threads, isArchived, router])

  // Dev auto-connect without requiring Settings screen
  React.useEffect(() => {
    if (connected || connecting) return
    try {
      if (isDevEnv()) {
        const h = devBridgeHost();
        const t = devBridgeToken();
        if (h) { try { setBridgeHost(h) } catch {} }
        if (t) { try { setBridgeToken(t) } catch {} }
        if (h) { try { connect() } catch {} }
      }
    } catch {}
  }, [connected, connecting, connect, setBridgeHost, setBridgeToken])

  // Fallback: if we can't connect or find threads within a short window, start a new thread
  React.useEffect(() => {
    const id = setTimeout(() => {
      try {
        if (!connected) router.replace('/thread/new' as any)
      } catch {}
    }, 2500)
    return () => clearTimeout(id)
  }, [connected, router])

  return (
    <View style={{ flex: 1, alignItems: 'center', justifyContent: 'center', backgroundColor: Colors.background }}>
      <ActivityIndicator size="small" color={Colors.secondary} />
      <View style={{ height: 8 }} />
      <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 13 }}>
        {connected ? 'Loading recent thread…' : 'Connecting to bridge…'}
      </Text>
    </View>
  )
}
