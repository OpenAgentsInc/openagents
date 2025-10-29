import React, { useMemo } from 'react';
import { useLocalSearchParams, Stack, useRouter } from 'expo-router';
import { ScrollView, Text, View, TextInput, Pressable } from 'react-native';
import { Colors } from '@/constants/theme';
import { Typography } from '@/constants/typography';
// Projects disabled temporarily
import { useHeaderTitle } from '@/lib/header-store';

export default function ProjectDetail() {
  const router = useRouter();
  useHeaderTitle('Project');
  return (
    <View style={{ flex: 1, backgroundColor: Colors.background, padding: 16, gap: 10, alignItems: 'center', justifyContent: 'center' }}>
      <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 16 }}>Projects are temporarily unavailable.</Text>
      <View style={{ height: 12 }} />
      <Button title="Back" onPress={() => router.back()} />
    </View>
  );
}

function Field({ label, value, onChange }: { label: string; value: string; onChange: (v: string) => void }) {
  return (
    <View style={{ gap: 6 }}>
      <Text style={{ color: Colors.secondary, fontFamily: Typography.bold, fontSize: 12 }}>{label}</Text>
      <TextInput
        value={value}
        onChangeText={onChange}
        autoCapitalize="none"
        autoCorrect={false}
        placeholderTextColor={Colors.secondary}
        style={{ borderWidth: 1, borderColor: Colors.border, padding: 10, backgroundColor: Colors.card, color: Colors.foreground, fontFamily: Typography.primary, fontSize: 13 }}
      />
    </View>
  );
}

function Multiline({ label, value, onChange }: { label: string; value: string; onChange: (v: string) => void }) {
  return (
    <View style={{ gap: 6 }}>
      <Text style={{ color: Colors.secondary, fontFamily: Typography.bold, fontSize: 12 }}>{label}</Text>
      <TextInput
        value={value}
        onChangeText={onChange}
        autoCapitalize="none"
        autoCorrect={false}
        multiline
        placeholderTextColor={Colors.secondary}
        style={{ borderWidth: 1, borderColor: Colors.border, padding: 10, backgroundColor: Colors.card, color: Colors.foreground, fontFamily: Typography.primary, fontSize: 13, minHeight: 80 }}
      />
    </View>
  );
}

function Button({ title, onPress }: { title: string; onPress: () => void }) {
  return (
    <Pressable onPress={onPress} style={{ backgroundColor: Colors.quaternary, paddingHorizontal: 16, paddingVertical: 12 }}>
      <Text style={{ color: Colors.foreground, fontFamily: Typography.bold }}>{title}</Text>
    </Pressable>
  );
}
