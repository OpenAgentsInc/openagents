import React, { useMemo } from 'react';
import { useLocalSearchParams, Stack, useRouter } from 'expo-router';
import { ScrollView, Text, View, TextInput, Pressable } from 'react-native';
import { Colors } from '@/constants/theme';
import { Typography } from '@/constants/typography';
import { useProjects } from '@/providers/projects';
import { useHeaderTitle } from '@/lib/header-store';

export default function ProjectDetail() {
  const { id } = useLocalSearchParams<{ id: string }>();
  const router = useRouter();
  useHeaderTitle('Project');
  const { projects, activeProject, setActive, sendForProject, save } = useProjects();
  const project = useMemo(() => projects.find(p => p.id === id) ?? activeProject, [projects, activeProject, id]);

  if (!project) {
    return (
      <View style={{ flex: 1, backgroundColor: Colors.background, padding: 16, gap: 10 }}>
        <Text style={{ color: Colors.foreground, fontFamily: Typography.bold, fontSize: 18 }}>Project not found</Text>
        <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>The project you tried to open doesn’t exist.</Text>
        <View style={{ flexDirection: 'row', gap: 8, marginTop: 8 }}>
          <Button title="Back" onPress={() => router.back()} />
          <Button title="Projects" onPress={() => router.replace('/projects')} />
        </View>
      </View>
    );
  }

  return (
    <View style={{ flex: 1, backgroundColor: Colors.background }}>
      <Stack.Screen options={{ headerShown: false }} />
      <ScrollView contentContainerStyle={{ padding: 16, gap: 10 }}>
        <Field label="Name" value={project.name} onChange={v => save({ ...project, name: v })} />
        <Field label="Working directory" value={project.workingDir} onChange={v => save({ ...project, workingDir: v })} />
        <Field label="Repo (owner/name)" value={project.repo?.remote ?? ''} onChange={v => save({ ...project, repo: { ...project.repo, remote: v } })} />
        <Field label="Branch" value={project.repo?.branch ?? ''} onChange={v => save({ ...project, repo: { ...project.repo, branch: v || undefined } })} />
        <Field label="Agent file (relative)" value={project.agentFile ?? ''} onChange={v => save({ ...project, agentFile: v || undefined })} />
        <Multiline label="Custom instructions" value={project.instructions ?? ''} onChange={v => save({ ...project, instructions: v || undefined })} />

        <View style={{ flexDirection: 'row', gap: 8, marginTop: 8 }}>
          <Button title="Set Active" onPress={() => setActive(project.id)} />
          <Button title="Open Thread" onPress={async () => {
            setActive(project.id);
            try {
              const create = (require('convex/react') as any).useMutation('threads:create') as (args?: { title?: string; projectId?: string }) => Promise<string>;
              const id2 = await create({ title: 'New Thread', projectId: project.id });
              router.push(`/convex/thread/${encodeURIComponent(String(id2))}`);
            } catch { router.push('/convex'); }
          }} />
          <Button title="Ping (cd)" onPress={() => sendForProject(project, 'Echo working dir and list top-level: run `pwd` then `ls -la`')} />
        </View>

        {!!project.todos?.length && (
          <View style={{ marginTop: 12 }}>
            <Text style={{ color: Colors.secondary, fontFamily: Typography.bold, fontSize: 12 }}>To‑dos</Text>
            {project.todos.map((t, i) => (
              <Text key={i} style={{ color: t.completed ? Colors.secondary : Colors.foreground, fontFamily: Typography.primary }}>
                {t.completed ? '☑︎' : '☐'} {t.text}
              </Text>
            ))}
          </View>
        )}
      </ScrollView>
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
