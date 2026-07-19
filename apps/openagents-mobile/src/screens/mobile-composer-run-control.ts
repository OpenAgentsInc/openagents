import { Badge, Button, IntentRef, Stack, StaticPayload, Text, type View } from "@effect-native/core"

import type { MobileAccessibilityProfile, KhalaRuntimeTurn } from "./khala-core"
import type { MobileRuntimeQueueReceipt } from "../conversation/mobile-runtime-queue"

export type MobileComposerRunAdmission = Readonly<{
  active: boolean
  badge: string | null
  detail: string | null
  placeholder: string
  submitLabel: string
  stopAvailable: boolean
  stopping: boolean
  confirming: boolean
  queueDetail: string | null
}>

export const projectMobileComposerRunAdmission = (input: Readonly<{
  turn: KhalaRuntimeTurn | null
  controlAvailable: boolean
  submittingAction: "cancel" | "close" | "resume" | "retry" | null
  stopConfirmationRunRef: string | null
  queueReceipt?: MobileRuntimeQueueReceipt | null
}>): MobileComposerRunAdmission => {
  const turn = input.turn
  const active = turn?.status === "queued" || turn?.status === "running" ||
    turn?.status === "waiting_for_input"
  if (!active || turn === null) {
    return {
      active: false,
      badge: null,
      detail: null,
      placeholder: "Continue conversation",
      submitLabel: "Send message",
      stopAvailable: false,
      stopping: false,
      confirming: false,
      queueDetail: null,
    }
  }
  const stopping = input.submittingAction === "cancel"
  const confirming = input.stopConfirmationRunRef === turn.runRef && !stopping
  const badge = turn.status === "queued"
    ? "Starting"
    : turn.status === "waiting_for_input" ? "Waiting" : "Running"
  const detail = stopping
    ? "Stop requested · awaiting confirmed runtime update."
    : confirming
      ? "Stop this turn? Running work may end before its next checkpoint."
      : !input.controlAvailable
        ? `${badge} · control is unavailable on this device.`
        : turn.status === "queued"
          ? "Start is awaiting runtime admission. You can keep drafting or stop the exact queued turn."
          : turn.status === "waiting_for_input"
            ? "Send queues a follow-up after this exact waiting turn. An empty composer action stops it."
            : "Send queues a follow-up after this exact running turn. An empty composer action stops it."
  return {
    active,
    badge,
    detail,
    placeholder: "Continue conversation",
    submitLabel: "Queue follow-up",
    stopAvailable: input.controlAvailable && input.submittingAction === null,
    stopping,
    confirming,
    queueDetail: (input.queueReceipt ?? null) === null
      ? null
      : input.queueReceipt!.outcome.admission.status === "accepted"
        ? "Admitted · delivery and promotion pending"
        : "Admission pending",
  }
}

export const renderMobileComposerRunControl = (
  turn: KhalaRuntimeTurn | null,
  admission: MobileComposerRunAdmission,
  accessibility: MobileAccessibilityProfile,
): ReadonlyArray<View> => {
  if (!admission.active || turn === null || admission.badge === null || admission.detail === null) return []
  return [Stack(
    {
      key: "mobile-composer-run-control",
      direction: "column",
      gap: "1",
      style: {
        width: "full",
        padding: "2",
        borderColor: "border",
        borderWidth: 1,
        borderRadius: "lg",
      },
      a11y: { role: "region", label: `${admission.badge} turn composer controls` },
    },
    [
      Stack(
        { key: "mobile-composer-run-status", direction: "row", gap: "2", align: "center" },
        [
          Badge({
            key: "mobile-composer-run-badge",
            label: admission.stopping ? "Stopping" : admission.badge,
            tone: admission.stopping || admission.confirming ? "warn" : "info",
          }),
          Text({
            key: "mobile-composer-run-detail",
            content: admission.detail,
            variant: "caption",
            color: admission.confirming ? "warning" : "textMuted",
            style: { flex: 1 },
          }),
        ],
      ),
      ...(admission.queueDetail === null
        ? []
        : [Stack(
            { key: "mobile-composer-queued-followup", direction: "row", gap: "2", align: "center" },
            [
              Badge({ key: "mobile-composer-queued-followup-badge", label: "Queued follow-up", tone: "info" }),
              Text({
                key: "mobile-composer-queued-followup-detail",
                content: admission.queueDetail,
                variant: "caption",
                color: "textMuted",
                style: { flex: 1 },
              }),
            ],
          )]),
      ...(admission.confirming
        ? [Stack(
            { key: "mobile-composer-stop-confirmation", direction: "row", gap: "2", align: "center" },
            [
              Button({
                key: "mobile-composer-stop-dismiss",
                label: "Keep running",
                variant: "ghost",
                onPress: IntentRef("RuntimeTurnStopConfirmationDismissed", StaticPayload({ runRef: turn.runRef })),
                style: { flex: 1, minHeight: accessibility.minTouchTarget },
              }),
              Button({
                key: "mobile-composer-stop-confirm",
                label: "Stop turn",
                variant: "secondary",
                onPress: IntentRef("RuntimeTurnStopConfirmed", StaticPayload({ runRef: turn.runRef })),
                style: { flex: 1, minHeight: accessibility.minTouchTarget },
                a11y: { label: "Confirm stop current turn" },
              }),
            ],
          )]
        : []),
    ],
  )]
}
