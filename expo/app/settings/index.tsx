import React from 'react'
import { View, Text, TextInput, Pressable, StyleSheet } from 'react-native'
import { useBridge } from '@/providers/ws'
import { useSettings } from '@/lib/settings-store'
import { useQuery } from 'convex/react'
import { parseBridgeCode } from '@/lib/pairing'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { useHeaderTitle } from '@/lib/header-store'

export default function SettingsScreen() {
  useHeaderTitle('Settings')
  const { bridgeHost, setBridgeHost, wsUrl, connected, connect, disconnect, attachPreface, setAttachPreface } = useBridge()
  const bridgeCode = useSettings((s) => s.bridgeCode)
  const setBridgeCode = useSettings((s) => s.setBridgeCode)
  const convexUrl = useSettings((s) => s.convexUrl)
  const setConvexUrl = useSettings((s) => s.setConvexUrl)
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
      const hostOnly = (stripped.split(':')[0] || '127.0.0.1')
      return `http://${hostOnly}:7788`
    } catch {
      return 'http://127.0.0.1:7788'
    }
  }, [bridgeHost])
  const convexThreads = (useQuery as any)('threads:list', {}) as any[] | undefined | null
  const convexStatus = React.useMemo(() => {
    if (convexThreads === undefined) return 'connecting'
    if (convexThreads === null) return 'function missing or error'
    return Array.isArray(convexThreads) ? `ok (${convexThreads.length} threads)` : 'ok'
  }, [convexThreads])
  return (
    <View style={styles.container}>
      <Text style={styles.title}>Connection</Text>
      <Text style={styles.label}>Bridge Code (single string)</Text>
      <TextInput
        value={bridgeCode}
        onChangeText={(v) => {
          setBridgeCode(v)
          const parsed = parseBridgeCode(v)
          if (parsed?.bridgeHost) setBridgeHost(parsed.bridgeHost)
          if (parsed?.convexUrl) setConvexUrl(parsed.convexUrl)
          // Auto-connect when a valid host is present
          try { if (parsed?.bridgeHost) connect() } catch {}
        }}
        autoCapitalize='none'
        autoCorrect={false}
        placeholder='paste code here'
        placeholderTextColor={Colors.secondary}
        style={styles.input}
      />
      <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12, marginBottom: 4 }}>WS endpoint: {`ws://${bridgeHost}/ws`}</Text>
      <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12, marginBottom: 4 }}>Convex base: {convexUrl || derivedConvexUrl}</Text>
      <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12, marginBottom: 8 }}>Convex status: {convexStatus}</Text>
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10, marginBottom: 8 }}>
        {!connected ? (<Button title='Connect' onPress={connect} />) : (<Button title='Disconnect' onPress={disconnect} />)}
        <StatusPill connected={connected} />
      </View>
      <Text style={styles.title}>Preferences</Text>
      <Text style={styles.label}>Attach preface to prompts</Text>
      <View style={{ flexDirection: 'row', gap: 8 }}>
        <Segmented title='Off' active={!attachPreface} onPress={() => setAttachPreface(false)} />
        <Segmented title='On' active={attachPreface} onPress={() => setAttachPreface(true)} />
      </View>
    </View>
  )
}

const styles = StyleSheet.create({
  container: { flex: 1, padding: 16, gap: 8, backgroundColor: Colors.background },
  title: { fontSize: 20, fontFamily: Typography.bold, color: Colors.foreground, marginTop: 8 },
  label: { color: Colors.secondary, fontFamily: Typography.bold, fontSize: 12, marginTop: 8 },
  input: { borderWidth: 1, borderColor: Colors.border, padding: 12, borderRadius: 0, backgroundColor: Colors.card, color: Colors.foreground, fontFamily: Typography.primary, fontSize: 13, marginBottom: 8 },
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
