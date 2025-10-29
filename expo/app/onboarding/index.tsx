import React from 'react'
import { View, Text, TextInput, Pressable, StyleSheet, ActivityIndicator } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { useHeaderTitle } from '@/lib/header-store'
import { useRouter } from 'expo-router'
import { useBridge } from '@/providers/ws'
import { useSettings } from '@/lib/settings-store'
// pairing helpers not needed here; IP-only flow
import { Ionicons } from '@expo/vector-icons'
import { useIsDevEnv } from '@/lib/env'
import { TailscalePeers } from '@/components/tailscale-peers'
import { LocalBridgeDiscovery } from '@/components/local-bridge-discovery'

export default function Onboarding() {
  useHeaderTitle('Connect')
  const router = useRouter()
  const { bridgeHost, setBridgeHost, connected, connecting, connect, disconnect } = useBridge()
  const { wsLastClose } = useBridge()
  const HARDCODED_IP = '100.72.151.98'
  const bridgeCode = useSettings((s) => s.bridgeCode)
  // Use a local input state to avoid programmatic TextInput updates from store
  const [bridgeCodeInput, setBridgeCodeInput] = React.useState<string>(() => HARDCODED_IP)
  const setBridgeToken = useSettings((s) => s.setBridgeToken)
  const bridgeToken = useSettings((s) => s.bridgeToken)
  const [codeError, setCodeError] = React.useState<string>('')
  const [inputDisabled, setInputDisabled] = React.useState<boolean>(false)
  // Environment-driven dev mode flag, centralized
  const isDevEnv = useIsDevEnv()

  // Convex removed; no status probe
  // Simplified parser: accept only an IP address and hardcode the bridge port
  const BRIDGE_PORT = 8787
  const parseAnyBridgeInput = React.useCallback((raw: string): { bridgeHost?: string; token?: string | null } | null => {
    try {
      const s = String(raw || '').trim()
      if (!s) return null
      const ipv4 = /^(?:\d{1,3}\.){3}\d{1,3}$/.test(s)
      const ipv6 = /^\[?[A-Fa-f0-9:]+\]?$/.test(s) && s.includes(':')
      if (ipv4) return { bridgeHost: `${s}:${BRIDGE_PORT}` }
      if (ipv6) { const unb = s.replace(/^\[/, '').replace(/\]$/, ''); return { bridgeHost: `[${unb}]:${BRIDGE_PORT}` } }
      return null
    } catch { return null }
  }, [])

  // Validate the current input only; do not mutate host/convex or connect automatically
  React.useEffect(() => {
    const trimmed = String(bridgeCodeInput || '').trim()
    if (!trimmed) { setCodeError(''); return }
    const parsed = parseAnyBridgeInput(trimmed)
    if (!parsed || !parsed.bridgeHost) setCodeError('Enter a valid IP address')
    else setCodeError('')
  }, [bridgeCodeInput, parseAnyBridgeInput])
  // No HTTP status probe in Tinyvex build

  const hasHost = React.useMemo(() => String(bridgeHost || '').trim().length > 0, [bridgeHost])
  const isConnecting = !!connecting
  const convexReady = true
  const statusText = (() => {
    if (isConnecting) return 'Connecting…'
    if (!connected) return hasHost ? 'Disconnected' : 'Enter Bridge Code'
    // No Convex status in this build
    if (convexReady) return 'Connected'
    return 'Disconnected'
  })()

  const lastWsErrorText = React.useMemo(() => {
    if (!wsLastClose || connected || String(bridgeCodeInput || '').trim() === '') return ''
    const code = wsLastClose.code
    const reason = String(wsLastClose.reason || '')
    if (code === 1006 || /refused|ECONNREFUSED/i.test(reason)) return 'Connection refused — is the bridge running and reachable?'
    if (/unauthorized|401/i.test(reason)) return 'Unauthorized — set Bridge Token in Settings.'
    return `WebSocket closed ${code ?? ''}${reason ? `: ${reason}` : ''}`.trim()
  }, [wsLastClose, connected, bridgeCodeInput])

  // Show the exact WS URL we will attempt (helps debugging)
  // No attempted URL display on this screen

  const trimmedCode = React.useMemo(() => String(bridgeCodeInput || '').trim(), [bridgeCodeInput])
  const likelyCode = React.useMemo(() => !!parseAnyBridgeInput(trimmedCode), [trimmedCode, parseAnyBridgeInput])

  // Auto-advance only when both sides are ready
  React.useEffect(() => {
    if (!connected || !convexReady) return
    try { router.replace('/thread?focus=1&new=1' as any) } catch {}
  }, [connected, convexReady])

  return (
    <View style={styles.container}>
      {/* Landing page feel: no explicit status banner */}
      <Text style={styles.label}>Desktop IP</Text>
      {/* Intentionally omit explanatory text here */}
      <View style={styles.inputWrapper}>
        <TextInput
          value={bridgeCodeInput}
          onChangeText={(v) => {
            setBridgeCodeInput(v)
            const trimmed = String(v || '').trim()
            if (!trimmed) {
              try { disconnect() } catch {}
              setBridgeHost('')
              setCodeError('')
              return
            }
            const parsed = parseAnyBridgeInput(trimmed)
            if (!parsed || !parsed.bridgeHost) { setCodeError('Enter a valid IP address'); return }
            setCodeError('')
          }}
          autoCapitalize='none'
          autoCorrect={false}
          keyboardType='numbers-and-punctuation'
          placeholder='Enter IP (e.g., 100.72.151.98)'
          placeholderTextColor={Colors.secondary}
          style={[styles.input, { paddingRight: 44 }]}
        />
        <View style={[styles.clearIconArea, { flexDirection: 'row' }]}>
          <Pressable
            onPress={() => { try { disconnect() } catch {}; setBridgeCodeInput(''); setBridgeHost(''); }}
            accessibilityLabel='Clear bridge host'
            style={{ position: 'absolute', right: 0 }}
          >
            <Ionicons name='trash-outline' size={16} color={Colors.secondary} />
          </Pressable>
        </View>
      </View>
      {!!codeError && (
        <Text style={styles.errorText}>{codeError}</Text>
      )}
      {/* Optional Bridge Token (required by desktop bridge by default) */}
      <Text style={[styles.label, { marginTop: 6 }]}>Bridge Token</Text>
      <View style={styles.inputWrapper}>
        <TextInput
          value={bridgeToken}
          onChangeText={(v) => { try { setBridgeToken(v) } catch {} }}
          autoCapitalize='none'
          autoCorrect={false}
          placeholder='Paste token from ~/.openagents/bridge.json'
          placeholderTextColor={Colors.secondary}
          style={[styles.input]}
        />
      </View>
      <View style={{ height: 12 }} />
      {/* Tailscale LocalAPI peers (Android best-effort) */}
      <TailscalePeers />
      <View style={{ height: 12 }} />
      {/* Local network scan for bridge hosts */}
      <LocalBridgeDiscovery
        onPick={(hp) => {
          try {
            // Update both the input field and the actual bridge host
            const s = String(hp || '').trim()
            let ipOnly = s
            if (s.startsWith('[')) {
              const end = s.indexOf(']')
              ipOnly = end > 1 ? s.slice(1, end) : s
            } else {
              ipOnly = s.split(':')[0] || s
            }
            setBridgeCodeInput(ipOnly)
            setCodeError('')
            setBridgeHost(s)
          } catch {}
        }}
      />
      <View style={{ height: 12 }} />
      <Pressable
        onPress={() => {
          if (connecting || codeError || !likelyCode) return
          try {
          const parsed = parseAnyBridgeInput(trimmedCode)
          if (parsed?.bridgeHost) setBridgeHost(parsed.bridgeHost)
          // Bridge token (if required) can be pasted in Settings
          } catch {}
          try {
            setInputDisabled(true)
            setTimeout(() => setInputDisabled(false), 400)
            connect()
          } catch {}
        }}
        accessibilityRole='button'
        accessibilityState={{ busy: connecting, disabled: connecting || !!codeError || !likelyCode }}
        disabled={connecting || !!codeError || !likelyCode}
        style={[styles.connectBtn as any, (connecting ? styles.connectingBtn : undefined) as any, ((codeError || !likelyCode) ? styles.connectDisabled : undefined) as any]}
      >
        <Text style={styles.connectText}>{connected ? 'Connected' : (connecting ? 'Connecting…' : (codeError ? 'Fix Code' : 'Connect'))}</Text>
      </Pressable>
      {!!lastWsErrorText && !connected && (
        <Text style={[styles.errorText, { marginTop: 8 }]}>
          {lastWsErrorText.includes('WebSocket closed')
            ? `${lastWsErrorText} — check Bridge Token in Settings.`
            : lastWsErrorText}
        </Text>
      )}
      {/* Spacer to push dev tools button to bottom */}
      {isDevEnv ? <View style={{ flex: 1 }} /> : null}
      {/* Dev-only quick link to the component library (env-gated) */}
      {isDevEnv ? (
        <Pressable
          onPress={() => { try { router.push('/library' as any) } catch {} }}
          accessibilityRole='button'
          style={styles.devToolsBtn as any}
        >
          <Text style={styles.devToolsText}>Open Component Library</Text>
        </Pressable>
      ) : null}
      {/* Do not display WebSocket URL on homepage */}
    </View>
  )
}

