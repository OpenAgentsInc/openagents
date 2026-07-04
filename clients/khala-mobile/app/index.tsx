import { View } from "react-native"

import { NavigationTile, Pill, ScreenShell } from "../src/components/shell"
import { KHALA_MOBILE_OTA_CONTRACT } from "../src/config/updates"

export default function HomeScreen() {
  return (
    <ScreenShell
      subtitle="Owner companion"
      title="Khala"
    >
      <View>
        <Pill>{KHALA_MOBILE_OTA_CONTRACT.channel}</Pill>
      </View>
      <View className="gap-3">
        <NavigationTile
          detail="Owner-private threads"
          href="/chat"
          title="Chat"
        />
        <NavigationTile
          detail="Slots and assignments"
          href="/fleet"
          title="Fleet"
        />
        <NavigationTile
          detail="Device and updates"
          href="/settings"
          title="Settings"
        />
      </View>
    </ScreenShell>
  )
}
