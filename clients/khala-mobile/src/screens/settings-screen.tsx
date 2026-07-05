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
  type FleetWorkerEntity,
} from "@openagentsinc/khala-sync"
import { ScrollView, View } from "react-native"
import Animated, { FadeIn } from "react-native-reanimated"

import { useKhalaAuth } from "../auth/khala-auth-context"
import { AppHeader } from "../components/app-header"
import { Frame, usePowerOnVisible } from "../components/frame"
import { KhalaButton } from "../components/khala-button"
import { KhalaScreen } from "../components/khala-screen"
import { KhalaText } from "../components/khala-text"
import { KHALA_SYNC_DEMO_FLEET_RUN_ID } from "../config/khala-sync-demo"
import type { OnDeviceReadinessRow } from "../native/on-device-readiness-core"
import { useOnDeviceReadiness } from "../native/use-on-device-readiness"
import {
  fleetAccountIdOf,
  fleetRunIdOf,
  fleetWorkerIdOf,
  formatAccountRefHash,
  sortAccountsByReadinessThenRef,
  sortWorkersByIdAsc,
} from "../sync/khala-fleet-collections-core"
import { useKhalaSyncCollection } from "../sync/use-khala-sync-collection"
import { MOTION_MEDIUM, MOTION_STAGGER_MS } from "../theme/motion"

const READINESS_COLOR: Record<FleetAccountEntity["readiness"], string> = {
  cooldown: "text-warning",
  ready: "text-success",
  unavailable: "text-danger",
  unknown: "text-textFaint",
}

const SectionLabel = ({ children }: { children: string }) => (
  <KhalaText className="mb-2" variant="label">
    {children}
  </KhalaText>
)

const FleetRunCard = ({ run }: { run: FleetRunEntity }) => {
  const visible = usePowerOnVisible()
  return (
    <Frame alwaysShowBorder visible={visible}>
      <View className="p-3">
        <View className="flex-row items-center justify-between">
          <KhalaText className="font-semibold" text={run.status} />
          <KhalaText text={run.workerKind} variant="faint" />
        </View>
        <KhalaText className="mt-1 text-textMuted" variant="faint">
          {run.desiredSlots} desired slots · {run.counters.activeAssignments} active ·{" "}
          {run.counters.completedAssignments} completed · {run.counters.failedAssignments} failed ·{" "}
          {run.counters.blockedAssignments} blocked
        </KhalaText>
      </View>
    </Frame>
  )
}

