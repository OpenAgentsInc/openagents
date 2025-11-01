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
        {/* Left circular + button — commented out for full-width input */}
        {/**
        <Pressable
          onPress={() => { try { console.log('composer:add'); } catch {} }}
          accessibilityRole="button"
          style={{ width: 36, height: 36, borderRadius: 18, backgroundColor: Colors.black, borderWidth: 1, borderColor: Colors.border, alignItems: 'center', justifyContent: 'center' }}
        >
          <Ionicons name="add" size={20} color={Colors.foreground} />
        </Pressable>
        */}

        {/* Input pill */}
        <View style={{ flex: 1, backgroundColor: Colors.card, borderRadius: 22, borderWidth: 1, borderColor: Colors.border, flexDirection: 'row', alignItems: 'center' }}>
          <TextInput
            value={text}
            onChangeText={handleChange}
            autoCapitalize="none"
            autoCorrect={false}
            placeholder={resolvedPlaceholder}
            returnKeyType="send"
            onSubmitEditing={doSend}
            ref={inputRef}
            testID="composer-input"
            accessibilityLabel="Prompt input"
            style={{ flex: 1, paddingHorizontal: 14, paddingVertical: 10, color: Colors.foreground, fontSize: 16, fontFamily: Typography.primary }}
            placeholderTextColor={Colors.secondary}
          />
          {/* Mic icon — commented out for now */}
          {/**
          <Pressable onPress={() => { try { console.log('composer:mic'); } catch {} }} accessibilityRole="button" style={{ paddingHorizontal: 10, paddingVertical: 8 }}>
            <Ionicons name="mic-outline" size={20} color={Colors.secondary} />
          </Pressable>
          */}
          {/* Submit button */}
          <Pressable
            onPress={doSend}
            disabled={!canSend}
            accessibilityRole="button"
            accessibilityLabel="Send"
            testID="composer-send"
            style={{ width: 36, height: 36, borderRadius: 18, backgroundColor: canSend ? Colors.foreground : Colors.border, alignItems: 'center', justifyContent: 'center', marginRight: 6 }}
          >
            <Ionicons name="arrow-up" size={18} color={canSend ? Colors.black : Colors.foreground} />
          </Pressable>
        </View>
      </View>
    </View>
  );
}

function truncate(value: string, max: number): string {
  if (value.length <= max) return value;
  return `${value.slice(0, max)}…`;
}