const styles = StyleSheet.create({
  container: { flex: 1, backgroundColor: Colors.background, padding: 24, alignItems: 'stretch', justifyContent: 'flex-start', paddingTop: 120 },
  title: { color: Colors.foreground, fontFamily: Typography.bold, fontSize: 28, marginBottom: 8, textAlign: 'left' },
  statusRow: { flexDirection: 'row', alignItems: 'center', gap: 8 },
  statusText: { color: Colors.secondary, fontFamily: Typography.bold, fontSize: 16 },
  label: { color: Colors.secondary, fontFamily: Typography.bold, fontSize: 12, marginBottom: 4 },
  hint: { color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12, marginBottom: 8 },
  inputWrapper: { position: 'relative', alignSelf: 'center', width: '100%', maxWidth: 680 },
  input: { borderWidth: 1, borderColor: Colors.border, padding: 12, borderRadius: 0, backgroundColor: Colors.card, color: Colors.foreground, fontFamily: Typography.primary, fontSize: 13, marginBottom: 8 },
  clearIconArea: { position: 'absolute', right: 8, top: 0, bottom: 8, width: 28, alignItems: 'center', justifyContent: 'center' },
  errorText: { color: Colors.danger, fontFamily: Typography.bold, fontSize: 12, marginBottom: 8 },
  connectBtn: { backgroundColor: Colors.success, paddingHorizontal: 18, paddingVertical: 12, alignSelf: 'center', borderWidth: 1, borderColor: Colors.success, width: 240, alignItems: 'center', justifyContent: 'center' },
  connectingBtn: { opacity: 0.75, borderColor: Colors.success },
  connectDisabled: { backgroundColor: Colors.border, borderColor: Colors.border },
  connectText: { color: Colors.foreground, fontFamily: Typography.bold, letterSpacing: 0.5, textAlign: 'center' },
  devToolsBtn: { marginTop: 16, borderWidth: 1, borderColor: Colors.border, backgroundColor: Colors.card, paddingVertical: 12, alignItems: 'center' },
  devToolsText: { color: Colors.secondary, fontFamily: Typography.bold },
})
