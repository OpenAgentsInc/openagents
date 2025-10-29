import React, { useMemo, useState } from 'react';
import { View, Text, TextInput, Pressable } from 'react-native';
import { Colors } from '@/constants/theme';
import { Typography } from '@/constants/typography';
// Projects disabled temporarily
import { useRouter } from 'expo-router';
import { useHeaderTitle } from '@/lib/header-store';

function slugify(s: string): string {
  return s.toLowerCase().trim().replace(/[^a-z0-9]+/g, '-').replace(/(^-|-$)+/g, '') || `proj-${Date.now()}`;
}

export default function NewProject() {
  const router = useRouter();
  useHeaderTitle('New Project');
  return (
    <View style={{ flex: 1, backgroundColor: Colors.background, alignItems: 'center', justifyContent: 'center', padding: 16 }}>
      <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 16, textAlign: 'center' }}>Projects are temporarily unavailable.</Text>
      <View style={{ height: 12 }} />
      <Pressable onPress={() => router.back()} style={{ backgroundColor: Colors.quaternary, paddingHorizontal: 16, paddingVertical: 12 }}>
        <Text style={{ color: Colors.foreground, fontFamily: Typography.bold }}>Back</Text>
      </Pressable>
    </View>
  );
}

function Field({ label, value, onChange, placeholder, autoFocus }: { label: string; value: string; onChange: (v: string) => void; placeholder?: string; autoFocus?: boolean }) {
  return (
    <View style={{ gap: 6 }}>
      <Text style={{ color: Colors.secondary, fontFamily: Typography.bold, fontSize: 12 }}>{label}</Text>
      <TextInput
        value={value}
        onChangeText={onChange}
        autoCapitalize="none"
        autoCorrect={false}
        placeholder={placeholder}
        placeholderTextColor={Colors.secondary}
        style={{ borderWidth: 1, borderColor: Colors.border, padding: 10, backgroundColor: Colors.card, color: Colors.foreground, fontFamily: Typography.primary, fontSize: 13 }}
        autoFocus={autoFocus}
      />
    </View>
  );
}

function Button({ title, onPress, disabled }: { title: string; onPress: () => void; disabled?: boolean }) {
  return (
    <Pressable onPress={onPress} disabled={disabled} style={{ backgroundColor: disabled ? Colors.border : Colors.quaternary, paddingHorizontal: 16, paddingVertical: 12 }}>
      <Text style={{ color: Colors.foreground, fontFamily: Typography.bold }}>{title}</Text>
    </Pressable>
  );
}
