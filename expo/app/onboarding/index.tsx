import React from 'react'
import { View, Text, Pressable } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { useHeaderTitle } from '@/lib/header-store'
import { useOnboarding } from '@/lib/onboarding-store'
import { useRouter } from 'expo-router'

export default function Onboarding() {
  useHeaderTitle('Welcome')
  const ob = useOnboarding()
  const router = useRouter()

  const finish = React.useCallback(() => {
    try { ob.setCompleted(true) } catch {}
    try { router.replace('/thread?focus=1&new=1' as any) } catch {}
  }, [ob, router])

  const skip = React.useCallback(() => {
    try { ob.setCompleted(true) } catch {}
    try { router.replace('/thread' as any) } catch {}
  }, [ob, router])

  return (
    <View style={{ flex: 1, backgroundColor: Colors.background, padding: 24, alignItems: 'center', justifyContent: 'center' }}>
      <Text style={{ color: Colors.foreground, fontFamily: Typography.bold, fontSize: 22, marginBottom: 8 }}>OpenAgents</Text>
      <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 14, textAlign: 'center', marginBottom: 24 }}>
        Connect to your local bridge and start a new thread. You can configure the bridge host in Settings anytime.
      </Text>
      <View style={{ flexDirection: 'row', gap: 12 }}>
        <Pressable onPress={skip} style={{ borderWidth: 1, borderColor: Colors.border, paddingHorizontal: 16, paddingVertical: 12 }}>
          <Text style={{ color: Colors.secondary, fontFamily: Typography.bold }}>Skip</Text>
        </Pressable>
        <Pressable onPress={finish} style={{ backgroundColor: Colors.quaternary, paddingHorizontal: 16, paddingVertical: 12 }}>
          <Text style={{ color: Colors.foreground, fontFamily: Typography.bold }}>Get Started</Text>
        </Pressable>
      </View>
    </View>
  )
}

