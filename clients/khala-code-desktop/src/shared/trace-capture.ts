import { Effect } from "effect"
import type {
  KhalaPrivacyRedactionResult,
  KhalaPrivacyRedactionServiceShape,
} from "@openagentsinc/khala-tools"

export const KHALA_CODE_DESKTOP_TRACE_CAPTURE_DISCLOSURE_REF =
  "data.free_tier_capture_disclosure.v1"
export const KHALA_CODE_DESKTOP_TRACE_CAPTURE_PROMISE_ID =
  "khala_code.free_plan_trace_capture.v1"
export const KHALA_CODE_DESKTOP_TRACE_CAPTURE_SCHEMA_VERSION =
  "openagents.khala_code.desktop_trace_capture.v1"
export const KHALA_CODE_DESKTOP_TRACE_CAPTURE_OWNER_GATE_ENV =
  "KHALA_CODE_DESKTOP_TRACE_CAPTURE_ENABLED"
export const KHALA_CODE_DESKTOP_TRACE_CAPTURE_INGEST_AUDIENCE = "owner_only"

export type KhalaCodeDesktopTraceCapturePlanKind = "free" | "paid"
export type KhalaCodeDesktopTraceCaptureEventRole =
  | "assistant"
  | "system"
  | "tool"
  | "user"

export type KhalaCodeDesktopTraceCaptureSessionEvent = Readonly<{
  eventId: string
  observedAt: string
  role: KhalaCodeDesktopTraceCaptureEventRole
  sessionId: string
  sourceRef?: string
  text: string
}>

export type KhalaCodeDesktopTraceCaptureOwnerOnlyRecord = Readonly<{
  disclosureRef: typeof KHALA_CODE_DESKTOP_TRACE_CAPTURE_DISCLOSURE_REF
  eventId: string
  ingestAudience: typeof KHALA_CODE_DESKTOP_TRACE_CAPTURE_INGEST_AUDIENCE
  observedAt: string
  promiseId: typeof KHALA_CODE_DESKTOP_TRACE_CAPTURE_PROMISE_ID
  redaction: KhalaPrivacyRedactionResult
  role: KhalaCodeDesktopTraceCaptureEventRole
  schemaVersion: typeof KHALA_CODE_DESKTOP_TRACE_CAPTURE_SCHEMA_VERSION
  sessionId: string
  sourceRef?: string
}>

export type KhalaCodeDesktopTraceCaptureOwnerOnlyReceipt = Readonly<{
  receiptRef: string
}>

export type KhalaCodeDesktopTraceCaptureNotCapturedReason =
  | "consent_disabled"
  | "empty_event_text"
  | "owner_ingest_failed"
  | "owner_ingest_unavailable"
  | "owner_not_armed"
  | "paid_plan_capture_excluded"
  | "redaction_failed"
  | "unsupported_plan"

export type KhalaCodeDesktopTraceCaptureMarker = Readonly<{
  payoutEligible: false
  revenueShareEligible: false
  settlementEligible: false
}>

export type KhalaCodeDesktopTraceCaptureResult =
  | Readonly<{
    blockerRefs: readonly string[]
    captured: false
    disclosureRef: typeof KHALA_CODE_DESKTOP_TRACE_CAPTURE_DISCLOSURE_REF
    marker: KhalaCodeDesktopTraceCaptureMarker
    promiseId: typeof KHALA_CODE_DESKTOP_TRACE_CAPTURE_PROMISE_ID
    reason: KhalaCodeDesktopTraceCaptureNotCapturedReason
    state: "not_captured"
  }>
  | Readonly<{
    captured: true
    disclosureRef: typeof KHALA_CODE_DESKTOP_TRACE_CAPTURE_DISCLOSURE_REF
    ingestAudience: typeof KHALA_CODE_DESKTOP_TRACE_CAPTURE_INGEST_AUDIENCE
    marker: KhalaCodeDesktopTraceCaptureMarker
    ownerOnlyReceiptRef: string
    promiseId: typeof KHALA_CODE_DESKTOP_TRACE_CAPTURE_PROMISE_ID
    redactedEvent: KhalaCodeDesktopTraceCaptureOwnerOnlyRecord
    state: "captured"
  }>

export type KhalaCodeDesktopTraceCaptureInput = Readonly<{
  consentEnabled: boolean
  event: KhalaCodeDesktopTraceCaptureSessionEvent
  ownerArmed: boolean
  ownerOnlyIngest?: (
    record: KhalaCodeDesktopTraceCaptureOwnerOnlyRecord,
  ) => Promise<KhalaCodeDesktopTraceCaptureOwnerOnlyReceipt>
  planCaptureExcluded: boolean
  planKind: KhalaCodeDesktopTraceCapturePlanKind
  redaction: KhalaPrivacyRedactionServiceShape
}>

