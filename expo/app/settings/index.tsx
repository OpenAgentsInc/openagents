import React from 'react'
import { View, Text, Pressable, StyleSheet } from 'react-native'
import { useBridge } from '@/providers/ws'
import { useSettings } from '@/lib/settings-store'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { useHeaderTitle } from '@/lib/header-store'
import type { SyncStatusTs } from '@/types/bridge/SyncStatusTs'
// removed QR/Icons in simplified settings

export default function SettingsScreen() {
  useHeaderTitle('Settings')
  const { bridgeHost, setBridgeHost, connected, connect, disconnect, addSubscriber, send } = useBridge()
  const [hostInput, setHostInput] = React.useState<string>(() => String(bridgeHost || ''))
  const bridgeToken = useSettings((s) => s.bridgeToken) // retained for future, not rendered
  const setBridgeToken = useSettings((s) => s.setBridgeToken) // retained for future, not rendered
  const updatesAutoPoll = useSettings((s) => s.updatesAutoPoll)
  const setUpdatesAutoPoll = useSettings((s) => s.setUpdatesAutoPoll)
  // Convex removed
  const syncEnabled = useSettings((s) => s.syncEnabled)
  const setSyncEnabled = useSettings((s) => s.setSyncEnabled)
  const twoWay = useSettings((s) => s.syncTwoWay)
  const setTwoWay = useSettings((s) => s.setSyncTwoWay)
  // Remote status mirrors (do not override persisted preferences)
  const [statusEnabled, setStatusEnabled] = React.useState<boolean | null>(null)
  const [statusTwoWay, setStatusTwoWay] = React.useState<boolean | null>(null)
  const [syncFiles, setSyncFiles] = React.useState<number>(0)
  const [syncBase, setSyncBase] = React.useState<string>('')
  const [syncLastRead, setSyncLastRead] = React.useState<number>(0)

  // Subscribe to bridge.sync_status updates while the screen is mounted
  React.useEffect(() => {
    const unsub = addSubscriber((raw) => {
      if (!raw || raw[0] !== '{') return
      try {
        const obj = JSON.parse(raw) as { type?: string } & { enabled?: boolean; twoWay?: boolean; watched?: any[] }
        if (obj?.type === 'bridge.sync_status') {
          const s = obj as unknown as SyncStatusTs & { type: 'bridge.sync_status' }
          setStatusEnabled(!!s.enabled)
          setStatusTwoWay(!!s.two_way)
          const w = Array.isArray(s.watched) && s.watched[0] ? s.watched[0] : null
          setSyncFiles(Number((w?.files as any) || 0))
          setSyncBase(String(w?.base || ''))
          setSyncLastRead(Number((w?.last_read as any) || 0))
        }
      } catch {}
    })
    return () => { try { unsub() } catch {} }
  }, [addSubscriber])

  const refreshSyncStatus = React.useCallback(() => {
    try { send(JSON.stringify({ control: 'sync.status' })) } catch {}
  }, [send])

  React.useEffect(() => {
    if (connected) {
      refreshSyncStatus()
    }
  }, [connected, refreshSyncStatus])

  const toggleSync = React.useCallback(() => {
    const next = !syncEnabled
    try { setSyncEnabled(next) } catch {}
    try { send(JSON.stringify({ control: 'sync.enable', enabled: next })) } catch {}
    setTimeout(refreshSyncStatus, 200)
  }, [syncEnabled, setSyncEnabled, send, refreshSyncStatus])

  const toggleTwoWay = React.useCallback(() => {
    const next = !twoWay
    try { setTwoWay(next) } catch {}
    try { send(JSON.stringify({ control: 'sync.two_way', enabled: next })) } catch {}
    setTimeout(refreshSyncStatus, 200)
  }, [twoWay, setTwoWay, send, refreshSyncStatus])

  const fullRescan = React.useCallback(() => {
    try { send(JSON.stringify({ control: 'sync.full_rescan' })) } catch {}
    setTimeout(refreshSyncStatus, 400)
  }, [send, refreshSyncStatus])
  return (
    <View style={styles.container}>
      <Text style={styles.title}>Connection</Text>
      {/* Bridge Host / Token / Endpoint are intentionally hidden to simplify the Settings page UI. */}
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10, marginBottom: 8 }}>
        {!connected ? (
          <Button title='Connect' onPress={() => { try { setBridgeHost(hostInput.trim()) } catch {}; connect() }} />
        ) : (
          <Button title='Disconnect' onPress={disconnect} />
        )}
        <StatusPill connected={connected} />
      </View>
      <View style={{ height: 16 }} />
      <Text style={styles.title}>Sync</Text>
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10, marginTop: 4 }}>
        <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>Sessions Watcher</Text>
        <Segmented title={syncEnabled ? 'On' : 'Off'} active={syncEnabled} onPress={toggleSync} />
      </View>
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10, marginTop: 8 }}>
        <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>Two‑way Writer</Text>
        <Segmented title={twoWay ? 'On' : 'Off'} active={twoWay} onPress={toggleTwoWay} />
      </View>
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10, marginTop: 8 }}>
        <Button title='Full Rescan' onPress={fullRescan} />
        <Button title='Refresh' onPress={refreshSyncStatus} />
      </View>
      {syncBase ? (
        <View style={{ marginTop: 8 }}>
          <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12 }}>
            Watching: {syncBase}
          </Text>
          <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12 }}>
            Files: {syncFiles}  Last read: {syncLastRead ? new Date(syncLastRead).toLocaleString() : '—'}
          </Text>
          {statusEnabled !== null || statusTwoWay !== null ? (
            <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12 }}>
              Bridge status — Sync: {String(statusEnabled)}  Two‑way: {String(statusTwoWay)}
            </Text>
          ) : null}
        </View>
      ) : null}
      <View style={{ height: 16 }} />
      <Text style={styles.title}>Updates</Text>
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10 }}>
        <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>Auto-check every 5s</Text>
        <Segmented title={updatesAutoPoll ? 'On' : 'Off'} active={updatesAutoPoll} onPress={() => { try { setUpdatesAutoPoll(!updatesAutoPoll) } catch {} }} />
      </View>
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
    <Pressable onPress={onPress} style={[base, active ? activeStyle : undefined]}>
      <Text style={[text, active ? textActive : undefined]}>{title}</Text>
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