const AccountCard = ({ account, index }: { account: FleetAccountEntity; index: number }) => {
  const visible = usePowerOnVisible(MOTION_STAGGER_MS * index)
  return (
    <Frame alwaysShowBorder visible={visible}>
      <View className="px-3 py-2">
        <View className="flex-row items-center justify-between">
          <KhalaText variant="mono">
            {formatAccountRefHash(account.accountRefHash)}
            {account.provider === undefined ? "" : ` · ${account.provider}`}
          </KhalaText>
          <KhalaText className={READINESS_COLOR[account.readiness]} variant="faint">
            {account.readiness}
            {account.rateLimitClass === undefined ? "" : ` · ${account.rateLimitClass}`}
          </KhalaText>
        </View>
        {account.capacityAvailable === undefined &&
        account.capacityBusy === undefined &&
        account.capacityQueued === undefined ? null : (
          <KhalaText className="mt-1" variant="faint">
            {account.capacityAvailable ?? 0} available ·{" "}
            {account.capacityBusy ?? 0} busy · {account.capacityQueued ?? 0} queued
          </KhalaText>
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
    fleetWorkerIdOf,
  )
  const accountState = useKhalaSyncCollection(
    scope,
    FLEET_ACCOUNT_ENTITY_TYPE,
    decodeFleetAccountEntity,
    fleetAccountIdOf,
  )

  if (KHALA_SYNC_DEMO_FLEET_RUN_ID === "") {
    return (
      <View className="gap-2">
        <SectionLabel>Fleet</SectionLabel>
        <KhalaText variant="muted">
          No fleet run configured. Set
          EXPO_PUBLIC_KHALA_SYNC_DEMO_FLEET_RUN_ID to the active Khala Code
          fleet run id to see it here.
        </KhalaText>
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
          <KhalaText text={runState.error ?? "Could not load fleet run."} variant="danger" />
        ) : run === undefined ? (
          <KhalaText variant="muted">
            {runState.status === "loading" ? "loading…" : "No run data for this id yet"}
          </KhalaText>
        ) : (
          <FleetRunCard run={run} />
        )}
      </View>

      <View className="gap-2">
        <SectionLabel>Connected accounts</SectionLabel>
        {accounts.length === 0 ? (
          <KhalaText variant="muted">
            {accountState.status === "loading" ? "loading…" : "No connected accounts synced yet"}
          </KhalaText>
        ) : (
          accounts.map((account: FleetAccountEntity, index) => (
            <AccountCard account={account} index={index} key={account.accountRefHash} />
          ))
        )}
      </View>

      <View className="gap-2">
        <SectionLabel>Workers</SectionLabel>
        {workers.length === 0 ? (
          <KhalaText variant="muted">
            {workerState.status === "loading" ? "loading…" : "No worker slots synced yet"}
          </KhalaText>
        ) : (
          workers.map((worker: FleetWorkerEntity, index) => (
            <Animated.View
              className="flex-row items-center justify-between rounded-xl border border-border bg-surfaceRaised px-3 py-2"
              entering={FadeIn.delay(MOTION_STAGGER_MS * index).duration(MOTION_MEDIUM)}
              key={worker.workerId}
            >
              <KhalaText text={worker.workerId} variant="mono" />
              <KhalaText text={worker.phase} variant="faint" />
            </Animated.View>
          ))
        )}
      </View>

      <KhalaText variant="faint">
        Account identity is a public-safe hashed ref only — no email or raw
        credential ever leaves the desktop, just readiness, provider, and
        dispatch-slot capacity.
      </KhalaText>
    </View>
  )
}

const AccountSection = () => {
  const { ownerUserId, signOut } = useKhalaAuth()
  return (
    <View className="gap-2">
      <SectionLabel>Account</SectionLabel>
      <KhalaText numberOfLines={1} variant="faint">
        {ownerUserId}
      </KhalaText>
      <KhalaButton onPress={() => void signOut()} text="Sign out" variant="danger" />
    </View>
  )
}

const ON_DEVICE_TONE_COLOR: Record<OnDeviceReadinessRow["tone"], string> = {
  danger: "text-danger",
  faint: "text-textFaint",
  success: "text-success",
  warning: "text-warning",
}

const OnDeviceCard = ({ row, index }: { row: OnDeviceReadinessRow; index: number }) => {
  const visible = usePowerOnVisible(MOTION_STAGGER_MS * index)
  return (
    <Frame alwaysShowBorder visible={visible}>
      <View className="px-3 py-2">
        <View className="flex-row items-center justify-between">
          <KhalaText text={row.label} variant="caption" />
          <KhalaText className={ON_DEVICE_TONE_COLOR[row.tone]} text={row.status} variant="faint" />
        </View>
        {row.detail === undefined ? null : (
          <KhalaText className="mt-1" text={row.detail} variant="faint" />
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
        <KhalaText text="checking…" variant="muted" />
      ) : readiness.status === "error" ? (
        <KhalaText text="Could not read native module readiness." variant="danger" />
      ) : (
        readiness.rows.map((row, index) => <OnDeviceCard index={index} key={row.key} row={row} />)
      )}
    </View>
  )
}

export const SettingsScreen = () => (
  <KhalaScreen preset="fixed">
    <AppHeader showMenu title="Settings" />
    <ScrollView contentContainerClassName="gap-6 px-4 py-4">
      <FleetSection />
      <OnDeviceSection />
      <AccountSection />
    </ScrollView>
  </KhalaScreen>
)
