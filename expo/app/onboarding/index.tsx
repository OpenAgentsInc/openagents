import React from 'react'
import { View, Text, TextInput, Pressable, StyleSheet } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { useHeaderTitle } from '@/lib/header-store'
import { useRouter } from 'expo-router'
import { useBridge } from '@/providers/ws'
import { useSettings } from '@/lib/settings-store'
import { useQuery } from 'convex/react'
import { parseBridgeCode } from '@/lib/pairing'
import { Ionicons } from '@expo/vector-icons'

export default function Onboarding() {
  useHeaderTitle('Connect')
  const router = useRouter()
  const { bridgeHost, setBridgeHost, connected, connect, disconnect } = useBridge()
  const bridgeCode = useSettings((s) => s.bridgeCode)
  const setBridgeCode = useSettings((s) => s.setBridgeCode)
  const convexUrl = useSettings((s) => s.convexUrl)
  const setConvexUrl = useSettings((s) => s.setConvexUrl)

  // Derive Convex URL from bridge host (same logic as Settings)
  const derivedConvexUrl = React.useMemo(() => {
    try {
      const val = String(bridgeHost || '').trim()
      const stripped = val
        .replace(/^ws:\/\//i, '')
        .replace(/^wss:\/\//i, '')
        .replace(/^http:\/\//i, '')
        .replace(/^https:\/\//i, '')
        .replace(/\/$/, '')
        .replace(/\/ws$/i, '')
        .replace(/\/$/, '')
      if (!stripped) return ''
      const hostOnly = (stripped.split(':')[0] || '')
      if (!hostOnly) return ''
      return `http://${hostOnly}:7788`
    } catch {
      return ''
    }
  }, [bridgeHost])

  // Convex readiness probe (mirrors Settings)
  const convexThreads = (useQuery as any)('threads:list', {}) as any[] | undefined | null
  const [httpStatus, setHttpStatus] = React.useState<string>('')
  React.useEffect(() => {
    let cancelled = false
    const base = String(convexUrl || derivedConvexUrl).trim()
    if (!base) { setHttpStatus(''); return }
    const url = base.replace(/\/$/, '') + '/instance_version'
    try {
      fetch(url).then(r => r.text().then(body => {
        if (cancelled) return
        setHttpStatus(`${r.status} ${body.trim()}`)
      })).catch(() => { if (!cancelled) setHttpStatus('error') })
    } catch { setHttpStatus('error') }
    return () => { cancelled = true }
  }, [convexUrl, derivedConvexUrl])
  const convexStatus = React.useMemo(() => {
    if (convexThreads === undefined) return `connecting (http ${httpStatus || '...'})`
    if (convexThreads === null) return `function missing or error (http ${httpStatus || '...'})`
    return Array.isArray(convexThreads) ? `ok (${convexThreads.length} threads)` : 'ok'
  }, [convexThreads, httpStatus])

  // Auto-advance when connected
  React.useEffect(() => {
    if (!connected) return
    try { router.replace('/thread?focus=1&new=1' as any) } catch {}
  }, [connected])

  return (
    <View style={styles.container}>
      <Text style={styles.title}>OpenAgents</Text>
      <Text style={styles.subtitle}>Enter your Bridge Code to connect to your desktop.</Text>
      <Text style={styles.label}>Bridge Code</Text>
      <View style={styles.inputWrapper}>
        <TextInput
          value={bridgeCode}
          onChangeText={(v) => {
            setBridgeCode(v)
            const trimmed = String(v || '').trim()
            if (!trimmed) {
              try { disconnect() } catch {}
              setBridgeHost('')
              setConvexUrl('')
              return
            }
            const parsed = parseBridgeCode(v)
            if (parsed?.bridgeHost) setBridgeHost(parsed.bridgeHost)
            if (parsed?.convexUrl) setConvexUrl(parsed.convexUrl)
            try { if (parsed?.bridgeHost) connect() } catch {}
          }}
          autoCapitalize='none'
          autoCorrect={false}
          placeholder='paste code here'
          placeholderTextColor={Colors.secondary}
          style={[styles.input, { paddingRight: 44 }]}
        />
        <Pressable onPress={() => { try { disconnect() } catch {}; setBridgeCode(''); setBridgeHost(''); setConvexUrl(''); }} accessibilityLabel='Clear bridge code' style={styles.clearIconArea}>
          <Ionicons name='trash-outline' size={16} color={Colors.secondary} />
        </Pressable>
      </View>
      <Text style={styles.meta}>WS endpoint: {bridgeHost ? `ws://${bridgeHost}/ws` : '(not configured)'}</Text>
      <Text style={styles.meta}>Convex base: {convexUrl || derivedConvexUrl || '(not configured)'}</Text>
      <Text style={styles.meta}>Convex status: {convexStatus || '(not connected)'}</Text>
      <View style={{ height: 12 }} />
      <Pressable onPress={() => { try { connect() } catch {} }} accessibilityRole='button' style={styles.connectBtn}>
        <Text style={styles.connectText}>{connected ? 'Connected' : 'Connect'}</Text>
      </Pressable>
      <View style={{ height: 24 }} />
      <Text style={styles.help}>Tip: On desktop, run cargo bridge, then paste the code above.</Text>
    </View>
  )
}

const styles = StyleSheet.create({
  container: { flex: 1, backgroundColor: Colors.background, padding: 24, alignItems: 'stretch' },
  title: { color: Colors.foreground, fontFamily: Typography.bold, fontSize: 22, marginBottom: 8 },
  subtitle: { color: Colors.secondary, fontFamily: Typography.primary, fontSize: 14, marginBottom: 16 },
  label: { color: Colors.secondary, fontFamily: Typography.bold, fontSize: 12, marginBottom: 4 },
  inputWrapper: { position: 'relative', alignSelf: 'center', width: '100%', maxWidth: 680 },
  input: { borderWidth: 1, borderColor: Colors.border, padding: 12, borderRadius: 0, backgroundColor: Colors.card, color: Colors.foreground, fontFamily: Typography.primary, fontSize: 13, marginBottom: 8 },
  clearIconArea: { position: 'absolute', right: 8, top: 0, bottom: 8, width: 28, alignItems: 'center', justifyContent: 'center' },
  meta: { color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12, marginBottom: 4 },
  connectBtn: { backgroundColor: Colors.quaternary, paddingHorizontal: 16, paddingVertical: 12, alignSelf: 'flex-start' },
  connectText: { color: Colors.foreground, fontFamily: Typography.bold },
  help: { color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12 },
})
