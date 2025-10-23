import { StyleSheet, View, Text, TextInput, Pressable } from 'react-native';
import { Colors } from '@/constants/theme';
import { Typography } from '@/constants/typography';
import { useWs } from '@/providers/ws';
import { useMemo } from 'react';

export default function SettingsScreen() {
  const { wsUrl, setWsUrl, connected, connect, disconnect, readOnly, setReadOnly, networkEnabled, setNetworkEnabled, approvals, setApprovals, attachPreface, setAttachPreface, clearLog } = useWs();
  const seg = useMemo(() => ({
    base: {
      paddingVertical: 8,
      paddingHorizontal: 12,
      borderRadius: 0,
      borderWidth: 1,
      borderColor: Colors.border,
      backgroundColor: Colors.card,
    },
    active: { backgroundColor: '#3F3F46', borderColor: '#4B5563' },
    text: { color: Colors.textSecondary, fontFamily: Typography.primary, fontSize: 12 },
    textActive: { color: '#fff', fontFamily: Typography.bold },
  }), []);
  return (
    <View style={styles.container}>
      <Text style={styles.title}>Connection</Text>
      <Text style={styles.label}>WebSocket URL</Text>
      <TextInput
        value={wsUrl}
        onChangeText={setWsUrl}
        autoCapitalize="none"
        autoCorrect={false}
        placeholder="ws://localhost:8787/ws"
        placeholderTextColor={Colors.textSecondary}
        style={styles.input}
      />
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10 }}>
        {!connected ? (
          <Button title="Connect" onPress={connect} />
        ) : (
          <Button title="Disconnect" onPress={disconnect} />
        )}
        <StatusPill connected={connected} />
      </View>

      <View style={{ height: 8 }} />
      <Button title="Clear Log" onPress={clearLog} />

      <View style={{ height: 16 }} />
      <Text style={styles.title}>Permissions</Text>
      <Text style={styles.label}>Filesystem</Text>
      <View style={{ flexDirection: 'row', gap: 8 }}>
        <Segmented title="Read‑only" active={readOnly} onPress={() => setReadOnly(true)} seg={seg} />
        <Segmented title="Write (workspace)" active={!readOnly} onPress={() => setReadOnly(false)} seg={seg} />
      </View>

      <Text style={styles.label}>Network</Text>
      <View style={{ flexDirection: 'row', gap: 8 }}>
        <Segmented title="Restricted" active={!networkEnabled} onPress={() => setNetworkEnabled(false)} seg={seg} />
        <Segmented title="Enabled" active={networkEnabled} onPress={() => setNetworkEnabled(true)} seg={seg} />
      </View>

      <Text style={styles.label}>Approvals</Text>
      <View style={{ flexDirection: 'row', gap: 8 }}>
        <Segmented title="never" active={approvals === 'never'} onPress={() => setApprovals('never')} seg={seg} />
        <Segmented title="on-request" active={approvals === 'on-request'} onPress={() => setApprovals('on-request')} seg={seg} />
        <Segmented title="on-failure" active={approvals === 'on-failure'} onPress={() => setApprovals('on-failure')} seg={seg} />
      </View>

      <Text style={styles.label}>Attach environment preface to prompts</Text>
      <View style={{ flexDirection: 'row', gap: 8 }}>
        <Segmented title="Off" active={!attachPreface} onPress={() => setAttachPreface(false)} seg={seg} />
        <Segmented title="On" active={attachPreface} onPress={() => setAttachPreface(true)} seg={seg} />
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    padding: 16,
    gap: 8,
    backgroundColor: Colors.background,
  },
  title: {
    fontSize: 20,
    fontFamily: Typography.bold,
    color: Colors.textPrimary,
  },
  label: {
    color: Colors.textSecondary,
    fontFamily: Typography.bold,
    fontSize: 12,
    marginTop: 8,
  },
  input: {
    borderWidth: 1,
    borderColor: Colors.border,
    padding: 12,
    borderRadius: 0,
    backgroundColor: Colors.card,
    color: Colors.textPrimary,
    fontFamily: Typography.primary,
    fontSize: 13,
  },
  body: {
    color: Colors.textSecondary,
    fontFamily: Typography.primary,
  },
});

function Button({ title, onPress }: { title: string; onPress: () => void }) {
  return (
    <Pressable onPress={onPress} style={{ backgroundColor: '#3F3F46', paddingHorizontal: 16, paddingVertical: 12, borderRadius: 0 }}>
      <Text style={{ color: '#fff', fontFamily: Typography.bold }}>{title}</Text>
    </Pressable>
  );
}

function Segmented({ title, active, onPress, seg }: { title: string; active: boolean; onPress: () => void; seg: any }) {
  return (
    <Pressable onPress={onPress} style={[seg.base, active && seg.active]}> 
      <Text style={[seg.text, active && seg.textActive]}>{title}</Text>
    </Pressable>
  );
}

function StatusPill({ connected }: { connected: boolean }) {
  return (
    <View style={{ paddingHorizontal: 10, paddingVertical: 6, borderRadius: 0, borderWidth: 1, borderColor: connected ? '#A3A3A3' : Colors.border, backgroundColor: connected ? 'rgba(255,255,255,0.06)' : 'rgba(255,255,255,0.02)' }}>
      <Text style={{ color: connected ? '#D4D4D8' : Colors.textSecondary, fontSize: 12, fontFamily: Typography.bold }}>
        {connected ? 'Connected' : 'Disconnected'}
      </Text>
    </View>
  );
}
