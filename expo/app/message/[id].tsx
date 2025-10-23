import React, { useState, useCallback, useMemo, useEffect } from 'react';
import { useLocalSearchParams, Stack } from 'expo-router';
import { ScrollView, Text, View, Pressable } from 'react-native';
import { getAllLogs, getLog, isHydrated, loadLogs, subscribe } from '@/lib/log-store';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { MarkdownBlock } from '@/components/jsonl/MarkdownBlock';
import { ReasoningCard } from '@/components/jsonl/ReasoningCard';
import { Colors } from '@/constants/theme';
import { Typography } from '@/constants/typography';
import * as Clipboard from 'expo-clipboard';

export default function MessageDetail() {
  const { id } = useLocalSearchParams<{ id: string }>();
  const num = Number(id);
  const logs = React.useSyncExternalStore(subscribe, getAllLogs, getAllLogs);
  const hydrated = isHydrated();
  useEffect(() => { if (!hydrated) loadLogs().catch(() => {}); }, [hydrated]);
  const detail = useMemo(() => {
    if (Number.isNaN(num)) return undefined;
    return logs.find((entry) => entry.id === num) ?? getLog(num);
  }, [logs, num]);
  const insets = useSafeAreaInsets();
  const [copied, setCopied] = useState(false)
  const copy = useCallback(async (text: string) => {
    try { await Clipboard.setStringAsync(text) } catch {}
    setCopied(true); setTimeout(() => setCopied(false), 800)
  }, [])
  const rawText = typeof detail?.text === 'string' ? detail.text : undefined;
  const isUserMsg = typeof rawText === 'string' && /^\s*>/.test(rawText);
  const cleanBody = typeof rawText === 'string' ? rawText.replace(/^\s*>\s?/, '') : '';

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
        {!detail ? (
          <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary }}>{hydrated ? 'Message not found.' : 'Loading message…'}</Text>
        ) : (
          <View style={{ gap: 12 }}>
            <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary }}>Kind: {detail.kind}</Text>
            <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary }}>ID: {detail.id}</Text>
            {detail.ts && (
              <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary }}>Time: {new Date(detail.ts).toLocaleString()}</Text>
            )}
            {detail.kind === 'md' || detail.text.startsWith('::md::') ? (
              <Pressable onLongPress={() => copy(detail.text.startsWith('::md::') ? detail.text.slice('::md::'.length) : detail.text)}>
                <MarkdownBlock markdown={detail.text.startsWith('::md::') ? detail.text.slice('::md::'.length) : detail.text} />
                {copied ? <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary, fontSize: 12, marginTop: 4 }}>Copied</Text> : null}
              </Pressable>
            ) : detail.kind === 'reason' || detail.text.startsWith('::reason::') ? (
              <Pressable onLongPress={() => copy(detail.text.startsWith('::reason::') ? detail.text.slice('::reason::'.length) : detail.text)}>
                <ReasoningCard item={{ type: 'reasoning', text: detail.text.startsWith('::reason::') ? detail.text.slice('::reason::'.length) : detail.text }} />
                {copied ? <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary, fontSize: 12, marginTop: 4 }}>Copied</Text> : null}
              </Pressable>
            ) : (
              <Pressable onLongPress={() => copy(isUserMsg ? cleanBody : (detail?.text ?? ''))}>
                <Text selectable={!isUserMsg} style={{ color: Colors.textPrimary, fontFamily: Typography.primary, lineHeight: 18 }}>{detail?.text}</Text>
                {copied ? <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary, fontSize: 12, marginTop: 4 }}>Copied</Text> : null}
              </Pressable>
            )}
          </View>
        )}
      </ScrollView>
    </View>
  );
}
