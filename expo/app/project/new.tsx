import React, { useMemo, useState } from 'react';
import { View, Text, TextInput, Pressable } from 'react-native';
import { Colors } from '@/constants/theme';
import { Typography } from '@/constants/typography';
import { useProjects } from '@/providers/projects';
import { useRouter } from 'expo-router';

function slugify(s: string): string {
  return s.toLowerCase().trim().replace(/[^a-z0-9]+/g, '-').replace(/(^-|-$)+/g, '') || `proj-${Date.now()}`;
}

export default function NewProject() {
  const router = useRouter();
  const { projects, save, setActive } = useProjects();
  const [name, setName] = useState('');
  const [workingDir, setWorkingDir] = useState('~/code');
  const [saving, setSaving] = useState(false);

  const disabled = !name.trim();

  const onCreate = async () => {
    if (disabled || saving) return;
    setSaving(true);
    try {
      let id = slugify(name);
      const taken = new Set(projects.map((p) => p.id));
      let i = 1;
      while (taken.has(id)) { id = `${slugify(name)}-${i++}`; }
      const now = Date.now();
      await save({
        id,
        name: name.trim(),
        voiceAliases: [],
        workingDir: workingDir.trim(),
        createdAt: now,
        updatedAt: now,
      } as any);
      await setActive(id);
      router.replace(`/project/${encodeURIComponent(id)}`);
    } finally { setSaving(false); }
  };

  return (
    <View style={{ flex: 1, backgroundColor: Colors.background, padding: 16, gap: 10 }}>
      <Text style={{ color: Colors.textPrimary, fontFamily: Typography.bold, fontSize: 18 }}>New Project</Text>
      <Field label="Name" value={name} onChange={setName} autoFocus />
      <Field label="Working directory" value={workingDir} onChange={setWorkingDir} placeholder="/Users/you/code/repo" />
      <View style={{ flexDirection: 'row', gap: 8, marginTop: 6 }}>
        <Button title="Create" onPress={onCreate} disabled={disabled || saving} />
        <Button title="Cancel" onPress={() => router.back()} />
      </View>
    </View>
  );
}

function Field({ label, value, onChange, placeholder, autoFocus }: { label: string; value: string; onChange: (v: string) => void; placeholder?: string; autoFocus?: boolean }) {
  return (
    <View style={{ gap: 6 }}>
      <Text style={{ color: Colors.textSecondary, fontFamily: Typography.bold, fontSize: 12 }}>{label}</Text>
      <TextInput
        value={value}
        onChangeText={onChange}
        autoCapitalize="none"
        autoCorrect={false}
        placeholder={placeholder}
        placeholderTextColor={Colors.textSecondary}
        style={{ borderWidth: 1, borderColor: Colors.border, padding: 10, backgroundColor: Colors.card, color: Colors.textPrimary, fontFamily: Typography.primary, fontSize: 13 }}
        autoFocus={autoFocus}
      />
    </View>
  );
}

function Button({ title, onPress, disabled }: { title: string; onPress: () => void; disabled?: boolean }) {
  return (
    <Pressable onPress={onPress} disabled={disabled} style={{ backgroundColor: disabled ? Colors.border : Colors.buttonBg, paddingHorizontal: 16, paddingVertical: 12 }}>
      <Text style={{ color: Colors.textPrimary, fontFamily: Typography.bold }}>{title}</Text>
    </Pressable>
  );
}
