import {
  decodeFleetApprovalEntity,
  decodeFleetRunEntity,
  decodeFleetSteerEntity,
  decodeFleetWorkerEntity,
  FLEET_APPROVAL_ENTITY_TYPE,
  FLEET_RUN_ENTITY_TYPE,
  FLEET_STEER_ENTITY_TYPE,
  FLEET_WORKER_ENTITY_TYPE,
  fleetRunScope,
} from "@openagentsinc/khala-sync"
import type { FleetRunControlAction } from "@openagentsinc/khala-fleet-intents"
import {
  fleetDispatchApprovalDecisionClientMutator,
  fleetDispatchRunControlClientMutator,
  fleetDispatchSteerMessageClientMutator,
} from "@openagentsinc/khala-sync-db-collection"
import type { DrawerNavigationProp } from "@react-navigation/drawer"
import { Effect } from "effect"
import { useCallback, useMemo, useState } from "react"
import {
  ScrollView,
  TouchableOpacity,
  View,
  type TextStyle,
  type ViewStyle,
} from "react-native"

import { useKhalaAuth } from "../auth/khala-auth-context"
import { Card, EmptyState, Header, Text, useAppTheme } from "../ignite"
import type { ThemedStyle } from "../ignite"
import type { AppDrawerParamList, AppDrawerScreenProps } from "../navigators/navigationTypes"
import {
  deriveFleetPeekViewModel,
  makeApprovalDecisionIntent,
  makeRunControlIntent,
  type FleetIntentIds,
} from "../sync/fleet-peek-core"
import { useKhalaMobileSyncPrimitives } from "../sync/khala-mobile-sync-runtime-context"
import { makeSafeRef } from "../sync/khala-sync-push-core"
import { useKhalaSyncScopeEntities } from "../sync/use-khala-sync-scope-entities"

/**
 * MH-6 (#8585): the mobile fleet peek. It OBSERVES the desktop-authority
 * projection of a FleetRun (run status, per-harness worker cards, pending
 * approvals, steer receipts) over Khala Sync, and lets the operator dispatch
 * the three MH-0 typed steering intents — pause/resume/drain/stop, allow/deny
 * a pending tool — by handing a `KhalaFleetIntent` to `session.mutate`. The
 * phone is never a second supervisor; the desktop/daemon authority observes
 * the intent (via the steering-intent watermark) and enforces it. Pure
 * derivation + intent construction lives in `fleet-peek-core.ts`.
 */

// The dogfood run ref — the desktop fixture FleetRun the phone peeks at. A
// richer run picker is MH-7's cockpit; MH-6 proves the steering loop.
const DOGFOOD_RUN_REF = "fleet.mh6.dogfood"

const nextIds = (prefix: string): FleetIntentIds => {
  const intentId = makeSafeRef(`intent.${prefix}`)
  return {
    createdAt: new Date().toISOString(),
    idempotencyKey: intentId,
    intentId,
  }
}

type FleetPeekScreenProps = AppDrawerScreenProps<"FleetPeek">

const RUN_CONTROL_LABEL: Record<FleetRunControlAction, string> = {
  drain: "Drain",
  pause: "Pause",
  resume: "Resume",
  stop: "Stop",
}

