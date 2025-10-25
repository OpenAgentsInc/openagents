import React from 'react'
import { ScrollView, Text, View } from 'react-native'
import { useSkills } from '@/providers/skills'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { useHeaderTitle, useHeaderSubtitle } from '@/lib/header-store'

export default function SkillsIndex() {
  const { skills } = useSkills()
  useHeaderTitle('Skills')
  useHeaderSubtitle('')
  return (
    <ScrollView style={{ flex: 1, backgroundColor: Colors.background }} contentContainerStyle={{ padding: 16 }}>
      <Text style={{ color: Colors.foreground, fontFamily: Typography.bold, fontSize: 20, marginBottom: 12 }}>Skills</Text>
      {skills.length === 0 ? (
        <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>No skills installed.</Text>
      ) : skills.map(s => (
        <View key={s.id + (s.source || '') + (s.projectId || '')} style={{ paddingVertical: 10, borderBottomWidth: 1, borderBottomColor: Colors.border }}>
          <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between' }}>
            <Text style={{ color: Colors.foreground, fontFamily: Typography.primary, fontSize: 16 }}>{s.name}</Text>
            {s.source ? (
              <Text style={{ color: Colors.secondary, fontFamily: Typography.bold, fontSize: 12 }}>
                {s.source === 'project' ? 'project' : s.source}
              </Text>
            ) : null}
          </View>
          <Text numberOfLines={2} style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 13 }}>{s.description}</Text>
        </View>
      ))}
    </ScrollView>
  )
}
