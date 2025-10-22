import React, { useEffect, useState } from 'react';
import { FlatList, Pressable, Text, View } from 'react-native';
import { useRouter } from 'expo-router';
import { getAllLogs, loadLogs } from '@/lib/log-store';
import { Colors } from '@/constants/theme';
import { Typography } from '@/constants/typography';

export default function HistoryScreen() {
  const router = useRouter();
  const [items, setItems] = useState(() => getAllLogs());
  useEffect(() => { (async ()=>{ await loadLogs(); setItems(getAllLogs()) })(); }, []);

  return (
    <View style={{ flex: 1, backgroundColor: Colors.background, paddingHorizontal: 8, paddingTop: 8 }}>
      <FlatList
        data={items.slice().reverse()}
        keyExtractor={(it) => String(it.id)}
        ItemSeparatorComponent={() => <View style={{ height: 8 }} />}
        renderItem={({ item }) => (
          <Pressable onPress={() => router.push('/session/current')} style={{ borderWidth: 1, borderColor: Colors.border, padding: 8 }}>
            <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary, fontSize: 12 }}>#{item.id} · {item.kind}</Text>
            <Text numberOfLines={2} style={{ color: Colors.textPrimary, fontFamily: Typography.primary, fontSize: 13, marginTop: 4 }}>{item.text.replace(/^::(md|reason)::/, '')}</Text>
          </Pressable>
        )}
        ListEmptyComponent={<Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary }}>No history yet.</Text>}
        contentContainerStyle={{ paddingBottom: 12 }}
      />
    </View>
  );
}
