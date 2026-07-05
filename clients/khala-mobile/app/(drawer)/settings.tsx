import {
  decodeFleetAccountEntity,
  decodeFleetRunEntity,
  decodeFleetWorkerEntity,
  fleetRunScope,
  FLEET_ACCOUNT_ENTITY_TYPE,
  FLEET_RUN_ENTITY_TYPE,
  FLEET_WORKER_ENTITY_TYPE,
  type FleetAccountEntity,
  type FleetRunEntity,
  type FleetWorkerEntity
} from "@openagentsinc/khala-sync"
import { ScrollView, Text, View } from "react-native"
import Animated, { FadeIn } from "react-native-reanimated"
import { SafeAreaView } from "react-native-safe-area-context"

import { AppHeader } from "../../src/components/app-header"
import { KHALA_SYNC_DEMO_FLEET_RUN_ID } from "../../src/config/khala-sync-demo"
import {
  fleetAccountIdOf,
  fleetRunIdOf,
  fleetWorkerIdOf,
  formatAccountRefHash,
  sortAccountsByReadinessThenRef,
  sortWorkersByIdAsc
} from "../../src/sync/khala-fleet-collections-core"
import { useKhalaSyncCollection } from "../../src/sync/use-khala-sync-collection"
import { MOTION_MEDIUM, MOTION_STAGGER_MS } from "../../src/theme/motion"

const READINESS_COLOR: Record<FleetAccountEntity["readiness"], string> = {
  cooldown: "text-warning",
  ready: "text-success",
  unavailable: "text-danger",
  unknown: "text-textFaint"
}

const SectionLabel = ({ children }: { children: string }) => (
  <Text className="mb-2 font-mono text-xs uppercase tracking-wide text-textFaint">
    {children}
  </Text>
)

const FleetSection = () => {
  const scope = KHALA_SYNC_DEMO_FLEET_RUN_ID === "" ? "" : String(fleetRunScope(KHALA_SYNC_DEMO_FLEET_RUN_ID))

  const runState = useKhalaSyncCollection(scope, FLEET_RUN_ENTITY_TYPE, decodeFleetRunEntity, fleetRunIdOf)
  const workerState = useKhalaSyncCollection(
    scope,
    FLEET_WORKER_ENTITY_TYPE,
    decodeFleetWorkerEntity,
    fleetWorkerIdOf
  )
  const accountState = useKhalaSyncCollection(
    scope,
    FLEET_ACCOUNT_ENTITY_TYPE,
    decodeFleetAccountEntity,
    fleetAccountIdOf
  )

  if (KHALA_SYNC_DEMO_FLEET_RUN_ID === "") {
    return (
      <View className="gap-2">
        <SectionLabel>Fleet</SectionLabel>
        <Text className="font-sans text-sm text-textMuted">
          No fleet run configured. Set
          EXPO_PUBLIC_KHALA_SYNC_DEMO_FLEET_RUN_ID to the active Khala Code
          fleet run id to see it here.
        </Text>
      </View>
    )
  }

  const run: FleetRunEntity | undefined = runState.items[0]
  const workers = sortWorkersByIdAsc(workerState.items)
  const accounts = sortAccountsByReadinessThenRef(accountState.items)

  return (
    <View className="gap-4">
      <View className="gap-2">
        <SectionLabel>Fleet run</SectionLabel>
        {runState.status === "error" ? (
          <Text className="font-sans text-sm text-danger">{runState.error}</Text>
        ) : run === undefined ? (
          <Text className="font-sans text-sm text-textMuted">
            {runState.status === "loading" ? "loading…" : "No run data for this id yet"}
          </Text>
        ) : (
          <Animated.View
            className="rounded-xl border border-border bg-surfaceRaised p-3"
            entering={FadeIn.duration(MOTION_MEDIUM)}
          >
            <View className="flex-row items-center justify-between">
              <Text className="font-sans text-base font-semibold text-text">{run.status}</Text>
              <Text className="font-mono text-xs text-textFaint">{run.workerKind}</Text>
            </View>
            <Text className="mt-1 font-mono text-xs text-textMuted">
              {run.desiredSlots} desired slots · {run.counters.activeAssignments} active ·{" "}
              {run.counters.completedAssignments} completed · {run.counters.failedAssignments} failed ·{" "}
              {run.counters.blockedAssignments} blocked
            </Text>
          </Animated.View>
        )}
      </View>

      <View className="gap-2">
        <SectionLabel>Connected accounts</SectionLabel>
        {accounts.length === 0 ? (
          <Text className="font-sans text-sm text-textMuted">
            {accountState.status === "loading" ? "loading…" : "No connected accounts synced yet"}
          </Text>
        ) : (
          accounts.map((account: FleetAccountEntity, index) => (
            <Animated.View
              className="rounded-xl border border-border bg-surfaceRaised px-3 py-2"
              entering={FadeIn.delay(MOTION_STAGGER_MS * index).duration(MOTION_MEDIUM)}
              key={account.accountRefHash}
            >
              <View className="flex-row items-center justify-between">
                <Text className="font-mono text-sm text-text">
                  {formatAccountRefHash(account.accountRefHash)}
                  {account.provider === undefined ? "" : ` · ${account.provider}`}
                </Text>
                <Text className={`font-mono text-xs ${READINESS_COLOR[account.readiness]}`}>
                  {account.readiness}
                  {account.rateLimitClass === undefined ? "" : ` · ${account.rateLimitClass}`}
                </Text>
              </View>
              {account.capacityAvailable === undefined &&
              account.capacityBusy === undefined &&
              account.capacityQueued === undefined ? null : (
                <Text className="mt-1 font-mono text-xs text-textFaint">
                  {account.capacityAvailable ?? 0} available ·{" "}
                  {account.capacityBusy ?? 0} busy · {account.capacityQueued ?? 0} queued
                </Text>
              )}
            </Animated.View>
          ))
        )}
      </View>

      <View className="gap-2">
        <SectionLabel>Workers</SectionLabel>
        {workers.length === 0 ? (
          <Text className="font-sans text-sm text-textMuted">
            {workerState.status === "loading" ? "loading…" : "No worker slots synced yet"}
          </Text>
        ) : (
          workers.map((worker: FleetWorkerEntity, index) => (
            <Animated.View
              className="flex-row items-center justify-between rounded-xl border border-border bg-surfaceRaised px-3 py-2"
              entering={FadeIn.delay(MOTION_STAGGER_MS * index).duration(MOTION_MEDIUM)}
              key={worker.workerId}
            >
              <Text className="font-mono text-sm text-text">{worker.workerId}</Text>
              <Text className="font-mono text-xs text-textFaint">{worker.phase}</Text>
            </Animated.View>
          ))
        )}
      </View>

      <Text className="font-mono text-xs text-textFaint">
        Account identity is a public-safe hashed ref only — no email or raw
        credential ever leaves the desktop, just readiness, provider, and
        dispatch-slot capacity.
      </Text>
    </View>
  )
}

export default function SettingsScreen() {
  return (
    <SafeAreaView className="flex-1 bg-bg" edges={["top", "bottom", "left", "right"]}>
      <AppHeader showMenu title="Settings" />
      <ScrollView contentContainerClassName="gap-6 px-4 py-4">
        <FleetSection />
      </ScrollView>
    </SafeAreaView>
  )
}
