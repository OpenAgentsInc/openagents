import React from 'react'
import { View, Text, StyleSheet, ActivityIndicator, Pressable, InteractionManager } from 'react-native'
import { CameraView, useCameraPermissions } from 'expo-camera'
import { useRouter } from 'expo-router'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { parseBridgeCode, normalizeBridgeCodeInput } from '@/lib/pairing'
import { useBridge } from '@/providers/ws'
import { useSettings } from '@/lib/settings-store'

export default function ScanScreen() {
  const router = useRouter()
  const { connect, setBridgeHost } = useBridge()
  // Avoid writing Bridge Code into the input to prevent UIKit text churn on navigation
  const setBridgeToken = useSettings((s) => s.setBridgeToken)
  const camRef = React.useRef<any>(null)
  const [permission, requestPermission] = useCameraPermissions()
  const [scanned, setScanned] = React.useState(false)
  const [cameraPaused, setCameraPaused] = React.useState(false)
  const [usingModal, setUsingModal] = React.useState<boolean>(false)
  const supportsModal = React.useMemo(() => {
    try {
      const anyCam: any = CameraView as any
      return !!(anyCam && anyCam.isModernBarcodeScannerAvailable && typeof anyCam.launchScanner === 'function' && typeof anyCam.onModernBarcodeScanned === 'function' && typeof anyCam.dismissScanner === 'function')
    } catch { return false }
  }, [])

  React.useEffect(() => {
    setUsingModal(supportsModal)
    if (!permission) { requestPermission().catch(() => {}) }
  }, [supportsModal])

  const handleData = React.useCallback((raw: string) => {
    const display = normalizeBridgeCodeInput(String(raw || ''))
    const parsed = parseBridgeCode(display)
    if (!parsed) return false
    // Set host/token and connect immediately without additional UI steps
    try { if (parsed.bridgeHost) setBridgeHost(parsed.bridgeHost) } catch {}
    try { if (parsed.token) setBridgeToken(parsed.token || '') } catch {}
    try { connect() } catch {}
    // Navigate straight to new thread; layout gating now allows this while connecting
    try { router.replace('/thread/new' as any) } catch {}
    return true
  }, [connect, router])

  // Modal scanner effect must not be declared conditionally; gate inside
  React.useEffect(() => {
    if (!usingModal) return
    let cancelled = false
    let sub: any = null
    const run = async () => {
      try {
        const camAny: any = CameraView as any
        await camAny.launchScanner?.({})
        sub = camAny.onModernBarcodeScanned?.(async (evt: any) => {
          if (cancelled || scanned) return
          setScanned(true)
          const data = String((evt && (evt.data || evt.rawValue || evt.text)) || '')
          const ok = handleData(data)
          try { camAny.dismissScanner?.() } catch {}
          // Give the modal a moment to fully dismiss before navigation/render churn
          await new Promise((r) => setTimeout(r, 150))
          if (!ok) {
            setTimeout(async () => {
              setScanned(false)
              try { await camAny.launchScanner?.({}) } catch {}
            }, 400)
          }
        })
      } catch {}
    }
    run()
    return () => { cancelled = true; try { (CameraView as any).dismissScanner?.() } catch {}; try { sub && sub.remove && sub.remove() } catch {} }
  }, [usingModal, scanned, handleData])
  if (usingModal) {
    return (
      <View style={styles.center}>
        <ActivityIndicator size="large" color={Colors.foreground} />
        <Text style={styles.hint}>Opening scanner…</Text>
      </View>
    )
  }
  if (!permission) {
    return (
      <View style={styles.center}>
        <ActivityIndicator size="large" color={Colors.foreground} />
        <Text style={styles.hint}>Requesting camera permission…</Text>
      </View>
    )
  }
  if (!permission.granted) {
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
      <CameraView
        ref={camRef}
        barcodeScannerSettings={{ barcodeTypes: ['qr'] as any }}
        onBarcodeScanned={(scanned || cameraPaused) ? undefined : async ({ data }: any) => {
          setScanned(true)
          // Immediately pause the camera preview to stop capture and free resources
          try { await camRef.current?.pausePreview?.(); setCameraPaused(true) } catch {}
          const ok = handleData(String(data || ''))
          if (!ok) {
            // Allow another try after brief pause
            setTimeout(async () => { try { await camRef.current?.resumePreview?.(); } catch {}; setCameraPaused(false); setScanned(false) }, 800)
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
