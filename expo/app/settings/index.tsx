import React from 'react'
import { View, Text, TextInput, Pressable, StyleSheet } from 'react-native'
import { useBridge } from '@/providers/ws'
import { useSettings } from '@/lib/settings-store'
import { useQuery } from 'convex/react'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { useHeaderTitle } from '@/lib/header-store'
import { Ionicons } from '@expo/vector-icons'
import { useRouter } from 'expo-router'

export default function SettingsScreen() {
  useHeaderTitle('Settings')
  const router = useRouter()
  const { bridgeHost, setBridgeHost, wsUrl, connected, connect, disconnect, connecting, wsLastClose } = useBridge()
  const [inputDisabled, setInputDisabled] = React.useState(false)
  const convexUrl = useSettings((s) => s.convexUrl)
  const setConvexUrl = useSettings((s) => s.setConvexUrl)
  const bridgeToken = useSettings((s) => s.bridgeToken)
  const setBridgeToken = useSettings((s) => s.setBridgeToken)
  const [hostInput, setHostInput] = React.useState<string>(() => String(bridgeHost || ''))
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
      <Text style={styles.label}>Bridge Host</Text>
      <View style={styles.inputWrapper}>
        <TextInput
          value={hostInput}
          onChangeText={(v) => { setHostInput(v) }}
          editable={!connecting && !inputDisabled}
          autoCapitalize='none'
          autoCorrect={false}
          placeholder='100.72.151.98:8787'
          placeholderTextColor={Colors.secondary}
          style={[styles.input]}
        />
      </View>
      <Text style={styles.label}>Bridge Token</Text>
      <View style={styles.inputWrapper}>
        <TextInput
          value={bridgeToken}
          onChangeText={(v) => { setBridgeToken(v) }}
          editable={!connecting && !inputDisabled}
          autoCapitalize='none'
          autoCorrect={false}
          placeholder='paste token (optional)'
          placeholderTextColor={Colors.secondary}
          style={[styles.input]}
        />
      </View>
      <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12, marginBottom: 4 }}>WS endpoint: {wsUrl || '(not configured)'}</Text>
      <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12, marginBottom: 4 }}>Convex base: {derivedConvexUrl || '(derived from host)'}</Text>
      <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12, marginBottom: 8 }}>Convex status: {convexStatus}</Text>
      {wsLastClose && !connected ? (
        <Text style={{ color: Colors.danger, fontFamily: Typography.bold, fontSize: 12, marginBottom: 8 }}>
          {(() => {
            const code = wsLastClose.code
            const reason = String(wsLastClose.reason || '')
            if (code === 1006 || /refused|ECONNREFUSED/i.test(reason)) return 'WS: Connection refused — is the bridge running and reachable?'
            if (/unauthorized|401/i.test(reason)) return 'WS: Unauthorized — set Bridge Token.'
            return `WS closed ${code ?? ''}${reason ? `: ${reason}` : ''}`.trim()
          })()}
        </Text>
      ) : null}
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10, marginBottom: 8 }}>
        {!connected ? (
          <Button title='Connect' onPress={() => { try { setBridgeHost(hostInput.trim()) } catch {}; try { setConvexUrl('') } catch {}; connect() }} />
        ) : (
          <Button title='Disconnect' onPress={disconnect} />
        )}
        <StatusPill connected={connected} />
      </View>
      {/* Preferences and QR code features removed for now */}
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
