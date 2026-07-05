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
import { Pressable, ScrollView, Text, View } from "react-native"
import Animated, { FadeIn } from "react-native-reanimated"
import { SafeAreaView } from "react-native-safe-area-context"

import { useKhalaAuth } from "../../src/auth/khala-auth-context"
import { AppHeader } from "../../src/components/app-header"
import { Frame, usePowerOnVisible } from "../../src/components/frame"
import { KHALA_SYNC_DEMO_FLEET_RUN_ID } from "../../src/config/khala-sync-demo"
import type { OnDeviceReadinessRow } from "../../src/native/on-device-readiness-core"
import { useOnDeviceReadiness } from "../../src/native/use-on-device-readiness"
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

/** Fleet run status card, framed with the ported Arwes `Frame` chrome (see
 * `docs/design/2026-07-05-arcade-ui-harvest-audit.md` §2.1) so it visually
 * "powers on" once real fleet-run data lands, per the harvest issue's own
 * guidance to reserve `Frame` for primary/active surfaces. */
const FleetRunCard = ({ run }: { run: FleetRunEntity }) => {
  const visible = usePowerOnVisible()
  return (
    <Frame alwaysShowBorder visible={visible}>
      <View className="p-3">
        <View className="flex-row items-center justify-between">
          <Text className="font-sans text-base font-semibold text-text">{run.status}</Text>
          <Text className="font-mono text-xs text-textFaint">{run.workerKind}</Text>
        </View>
        <Text className="mt-1 font-mono text-xs text-textMuted">
          {run.desiredSlots} desired slots · {run.counters.activeAssignments} active ·{" "}
          {run.counters.completedAssignments} completed · {run.counters.failedAssignments} failed ·{" "}
          {run.counters.blockedAssignments} blocked
        </Text>
      </View>
    </Frame>
  )
}

/** Connected fleet-account card, same "power on" `Frame` treatment as
 * `FleetRunCard`, staggered per row via `MOTION_STAGGER_MS * index` to match
 * the existing entrance-stagger convention used elsewhere on this screen. */
const AccountCard = ({ account, index }: { account: FleetAccountEntity; index: number }) => {
  const visible = usePowerOnVisible(MOTION_STAGGER_MS * index)
  return (
    <Frame alwaysShowBorder visible={visible}>
      <View className="px-3 py-2">
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
      </View>
    </Frame>
  )
}

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
          <FleetRunCard run={run} />
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
            <AccountCard account={account} index={index} key={account.accountRefHash} />
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

const AccountSection = () => {
  const { ownerUserId, signOut } = useKhalaAuth()
  return (
    <View className="gap-2">
      <SectionLabel>Account</SectionLabel>
      <Text className="font-mono text-xs text-textFaint" numberOfLines={1}>
        {ownerUserId}
      </Text>
      <Pressable
        accessibilityRole="button"
        className="items-center rounded-xl border border-border bg-surfaceRaised py-3"
        onPress={() => void signOut()}
      >
        <Text className="font-sans text-sm text-danger">Sign out</Text>
      </Pressable>
    </View>
  )
}

const ON_DEVICE_TONE_COLOR: Record<OnDeviceReadinessRow["tone"], string> = {
  danger: "text-danger",
  faint: "text-textFaint",
  success: "text-success",
  warning: "text-warning"
}

/** Speech (push-to-talk) + Apple Foundation Models readiness, ported from
 * `src/legacy-screens/settings.tsx` (issue #8350's TS-8 gap: those two Expo
 * native modules were real and tested but only ever rendered on a screen
 * `app/` never routed to). Same "power on" `Frame` treatment as the fleet
 * section above. Read-only status, same as the legacy screen — the speech
 * module's actual dictation call is wired into the chat composer's mic
 * button (`src/components/chat-composer.tsx`), not here; this section is
 * purely the availability probe both modules already exposed. */
const OnDeviceCard = ({ row, index }: { row: OnDeviceReadinessRow; index: number }) => {
  const visible = usePowerOnVisible(MOTION_STAGGER_MS * index)
  return (
    <Frame alwaysShowBorder visible={visible}>
      <View className="px-3 py-2">
        <View className="flex-row items-center justify-between">
          <Text className="font-sans text-sm text-text">{row.label}</Text>
          <Text className={`font-mono text-xs ${ON_DEVICE_TONE_COLOR[row.tone]}`}>{row.status}</Text>
        </View>
        {row.detail === undefined ? null : (
          <Text className="mt-1 font-mono text-xs text-textFaint">{row.detail}</Text>
        )}
      </View>
    </Frame>
  )
}

const OnDeviceSection = () => {
  const readiness = useOnDeviceReadiness()
  return (
    <View className="gap-2">
      <SectionLabel>On-device</SectionLabel>
      {readiness.status === "loading" ? (
        <Text className="font-sans text-sm text-textMuted">checking…</Text>
      ) : readiness.status === "error" ? (
        <Text className="font-sans text-sm text-danger">Could not read native module readiness.</Text>
      ) : (
        readiness.rows.map((row, index) => <OnDeviceCard index={index} key={row.key} row={row} />)
      )}
    </View>
  )
}

export default function SettingsScreen() {
  return (
    <SafeAreaView className="flex-1 bg-bg" edges={["top", "bottom", "left", "right"]}>
      <AppHeader showMenu title="Settings" />
      <ScrollView contentContainerClassName="gap-6 px-4 py-4">
        <FleetSection />
        <OnDeviceSection />
        <AccountSection />
      </ScrollView>
    </SafeAreaView>
  )
}
