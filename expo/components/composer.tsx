import React, { useCallback, useState } from 'react';
import { View, TextInput, Pressable, Text, Platform } from 'react-native';
import { Colors } from '@/constants/theme';
import { Typography } from '@/constants/typography';

export function Composer({ onSend, connected = true, placeholder = 'Type a message' }: { onSend: (text: string) => void; connected?: boolean; placeholder?: string }) {
  const [text, setText] = useState('');
  const ui = { button: '#3F3F46' } as const;

  const doSend = useCallback(() => {
    const base = text.trim();
    if (!base) return;
    onSend(base);
    setText('');
  }, [text, onSend]);

  return (
    <View style={{ gap: 6, paddingBottom: 6 }}>
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
        <TextInput
          value={text}
          onChangeText={setText}
          autoCapitalize="none"
          autoCorrect={false}
          placeholder={placeholder}
          returnKeyType="send"
          onSubmitEditing={doSend}
          style={{ flex: 1, borderWidth: 1, borderColor: Colors.border, paddingHorizontal: 12, paddingVertical: 10, backgroundColor: '#0F1217', color: Colors.textPrimary, fontSize: 13, fontFamily: Typography.primary, borderRadius: 0 }}
          placeholderTextColor={Colors.textSecondary}
        />
        <Pressable
          onPress={doSend}
          disabled={!connected || !text.trim()}
          style={{ backgroundColor: !connected || !text.trim() ? Colors.border : ui.button, paddingHorizontal: 16, paddingVertical: 12, borderRadius: 0 }}
          accessibilityRole="button"
        >
          <Text style={{ color: '#fff', fontFamily: Typography.bold }}>Send</Text>
        </Pressable>
      </View>
    </View>
  );
}

