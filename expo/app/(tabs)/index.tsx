import React, { useEffect } from 'react'
import { Text, View } from 'react-native'
import { SafeAreaView } from 'react-native-safe-area-context'
import { useRouter } from 'expo-router'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'

export default function HomeRedirect() {
  const router = useRouter()
  useEffect(() => { router.replace('/(tabs)/session') }, [router])
  return (
    <SafeAreaView style={{ flex: 1, backgroundColor: Colors.background }}>
      <View style={{ flex: 1, alignItems: 'center', justifyContent: 'center' }}>
        <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary }}>Opening session…</Text>
      </View>
    </SafeAreaView>
  )
}
