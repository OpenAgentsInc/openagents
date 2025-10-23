import React, { useEffect, useMemo } from 'react';
import { FlatList, Pressable, Text, View } from 'react-native';
import { useRouter } from 'expo-router';
import { useWs } from '@/providers/ws';
import { useSessions } from '@/lib/sessions-store';
import { Colors } from '@/constants/theme';
import { Typography } from '@/constants/typography';
import { useHeaderTitle } from '@/lib/header-store';

export default function HistoryScreen() {
  const router = useRouter();
  useHeaderTitle('History');
  const { wsUrl } = useWs();
  const history = useSessions((s) => s.history);
  const loading = useSessions((s) => s.loadingHistory);
  const loadHistory = useSessions((s) => s.loadHistory);
  useEffect(() => { loadHistory(wsUrl).catch(() => {}); }, [loadHistory, wsUrl]);
  const data = useMemo(() => history, [history]);

  return (
    <View style={{ flex: 1, backgroundColor: Colors.background, paddingHorizontal: 8, paddingTop: 8 }}>
      <FlatList
        data={data}
        keyExtractor={(it) => String(it.id)}
        ItemSeparatorComponent={() => <View style={{ height: 8 }} />}
        renderItem={({ item }) => (
          <Pressable onPress={() => router.push('/(tabs)/session')} style={{ borderWidth: 1, borderColor: Colors.border, padding: 8 }}>
            <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12 }}>{new Date(item.mtime * 1000).toLocaleString()}</Text>
            <Text numberOfLines={1} style={{ color: Colors.foreground, fontFamily: Typography.bold, fontSize: 14, marginTop: 2 }}>{item.title || '(no title)'}</Text>
            <Text numberOfLines={2} style={{ color: Colors.foreground, fontFamily: Typography.primary, fontSize: 13, marginTop: 4 }}>{item.snippet}</Text>
          </Pressable>
        )}
        ListEmptyComponent={
          <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>
            {loading ? 'Loading…' : 'No history yet.'}
          </Text>
        }
        contentContainerStyle={{ paddingBottom: 12 }}
      />
    </View>
  );
}
