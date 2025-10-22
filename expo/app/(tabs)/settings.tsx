import { StyleSheet, View, Text, TextInput, Pressable } from 'react-native';
import { Colors } from '@/constants/theme';
import { Typography } from '@/constants/typography';
import { useWs } from '@/providers/ws';

export default function SettingsScreen() {
  const { wsUrl, setWsUrl, connected, connect, disconnect } = useWs();
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

      {/* OTA Update test controls removed */}
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
    borderRadius: 12,
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
    <Pressable onPress={onPress} style={{ backgroundColor: '#3F3F46', paddingHorizontal: 16, paddingVertical: 12, borderRadius: 10 }}>
      <Text style={{ color: '#fff', fontFamily: Typography.bold }}>{title}</Text>
    </Pressable>
  );
}

function StatusPill({ connected }: { connected: boolean }) {
  return (
    <View style={{ paddingHorizontal: 10, paddingVertical: 6, borderRadius: 999, borderWidth: 1, borderColor: connected ? '#A3A3A3' : Colors.border, backgroundColor: connected ? 'rgba(255,255,255,0.06)' : 'rgba(255,255,255,0.02)' }}>
      <Text style={{ color: connected ? '#D4D4D8' : Colors.textSecondary, fontSize: 12, fontFamily: Typography.bold }}>
        {connected ? 'Connected' : 'Disconnected'}
      </Text>
    </View>
  );
}
