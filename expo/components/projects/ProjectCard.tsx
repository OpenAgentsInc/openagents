import React from 'react';
import { View, Text, Pressable } from 'react-native';
import { Colors } from '@/constants/theme';
import { Typography } from '@/constants/typography';
import { Ionicons } from '@expo/vector-icons';
import type { Project } from '@/lib/projects-store';

export function ProjectCard({ project, onPress }: { project: Project; onPress?: () => void }) {
  const needs = project.attentionCount ?? 0;
  const running = project.runningAgents ?? 0;

  return (
    <Pressable onPress={onPress} style={{ borderWidth: 1, borderColor: Colors.border, backgroundColor: Colors.card, borderRadius: 0, padding: 12 }}>
      <View style={{ flexDirection: 'row', justifyContent: 'space-between', alignItems: 'center' }}>
        <Text style={{ color: Colors.textPrimary, fontFamily: Typography.bold, fontSize: 16 }}>{project.name}</Text>
        <View style={{ flexDirection: 'row', gap: 8 }}>
          <Pill icon="construct-outline" label={`${running}`} tone={Colors.textSecondary} />
          <Pill icon="alert-circle-outline" label={`${needs}`} tone={needs > 0 ? '#F59E0B' : Colors.textSecondary} />
        </View>
      </View>

      {!!project.repo?.remote && (
        <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary, marginTop: 4 }}>
          Repo: {project.repo.remote}{project.repo.branch ? `#${project.repo.branch}` : ''}
        </Text>
      )}
      {!!project.workingDir && (
        <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary, marginTop: 2 }}>
          Dir: {project.workingDir}
        </Text>
      )}
      {!!project.agentFile && (
        <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary, marginTop: 2 }}>
          Agent: {project.agentFile}
        </Text>
      )}
      {typeof project.lastActivity === 'number' && (
        <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary, marginTop: 6, fontSize: 12 }}>
          Last activity: {new Date(project.lastActivity).toLocaleString()}
        </Text>
      )}
    </Pressable>
  );
}

function Pill({ icon, label, tone = Colors.textSecondary }: { icon: any; label: string; tone?: string }) {
  return (
    <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6, borderWidth: 1, borderColor: Colors.border, paddingHorizontal: 8, paddingVertical: 4, borderRadius: 0 }}>
      <Ionicons name={icon} size={12} color={tone} />
      <Text style={{ color: tone, fontFamily: Typography.bold, fontSize: 12 }}>{label}</Text>
    </View>
  );
}
