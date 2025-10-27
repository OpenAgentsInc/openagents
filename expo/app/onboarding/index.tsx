import React from 'react'
import { View, Text, TextInput, Pressable, StyleSheet, ActivityIndicator } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { useHeaderTitle } from '@/lib/header-store'
import { useRouter } from 'expo-router'
import { useBridge } from '@/providers/ws'
import { useSettings } from '@/lib/settings-store'
import { parseBridgeCode } from '@/lib/pairing'
import { Ionicons } from '@expo/vector-icons'

export default function Onboarding() {
  useHeaderTitle('Connect')
  const router = useRouter()
  const { bridgeHost, setBridgeHost, connected, connecting, connect, disconnect } = useBridge()
  const bridgeCode = useSettings((s) => s.bridgeCode)
  const setBridgeCode = useSettings((s) => s.setBridgeCode)
  const convexUrl = useSettings((s) => s.convexUrl)
  const setConvexUrl = useSettings((s) => s.setConvexUrl)
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
  // Validate code on mount and whenever it changes to avoid flicker
  React.useEffect(() => {
    const trimmed = String(bridgeCode || '').trim()
    if (!trimmed) { setCodeError(''); return }
    const parsed = parseBridgeCode(trimmed)
    if (!parsed || !parsed.bridgeHost) {
      setCodeError('Invalid bridge code')
      try { disconnect() } catch {}
      setBridgeHost('')
      setConvexUrl('')
    } else {
      setCodeError('')
    }
  }, [bridgeCode])
  React.useEffect(() => {
    if (httpStatus) { try { console.log('[onboarding] httpStatus:', httpStatus) } catch {} }
  }, [httpStatus])

  const hasHost = React.useMemo(() => String(bridgeHost || '').trim().length > 0, [bridgeHost])
  const isConnecting = !!connecting
  const statusText = connected ? 'Connected' : (isConnecting ? 'Connecting…' : (hasHost ? 'Disconnected' : 'Enter Bridge Code'))

  // Auto-advance when connected
  React.useEffect(() => {
    if (!connected) return
    try { router.replace('/thread?focus=1&new=1' as any) } catch {}
  }, [connected])

  return (
    <View style={styles.container}>
      {isConnecting ? (<>
        <ActivityIndicator size="large" color={Colors.foreground} />
        <View style={{ height: 16 }} />
      </>) : null}
      <Text style={styles.title}>{statusText}</Text>
      <View style={{ height: 24 }} />
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
              setCodeError('')
              return
            }
            const parsed = parseBridgeCode(v)
            if (!parsed || !parsed.bridgeHost) {
              setCodeError('Invalid bridge code')
              try { disconnect() } catch {}
              setBridgeHost('')
              setConvexUrl('')
              return
            }
            setCodeError('')
            if (parsed?.bridgeHost) setBridgeHost(parsed.bridgeHost)
            if (parsed?.convexUrl) setConvexUrl(parsed.convexUrl)
            try { connect() } catch {}
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
      {!!codeError && (
        <Text style={styles.errorText}>{codeError}</Text>
      )}
      <View style={{ height: 12 }} />
      <Pressable
        onPress={() => { if (!connecting && !codeError) { try { connect() } catch {} } }}
        accessibilityRole='button'
        accessibilityState={{ busy: connecting }}
        style={[styles.connectBtn as any, (connecting ? styles.connectingBtn : undefined) as any, (codeError ? styles.connectDisabled : undefined) as any]}
      >
        <Text style={styles.connectText}>{connected ? 'Connected' : (connecting ? 'Connecting…' : (codeError ? 'Fix Code' : 'Connect'))}</Text>
      </Pressable>
    </View>
  )
}

const styles = StyleSheet.create({
  container: { flex: 1, backgroundColor: Colors.background, padding: 24, alignItems: 'stretch', justifyContent: 'flex-start', paddingTop: 24 },
  title: { color: Colors.foreground, fontFamily: Typography.bold, fontSize: 28, marginBottom: 8, textAlign: 'left' },
  label: { color: Colors.secondary, fontFamily: Typography.bold, fontSize: 12, marginBottom: 4 },
  inputWrapper: { position: 'relative', alignSelf: 'center', width: '100%', maxWidth: 680 },
  input: { borderWidth: 1, borderColor: Colors.border, padding: 12, borderRadius: 0, backgroundColor: Colors.card, color: Colors.foreground, fontFamily: Typography.primary, fontSize: 13, marginBottom: 8 },
  clearIconArea: { position: 'absolute', right: 8, top: 0, bottom: 8, width: 28, alignItems: 'center', justifyContent: 'center' },
  errorText: { color: Colors.danger, fontFamily: Typography.bold, fontSize: 12, marginBottom: 8 },
  connectBtn: { backgroundColor: Colors.quaternary, paddingHorizontal: 16, paddingVertical: 12, alignSelf: 'flex-start' },
  connectingBtn: { backgroundColor: Colors.activePurple },
  connectDisabled: { backgroundColor: Colors.border },
  connectText: { color: Colors.foreground, fontFamily: Typography.bold },
})
