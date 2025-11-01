import React, { useCallback, useState } from 'react';
import { View, TextInput, Pressable, Text, Keyboard } from 'react-native';
import { Ionicons } from '@expo/vector-icons';
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
  inputRef?: React.RefObject<TextInput | null>;
};

const BUTTON_COLOR = Colors.quaternary;

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
  inputRef,
}: ComposerProps) {
  const [text, setText] = useState('');
  const trimmed = text.trim();
  const resolvedPlaceholder = placeholder ?? 'Ask Codex';
  const canSend = connected && trimmed.length > 0;
  const canInterrupt = Boolean(onInterrupt) && connected && isRunning;

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
    try { inputRef?.current?.blur(); Keyboard.dismiss(); } catch {}
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
    <View style={{ paddingBottom: 2 }}>
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10 }}>
        <Pressable
          onPress={handleInterrupt}
          accessibilityRole="button"
          accessibilityLabel="Interrupt"
          disabled={!canInterrupt}
          hitSlop={{ top: 8, bottom: 8, left: 8, right: 8 }}
          style={{ opacity: canInterrupt ? 1 : 0.4 }}
        >
          <Ionicons name="stop-circle-outline" size={22} color={BUTTON_COLOR} />
        </Pressable>
        <View style={{ flex: 1, borderColor: Colors.border, borderWidth: 1, backgroundColor: Colors.card, paddingHorizontal: 12, paddingVertical: 8 }}>
          <TextInput
            ref={inputRef}
            placeholder={resolvedPlaceholder}
            placeholderTextColor={Colors.tertiary}
            value={text}
            onChangeText={handleChange}
            onSubmitEditing={doSend}
            returnKeyType="send"
            blurOnSubmit={false}
            multiline
            style={{ minHeight: 20, maxHeight: 120, color: Colors.foreground, fontFamily: Typography.primary }}
            testID="composer-input"
          />
        </View>
        <Pressable
          onPress={doSend}
          accessibilityRole="button"
          accessibilityLabel="Send"
          disabled={!canSend}
          hitSlop={{ top: 8, bottom: 8, left: 8, right: 8 }}
          style={{ opacity: canSend ? 1 : 0.4 }}
          testID="composer-send"
        >
          <Ionicons name="send-sharp" size={22} color={BUTTON_COLOR} />
        </Pressable>
      </View>

      {queuedMessages.length > 0 ? (
        <View style={{ marginTop: 8, paddingHorizontal: 2 }}>
          <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12 }}>
            queued: {queuedMessages.length}
          </Text>
        </View>
      ) : null}
    </View>
  );
}

