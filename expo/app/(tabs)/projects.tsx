import React from 'react';
import { ScrollView, Text, View, Pressable } from 'react-native';
import { Colors } from '@/constants/theme';
import { Typography } from '@/constants/typography';
import { useProjects } from '@/providers/projects';
import { ProjectCard } from '@/components/projects/ProjectCard';
import { useRouter } from 'expo-router';
import { useHeaderTitle } from '@/lib/header-store';

export default function ProjectsList() {
  const { projects, setActive } = useProjects();
  const router = useRouter();
  useHeaderTitle('Projects');

  return (
    <ScrollView style={{ flex: 1, backgroundColor: Colors.background }} contentContainerStyle={{ padding: 16, gap: 14 }}>
      <Pressable onPress={() => router.push('/project/new')} style={{ alignSelf: 'flex-end', backgroundColor: Colors.buttonBg, paddingHorizontal: 12, paddingVertical: 8 }}>
        <Text style={{ color: Colors.textPrimary, fontFamily: Typography.bold }}>New</Text>
      </Pressable>

      <View style={{ gap: 10 }}>
        {projects.map(p => (
          <ProjectCard
            key={p.id}
            project={p}
            onPress={async () => {
              await setActive(p.id);
              router.push(`/project/${encodeURIComponent(p.id)}`);
            }}
          />
        ))}
        {projects.length === 0 && (
          <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary }}>No projects yet.</Text>
        )}
      </View>
    </ScrollView>
  );
}