export const khalaCodeDesktopTraceCaptureMarker = ():
  KhalaCodeDesktopTraceCaptureMarker => ({
    payoutEligible: false,
    revenueShareEligible: false,
    settlementEligible: false,
  })

const notCaptured = (
  reason: KhalaCodeDesktopTraceCaptureNotCapturedReason,
  blockerRefs: readonly string[] = [],
): KhalaCodeDesktopTraceCaptureResult => ({
  blockerRefs,
  captured: false,
  disclosureRef: KHALA_CODE_DESKTOP_TRACE_CAPTURE_DISCLOSURE_REF,
  marker: khalaCodeDesktopTraceCaptureMarker(),
  promiseId: KHALA_CODE_DESKTOP_TRACE_CAPTURE_PROMISE_ID,
  reason,
  state: "not_captured",
})

const redactTraceEvent = async (
  event: KhalaCodeDesktopTraceCaptureSessionEvent,
  redaction: KhalaPrivacyRedactionServiceShape,
): Promise<KhalaPrivacyRedactionResult> => {
  const effect = event.role === "assistant"
    ? redaction.protectModelText(event.text)
    : redaction.protectUserText(event.text)
  return Effect.runPromise(effect)
}

export const captureKhalaCodeDesktopTraceEvent = async (
  input: KhalaCodeDesktopTraceCaptureInput,
): Promise<KhalaCodeDesktopTraceCaptureResult> => {
  if (!input.consentEnabled) return notCaptured("consent_disabled")
  if (input.planCaptureExcluded) return notCaptured("paid_plan_capture_excluded")
  if (input.planKind !== "free") return notCaptured("unsupported_plan")
  if (!input.ownerArmed) {
    return notCaptured("owner_not_armed", [
      "blocker.owner.khala_code_desktop_trace_capture_arming_missing",
    ])
  }
  if (input.event.text.trim().length === 0) return notCaptured("empty_event_text")
  if (input.ownerOnlyIngest === undefined) {
    return notCaptured("owner_ingest_unavailable", [
      "blocker.owner.khala_code_desktop_owner_only_ingest_sink_missing",
    ])
  }

  let redaction: KhalaPrivacyRedactionResult
  try {
    redaction = await redactTraceEvent(input.event, input.redaction)
  } catch {
    return notCaptured("redaction_failed", [
      "blocker.khala_code_desktop_trace_capture_redaction_failed",
    ])
  }

  const record: KhalaCodeDesktopTraceCaptureOwnerOnlyRecord = {
    disclosureRef: KHALA_CODE_DESKTOP_TRACE_CAPTURE_DISCLOSURE_REF,
    eventId: input.event.eventId,
    ingestAudience: KHALA_CODE_DESKTOP_TRACE_CAPTURE_INGEST_AUDIENCE,
    observedAt: input.event.observedAt,
    promiseId: KHALA_CODE_DESKTOP_TRACE_CAPTURE_PROMISE_ID,
    redaction,
    role: input.event.role,
    schemaVersion: KHALA_CODE_DESKTOP_TRACE_CAPTURE_SCHEMA_VERSION,
    sessionId: input.event.sessionId,
    ...(input.event.sourceRef === undefined ? {} : { sourceRef: input.event.sourceRef }),
  }

  try {
    const receipt = await input.ownerOnlyIngest(record)
    return {
      captured: true,
      disclosureRef: KHALA_CODE_DESKTOP_TRACE_CAPTURE_DISCLOSURE_REF,
      ingestAudience: KHALA_CODE_DESKTOP_TRACE_CAPTURE_INGEST_AUDIENCE,
      marker: khalaCodeDesktopTraceCaptureMarker(),
      ownerOnlyReceiptRef: receipt.receiptRef,
      promiseId: KHALA_CODE_DESKTOP_TRACE_CAPTURE_PROMISE_ID,
      redactedEvent: record,
      state: "captured",
    }
  } catch {
    return notCaptured("owner_ingest_failed", [
      "blocker.khala_code_desktop_trace_capture_owner_only_ingest_failed",
    ])
  }
}

export const khalaCodeDesktopTraceCaptureOwnerArmed = (
  env: Readonly<Record<string, string | undefined>>,
): boolean =>
  env[KHALA_CODE_DESKTOP_TRACE_CAPTURE_OWNER_GATE_ENV]?.trim() === "1"
