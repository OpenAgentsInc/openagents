import React from 'react';
import { useLocalSearchParams, Stack } from 'expo-router';
import { ScrollView, Text, View } from 'react-native';
import { getLog } from '@/lib/log-store';
import Markdown from 'react-native-markdown-display';
import { Colors } from '@/constants/theme';
import { Typography } from '@/constants/typography';

export default function MessageDetail() {
  const { id } = useLocalSearchParams<{ id: string }>();
  const num = Number(id);
  const detail = getLog(num);

  return (
    <View style={{ flex: 1, backgroundColor: Colors.background }}>
      <Stack.Screen options={{ title: `Message ${id}`, headerTitleStyle: { fontFamily: Typography.bold }, headerBackTitleVisible: false, headerBackTitle: '' }} />
      <ScrollView contentContainerStyle={{ padding: 16 }}>
        {!detail && (
          <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary }}>Message not found.</Text>
        )}
        {detail && (
          <View style={{ gap: 12 }}>
            <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary }}>Kind: {detail.kind}</Text>
            <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary }}>ID: {detail.id}</Text>
            {detail.ts && (
              <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary }}>Time: {new Date(detail.ts).toLocaleString()}</Text>
            )}
            {detail.kind === 'md' || detail.text.startsWith('::md::') ? (
              <Markdown style={{
                body: { color: Colors.textPrimary, fontFamily: Typography.primary, fontSize: 13, lineHeight: 18 },
                code_inline: { backgroundColor: '#0F1217', color: Colors.textPrimary, borderWidth: 1, borderColor: Colors.border, paddingHorizontal: 4, paddingVertical: 2 },
                code_block: { backgroundColor: '#0F1217', color: Colors.textPrimary, borderWidth: 1, borderColor: Colors.border, padding: 8 },
                fence: { backgroundColor: '#0F1217', color: Colors.textPrimary, borderWidth: 1, borderColor: Colors.border, padding: 8 },
              }}>
                {detail.text.startsWith('::md::') ? detail.text.slice('::md::'.length) : detail.text}
              </Markdown>
            ) : detail.kind === 'reason' || detail.text.startsWith('::reason::') ? (
              <Markdown style={{
                body: { color: Colors.textSecondary, fontFamily: Typography.primary, fontSize: 13, lineHeight: 18 },
                code_inline: { backgroundColor: '#0F1217', color: Colors.textSecondary, borderWidth: 1, borderColor: Colors.border, paddingHorizontal: 4, paddingVertical: 2 },
                code_block: { backgroundColor: '#0F1217', color: Colors.textSecondary, borderWidth: 1, borderColor: Colors.border, padding: 8 },
                fence: { backgroundColor: '#0F1217', color: Colors.textSecondary, borderWidth: 1, borderColor: Colors.border, padding: 8 },
              }}>
                {detail.text.startsWith('::reason::') ? detail.text.slice('::reason::'.length) : detail.text}
              </Markdown>
            ) : (
              <Text selectable style={{ color: Colors.textPrimary, fontFamily: Typography.primary, lineHeight: 18 }}>{detail.text}</Text>
            )}
          </View>
        )}
      </ScrollView>
    </View>
  );
}
