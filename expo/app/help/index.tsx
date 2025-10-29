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
      <Pressable onPress={showOnboarding} style={{ backgroundColor: Colors.quaternary, paddingHorizontal: 16, paddingVertical: 12, alignSelf: 'flex-start', marginBottom: 16 }}>
        <Text style={{ color: Colors.foreground, fontFamily: Typography.bold }}>Show Onboarding</Text>
      </Pressable>

      <Text style={{ color: Colors.foreground, fontFamily: Typography.bold, fontSize: 16, marginBottom: 8 }}>FAQ</Text>
      <FAQ />
    </View>
  )
}

function FAQItem({ q, a }: { q: string; a: React.ReactNode }) {
  const [open, setOpen] = React.useState(false)
  return (
    <View style={{ borderTopWidth: 1, borderColor: Colors.border }}>
      <Pressable onPress={() => setOpen(v => !v)} style={{ paddingVertical: 12 }}>
        <Text style={{ color: Colors.foreground, fontFamily: Typography.bold }}>{q}</Text>
      </Pressable>
      {open ? (
        <View style={{ paddingBottom: 12 }}>
          {typeof a === 'string' ? (
            <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>{a}</Text>
          ) : a}
        </View>
      ) : null}
    </View>
  )
}

function FAQ() {
  return (
    <View>
      <FAQItem
        q="What is OpenAgents?"
        a={
          <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>
            OpenAgents is a mobile command center for your coding agent. It connects to a local bridge on your desktop and streams a live feed of the agent’s work. Today, Codex is supported; more agents may be added.
          </Text>
        }
      />
      <FAQItem
        q="Do I need centralized servers?"
        a={
          <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>
            No. The app talks directly to your desktop over a WebSocket bridge. History is scanned from your local Codex sessions directory. There are no centralized servers involved.
          </Text>
        }
      />
      <FAQItem
        q="How do I connect over Tailscale?"
        a={
          <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>
            Ensure the bridge binds to 0.0.0.0 (e.g., cargo run -p oa-bridge -- --bind 0.0.0.0:8787). In Settings, set Bridge Host to your Tailscale IP + port (e.g., 100.x.x.x:8787). The app will normalize the URL and connect.
          </Text>
        }
      />
      <FAQItem
        q="How do I install Codex?"
        a={
          <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>
            Install the codex CLI binary on your desktop and make sure it’s in your PATH. Then run the OpenAgents bridge from the repository. The app will stream the JSONL output.
          </Text>
        }
      />
      <FAQItem
        q="Can I resume historical conversations?"
        a={
          <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>
            Yes. Open a thread from the History list and send a message — the bridge will resume the thread when supported by your Codex build. If a file lacks a resume token, it starts a new thread.
          </Text>
        }
      />
      <FAQItem
        q="Why can’t I connect?"
        a={
          <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>
            Verify the bridge is running and reachable. Check that the Bridge Host matches host:port, and that your device can reach the desktop over Wi‑Fi/VPN. The header dot shows live connection state.
          </Text>
        }
      />
    </View>
  )
}
