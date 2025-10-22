import React from 'react';
import { useLocalSearchParams, Stack } from 'expo-router';
import { ScrollView, Text, View } from 'react-native';
import { getLog } from '@/lib/log-store';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { MarkdownBlock } from '@/components/jsonl/MarkdownBlock';
import { ReasoningCard } from '@/components/jsonl/ReasoningCard';
import { Colors } from '@/constants/theme';
import { Typography } from '@/constants/typography';

export default function MessageDetail() {
  const { id } = useLocalSearchParams<{ id: string }>();
  const num = Number(id);
  const detail = getLog(num);
  const insets = useSafeAreaInsets();

  return (
    <View style={{ flex: 1, backgroundColor: Colors.background }}>
      <Stack.Screen
        options={{
          title: `Message ${id}`,
          headerTitleStyle: { fontFamily: Typography.bold },
          headerBackTitle: '',
          headerBackButtonDisplayMode: 'minimal',
        }}
      />
      <ScrollView
        contentContainerStyle={{
          paddingTop: 16,
          paddingHorizontal: 16,
          paddingBottom: 32 + insets.bottom,
        }}
      >
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
              <MarkdownBlock markdown={detail.text.startsWith('::md::') ? detail.text.slice('::md::'.length) : detail.text} />
            ) : detail.kind === 'reason' || detail.text.startsWith('::reason::') ? (
              <ReasoningCard item={{ type: 'reasoning', text: detail.text.startsWith('::reason::') ? detail.text.slice('::reason::'.length) : detail.text }} />
            ) : (
              <Text selectable style={{ color: Colors.textPrimary, fontFamily: Typography.primary, lineHeight: 18 }}>{detail.text}</Text>
            )}
          </View>
        )}
      </ScrollView>
    </View>
  );
}
