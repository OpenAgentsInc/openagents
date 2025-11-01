import React from 'react'
import { View, Text, TextInput, Pressable, StyleSheet, ActivityIndicator } from 'react-native'
import { useRouter } from 'expo-router'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { normalizeBridgeCodeInput, parseBridgeCode } from '@/lib/pairing'
import { useBridge } from '@/providers/ws'
import { useSettings } from '@/lib/settings-store'

export default function ManualCodeScreen() {
  const router = useRouter()
  const { connect, connecting, setBridgeHost } = useBridge()
  const setBridgeToken = useSettings((s) => s.setBridgeToken)
  const setBridgeCode = useSettings((s) => s.setBridgeCode)

  const [raw, setRaw] = React.useState<string>('')
  const [display, setDisplay] = React.useState<string>('')
  const [error, setError] = React.useState<string>('')

  // Re-validate whenever display changes
  React.useEffect(() => {
    const trimmed = String(display || '').trim()
    if (!trimmed) { setError(''); return }
    const parsed = parseBridgeCode(trimmed)
    if (!parsed || !parsed.bridgeHost) { setError('Enter a valid pairing code or link') }
    else { setError('') }
  }, [display])

  const canConnect = React.useMemo(() => {
    if (!display || !!error) return false
    const parsed = parseBridgeCode(display)
    return !!(parsed && parsed.bridgeHost)
  }, [display, error])

  const onConnect = React.useCallback(() => {
    if (!canConnect || connecting) return
    try { setBridgeCode(display) } catch {}
    const parsed = parseBridgeCode(display)
    if (!parsed || !parsed.bridgeHost) { setError('Unrecognized code') ; return }
    try { if (parsed.bridgeHost) setBridgeHost(parsed.bridgeHost) } catch {}
    try { if (parsed.token) setBridgeToken(parsed.token || '') } catch {}
    try { connect() } catch {}
    try { router.replace('/thread/new' as any) } catch {}
  }, [canConnect, connecting, display])

  return (
    <View style={styles.container}>
      <Text style={styles.title}>Enter pairing code</Text>
      <Text style={styles.hint}>Paste pairing code or link</Text>
      <View style={styles.inputWrapper}>
        <TextInput
          value={raw}
          onChangeText={(v) => {
            setRaw(v)
            try { setDisplay(normalizeBridgeCodeInput(v)) } catch { setDisplay(String(v || '').trim()) }
          }}
          autoCapitalize='none'
          autoCorrect={false}
          placeholder='Paste pairing code or link'
          placeholderTextColor={Colors.secondary}
          style={styles.input}
        />
      </View>
      {!!error && <Text style={styles.error}>{error}</Text>}
      <View style={{ height: 12 }} />
      <Pressable
        onPress={onConnect}
        accessibilityRole='button'
        accessibilityState={{ busy: connecting, disabled: connecting || !canConnect }}
        disabled={connecting || !canConnect}
        style={[styles.connectBtn as any, (connecting || !canConnect) ? styles.connectDisabled : undefined]}
      >
        {connecting ? (
          <>
            <ActivityIndicator color={Colors.foreground} />
            <Text style={[styles.connectText, { marginTop: 8 }]}>Connecting…</Text>
          </>
        ) : (
          <Text style={styles.connectText}>Connect</Text>
        )}
      </Pressable>
      <Pressable onPress={() => { try { router.back() } catch {} }} accessibilityRole='button' style={{ marginTop: 16 }}>
        <Text style={{ color: Colors.secondary, fontFamily: Typography.bold }}>Cancel</Text>
      </Pressable>
    </View>
  )
}

const styles = StyleSheet.create({
  container: { flex: 1, backgroundColor: Colors.black, padding: 24, alignItems: 'center', justifyContent: 'center' },
  title: { color: Colors.foreground, fontFamily: Typography.bold, fontSize: 22, marginBottom: 6 },
  hint: { color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12, marginBottom: 16 },
  inputWrapper: { width: '100%', maxWidth: 680 },
  input: { borderWidth: 1, borderColor: Colors.border, padding: 12, borderRadius: 0, backgroundColor: Colors.card, color: Colors.foreground, fontFamily: Typography.primary, fontSize: 13 },
  error: { color: Colors.danger, fontFamily: Typography.bold, fontSize: 12, marginTop: 8 },
  connectBtn: { marginTop: 12, backgroundColor: Colors.success, paddingHorizontal: 18, paddingVertical: 12, alignSelf: 'center', borderWidth: 1, borderColor: Colors.success, width: 240, alignItems: 'center', justifyContent: 'center' },
  connectDisabled: { backgroundColor: Colors.border, borderColor: Colors.border },
  connectText: { color: Colors.foreground, fontFamily: Typography.bold, letterSpacing: 0.5, textAlign: 'center' },
})

