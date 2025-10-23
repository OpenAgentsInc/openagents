import React from 'react'
import { View, Text, Pressable } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { useHeaderTitle } from '@/lib/header-store'
import { useOnboarding } from '@/lib/onboarding-store'
import { useRouter } from 'expo-router'

export default function HelpScreen() {
  useHeaderTitle('Help')
  const ob = useOnboarding()
  const router = useRouter()

  const showOnboarding = React.useCallback(() => {
    try { ob.setCompleted(false) } catch {}
    try { router.replace('/onboarding' as any) } catch {}
  }, [ob, router])

  return (
    <View style={{ flex: 1, backgroundColor: Colors.background, padding: 16 }}>
      <Text style={{ color: Colors.foreground, fontFamily: Typography.bold, fontSize: 18, marginBottom: 8 }}>Help</Text>
      <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, marginBottom: 16 }}>
        Need a quick refresher? You can re-run the onboarding steps.
      </Text>
      <Pressable onPress={showOnboarding} style={{ backgroundColor: Colors.quaternary, paddingHorizontal: 16, paddingVertical: 12, alignSelf: 'flex-start' }}>
        <Text style={{ color: Colors.foreground, fontFamily: Typography.bold }}>Show Onboarding</Text>
      </Pressable>
    </View>
  )
}

