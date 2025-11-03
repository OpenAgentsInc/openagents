import React from 'react'
import { ActivityIndicator, Text, View, ScrollView } from 'react-native'
import { useRouter } from 'expo-router'
import { useBridge } from '@/providers/ws'
import { useArchiveStore } from '@openagentsinc/core'
import { useTinyvexThreads } from 'tinyvex/react'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { useSettings } from '@/lib/settings-store'
import { devBridgeHost, devBridgeToken, isDevEnv } from '@/lib/env'
import { useAppLogStore } from '@openagentsinc/core'
import { AnsiText } from '@/components/ansi-text'

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
    <View style={{ flex: 1, paddingTop: 40, backgroundColor: Colors.background }}>
      <View style={{ alignItems: 'center', justifyContent: 'center' }}>
        <ActivityIndicator size="small" color={Colors.secondary} />
        <View style={{ height: 8 }} />
        <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 13 }}>
          {connected ? 'Loading recent thread…' : 'Connecting to bridge…'}
        </Text>
      </View>
      {/* Live logs while waiting for bridge/threads */}
      <View style={{ height: 12 }} />
      <View style={{ flex: 1, paddingHorizontal: 12 }}>
        <Text style={{ color: Colors.secondary, fontFamily: Typography.bold, fontSize: 12, marginBottom: 6 }}>Console</Text>
        <ScrollView
          style={{ flex: 1, borderWidth: 1, borderColor: Colors.border, borderRadius: 6, backgroundColor: '#0e0e0e' }}
          contentContainerStyle={{ padding: 10, gap: 2 }}
        >
          <LogLines />
        </ScrollView>
      </View>
    </View>
  )
}

function LogLines() {
  const logs = useAppLogStore((s) => s.logs)
  // Pick last 120 lines and stringify details when present
  const items = React.useMemo(() => {
    const list = Array.isArray(logs) ? logs.slice(-120) : []
    return list.map((l, idx) => {
      let line = ''
      try {
        const details = l.details != null ? (typeof l.details === 'string' ? l.details : JSON.stringify(l.details)) : ''
        if (l.event === 'bridge.sidecar' && details) {
          // details is { line }
          const obj = l.details as any
          line = String(obj && obj.line ? obj.line : details)
        } else {
          line = `[${l.level}] ${l.event}${details ? ' ' + details : ''}`
        }
      } catch {
        line = `${l.event}`
      }
      const anyL: any = l as any
      return { id: (anyL.id as string | undefined) ?? `${l.ts}-${idx}`, text: line }
    })
  }, [logs])
  return (
    <>
      {items.map((it) => (
        <AnsiText key={it.id} line={it.text} />
      ))}
    </>
  )
}
