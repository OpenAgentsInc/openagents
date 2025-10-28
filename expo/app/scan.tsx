import React from 'react'
import { View, Text, StyleSheet, ActivityIndicator, Pressable } from 'react-native'
import { BarCodeScanner } from 'expo-barcode-scanner'
import { useRouter } from 'expo-router'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { parseBridgeCode } from '@/lib/pairing'
import { useBridge } from '@/providers/ws'
import { useSettings } from '@/lib/settings-store'

export default function ScanScreen() {
  const router = useRouter()
  const { connect, setBridgeHost } = useBridge()
  const setBridgeCode = useSettings((s) => s.setBridgeCode)
  const setConvexUrl = useSettings((s) => s.setConvexUrl)
  const setBridgeToken = useSettings((s) => s.setBridgeToken)
  const [hasPermission, setHasPermission] = React.useState<boolean | null>(null)
  const [scanned, setScanned] = React.useState(false)

  React.useEffect(() => {
    let cancelled = false
    BarCodeScanner.requestPermissionsAsync().then(({ status }) => {
      if (!cancelled) setHasPermission(status === 'granted')
    }).catch(() => { if (!cancelled) setHasPermission(false) })
    return () => { cancelled = true }
  }, [])

  const handleData = React.useCallback((raw: string) => {
    const trimmed = String(raw || '').trim()
    const parsed = parseBridgeCode(trimmed)
    if (!parsed) return false
    try { setBridgeCode(trimmed) } catch {}
    try { if (parsed.bridgeHost) setBridgeHost(parsed.bridgeHost) } catch {}
    try { if (parsed.convexUrl) setConvexUrl(parsed.convexUrl) } catch {}
    try { if (parsed.token) setBridgeToken(parsed.token || '') } catch {}
    try { connect() } catch {}
    try { router.replace('/onboarding' as any) } catch {}
    return true
  }, [connect, router])

  if (hasPermission === null) {
    return (
      <View style={styles.center}>
        <ActivityIndicator size="large" color={Colors.foreground} />
        <Text style={styles.hint}>Requesting camera permission…</Text>
      </View>
    )
  }
  if (hasPermission === false) {
    return (
      <View style={styles.center}>
        <Text style={styles.error}>Camera permission denied</Text>
        <Text style={styles.hint}>Enable camera access in Settings to scan a code.</Text>
        <Pressable onPress={() => router.back()} style={styles.btn}><Text style={styles.btnText}>Go back</Text></Pressable>
      </View>
    )
  }

  return (
    <View style={{ flex: 1, backgroundColor: '#000' }}>
      <BarCodeScanner
        onBarCodeScanned={scanned ? undefined : ({ data }) => {
          setScanned(true)
          const ok = handleData(String(data || ''))
          if (!ok) {
            // Allow another try after brief pause
            setTimeout(() => setScanned(false), 800)
          }
        }}
        style={{ flex: 1 }}
      />
      <View style={styles.overlay}>
        <Text style={styles.overlayText}>Align the QR in the frame to connect</Text>
        <Pressable onPress={() => router.back()} style={styles.overlayBtn}><Text style={styles.overlayBtnText}>Cancel</Text></Pressable>
      </View>
    </View>
  )
}

const styles = StyleSheet.create({
  center: { flex: 1, backgroundColor: Colors.background, alignItems: 'center', justifyContent: 'center', padding: 24 },
  error: { color: Colors.danger, fontFamily: Typography.bold, fontSize: 16, marginBottom: 8 },
  hint: { color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12, marginTop: 8 },
  btn: { marginTop: 16, backgroundColor: Colors.quaternary, paddingHorizontal: 16, paddingVertical: 12 },
  btnText: { color: Colors.foreground, fontFamily: Typography.bold },
  overlay: { position: 'absolute', left: 0, right: 0, bottom: 0, padding: 16, alignItems: 'center', gap: 8 },
  overlayText: { color: '#fff', fontFamily: Typography.bold, fontSize: 14 },
  overlayBtn: { backgroundColor: 'rgba(0,0,0,0.6)', paddingHorizontal: 16, paddingVertical: 10 },
  overlayBtnText: { color: '#fff', fontFamily: Typography.bold },
})
