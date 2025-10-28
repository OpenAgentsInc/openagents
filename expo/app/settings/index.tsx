import React from 'react'
import { View, Text, TextInput, Pressable, StyleSheet } from 'react-native'
import { useBridge } from '@/providers/ws'
import { useSettings } from '@/lib/settings-store'
import { useQuery } from 'convex/react'
import { parseBridgeCode, normalizeBridgeCodeInput } from '@/lib/pairing'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { useHeaderTitle } from '@/lib/header-store'
import { Ionicons } from '@expo/vector-icons'
import { useRouter } from 'expo-router'

export default function SettingsScreen() {
  useHeaderTitle('Settings')
  const router = useRouter()
  const { bridgeHost, setBridgeHost, connected, connect, disconnect, connecting } = useBridge()
  const [inputDisabled, setInputDisabled] = React.useState(false)
  const bridgeCode = useSettings((s) => s.bridgeCode)
  const [bridgeCodeInput, setBridgeCodeInput] = React.useState<string>(() => String(bridgeCode || ''))
  const convexUrl = useSettings((s) => s.convexUrl)
  const setConvexUrl = useSettings((s) => s.setConvexUrl)
  const setBridgeToken = useSettings((s) => s.setBridgeToken)
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
  return (
    <View style={styles.container}>
      <Text style={styles.title}>Connection</Text>
      <Text style={styles.label}>Bridge Code</Text>
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
              return
            }
            const parsed = parseBridgeCode(display)
            if (parsed?.bridgeHost) setBridgeHost(parsed.bridgeHost)
            if (parsed?.convexUrl) setConvexUrl(parsed.convexUrl)
            if (parsed?.token) setBridgeToken(parsed.token || '')
            // Do not auto-connect on input; user must press Connect
          }}
          editable={!connecting && !inputDisabled}
          autoCapitalize='none'
          autoCorrect={false}
          placeholder='paste code here'
          placeholderTextColor={Colors.secondary}
          style={[styles.input, { paddingRight: 44 }]}
        />
        {(() => {
          const hasText = String(bridgeCodeInput || '').trim().length > 0
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
      <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12, marginBottom: 4 }}>WS endpoint: {bridgeHost ? `ws://${bridgeHost}/ws` : '(not configured)'}</Text>
      <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12, marginBottom: 4 }}>Convex base: {convexUrl || derivedConvexUrl || '(not configured)'}</Text>
      <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12, marginBottom: 8 }}>Convex status: {convexStatus}</Text>
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10, marginBottom: 8 }}>
        {!connected ? (<Button title='Connect' onPress={connect} />) : (<Button title='Disconnect' onPress={disconnect} />)}
        <StatusPill connected={connected} />
      </View>
      {/** Preferences hidden: Attach preface defaults to ON and is not user-configurable in this version
      <Text style={styles.title}>Preferences</Text>
      <Text style={styles.label}>Attach preface to prompts</Text>
      <View style={{ flexDirection: 'row', gap: 8 }}>
        <Segmented title='Off' active={!attachPreface} onPress={() => setAttachPreface(false)} />
        <Segmented title='On' active={attachPreface} onPress={() => setAttachPreface(true)} />
      </View>
      */}
    </View>
  )
}

const styles = StyleSheet.create({
  container: { flex: 1, padding: 16, gap: 8, backgroundColor: Colors.background },
  title: { fontSize: 20, fontFamily: Typography.bold, color: Colors.foreground, marginTop: 8 },
  label: { color: Colors.secondary, fontFamily: Typography.bold, fontSize: 12, marginTop: 8 },
  inputWrapper: { position: 'relative', alignSelf: 'center', width: '100%', maxWidth: 680 },
  input: { borderWidth: 1, borderColor: Colors.border, padding: 12, borderRadius: 0, backgroundColor: Colors.card, color: Colors.foreground, fontFamily: Typography.primary, fontSize: 13, marginBottom: 8 },
  clearIconArea: { position: 'absolute', right: 8, top: 0, bottom: 8, width: 28, alignItems: 'center', justifyContent: 'center' },
});

function Button({ title, onPress }: { title: string; onPress: () => void }) {
  return (
    <Pressable onPress={onPress} style={{ backgroundColor: Colors.quaternary, paddingHorizontal: 16, paddingVertical: 12, borderRadius: 0 }}>
      <Text style={{ color: Colors.foreground, fontFamily: Typography.bold }}>{title}</Text>
    </Pressable>
  )
}

function Segmented({ title, active, onPress }: { title: string; active: boolean; onPress: () => void }) {
  const base = { paddingHorizontal: 10, paddingVertical: 8, borderWidth: 1, borderColor: Colors.border }
  const activeStyle = { backgroundColor: Colors.quaternary }
  const text = { color: Colors.secondary, fontFamily: Typography.primary }
  const textActive = { color: Colors.foreground, fontFamily: Typography.bold }
  return (
    <Pressable onPress={onPress} style={[base as any, active && activeStyle as any]}>
      <Text style={[text as any, active && textActive as any]}>{title}</Text>
    </Pressable>
  )
}

function StatusPill({ connected }: { connected: boolean }) {
  return (
    <View style={{ paddingHorizontal: 10, paddingVertical: 6, borderRadius: 0, borderWidth: 1, borderColor: connected ? Colors.gray : Colors.border, backgroundColor: Colors.card }}>
      <Text style={{ color: Colors.secondary, fontSize: 12, fontFamily: Typography.bold }}>
        {connected ? 'Connected' : 'Disconnected'}
      </Text>
    </View>
  )
}
