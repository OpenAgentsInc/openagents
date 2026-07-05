import { useMemo } from "react"
import { Text, View } from "react-native"

import { Pill, ScreenShell, StatLine } from "../components/shell"
import { createMobileKhalaSyncPreviewState } from "../sync/khala-sync-mobile"

export default function FleetScreen() {
  const { fleetRun } = useMemo(createMobileKhalaSyncPreviewState, [])

  return (
    <ScreenShell
      subtitle="Run status"
      title="Fleet"
    >
      <View className="gap-3 rounded-xl border border-border bg-surfaceRaised p-4">
        <View className="flex-row items-center justify-between gap-3">
          <Text className="shrink font-sans text-lg font-semibold text-text">
            {fleetRun.runId}
          </Text>
          <Pill tone="success">{fleetRun.status}</Pill>
        </View>
        <StatLine
          label="Desired slots"
          value={String(fleetRun.desiredSlots)}
        />
        <StatLine
          label="Active"
          value={String(fleetRun.counters.activeAssignments)}
        />
        <StatLine
          label="Completed"
          value={String(fleetRun.counters.completedAssignments)}
        />
        <StatLine
          label="Blocked"
          value={String(fleetRun.counters.blockedAssignments)}
        />
        <StatLine
          label="Work units"
          value={String(fleetRun.counters.workUnitsTotal)}
        />
      </View>
    </ScreenShell>
  )
}
