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
    <View style={{ flex: 1, backgroundColor: Colors.background, padding: 24 }}>
      <Text style={{ color: Colors.foreground, fontFamily: Typography.bold, fontSize: 22, marginBottom: 8 }}>OpenAgents</Text>
      <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 14, marginBottom: 16 }}>
        OpenAgents lets you connect to your local coding agent. Today we support Codex; more agents can be added over time.
      </Text>
      <Text style={{ color: Colors.foreground, fontFamily: Typography.bold, marginTop: 6, marginBottom: 6 }}>Requirements</Text>
      <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 14, marginBottom: 12 }}>
        • Codex CLI installed on your desktop (the <Text style={{ fontFamily: Typography.bold, color: Colors.foreground }}>codex</Text> binary in your PATH).
        {'\n'}• The OpenAgents bridge running on your machine (default port 8787).
        {'\n'}• If connecting over LAN/VPN (e.g., Tailscale), use your device’s <Text style={{ fontFamily: Typography.bold, color: Colors.foreground }}>host:port</Text> for Bridge Host in Settings.
      </Text>
      <Text style={{ color: Colors.foreground, fontFamily: Typography.bold, marginTop: 6, marginBottom: 6 }}>Quick Start</Text>
      <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 14, marginBottom: 16 }}>
        1) Start the bridge on your desktop: <Text style={{ fontFamily: Typography.bold, color: Colors.foreground }}>cargo run -p codex-bridge -- --bind 0.0.0.0:8787</Text>{'\n'}
        2) In the app, open Settings and set Bridge Host (e.g., <Text style={{ fontFamily: Typography.bold, color: Colors.foreground }}>localhost:8787</Text> or <Text style={{ fontFamily: Typography.bold, color: Colors.foreground }}>100.x.x.x:8787</Text>).{'\n'}
        3) Start a New Thread.
      </Text>
      <View style={{ flexDirection: 'row', gap: 12, marginTop: 8 }}>
        <Pressable onPress={skip} style={{ borderWidth: 1, borderColor: Colors.border, paddingHorizontal: 16, paddingVertical: 12 }}>
          <Text style={{ color: Colors.secondary, fontFamily: Typography.bold }}>Skip</Text>
        </Pressable>
        <Pressable onPress={finish} style={{ backgroundColor: Colors.quaternary, paddingHorizontal: 16, paddingVertical: 12 }}>
          <Text style={{ color: Colors.foreground, fontFamily: Typography.bold }}>Get Started</Text>
        </Pressable>
        <Pressable onPress={() => { try { router.push('/settings') } catch {} }} style={{ borderWidth: 1, borderColor: Colors.border, paddingHorizontal: 16, paddingVertical: 12 }}>
          <Text style={{ color: Colors.secondary, fontFamily: Typography.bold }}>Open Settings</Text>
        </Pressable>
      </View>
    </View>
  )
}
