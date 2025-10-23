import React, { useCallback, useState } from 'react';
import { View, TextInput, Pressable, Text } from 'react-native';
import { Colors } from '@/constants/theme';
import { Typography } from '@/constants/typography';

type ComposerProps = {
  onSend: (text: string) => void;
  onQueue?: (text: string) => void;
  onInterrupt?: () => void;
  connected?: boolean;
  placeholder?: string;
  isRunning?: boolean;
  queuedMessages?: string[];
  prefill?: string | null;
  onDraftChange?: (text: string) => void;
};

const BUTTON_COLOR = '#3F3F46';
const INTERRUPT_COLOR = '#DC2626';

export function Composer({
  onSend,
  onQueue,
  onInterrupt,
  connected = true,
  placeholder,
  isRunning = false,
  queuedMessages = [],
  prefill,
  onDraftChange,
}: ComposerProps) {
  const [text, setText] = useState('');
  const trimmed = text.trim();
  const resolvedPlaceholder = placeholder ?? (isRunning ? 'Queue a follow-up message' : 'Type a message');
  const canSend = connected && trimmed.length > 0;
  const canInterrupt = Boolean(onInterrupt) && connected && isRunning;
  const sendLabel = isRunning && onQueue ? 'Queue' : 'Send';
  const queueCount = queuedMessages.length;
  const previewMessages = queuedMessages.slice(0, 3);

  React.useEffect(() => {
    if (prefill !== undefined) {
      const value = prefill ?? '';
      setText(value);
      onDraftChange?.(value);
    }
  }, [prefill, onDraftChange]);

  const doSend = useCallback(() => {
    if (!connected) return;
    const base = trimmed;
    if (!base) return;
    if (isRunning && onQueue) {
      onQueue(base);
    } else {
      onSend(base);
    }
    setText('');
    onDraftChange?.('');
  }, [trimmed, connected, isRunning, onQueue, onSend, onDraftChange]);

  const handleChange = useCallback((value: string) => {
    setText(value);
    onDraftChange?.(value);
  }, [onDraftChange]);

  const handleInterrupt = useCallback(() => {
    if (!canInterrupt) return;
    try { onInterrupt?.(); } catch {}
  }, [canInterrupt, onInterrupt]);

  return (
    <View style={{ gap: 6, paddingBottom: 6 }}>
      {queueCount > 0 && (
        <View style={{ borderWidth: 1, borderColor: Colors.border, backgroundColor: '#12141C', paddingHorizontal: 10, paddingVertical: 8, gap: 4 }}>
          {previewMessages.map((item, idx) => (
            <Text key={`${idx}-${item}`} style={{ color: Colors.textPrimary, fontFamily: Typography.primary, fontSize: 12, opacity: 0.75 }}>
              ↳ {truncate(item, 120)}
            </Text>
          ))}
          {queueCount > 3 && (
            <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary, fontSize: 12 }}>
              …and {queueCount - 3} more
            </Text>
          )}
        </View>
      )}
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
        {isRunning && (
          <Pressable
            onPress={handleInterrupt}
            disabled={!canInterrupt}
            accessibilityRole="button"
            style={{
              backgroundColor: canInterrupt ? INTERRUPT_COLOR : Colors.border,
              paddingHorizontal: 14,
              paddingVertical: 12,
              borderRadius: 0,
            }}
          >
            <Text style={{ color: '#fff', fontFamily: Typography.bold }}>Interrupt</Text>
          </Pressable>
        )}
        <TextInput
          value={text}
          onChangeText={handleChange}
          autoCapitalize="none"
          autoCorrect={false}
          placeholder={resolvedPlaceholder}
          returnKeyType="send"
          onSubmitEditing={doSend}
          style={{ flex: 1, borderWidth: 1, borderColor: Colors.border, paddingHorizontal: 12, paddingVertical: 10, backgroundColor: '#0F1217', color: Colors.textPrimary, fontSize: 13, fontFamily: Typography.primary, borderRadius: 0 }}
          placeholderTextColor={Colors.textSecondary}
        />
        <Pressable
          onPress={doSend}
          disabled={!canSend}
          style={{ backgroundColor: canSend ? BUTTON_COLOR : Colors.border, paddingHorizontal: 16, paddingVertical: 12, borderRadius: 0 }}
          accessibilityRole="button"
        >
          <Text style={{ color: '#fff', fontFamily: Typography.bold }}>{sendLabel}</Text>
        </Pressable>
      </View>
    </View>
  );
}

function truncate(value: string, max: number): string {
  if (value.length <= max) return value;
  return `${value.slice(0, max)}…`;
}
