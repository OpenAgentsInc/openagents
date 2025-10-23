import React, { useEffect, useMemo } from 'react';
import { FlatList, Pressable, Text, View } from 'react-native';
import { useRouter } from 'expo-router';
import { getAllLogs, loadLogs, subscribe } from '@/lib/log-store';
import { Colors } from '@/constants/theme';
import { Typography } from '@/constants/typography';

export default function HistoryScreen() {
  const router = useRouter();
  const items = React.useSyncExternalStore(subscribe, getAllLogs, getAllLogs);
  const [hydrating, setHydrating] = React.useState(true);
  useEffect(() => {
    let alive = true;
    loadLogs().catch(() => {}).finally(() => { if (alive) setHydrating(false); });
    return () => { alive = false; };
  }, []);
  const data = useMemo(() => items.slice().reverse(), [items]);

  return (
    <View style={{ flex: 1, backgroundColor: Colors.background, paddingHorizontal: 8, paddingTop: 8 }}>
      <FlatList
        data={data}
        keyExtractor={(it) => String(it.id)}
        ItemSeparatorComponent={() => <View style={{ height: 8 }} />}
        renderItem={({ item }) => (
          <Pressable onPress={() => router.push(`/message/${(item as any).detailId ?? item.id}`)} style={{ borderWidth: 1, borderColor: Colors.border, padding: 8 }}>
            <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary, fontSize: 12 }}>#{item.id} · {item.kind}</Text>
            <Text numberOfLines={2} style={{ color: Colors.textPrimary, fontFamily: Typography.primary, fontSize: 13, marginTop: 4 }}>{item.text.replace(/^::(md|reason)::/, '')}</Text>
          </Pressable>
        )}
        ListEmptyComponent={
          <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary }}>
            {hydrating ? 'Loading…' : 'No history yet.'}
          </Text>
        }
        contentContainerStyle={{ paddingBottom: 12 }}
      />
    </View>
  );
}