export const FleetPeekScreen = ({ navigation }: FleetPeekScreenProps) => {
  const { themed } = useAppTheme()
  const { overlay, session, status: syncStatus, store } =
    useKhalaMobileSyncPrimitives()
  const { demoMode } = useKhalaAuth()
  const [actionError, setActionError] = useState<string | null>(null)
  const [busyAction, setBusyAction] = useState<string | null>(null)

  const scope = String(fleetRunScope(DOGFOOD_RUN_REF))

  const runState = useKhalaSyncScopeEntities({
    decode: decodeFleetRunEntity,
    entityType: FLEET_RUN_ENTITY_TYPE,
    overlay,
    scope,
    session,
    store,
  })
  const workerState = useKhalaSyncScopeEntities({
    decode: decodeFleetWorkerEntity,
    entityType: FLEET_WORKER_ENTITY_TYPE,
    overlay,
    scope,
    session,
    store,
  })
  const approvalState = useKhalaSyncScopeEntities({
    decode: decodeFleetApprovalEntity,
    entityType: FLEET_APPROVAL_ENTITY_TYPE,
    overlay,
    scope,
    session,
    store,
  })
  const steerState = useKhalaSyncScopeEntities({
    decode: decodeFleetSteerEntity,
    entityType: FLEET_STEER_ENTITY_TYPE,
    overlay,
    scope,
    session,
    store,
  })

  const vm = useMemo(
    () =>
      deriveFleetPeekViewModel({
        approvals: approvalState.items,
        run: runState.items[0] ?? null,
        steers: steerState.items,
        workers: workerState.items,
      }),
    [approvalState.items, runState.items, steerState.items, workerState.items],
  )

  const dispatchIntent = useCallback(
    async (label: string, effect: () => Effect.Effect<unknown, unknown>) => {
      if (session === null) {
        setActionError("Sync is not ready yet.")
        return
      }
      setBusyAction(label)
      setActionError(null)
      try {
        await Effect.runPromise(effect())
      } catch (error) {
        setActionError(error instanceof Error ? error.message : String(error))
      } finally {
        setBusyAction(null)
      }
    },
    [session],
  )

  const onRunControl = (action: FleetRunControlAction) =>
    void dispatchIntent(`run:${action}`, () =>
      session!.mutate(
        fleetDispatchRunControlClientMutator,
        makeRunControlIntent({ action, ids: nextIds(action), runRef: DOGFOOD_RUN_REF }),
      ),
    )

  const onApproval = (approvalRef: string, decision: "allow" | "deny") =>
    void dispatchIntent(`approval:${approvalRef}:${decision}`, () =>
      session!.mutate(
        fleetDispatchApprovalDecisionClientMutator,
        makeApprovalDecisionIntent({
          approvalRef,
          decision,
          ids: nextIds(`approval.${decision}`),
          runRef: DOGFOOD_RUN_REF,
        }),
      ),
    )

  return (
    <ScrollView contentContainerStyle={themed($container)}>
      <Header
        title="Fleet"
        leftIcon="☰"
        onLeftPress={() =>
          navigation.getParent<DrawerNavigationProp<AppDrawerParamList>>()?.openDrawer()
        }
      />
      {demoMode ? (
        <View style={themed($banner)}>
          <Text size="xxs" style={themed($accent)} text="Demo mode — example data" />
        </View>
      ) : null}

      <Card
        style={themed($card)}
        ContentComponent={
          <View style={themed($runRow)}>
            <View style={themed($grow)}>
              <Text preset="subheading" text={DOGFOOD_RUN_REF} />
              <Text
                size="xs"
                style={themed($dim)}
                text={`status ${vm.runStatus} · slots ${vm.desiredSlots} · ${
                  syncStatus === "ready" ? "live" : "syncing"
                }`}
              />
            </View>
          </View>
        }
      />

      <View style={themed($pillRow)}>
        {(["codex", "claude", "grok"] as const).map((h) => (
          <View key={h} style={themed($pill)}>
            <Text size="xxs" style={themed($accent)} text={`${h} ${vm.harnessCounts[h]}`} />
          </View>
        ))}
      </View>

      <View style={themed($actionRow)}>
        {vm.availableRunControls.map((action) => (
          <TouchableOpacity
            key={action}
            accessibilityRole="button"
            accessibilityLabel={`${RUN_CONTROL_LABEL[action]} run`}
            activeOpacity={0.8}
            disabled={busyAction !== null || session === null}
            hitSlop={8}
            onPress={() => onRunControl(action)}
            style={themed($actionButton)}
          >
            <Text size="xs" weight="medium" style={themed($accent)} text={RUN_CONTROL_LABEL[action]} />
          </TouchableOpacity>
        ))}
      </View>

      {actionError === null ? null : (
        <Text size="xs" style={themed($danger)} text={actionError} />
      )}

      <Text preset="formLabel" text="Workers" style={themed($sectionLabel)} />
      {vm.workers.length === 0 ? (
        <EmptyState heading="No workers yet" />
      ) : (
        vm.workers.map((w) => (
          <Card
            key={w.workerId}
            style={themed($card)}
            ContentComponent={
              <View style={themed($runRow)}>
                <View style={themed($grow)}>
                  <Text size="sm" weight="medium" text={w.workerId} numberOfLines={1} />
                  <Text size="xs" style={themed($dim)} text={`${w.harness} · ${w.phase}`} />
                </View>
              </View>
            }
          />
        ))
      )}

      <Text preset="formLabel" text="Pending approvals" style={themed($sectionLabel)} />
      {vm.pendingApprovals.length === 0 ? (
        <EmptyState heading="Nothing waiting on you" />
      ) : (
        vm.pendingApprovals.map((a) => (
          <Card
            key={a.approvalRef}
            style={themed($card)}
            ContentComponent={
              <View style={themed($grow)}>
                <Text size="sm" weight="medium" text={a.approvalRef} numberOfLines={1} />
                <Text
                  size="xs"
                  style={themed($dim)}
                  text={`${a.toolClass ?? "tool"}${a.workerId === undefined ? "" : ` · ${a.workerId}`}`}
                />
                <View style={themed($actionRow)}>
                  <TouchableOpacity
                    accessibilityRole="button"
                    accessibilityLabel={`Allow ${a.approvalRef}`}
                    activeOpacity={0.8}
                    disabled={busyAction !== null || session === null}
                    onPress={() => onApproval(a.approvalRef, "allow")}
                    style={themed($actionButton)}
                  >
                    <Text size="xs" weight="medium" style={themed($accent)} text="Allow" />
                  </TouchableOpacity>
                  <TouchableOpacity
                    accessibilityRole="button"
                    accessibilityLabel={`Deny ${a.approvalRef}`}
                    activeOpacity={0.8}
                    disabled={busyAction !== null || session === null}
                    onPress={() => onApproval(a.approvalRef, "deny")}
                    style={themed($actionButtonDanger)}
                  >
                    <Text size="xs" weight="medium" style={themed($danger)} text="Deny" />
                  </TouchableOpacity>
                </View>
              </View>
            }
          />
        ))
      )}
    </ScrollView>
  )
}

