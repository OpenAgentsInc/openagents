import React from 'react'
import { View, Text, TextInput, Pressable, StyleSheet, ActivityIndicator } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { useHeaderTitle } from '@/lib/header-store'
import { useRouter } from 'expo-router'
import { useBridge } from '@/providers/ws'
import { useSettings } from '@/lib/settings-store'
import { parseBridgeCode, normalizeBridgeCodeInput } from '@/lib/pairing'
import { useQuery } from 'convex/react'
import { Ionicons } from '@expo/vector-icons'

export default function Onboarding() {
  useHeaderTitle('Connect')
  const router = useRouter()
  const { bridgeHost, setBridgeHost, connected, connecting, connect, disconnect } = useBridge()
  const bridgeCode = useSettings((s) => s.bridgeCode)
  // Use a local input state to avoid programmatic TextInput updates from store
  const [bridgeCodeInput, setBridgeCodeInput] = React.useState<string>(() => String(bridgeCode || ''))
  const convexUrl = useSettings((s) => s.convexUrl)
  const setConvexUrl = useSettings((s) => s.setConvexUrl)
  const setBridgeToken = useSettings((s) => s.setBridgeToken)
  const [codeError, setCodeError] = React.useState<string>('')

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

  // Convex readiness probe (HTTP only; no WebSocket until connected)
  const [httpStatus, setHttpStatus] = React.useState<string>('')
  React.useEffect(() => {
    let cancelled = false
    const base = String(convexUrl || derivedConvexUrl).trim()
    if (!base) { setHttpStatus(''); return }
    const url = base.replace(/\/$/, '') + '/instance_version'
    try {
      fetch(url).then(r => r.text().then(body => {
        if (cancelled) return
        const msg = `${r.status} ${body.trim()}`
        try { console.log('[onboarding] convex http status:', msg) } catch {}
        setHttpStatus(msg)
      })).catch(() => { if (!cancelled) { try { console.log('[onboarding] convex http status: error') } catch {}; setHttpStatus('error') } })
    } catch { setHttpStatus('error') }
    return () => { cancelled = true }
  }, [convexUrl, derivedConvexUrl])
  // Validate code on change; do not mutate host/convex or connect automatically
  React.useEffect(() => {
    const trimmed = String(bridgeCode || '').trim()
    if (!trimmed) { setCodeError(''); return }
    const parsed = parseBridgeCode(trimmed)
    if (!parsed || !parsed.bridgeHost) setCodeError('Invalid bridge code')
    else setCodeError('')
  }, [bridgeCode])
  React.useEffect(() => {
    if (httpStatus) { try { console.log('[onboarding] httpStatus:', httpStatus) } catch {} }
  }, [httpStatus])

  const hasHost = React.useMemo(() => String(bridgeHost || '').trim().length > 0, [bridgeHost])
  const isConnecting = !!connecting
  const convexThreads = (useQuery as any)(connected ? 'threads:list' : 'threads:list', connected ? {} : 'skip') as any[] | undefined | null
  const convexReady = React.useMemo(() => Array.isArray(convexThreads), [convexThreads])
  const convexLoading = connected && convexThreads === undefined
  const convexError = connected && convexThreads === null
  const statusText = (() => {
    if (isConnecting) return 'Connecting…'
    if (!connected) return hasHost ? 'Disconnected' : 'Enter Bridge Code'
    if (convexLoading) return 'Bridge connected — starting Convex…'
    if (convexError) return 'Bridge connected — Convex unavailable'
    if (convexReady) return 'Connected'
    return 'Disconnected'
  })()

  const trimmedCode = React.useMemo(() => String(bridgeCode || '').trim(), [bridgeCode])
  const likelyCode = React.useMemo(() => {
    if (!trimmedCode) return false
    if (trimmedCode.startsWith('openagents://') || trimmedCode.startsWith('oa://') || trimmedCode.startsWith('{')) {
      try { return !!parseBridgeCode(trimmedCode) } catch { return false }
    }
    // Heuristic: base64url for a JSON object often begins with 'ey'
    if (/^ey[A-Za-z0-9_-]{10,}$/.test(trimmedCode)) return true
    try { return !!parseBridgeCode(trimmedCode) } catch { return false }
  }, [trimmedCode])

  // Auto-advance only when both sides are ready
  React.useEffect(() => {
    if (!connected || !convexReady) return
    try { router.replace('/thread?focus=1&new=1' as any) } catch {}
  }, [connected, convexReady])

  return (
    <View style={styles.container}>
      <View style={styles.statusRow}>
        <Text style={styles.statusText}>{statusText}</Text>
        {(isConnecting || convexLoading) ? (<ActivityIndicator size="small" color={Colors.foreground} />) : null}
      </View>
      <View style={{ height: 16 }} />
      <Text style={styles.label}>Bridge Code</Text>
      <Text style={styles.hint}>Run `npx tricoder@0.2.0` from your desktop</Text>
      <View style={styles.inputWrapper}>
        <TextInput
          value={bridgeCodeInput}
          onChangeText={(v) => {
            const display = normalizeBridgeCodeInput(v)
            setBridgeCodeInput(display)
            const trimmed = String(v || '').trim()
            if (!trimmed) {
              try { disconnect() } catch {}
              setBridgeHost('')
              setConvexUrl('')
              setCodeError('')
              return
            }
            const parsed = parseBridgeCode(display)
            if (!parsed || !parsed.bridgeHost) {
              setCodeError('Invalid bridge code')
              return
            }
            setCodeError('')
            try { if (parsed?.token) setBridgeToken(parsed.token || '') } catch {}
            // Do not auto-connect on input; host/convex will be applied on Connect
          }}
          autoCapitalize='none'
          autoCorrect={false}
          placeholder='paste code here'
          placeholderTextColor={Colors.secondary}
          style={[styles.input, { paddingRight: 44 }]}
        />
        {(() => {
          const hasText = String(bridgeCode || '').trim().length > 0
          return (
            <View style={[styles.clearIconArea, { flexDirection: 'row' }]}>
              <Pressable
                onPress={() => { try { router.push('/scan' as any) } catch {} }}
                accessibilityLabel='Scan QR code'
                style={{ opacity: hasText ? 0 : 1, pointerEvents: hasText ? 'none' as any : 'auto' }}
              >
                <Ionicons name='qr-code-outline' size={16} color={Colors.secondary} />
              </Pressable>
              <Pressable
                onPress={() => { try { disconnect() } catch {}; setBridgeCodeInput(''); setBridgeHost(''); setConvexUrl(''); }}
                accessibilityLabel='Clear bridge code'
                style={{ position: 'absolute', right: 0, opacity: hasText ? 1 : 0, pointerEvents: hasText ? 'auto' as any : 'none' }}
              >
                <Ionicons name='trash-outline' size={16} color={Colors.secondary} />
              </Pressable>
            </View>
          )
        })()}
      </View>
      {!!codeError && (
        <Text style={styles.errorText}>{codeError}</Text>
      )}
      <View style={{ height: 12 }} />
      <Pressable
        onPress={() => {
          if (connecting || codeError || !likelyCode) return
          try {
          const parsed = parseBridgeCode(normalizeBridgeCodeInput(trimmedCode))
          if (parsed?.bridgeHost) setBridgeHost(parsed.bridgeHost)
          if (parsed?.convexUrl) setConvexUrl(parsed.convexUrl)
          if (parsed?.token) setBridgeToken(parsed.token || '')
          } catch {}
          try { connect() } catch {}
        }}
        accessibilityRole='button'
        accessibilityState={{ busy: connecting }}
        style={[styles.connectBtn as any, (connecting ? styles.connectingBtn : undefined) as any, ((codeError || !likelyCode) ? styles.connectDisabled : undefined) as any]}
      >
        <Text style={styles.connectText}>{connected ? 'Connected' : (connecting ? 'Connecting…' : (codeError ? 'Fix Code' : 'Connect'))}</Text>
      </Pressable>
    </View>
  )
}

