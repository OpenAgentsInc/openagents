import React from 'react';
import { ScrollView, Text, View } from 'react-native';
import { Colors } from '@/constants/theme';
import { Typography } from '@/constants/typography';
import { useProjects } from '@/providers/projects';
import { ProjectCard } from '@/components/projects/ProjectCard';
import { useRouter } from 'expo-router';
import { getAllLogs } from '@/lib/log-store';
import { useHeaderTitle } from '@/lib/header-store';

export default function Dashboard() {
  const { projects, setActive } = useProjects();
  const router = useRouter();
  useHeaderTitle('Dashboard');
  const recent = getAllLogs().slice(-6).reverse();

  return (
    <ScrollView style={{ flex: 1, backgroundColor: Colors.background }} contentContainerStyle={{ padding: 16, gap: 14 }}>
      <Text style={{ color: Colors.foreground, fontFamily: Typography.bold, fontSize: 18 }}>Dashboard</Text>

      <Text style={{ color: Colors.secondary, fontFamily: Typography.bold, fontSize: 12, marginTop: 4 }}>Projects</Text>
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
          <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>
            No projects yet. Use the Projects tab to add one.
          </Text>
        )}
      </View>

      <Text style={{ color: Colors.secondary, fontFamily: Typography.bold, fontSize: 12, marginTop: 14 }}>Recent</Text>
      <View style={{ gap: 6 }}>
        {recent.map(r => (
          <View key={r.id} style={{ borderWidth: 1, borderColor: Colors.border, padding: 8 }}>
            <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12 }}>#{r.id} · {r.kind}</Text>
            <Text numberOfLines={2} style={{ color: Colors.foreground, fontFamily: Typography.primary, fontSize: 13, marginTop: 4 }}>{String(r.text ?? '').replace(/^::(md|reason)::/, '')}</Text>
          </View>
        ))}
        {recent.length === 0 && (
          <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>No recent activity.</Text>
        )}
      </View>
    </ScrollView>
  );
}
