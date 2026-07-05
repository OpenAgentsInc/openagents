import { useEffect, useState } from "react"
import { View } from "react-native"

import { Pill, ScreenShell, StatLine } from "../components/shell"
import { KHALA_MOBILE_OTA_CONTRACT } from "../config/updates"
import { readNativeReadiness } from "../native/modules"
import { loadKhalaApiKey } from "../security/keychain"

type Readiness = Readonly<{
  speech: string
  appleFM: string
}>

export default function SettingsScreen() {
  const [hasKey, setHasKey] = useState<boolean | null>(null)
  const [readiness, setReadiness] = useState<Readiness>({
    appleFM: "checking",
    speech: "checking"
  })

  useEffect(() => {
    void loadKhalaApiKey().then(key => setHasKey(key !== null))
    void readNativeReadiness().then(result => {
      setReadiness({
        appleFM: result.appleFM.status,
        speech: result.speech.status
      })
    }).catch(() => {
      setReadiness({ appleFM: "unavailable", speech: "unavailable" })
    })
  }, [])

  return (
    <ScreenShell
      subtitle="Local device state"
      title="Settings"
    >
      <View className="gap-3 rounded-xl border border-border bg-surfaceRaised p-4">
        <Pill tone={hasKey ? "success" : "warning"}>
          {hasKey === null ? "checking" : hasKey ? "keychain" : "no key"}
        </Pill>
        <StatLine
          label="Update owner"
          value={KHALA_MOBILE_OTA_CONTRACT.owner}
        />
        <StatLine
          label="Channel"
          value={KHALA_MOBILE_OTA_CONTRACT.channel}
        />
        <StatLine
          label="Speech"
          value={readiness.speech}
        />
        <StatLine
          label="Apple FM"
          value={readiness.appleFM}
        />
      </View>
    </ScreenShell>
  )
}