const styles = StyleSheet.create({
  container: { flex: 1, backgroundColor: Colors.background, padding: 24, alignItems: 'stretch', justifyContent: 'flex-start', paddingTop: 24 },
  title: { color: Colors.foreground, fontFamily: Typography.bold, fontSize: 28, marginBottom: 8, textAlign: 'left' },
  statusRow: { flexDirection: 'row', alignItems: 'center', gap: 8 },
  statusText: { color: Colors.secondary, fontFamily: Typography.bold, fontSize: 16 },
  label: { color: Colors.secondary, fontFamily: Typography.bold, fontSize: 12, marginBottom: 4 },
  hint: { color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12, marginBottom: 8 },
  inputWrapper: { position: 'relative', alignSelf: 'center', width: '100%', maxWidth: 680 },
  input: { borderWidth: 1, borderColor: Colors.border, padding: 12, borderRadius: 0, backgroundColor: Colors.card, color: Colors.foreground, fontFamily: Typography.primary, fontSize: 13, marginBottom: 8 },
  clearIconArea: { position: 'absolute', right: 8, top: 0, bottom: 8, width: 28, alignItems: 'center', justifyContent: 'center' },
  errorText: { color: Colors.danger, fontFamily: Typography.bold, fontSize: 12, marginBottom: 8 },
  connectBtn: { backgroundColor: Colors.quaternary, paddingHorizontal: 16, paddingVertical: 12, alignSelf: 'flex-start' },
  connectingBtn: { backgroundColor: Colors.activePurple },
  connectDisabled: { backgroundColor: Colors.border },
  connectText: { color: Colors.foreground, fontFamily: Typography.bold },
})
