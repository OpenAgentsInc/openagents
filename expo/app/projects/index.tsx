import React from 'react';
import { ScrollView, Text, View, Pressable } from 'react-native';
import { Colors } from '@/constants/theme';
import { Typography } from '@/constants/typography';
// Projects disabled temporarily
import { ProjectCard } from '@/components/projects/ProjectCard';
import { useRouter } from 'expo-router';
import { useHeaderTitle } from '@/lib/header-store';

export default function ProjectsList() {
  useHeaderTitle('Projects');
  return (
    <View style={{ flex: 1, backgroundColor: Colors.background, alignItems: 'center', justifyContent: 'center' }}>
      <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 16 }}>Projects are temporarily unavailable.</Text>
    </View>
  );
}
