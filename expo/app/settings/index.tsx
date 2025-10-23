import React from 'react'
import { View, Text, TextInput, Pressable, StyleSheet } from 'react-native'
import { useWs } from '@/providers/ws'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { useHeaderTitle } from '@/lib/header-store'

export default function SettingsScreen() {
  useHeaderTitle('Settings')
  const { wsUrl, setWsUrl, connected, connect, disconnect, clearLog, readOnly, setReadOnly, networkEnabled, setNetworkEnabled, approvals, setApprovals, attachPreface, setAttachPreface } = useWs()
  return (
    <View style={styles.container}>
      <Text style={styles.title}>Connection</Text>
      <Text style={styles.label}>WebSocket URL</Text>
      <TextInput value={wsUrl} onChangeText={setWsUrl} autoCapitalize='none' autoCorrect={false} placeholder='ws://localhost:8787/ws' placeholderTextColor={Colors.secondary} style={styles.input} />
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10, marginBottom: 8 }}>
        {!connected ? (<Button title='Connect' onPress={connect} />) : (<Button title='Disconnect' onPress={disconnect} />)}
        <StatusPill connected={connected} />
      </View>
      <Button title='Clear Log' onPress={clearLog} />
      <Text style={styles.title}>Preferences</Text>
      <Text style={styles.label}>Filesystem</Text>
      <View style={{ flexDirection: 'row', gap: 8 }}>
        <Segmented title='Read‑only' active={readOnly} onPress={() => setReadOnly(true)} />
        <Segmented title='Write (workspace)' active={!readOnly} onPress={() => setReadOnly(false)} />
      </View>
      <Text style={styles.label}>Network</Text>
      <View style={{ flexDirection: 'row', gap: 8 }}>
        <Segmented title='Restricted' active={!networkEnabled} onPress={() => setNetworkEnabled(false)} />
        <Segmented title='Enabled' active={networkEnabled} onPress={() => setNetworkEnabled(true)} />
      </View>
      <Text style={styles.label}>Approvals</Text>
      <View style={{ flexDirection: 'row', gap: 8 }}>
        <Segmented title='never' active={approvals==='never'} onPress={() => setApprovals('never')} />
        <Segmented title='on-request' active={approvals==='on-request'} onPress={() => setApprovals('on-request')} />
        <Segmented title='on-failure' active={approvals==='on-failure'} onPress={() => setApprovals('on-failure')} />
      </View>
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

