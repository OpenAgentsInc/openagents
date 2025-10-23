import React from 'react'
import { View, Text } from 'react-native'
import { useHeaderTitle } from '@/lib/header-store'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'

export default function DashboardScreen() {
  useHeaderTitle('Dashboard')
  return (
    <View style={{ flex: 1, backgroundColor: Colors.background, alignItems: 'center', justifyContent: 'center' }}>
      <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>Dashboard</Text>
    </View>
  )
}