const $container: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  gap: spacing.sm,
  paddingBottom: spacing.xl,
})

const $banner: ThemedStyle<ViewStyle> = ({ colors, spacing }) => ({
  alignItems: "center",
  backgroundColor: colors.palette.neutral200,
  paddingVertical: spacing.xxs,
})

const $card: ThemedStyle<ViewStyle> = ({ colors, spacing }) => ({
  backgroundColor: colors.palette.neutral200,
  borderColor: colors.palette.neutral400,
  marginHorizontal: spacing.md,
})

const $runRow: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  alignItems: "center",
  flexDirection: "row",
  gap: spacing.sm,
  justifyContent: "space-between",
})

const $grow: ThemedStyle<ViewStyle> = ({ spacing }) => ({ flex: 1, gap: spacing.xxxs })

const $pillRow: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  flexDirection: "row",
  gap: spacing.xs,
  paddingHorizontal: spacing.md,
})

const $pill: ThemedStyle<ViewStyle> = ({ colors, spacing }) => ({
  backgroundColor: colors.palette.neutral200,
  borderColor: colors.palette.neutral400,
  borderRadius: 999,
  borderWidth: 1,
  paddingHorizontal: spacing.sm,
  paddingVertical: spacing.xxs,
})

const $actionRow: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  flexDirection: "row",
  flexWrap: "wrap",
  gap: spacing.xs,
  paddingHorizontal: spacing.md,
  paddingTop: spacing.xs,
})

const $actionButton: ThemedStyle<ViewStyle> = ({ colors, spacing }) => ({
  borderColor: colors.tint,
  borderRadius: 8,
  borderWidth: 1,
  paddingHorizontal: spacing.sm,
  paddingVertical: spacing.xs,
})

const $actionButtonDanger: ThemedStyle<ViewStyle> = ({ colors, spacing }) => ({
  borderColor: colors.error,
  borderRadius: 8,
  borderWidth: 1,
  paddingHorizontal: spacing.sm,
  paddingVertical: spacing.xs,
})

const $sectionLabel: ThemedStyle<TextStyle> = ({ spacing }) => ({
  paddingHorizontal: spacing.md,
  paddingTop: spacing.sm,
})

const $dim: ThemedStyle<TextStyle> = ({ colors }) => ({ color: colors.textDim })
const $accent: ThemedStyle<TextStyle> = ({ colors }) => ({ color: colors.tint })
const $danger: ThemedStyle<TextStyle> = ({ colors }) => ({ color: colors.error })
