import {
  SARAH_VOICE_ALPHA_COHORT_REF,
  decodeSarahEditorCommand,
  sarahEditorCommandRequiresConfirmation,
  type SarahEditorCommand,
  validateSarahEditorCommandTarget,
} from "@openagentsinc/audio-contract";
import { createHash } from "node:crypto";
import type { SyncSql, SyncTransactionSql } from "./sql.js";

export type SarahVoiceSessionState =
  | "reserved"
  | "connected"
  | "accounting_uncertain"
  | "settled"
  | "released"
  | "failed";
export type SarahVoiceClientProfile =
  | "omega_editor"
  | "mobile_voice_only"
  | "mobile_command_center";
export type SarahVoiceCreditMode =
  | "metered"
  | "staging_owner_entitlement"
  | "owner_waived_unmetered";
export type SarahVoiceTransportKind = "custom_wss_v1" | "livekit_room_v1";
export const SARAH_LIVEKIT_MAX_ACTIVE_ROOMS = 20;

export type SarahVoiceCreditEntitlement = Readonly<{
  entitlementRef: string;
  ownerUserId: string;
  expiresAt: string;
}>;

export type SarahVoiceAlphaMembership = Readonly<{
  membershipRef: string;
  cohortRef: string;
  ownerUserId: string;
}>;

export type SarahVoiceAdmissionRecord = Readonly<{
  admissionRef: string;
  admissionExpiresAt: string;
}>;

export type SarahVoiceSettlementProjection =
  | Readonly<{
      sessionRef: string;
      state: "settled" | "released";
      creditMode: SarahVoiceCreditMode;
      finalChargeMsat: number;
      spendableRemainingCreditMsat: number | null;
      settlementReceiptRef: string;
      acceptanceEvidence?: SarahVoiceLiveKitAcceptanceEvidence;
      accountingWaiver?: Readonly<{
        authority: "owner_waived_unmetered_v1";
        providerAccountingStatus: "uncertain";
        waiverReceiptRef: string;
        waiverPayloadDigest: string;
        providerEvidenceDigest: string;
      }>;
      unmeteredAuthorityCapture?: SarahVoiceUnmeteredAuthorityCapture;
    }>
  | Readonly<{
      sessionRef: string;
      state: "accounting_uncertain";
      creditMode: SarahVoiceCreditMode;
      recordedChargeMsat: number;
      reservedMsat: number;
      holdPreserved: boolean;
      noHoldCreated: boolean;
      reason: string;
      accountingEscalation?: Readonly<{
        escalationRef: string;
        escalatedAt: string;
      }>;
      failureEvidence?: SarahVoiceLiveKitFailureEvidence;
      unmeteredAuthorityCapture?: SarahVoiceUnmeteredAuthorityCapture;
    }>;

export type SarahVoiceLiveKitFailureEvidence = Readonly<{
  principal: "principal.sarah";
  generation: number;
  identityDigests: Readonly<{
    job: string;
    providerSession: string;
    providerConfiguration: string;
    hold: string;
    usage: string;
  }>;
  usage: Readonly<{
    inputTokens: number;
    outputTokens: number;
    cachedInputTokens: number;
    audioInputTokens: number;
    audioOutputTokens: number;
    recordedChargeMsat: number;
    responseCount: number;
    transcriptionCount: number;
    cancelledResponseCount: number;
  }>;
  providerAccountingStatus: "uncertain";
  workerJobCount: number;
  providerSessionCount: number;
  providerAdmittedAt: string;
  workerClosedAt: string | null;
  holdPreserved: boolean;
  noHoldCreated: boolean;
}>;

export type SarahVoiceLiveKitAcceptanceEvidence = Readonly<{
  principal: "principal.sarah";
  identityDigests: Readonly<{
    job: string;
    providerSession: string;
    providerConfiguration: string;
    context: string;
    capability: string;
    hold: string;
    usage: string;
    settlement: string;
  }>;
  usage: Readonly<{
    inputTokens: number;
    outputTokens: number;
    cachedInputTokens: number;
    audioInputTokens: number;
    audioOutputTokens: number;
    chargeMsat: number;
    responseCount: number;
    transcriptionCount: number;
    cancelledResponseCount: number;
  }>;
  providerAccountingStatus: "exact";
  workerJobCount: number;
  providerSessionCount: number;
  workerClosedAt: string;
  providerAdmittedAt: string;
}>;

export type SarahVoiceUnmeteredAuthorityCapture = Readonly<{
  schema: "openagents.sarah.unmetered-authority-capture.v1";
  authority: "owner_waived_unmetered_v1";
  generation: number;
  sessionRefDigest: string;
  startLedgerStateDigest: string;
  endLedgerStateDigest: string;
  startBalanceStateDigest: string;
  endBalanceStateDigest: string;
  ledgerMutationCount: number;
  captureReceiptRef: string;
  captureDigest: string;
}>;

export type SarahVoiceSessionRecord = Readonly<{
  sessionRef: string;
  ownerUserId: string;
  ownerActorRef: string;
  deviceRef: string;
  threadRef: string;
  generation: number;
  disclosureRef: string;
  clientProfile: SarahVoiceClientProfile;
  transportKind: SarahVoiceTransportKind;
  creditMode: SarahVoiceCreditMode;
  entitlementRef: string | null;
  admissionCohortRef: string | null;
  state: SarahVoiceSessionState;
  reservedMsat: number;
  chargedMsat: number;
  ticketExpiresAt: string;
  sessionExpiresAt: string;
  settlementReceiptRef: string | null;
}>;

export type SarahVoiceReservationRecord = SarahVoiceSessionRecord &
  Readonly<{
    admissionExpiresAt: string | undefined;
    admissionTermsDigest: string | undefined;
    replayed: boolean;
  }>;

export type SarahVoiceUsage = Readonly<{
  usageKind?: "response" | "transcription";
  providerResponseRef: string;
  providerStatus?: "completed" | "cancelled" | "failed" | "incomplete";
  inputTokens: number;
  outputTokens: number;
  cachedInputTokens: number;
  audioInputTokens: number;
  audioOutputTokens: number;
  chargeMsat: number;
  observedAt: string;
}>;

type SarahVoiceProviderUsage = Omit<SarahVoiceUsage, "chargeMsat"> &
  Readonly<{ chargeMsat?: number }>;

export type SarahVoiceLiveKitWorkerStopReason =
  | "hold_exhausted"
  | "membership_revoked"
  | "operator_stop"
  | "session_expired"
  | "worker_unavailable";

type SarahVoiceLiveKitWorkerEventCommon = Readonly<{
  workerControlTokenDigest: string;
  workerJobRef: string;
  sessionRef: string;
  generation: number;
  eventRef: string;
  eventPayloadDigest: string;
  nowIso: string;
}>;

export type SarahVoiceLiveKitWorkerEvent =
  | (SarahVoiceLiveKitWorkerEventCommon &
      Readonly<{
        eventKind: "worker_connected";
        workerRoomSid: string;
      }>)
  | (SarahVoiceLiveKitWorkerEventCommon &
      Readonly<{
        eventKind: "provider_admitted";
        providerSessionRefDigest: string;
        providerConfigurationDigest: string;
      }>)
  | (SarahVoiceLiveKitWorkerEventCommon &
      Readonly<{
        eventKind: "lease_check";
      }>)
  | (SarahVoiceLiveKitWorkerEventCommon &
      Readonly<{
        eventKind: "interrupt_applied";
        interruptSequence: number;
      }>)
  | (SarahVoiceLiveKitWorkerEventCommon &
      Readonly<{
        eventKind: "provider_disconnect_fault_applied";
        requestRef: string;
        providerSessionRefDigest: string;
      }>)
  | (SarahVoiceLiveKitWorkerEventCommon &
      Readonly<{
        eventKind: "response_usage";
        usage: Omit<SarahVoiceUsage, "observedAt" | "chargeMsat"> &
          Required<Pick<SarahVoiceUsage, "providerStatus">>;
      }>)
  | (SarahVoiceLiveKitWorkerEventCommon &
      Readonly<{
        eventKind: "transcription_usage";
        usage: Omit<SarahVoiceUsage, "observedAt" | "chargeMsat">;
      }>)
  | (SarahVoiceLiveKitWorkerEventCommon &
      Readonly<{
        eventKind: "close";
        closeReason: string;
        accountingStatus: "exact" | "uncertain";
      }>);

export type SarahVoiceLiveKitWorkerEventResult = Readonly<{
  observedAt: string;
  replayed: boolean;
  interruptSequence?: number;
  providerDisconnectFault?: Readonly<{
    requestRef: string;
    providerSessionRefDigest: string;
  }>;
  stopReason?: SarahVoiceLiveKitWorkerStopReason;
}>;

export type SarahVoiceLiveKitProviderDisconnectFaultResult = Readonly<{
  requestRef: string;
  sessionRef: string;
  generation: number;
  providerSessionRefDigest: string;
  state: "requested" | "applied";
  replayed: boolean;
}>;

export type SarahVoiceLiveKitEditorCommand = Exclude<
  SarahEditorCommand,
  Readonly<{ _tag: "open_path" }>
>;

export type SarahVoiceLiveKitToolProposal = Readonly<{
  proposalRef: string;
  proposalDigest: string;
  command: SarahVoiceLiveKitEditorCommand;
  confirmationRequired: boolean;
  expiresAtMs: number;
}>;

export type SarahVoiceLiveKitToolState =
  | Readonly<{ state: "waiting_decision" }>
  | Readonly<{ state: "declined" }>
  | Readonly<{ state: "execute_sent" }>
  | Readonly<{
      state: "outcome";
      outcomeRef: string;
      ok: boolean;
      summary: string;
    }>;

export type SarahVoiceLiveKitRoomContext =
  | Readonly<{ kind: "private" }>
  | Readonly<{
      kind: "community";
      communityRef: string;
      channelRef: string;
      membershipRevision: string;
    }>;

export type SarahVoiceLiveKitCleanup = Readonly<{
  sessionRef: string;
  generation: number;
  roomRef: string;
  roomEpoch: number;
  dispatchRef: string;
  sarahPresenceLeaseRef: string;
}>;

export type SarahVoiceLiveKitCleanupClaim = SarahVoiceLiveKitCleanup &
  Readonly<{ cleanupAttemptedAt: string; cleanupAttemptCount: number }>;

export type SarahVoiceLiveKitBindingState =
  | "prepared"
  | "active"
  | "cleanup_ready"
  | "cleanup_failed"
  | "cleanup_abandoned"
  | "cleaned";

/**
 * EP263-LK H4 (#9282): how many times the reconciler will try to delete one
 * room before it stops. Eight attempts over the backoff ladder below spans
 * roughly half an hour, which is long enough to ride out a transient broker or
 * SFU fault and short enough that an unreachable room stops burning the loop.
 */
export const SARAH_LIVEKIT_MAX_CLEANUP_ATTEMPTS = 8;

export const SARAH_LIVEKIT_BASE_CLEANUP_BACKOFF_SECONDS = 15;

export const SARAH_LIVEKIT_MAX_CLEANUP_BACKOFF_SECONDS = 900;

export type SarahVoiceLiveKitCleanupOutcome = Readonly<{
  state: SarahVoiceLiveKitBindingState;
  cleanupAttemptCount: number;
}>;

export type SarahVoiceLiveKitProvisioningIntentState =
  | "pending"
  | "reconciling"
  | "bound"
  | "cleanup_failed"
  | "cleanup_abandoned"
  | "cleaned";

/**
 * EP263-LK H4 follow-up (#9282): the provisioning-intent reconciler is bounded
 * on the same ladder as the room-binding cleanup above. The two loops fail for
 * the same reason — a broker key that can never be cleaned — so they give up on
 * the same schedule and an operator only has one number to remember.
 */
export const SARAH_LIVEKIT_MAX_PROVISIONING_ATTEMPTS = SARAH_LIVEKIT_MAX_CLEANUP_ATTEMPTS;

export type SarahVoiceLiveKitProvisioningOutcome = Readonly<{
  state: SarahVoiceLiveKitProvisioningIntentState;
  cleanupAttemptCount: number;
}>;

export type SarahVoiceLiveKitProvisioningIntent = Readonly<{
  sessionRef: string;
  generation: number;
  idempotencyKey: string;
  provisioningOwnerRef: string;
  cleanupAttemptCount: number;
}>;

export type SarahVoiceLiveKitWorkerClaim = Readonly<{
  sessionRef: string;
  generation: number;
  ownerUserId: string;
  capabilityProfile: SarahVoiceClientProfile;
  roomContext: SarahVoiceLiveKitRoomContext;
  admissionDigest: string;
  sessionExpiresAt: string;
}>;

export type SarahVoiceLiveKitWorkerReadiness = "waiting" | "claimed" | "closed";

export type SarahVoiceAccountingReconciliationUsage = Readonly<{
  usageKind: "response" | "transcription";
  providerResponseRef: string;
  providerStatus?: "completed" | "cancelled" | "failed" | "incomplete";
  inputTokens: number;
  outputTokens: number;
  cachedInputTokens: number;
  audioInputTokens: number;
  audioOutputTokens: number;
}>;

export type SarahVoiceAccountingReconciliationResult = Readonly<{
  reconciliationRef: string;
  reconciliationReceiptRef: string;
  sessionRef: string;
  state: "settled" | "released";
  finalChargeMsat: number;
  settlementReceiptRef: string;
  replayed: boolean;
}>;

export type SarahVoiceAccountingWaiverResult = Readonly<{
  waiverRef: string;
  waiverReceiptRef: string;
  sessionRef: string;
  state: "released";
  releasedHoldMsat: number;
  recordedChargeWaivedMsat: number;
  providerAccountingStatus: "uncertain";
  authority: "owner_waived_unmetered_v1";
  replayed: boolean;
}>;

export type SarahVoiceLiveKitMembershipLease = Readonly<{
  ownerUserId: string;
  sarahPresenceLeaseRef: string;
  roomContext: SarahVoiceLiveKitRoomContext;
}>;

// Stop expiry must outlive the 30s worker drain, 45s child shutdown, 10s
// provider terminal wait, and one worst-case 25.8s durable control delivery.
export const SARAH_LIVEKIT_WORKER_DRAIN_TIMEOUT_MS = 150_000;
export const SARAH_LIVEKIT_WORKER_HEARTBEAT_TIMEOUT_MS = 30_000;

/**
 * How long an `accounting_uncertain` hold may sit before it is worth an
 * operator's attention.
 *
 * Before this bound, `sarah_realtime_voice_owner_active_idx` treats a metered
 * uncertain hold as active and refuses a later voice generation. Owner-waived
 * sessions have no hold and do not take this path. At the bound, maintenance
 * records a durable escalation and the partial index removes only that row's
 * concurrency lock. The full hold and uncertain state remain until exact
 * provider reconciliation.
 */
export const SARAH_VOICE_ACCOUNTING_UNCERTAIN_STUCK_MS = 900_000;
export const SARAH_VOICE_ACCOUNTING_ESCALATION_BATCH_SIZE = 100;

export type SarahVoiceAccountingEscalationResult = Readonly<{
  escalated: number;
  owners: number;
  oldestAgeMs: number;
}>;

export class SarahVoiceInsufficientCreditError extends Error {
  override readonly name = "SarahVoiceInsufficientCreditError";
}

export class SarahVoiceConcurrentSessionError extends Error {
  override readonly name = "SarahVoiceConcurrentSessionError";
}

export class SarahVoiceSessionRejectedError extends Error {
  override readonly name = "SarahVoiceSessionRejectedError";
}

export class SarahVoiceAdmissionRejectedError extends Error {
  override readonly name = "SarahVoiceAdmissionRejectedError";
}

/**
 * A second live client claimed one already-admitted participant identity.
 *
 * This is deliberately distinct from the generic rejection: a duplicate
 * participant is not a stale, revoked, or expired join, it is two clients
 * racing one room seat. Callers that only refuse cannot tell an operator which
 * drill they just satisfied, so the class is separate and the routes map it to
 * its own `duplicate_participant_refused` code.
 */
export class SarahVoiceDuplicateParticipantError extends Error {
  override readonly name = "SarahVoiceDuplicateParticipantError";
}

export class SarahVoiceLiveKitCapacityError extends Error {
  override readonly name = "SarahVoiceLiveKitCapacityError";
}

export class SarahVoiceStorageError extends Error {
  override readonly name = "SarahVoiceStorageError";
  constructor(
    message: string,
    override readonly cause: unknown,
  ) {
    super(message);
  }
}

type SessionRow = Readonly<{
  session_ref: string;
  owner_user_id: string;
  owner_actor_ref: string;
  device_ref: string;
  thread_ref: string;
  generation: number | string;
  disclosure_ref: string;
  client_profile: SarahVoiceClientProfile;
  transport_kind: SarahVoiceTransportKind;
  credit_mode: SarahVoiceCreditMode;
  entitlement_ref: string | null;
  admission_cohort_ref: string | null;
  state: SarahVoiceSessionState;
  reserved_msat: number | string;
  charged_msat: number | string;
  ticket_expires_at: string;
  session_expires_at: string;
  settlement_receipt_ref: string | null;
}>;

const toSafeInteger = (value: number | string, label: string): number => {
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed < 0) {
    throw new SarahVoiceStorageError(`${label} is outside the safe integer range`, value);
  }
  return parsed;
};

const toRecord = (row: SessionRow): SarahVoiceSessionRecord => ({
  sessionRef: row.session_ref,
  ownerUserId: row.owner_user_id,
  ownerActorRef: row.owner_actor_ref,
  deviceRef: row.device_ref,
  threadRef: row.thread_ref,
  generation: toSafeInteger(row.generation, "generation"),
  disclosureRef: row.disclosure_ref,
  clientProfile: row.client_profile,
  transportKind: row.transport_kind,
  creditMode: row.credit_mode,
  entitlementRef: row.entitlement_ref,
  admissionCohortRef: row.admission_cohort_ref,
  state: row.state,
  reservedMsat: toSafeInteger(row.reserved_msat, "reserved_msat"),
  chargedMsat: toSafeInteger(row.charged_msat, "charged_msat"),
  ticketExpiresAt: row.ticket_expires_at,
  sessionExpiresAt: row.session_expires_at,
  settlementReceiptRef: row.settlement_receipt_ref,
});

const first = <A>(rows: ReadonlyArray<A>): A | undefined => rows[0];

const acceptanceDigest = (value: string): string =>
  createHash("sha256").update(value).digest("hex");

const unmeteredLedgerStateDigest = (input: Readonly<{
  sessionRef: string;
  generation: number;
  reservedMsat: number;
  chargedMsat: number;
  payInCount: number;
  payInLegCount: number;
}>): string =>
  acceptanceDigest(
    JSON.stringify({
      schema: "openagents.sarah.unmetered-ledger-state.v1",
      ...input,
    }),
  );

const unmeteredBalanceStateDigest = (input: Readonly<{
  ownerActorRef: string;
  rowPresent: boolean;
  balanceMsat: number | null;
  heldMsat: number | null;
  updatedAt: string | null;
}>): string =>
  acceptanceDigest(
    JSON.stringify({
      schema: "openagents.sarah.unmetered-balance-state.v1",
      ...input,
    }),
  );

const plusMillisecondsIso = (value: string, milliseconds: number): string => {
  const epochMilliseconds = Date.parse(value);
  if (!Number.isSafeInteger(epochMilliseconds)) {
    throw new SarahVoiceSessionRejectedError("The Sarah voice event timestamp is invalid");
  }
  return new Date(epochMilliseconds + milliseconds).toISOString();
};

const isUniqueViolation = (error: unknown): boolean =>
  typeof error === "object" &&
  error !== null &&
  "code" in error &&
  (error as { code?: unknown }).code === "23505";

export type SarahRealtimeVoiceStore = ReturnType<typeof makeSarahRealtimeVoiceStore>;

export const makeSarahRealtimeVoiceStore = (sql: SyncSql) => {
  const issueAdmission = async (
    input: Readonly<{
      admissionRef: string;
      ownerUserId: string;
      deviceRef: string;
      threadRef: string;
      sessionRef: string;
      generation: number;
      disclosureRef: string;
      clientProfile: SarahVoiceClientProfile;
      admissionCohortRef: string;
      creditMode: SarahVoiceCreditMode;
      creditRateMsatPerMillionTokens: number;
      termsDigest: string;
      spendableRemainingCreditMsat: number | null;
      nowIso: string;
      expiresAt: string;
    }>,
  ): Promise<SarahVoiceAdmissionRecord> => {
    try {
      if (
        !Number.isSafeInteger(input.creditRateMsatPerMillionTokens) ||
        input.creditRateMsatPerMillionTokens <= 0
      ) {
        throw new SarahVoiceAdmissionRejectedError(
          "The Sarah voice admission credit rate is invalid",
        );
      }
      const rows = (await sql`
        INSERT INTO sarah_voice_admissions (
          admission_ref, owner_user_id, device_ref, thread_ref, session_ref,
          generation, disclosure_ref, client_profile, admission_cohort_ref,
          credit_mode, credit_rate_msat_per_million_tokens, terms_digest,
          spendable_remaining_credit_msat, state,
          issued_at, expires_at, consumed_at
        ) VALUES (
          ${input.admissionRef}, ${input.ownerUserId}, ${input.deviceRef},
          ${input.threadRef}, ${input.sessionRef}, ${input.generation},
          ${input.disclosureRef}, ${input.clientProfile},
          ${input.admissionCohortRef}, ${input.creditMode},
          ${input.creditRateMsatPerMillionTokens},
          ${input.termsDigest}, ${input.spendableRemainingCreditMsat}, 'active',
          ${input.nowIso}, ${input.expiresAt}, NULL
        )
        RETURNING admission_ref, expires_at
      `) as ReadonlyArray<{ admission_ref: string; expires_at: string }>;
      const row = first(rows);
      if (row === undefined) {
        throw new SarahVoiceStorageError("The admission did not return a row", null);
      }
      return {
        admissionRef: row.admission_ref,
        admissionExpiresAt: row.expires_at,
      };
    } catch (error) {
      if (error instanceof SarahVoiceStorageError) throw error;
      throw new SarahVoiceStorageError("Sarah voice admission issue failed", error);
    }
  };

  const readActiveAlphaMembership = async (
    input: Readonly<{
      ownerUserId: string;
      cohortRef: string;
      nowIso: string;
    }>,
  ): Promise<SarahVoiceAlphaMembership | undefined> => {
    try {
      const rows = (await sql`
        SELECT membership_ref, cohort_ref, owner_user_id
        FROM sarah_voice_alpha_memberships
        WHERE owner_user_id = ${input.ownerUserId}
          AND cohort_ref = ${input.cohortRef}
          AND state = 'active'
          AND admitted_at <= ${input.nowIso}
          AND EXISTS (
            SELECT 1 FROM users
            WHERE id = ${input.ownerUserId}
              AND status = 'active'
              AND deleted_at IS NULL
          )
      `) as ReadonlyArray<{
        membership_ref: string;
        cohort_ref: string;
        owner_user_id: string;
      }>;
      const row = first(rows);
      return row === undefined
        ? undefined
        : {
            membershipRef: row.membership_ref,
            cohortRef: row.cohort_ref,
            ownerUserId: row.owner_user_id,
          };
    } catch (error) {
      throw new SarahVoiceStorageError("Sarah voice alpha membership lookup failed", error);
    }
  };

  const readSpendableCredit = async (
    input: Readonly<{ ownerUserId: string; ownerActorRef: string }>,
  ): Promise<number> => {
    try {
      const rows = (await sql`
        SELECT GREATEST(balance.balance_msat - balance.held_msat, 0) AS spendable_msat
        FROM users
        JOIN agent_balances AS balance
          ON balance.actor_ref = ${input.ownerActorRef}
        WHERE users.id = ${input.ownerUserId}
          AND users.status = 'active'
          AND users.deleted_at IS NULL
      `) as ReadonlyArray<{ spendable_msat: number | string }>;
      const row = first(rows);
      return row === undefined ? 0 : toSafeInteger(row.spendable_msat, "spendable_msat");
    } catch (error) {
      throw new SarahVoiceStorageError("Sarah voice spendable credit lookup failed", error);
    }
  };

  const readActiveStagingOwnerEntitlement = async (
    input: Readonly<{
      ownerUserId: string;
      entitlementRef: string;
      nowIso: string;
    }>,
  ): Promise<SarahVoiceCreditEntitlement | undefined> => {
    try {
      const rows = (await sql`
        SELECT entitlement_ref, owner_user_id, expires_at
        FROM sarah_voice_credit_entitlements
        WHERE owner_user_id = ${input.ownerUserId}
          AND entitlement_ref = ${input.entitlementRef}
          AND environment = 'staging'
          AND state = 'active'
          AND activated_at <= ${input.nowIso}
          AND expires_at > ${input.nowIso}
      `) as ReadonlyArray<{
        entitlement_ref: string;
        owner_user_id: string;
        expires_at: string;
      }>;
      const row = first(rows);
      return row === undefined
        ? undefined
        : {
            entitlementRef: row.entitlement_ref,
            ownerUserId: row.owner_user_id,
            expiresAt: row.expires_at,
          };
    } catch (error) {
      throw new SarahVoiceStorageError("Sarah voice entitlement lookup failed", error);
    }
  };

  const ensureStagingOwnerEntitlement = async (
    input: Readonly<{
      ownerUserId: string;
      entitlementRef: string;
      expiresAt: string;
      nowIso: string;
    }>,
  ): Promise<SarahVoiceCreditEntitlement | undefined> => {
    try {
      return await sql.begin(async (tx) => {
        const inserted = (await tx`
          INSERT INTO sarah_voice_credit_entitlements (
            entitlement_ref, owner_user_id, environment, state,
            activated_at, expires_at, activation_reason,
            activation_actor_ref, activation_source, updated_at
          )
          SELECT ${input.entitlementRef}, ${input.ownerUserId}, 'staging',
            'active', ${input.nowIso}, ${input.expiresAt},
            'Owner-approved staging Sarah voice access for issue 9272',
            'operator:staging_owner_voice_entitlement',
            'cloudrun:staging_owner_account_match', ${input.nowIso}
          FROM users
          WHERE id = ${input.ownerUserId}
            AND status = 'active'
            AND deleted_at IS NULL
          ON CONFLICT (owner_user_id) DO NOTHING
          RETURNING entitlement_ref
        `) as ReadonlyArray<{ entitlement_ref: string }>;

        if (first(inserted) !== undefined) {
          await tx`
            INSERT INTO sarah_voice_credit_entitlement_audit (
              event_ref, entitlement_ref, action, actor_ref, reason,
              source, occurred_at
            ) VALUES (
              ${`${input.entitlementRef}:activated`}, ${input.entitlementRef},
              'activated', 'operator:staging_owner_voice_entitlement',
              'Owner-approved staging Sarah voice access for issue 9272',
              'cloudrun:staging_owner_account_match', ${input.nowIso}
            )
            ON CONFLICT (event_ref) DO NOTHING
          `;
        }

        const rows = (await tx`
          SELECT entitlement_ref, owner_user_id, expires_at
          FROM sarah_voice_credit_entitlements
          WHERE owner_user_id = ${input.ownerUserId}
            AND entitlement_ref = ${input.entitlementRef}
            AND environment = 'staging'
            AND state = 'active'
            AND activated_at <= ${input.nowIso}
            AND expires_at > ${input.nowIso}
          FOR SHARE
        `) as ReadonlyArray<{
          entitlement_ref: string;
          owner_user_id: string;
          expires_at: string;
        }>;
        const row = first(rows);
        return row === undefined
          ? undefined
          : {
              entitlementRef: row.entitlement_ref,
              ownerUserId: row.owner_user_id,
              expiresAt: row.expires_at,
            };
      });
    } catch (error) {
      throw new SarahVoiceStorageError("Sarah voice entitlement lookup failed", error);
    }
  };

  const reserve = async (
    input: Readonly<{
      sessionRef: string;
      reservationRef: string;
      ownerUserId: string;
      ownerActorRef: string;
      deviceRef: string;
      threadRef: string;
      generation: number;
      ticketDigest: string;
      disclosureRef: string;
      clientProfile: SarahVoiceClientProfile;
      transportKind?: SarahVoiceTransportKind;
      creditMode: SarahVoiceCreditMode;
      entitlementRef: string | null;
      admissionCohortRef: string;
      creditRateMsatPerMillionTokens: number;
      reservedMsat: number;
      ticketExpiresAt: string;
      sessionExpiresAt: string;
      nowIso: string;
      admissionBinding?: Readonly<{
        admissionRef: string;
        termsDigest: string;
        creditRateMsatPerMillionTokens: number;
        spendableRemainingCreditMsat: number | null;
      }>;
    }>,
  ): Promise<SarahVoiceReservationRecord> => {
    try {
      if (
        !Number.isSafeInteger(input.creditRateMsatPerMillionTokens) ||
        input.creditRateMsatPerMillionTokens <= 0 ||
        (input.admissionBinding !== undefined &&
          input.admissionBinding.creditRateMsatPerMillionTokens !==
            input.creditRateMsatPerMillionTokens)
      ) {
        throw new SarahVoiceAdmissionRejectedError(
          "The Sarah voice reservation credit rate does not match admission",
        );
      }
      return await sql.begin(async (tx) => {
        let admissionExpiresAt: string | undefined;
        const users = (await tx`
          SELECT id
          FROM users
          WHERE id = ${input.ownerUserId}
            AND status = 'active'
            AND deleted_at IS NULL
          FOR UPDATE
        `) as ReadonlyArray<{ id: string }>;
        if (first(users) === undefined) {
          throw new SarahVoiceSessionRejectedError("The user is not active");
        }

        const replayRows = (await tx`
          SELECT session.session_ref, session.owner_user_id,
            session.owner_actor_ref, session.device_ref, session.thread_ref,
            session.generation, session.disclosure_ref,
            session.client_profile, session.transport_kind,
            session.credit_mode, session.entitlement_ref,
            session.admission_cohort_ref, session.state,
            session.credit_rate_msat_per_million_tokens,
            session.reserved_msat, session.charged_msat,
            session.ticket_expires_at, session.session_expires_at,
            session.settlement_receipt_ref,
            admission.expires_at AS admission_expires_at,
            admission.terms_digest AS admission_terms_digest
          FROM sarah_realtime_voice_sessions AS session
          LEFT JOIN sarah_voice_admissions AS admission
            ON admission.session_ref = session.session_ref
            AND admission.admission_ref = ${input.admissionBinding?.admissionRef ?? null}
            AND admission.state = 'consumed'
          WHERE session.session_ref = ${input.sessionRef}
            AND session.reservation_ref = ${input.reservationRef}
            AND session.owner_user_id = ${input.ownerUserId}
            AND session.owner_actor_ref = ${input.ownerActorRef}
            AND session.device_ref = ${input.deviceRef}
            AND session.thread_ref = ${input.threadRef}
            AND session.generation = ${input.generation}
            AND session.ticket_digest = ${input.ticketDigest}
            AND session.disclosure_ref = ${input.disclosureRef}
            AND session.client_profile = ${input.clientProfile}
            AND session.transport_kind = ${input.transportKind ?? "custom_wss_v1"}
            AND session.credit_mode = ${input.creditMode}
            AND session.entitlement_ref IS NOT DISTINCT FROM ${input.entitlementRef}
            AND session.admission_cohort_ref = ${input.admissionCohortRef}
            AND session.credit_rate_msat_per_million_tokens =
              ${input.creditRateMsatPerMillionTokens}
            AND session.reserved_msat = ${input.reservedMsat}
            AND session.state = 'reserved'
            AND session.ticket_expires_at > ${input.nowIso}
            AND session.session_expires_at > ${input.nowIso}
            AND (
              ${input.admissionBinding === undefined}
              OR admission.admission_ref IS NOT NULL
            )
          FOR UPDATE OF session
        `) as ReadonlyArray<
          SessionRow &
            Readonly<{
              admission_expires_at: string | null;
              admission_terms_digest: string | null;
            }>
        >;
        const replay = first(replayRows);
        if (replay !== undefined) {
          return {
            ...toRecord(replay),
            admissionExpiresAt: replay.admission_expires_at ?? undefined,
            admissionTermsDigest: replay.admission_terms_digest ?? undefined,
            replayed: true,
          };
        }

        if (input.admissionBinding !== undefined) {
          const admissions = (await tx`
            SELECT admission_ref, expires_at
            FROM sarah_voice_admissions
            WHERE admission_ref = ${input.admissionBinding.admissionRef}
              AND owner_user_id = ${input.ownerUserId}
              AND device_ref = ${input.deviceRef}
              AND thread_ref = ${input.threadRef}
              AND session_ref = ${input.sessionRef}
              AND generation = ${input.generation}
              AND disclosure_ref = ${input.disclosureRef}
              AND client_profile = ${input.clientProfile}
              AND admission_cohort_ref = ${input.admissionCohortRef}
              AND credit_mode = ${input.creditMode}
              AND credit_rate_msat_per_million_tokens =
                ${input.admissionBinding.creditRateMsatPerMillionTokens}
              AND credit_rate_msat_per_million_tokens =
                ${input.creditRateMsatPerMillionTokens}
              AND terms_digest = ${input.admissionBinding.termsDigest}
              AND spendable_remaining_credit_msat IS NOT DISTINCT FROM
                ${input.admissionBinding.spendableRemainingCreditMsat}
              AND state = 'active'
              AND expires_at > ${input.nowIso}
            FOR UPDATE
          `) as ReadonlyArray<{ admission_ref: string; expires_at: string }>;
          const admission = first(admissions);
          if (admission === undefined) {
            throw new SarahVoiceAdmissionRejectedError(
              "The Sarah voice admission is missing, changed, expired, or already consumed",
            );
          }
          const admissionExpiresAtMs = Date.parse(admission.expires_at);
          const nowMs = Date.parse(input.nowIso);
          if (
            !Number.isSafeInteger(admissionExpiresAtMs) ||
            !Number.isSafeInteger(nowMs) ||
            admissionExpiresAtMs <= nowMs
          ) {
            throw new SarahVoiceAdmissionRejectedError(
              "The Sarah voice admission expiry is invalid",
            );
          }
          admissionExpiresAt = admission.expires_at;
        }

        const priorSessions = (await tx`
          SELECT generation, state, credit_mode
          FROM sarah_realtime_voice_sessions
          WHERE owner_user_id = ${input.ownerUserId}
            AND thread_ref = ${input.threadRef}
          ORDER BY generation DESC, created_at DESC
          LIMIT 1
          FOR UPDATE
        `) as ReadonlyArray<{
          generation: number | string;
          state: SarahVoiceSessionState;
          credit_mode: SarahVoiceCreditMode;
        }>;
        const priorSession = first(priorSessions);
        if (priorSession !== undefined) {
          const priorGeneration = toSafeInteger(priorSession.generation, "generation");
          if (
            priorSession.state !== "settled" &&
            priorSession.state !== "released" &&
            !(
              priorSession.state === "accounting_uncertain" &&
              priorSession.credit_mode === "owner_waived_unmetered"
            )
          ) {
            throw new SarahVoiceConcurrentSessionError(
              "The prior Sarah voice generation has not completed accounting",
            );
          }
          if (input.generation <= priorGeneration) {
            throw new SarahVoiceSessionRejectedError("The Sarah voice generation must advance");
          }
        }

        if (input.creditMode === "metered" || input.creditMode === "owner_waived_unmetered") {
          const memberships = (await tx`
            SELECT membership_ref
            FROM sarah_voice_alpha_memberships
            WHERE owner_user_id = ${input.ownerUserId}
              AND cohort_ref = ${input.admissionCohortRef}
              AND state = 'active'
              AND admitted_at <= ${input.nowIso}
            FOR SHARE
          `) as ReadonlyArray<{ membership_ref: string }>;
          if (first(memberships) === undefined) {
            throw new SarahVoiceSessionRejectedError(
              "The Sarah voice alpha membership is not active",
            );
          }
          if (input.creditMode === "metered" && input.admissionBinding !== undefined) {
            const balances = (await tx`
              SELECT balance_msat - held_msat AS spendable_msat
              FROM agent_balances
              WHERE actor_ref = ${input.ownerActorRef}
              FOR UPDATE
            `) as ReadonlyArray<{ spendable_msat: number | string }>;
            const balance = first(balances);
            if (
              balance === undefined ||
              toSafeInteger(balance.spendable_msat, "spendable_msat") !==
                input.admissionBinding.spendableRemainingCreditMsat
            ) {
              throw new SarahVoiceAdmissionRejectedError(
                "The spendable credit changed after Sarah voice admission",
              );
            }
          }
          if (input.creditMode === "metered") {
            const balances = (await tx`
              UPDATE agent_balances
              SET held_msat = held_msat + ${input.reservedMsat},
                  updated_at = ${input.nowIso}
              WHERE actor_ref = ${input.ownerActorRef}
                AND balance_msat - held_msat >= ${input.reservedMsat}
              RETURNING actor_ref
            `) as ReadonlyArray<{ actor_ref: string }>;
            if (first(balances) === undefined) {
              throw new SarahVoiceInsufficientCreditError(
                "The account has insufficient available credit",
              );
            }
          } else if (input.reservedMsat !== 0 || input.entitlementRef !== null) {
            throw new SarahVoiceAdmissionRejectedError(
              "Owner-waived Sarah voice must not reserve credit",
            );
          }
        } else {
          const entitlements = (await tx`
            SELECT entitlement_ref
            FROM sarah_voice_credit_entitlements
            WHERE entitlement_ref = ${input.entitlementRef}
              AND owner_user_id = ${input.ownerUserId}
              AND environment = 'staging'
              AND state = 'active'
              AND activated_at <= ${input.nowIso}
              AND expires_at > ${input.nowIso}
            FOR SHARE
          `) as ReadonlyArray<{ entitlement_ref: string }>;
          if (first(entitlements) === undefined) {
            throw new SarahVoiceSessionRejectedError("The staging voice entitlement is not active");
          }
        }

        if (input.admissionBinding !== undefined) {
          const consumed = (await tx`
            UPDATE sarah_voice_admissions
            SET state = 'consumed', consumed_at = ${input.nowIso}
            WHERE admission_ref = ${input.admissionBinding.admissionRef}
              AND state = 'active'
            RETURNING admission_ref
          `) as ReadonlyArray<{ admission_ref: string }>;
          if (first(consumed) === undefined) {
            throw new SarahVoiceAdmissionRejectedError(
              "The Sarah voice admission could not be consumed",
            );
          }
        }

        const rows = (await tx`
          INSERT INTO sarah_realtime_voice_sessions (
            session_ref, reservation_ref, owner_user_id, owner_actor_ref,
            device_ref, thread_ref, generation, ticket_digest, disclosure_ref,
            client_profile, transport_kind, credit_mode, entitlement_ref, state, reserved_msat,
            admission_cohort_ref, credit_rate_msat_per_million_tokens,
            charged_msat, ticket_expires_at,
            session_expires_at, created_at, updated_at
          ) VALUES (
            ${input.sessionRef}, ${input.reservationRef}, ${input.ownerUserId},
            ${input.ownerActorRef}, ${input.deviceRef}, ${input.threadRef},
            ${input.generation}, ${input.ticketDigest}, ${input.disclosureRef},
            ${input.clientProfile}, ${input.transportKind ?? "custom_wss_v1"},
            ${input.creditMode}, ${input.entitlementRef},
            'reserved', ${input.reservedMsat}, ${input.admissionCohortRef},
            ${input.creditRateMsatPerMillionTokens}, 0,
            ${input.ticketExpiresAt},
            ${input.sessionExpiresAt}, ${input.nowIso}, ${input.nowIso}
          )
          RETURNING session_ref, owner_user_id, owner_actor_ref, device_ref,
            thread_ref, generation, disclosure_ref, client_profile,
            transport_kind, credit_mode,
            entitlement_ref, admission_cohort_ref, state, reserved_msat,
            charged_msat, ticket_expires_at, session_expires_at,
            settlement_receipt_ref
        `) as ReadonlyArray<SessionRow>;
        const row = first(rows);
        if (row === undefined) {
          throw new SarahVoiceStorageError("The reservation did not return a row", null);
        }
        if (input.creditMode === "owner_waived_unmetered") {
          const balanceRows = (await tx`
            SELECT balance_msat, held_msat, updated_at
            FROM agent_balances
            WHERE actor_ref = ${input.ownerActorRef}
            FOR SHARE
          `) as ReadonlyArray<{
            balance_msat: number | string;
            held_msat: number | string;
            updated_at: string;
          }>;
          const balance = first(balanceRows);
          const startBalanceStateDigest = unmeteredBalanceStateDigest({
            ownerActorRef: input.ownerActorRef,
            rowPresent: balance !== undefined,
            balanceMsat:
              balance === undefined ? null : toSafeInteger(balance.balance_msat, "balance_msat"),
            heldMsat:
              balance === undefined ? null : toSafeInteger(balance.held_msat, "held_msat"),
            updatedAt: balance?.updated_at ?? null,
          });
          const startLedgerStateDigest = unmeteredLedgerStateDigest({
            sessionRef: input.sessionRef,
            generation: input.generation,
            reservedMsat: 0,
            chargedMsat: 0,
            payInCount: 0,
            payInLegCount: 0,
          });
          await tx`
            INSERT INTO sarah_voice_unmetered_authority_captures (
              session_ref, generation, authority, start_ledger_state_digest,
              start_balance_state_digest, ledger_mutation_count, created_at
            ) VALUES (
              ${input.sessionRef}, ${input.generation},
              'owner_waived_unmetered_v1', ${startLedgerStateDigest},
              ${startBalanceStateDigest}, 0,
              ${input.nowIso}
            )
          `;
        }
        return {
          ...toRecord(row),
          admissionExpiresAt,
          admissionTermsDigest: input.admissionBinding?.termsDigest,
          replayed: false,
        };
      });
    } catch (error) {
      if (
        error instanceof SarahVoiceInsufficientCreditError ||
        error instanceof SarahVoiceConcurrentSessionError ||
        error instanceof SarahVoiceSessionRejectedError ||
        error instanceof SarahVoiceAdmissionRejectedError
      ) {
        throw error;
      }
      if (isUniqueViolation(error)) {
        throw new SarahVoiceConcurrentSessionError(
          "The user already has an active Sarah voice session",
        );
      }
      throw new SarahVoiceStorageError("Sarah voice reservation failed", error);
    }
  };

  const connect = async (
    input: Readonly<{
      sessionRef: string;
      ticketDigest: string;
      nowIso: string;
    }>,
  ): Promise<SarahVoiceSessionRecord> => {
    try {
      return await sql.begin(async (tx) => {
        const rows = (await tx`
          UPDATE sarah_realtime_voice_sessions
          SET state = 'connected',
              ticket_digest = NULL,
              connected_at = COALESCE(connected_at, ${input.nowIso}),
              updated_at = ${input.nowIso}
          WHERE session_ref = ${input.sessionRef}
            AND ticket_digest = ${input.ticketDigest}
            AND (
              state = 'reserved'
              OR (state = 'connected' AND transport_kind = 'livekit_room_v1')
            )
            AND ticket_expires_at > ${input.nowIso}
            AND session_expires_at > ${input.nowIso}
          RETURNING session_ref, owner_user_id, owner_actor_ref, device_ref,
            thread_ref, generation, disclosure_ref, client_profile,
            transport_kind, credit_mode,
            entitlement_ref, admission_cohort_ref, state, reserved_msat,
            charged_msat, ticket_expires_at, session_expires_at,
            settlement_receipt_ref
        `) as ReadonlyArray<SessionRow>;
        const row = first(rows);
        if (row === undefined) {
          throw new SarahVoiceSessionRejectedError("The Sarah voice ticket is invalid or expired");
        }
        return toRecord(row);
      });
    } catch (error) {
      if (error instanceof SarahVoiceSessionRejectedError) throw error;
      throw new SarahVoiceStorageError("Sarah voice connection failed", error);
    }
  };

  const prepareLiveKitProvisioningIntent = async (
    input: Readonly<{
      sessionRef: string;
      ownerUserId: string;
      deviceRef: string;
      threadRef: string;
      generation: number;
      capabilityProfile: SarahVoiceClientProfile;
      admissionRef: string;
      admissionDigest: string;
      idempotencyKey: string;
      workerControlTokenDigest: string;
      roomContext: SarahVoiceLiveKitRoomContext;
      nowIso: string;
    }>,
  ): Promise<void> => {
    try {
      await sql.begin(async (tx) => {
        await tx`SELECT pg_advisory_xact_lock(1935763522)`;
        const authorized = (await tx`
          SELECT session.session_ref
          FROM sarah_realtime_voice_sessions AS session
          INNER JOIN sarah_voice_admissions AS admission
            ON admission.session_ref = session.session_ref
          WHERE session.session_ref = ${input.sessionRef}
            AND session.owner_user_id = ${input.ownerUserId}
            AND session.device_ref = ${input.deviceRef}
            AND session.thread_ref = ${input.threadRef}
            AND session.generation = ${input.generation}
            AND session.client_profile = ${input.capabilityProfile}
            AND session.transport_kind = 'livekit_room_v1'
            AND session.state = 'reserved'
            AND admission.admission_ref = ${input.admissionRef}
            AND admission.terms_digest = ${input.admissionDigest}
            AND admission.state = 'consumed'
          FOR UPDATE OF session
        `) as ReadonlyArray<{ session_ref: string }>;
        if (first(authorized) === undefined) {
          throw new SarahVoiceAdmissionRejectedError(
            "The LiveKit provisioning intent is not authorized",
          );
        }
        const capacity = (await tx`
          SELECT COUNT(*) AS active_room_count
          FROM sarah_livekit_provisioning_intents AS intent
          INNER JOIN sarah_realtime_voice_sessions AS session
            ON session.session_ref = intent.session_ref
          WHERE intent.session_ref <> ${input.sessionRef}
            AND intent.state IN ('pending', 'reconciling', 'bound')
            AND session.state IN ('reserved', 'connected')
            AND session.session_expires_at > ${input.nowIso}
        `) as ReadonlyArray<{ active_room_count: number | string }>;
        const activeRoomCount = toSafeInteger(
          first(capacity)?.active_room_count ?? 0,
          "active_room_count",
        );
        if (activeRoomCount >= SARAH_LIVEKIT_MAX_ACTIVE_ROOMS) {
          throw new SarahVoiceLiveKitCapacityError(
            "The Sarah LiveKit active-room capacity is exhausted",
          );
        }
        await tx`
          INSERT INTO sarah_livekit_provisioning_intents (
            session_ref, generation, idempotency_key, owner_user_id,
            device_ref, thread_ref, capability_profile, admission_ref,
            admission_digest, room_context_kind, community_ref, channel_ref,
            membership_revision, worker_control_token_digest, state,
            created_at, updated_at
          ) VALUES (
            ${input.sessionRef}, ${input.generation}, ${input.idempotencyKey},
            ${input.ownerUserId}, ${input.deviceRef}, ${input.threadRef},
            ${input.capabilityProfile}, ${input.admissionRef},
            ${input.admissionDigest}, ${input.roomContext.kind},
            ${input.roomContext.kind === "community" ? input.roomContext.communityRef : null},
            ${input.roomContext.kind === "community" ? input.roomContext.channelRef : null},
            ${input.roomContext.kind === "community" ? input.roomContext.membershipRevision : null},
            ${input.workerControlTokenDigest}, 'pending', ${input.nowIso},
            ${input.nowIso}
          )
          ON CONFLICT (session_ref) DO NOTHING
        `;
        const intents = (await tx`
          SELECT session_ref
          FROM sarah_livekit_provisioning_intents
          WHERE session_ref = ${input.sessionRef}
            AND generation = ${input.generation}
            AND idempotency_key = ${input.idempotencyKey}
            AND owner_user_id = ${input.ownerUserId}
            AND device_ref = ${input.deviceRef}
            AND thread_ref = ${input.threadRef}
            AND capability_profile = ${input.capabilityProfile}
            AND admission_ref = ${input.admissionRef}
            AND admission_digest = ${input.admissionDigest}
            AND room_context_kind = ${input.roomContext.kind}
            AND community_ref IS NOT DISTINCT FROM ${
              input.roomContext.kind === "community" ? input.roomContext.communityRef : null
            }
            AND channel_ref IS NOT DISTINCT FROM ${
              input.roomContext.kind === "community" ? input.roomContext.channelRef : null
            }
            AND membership_revision IS NOT DISTINCT FROM ${
              input.roomContext.kind === "community" ? input.roomContext.membershipRevision : null
            }
            AND worker_control_token_digest = ${input.workerControlTokenDigest}
            AND state IN ('pending', 'bound')
        `) as ReadonlyArray<{ session_ref: string }>;
        if (first(intents) === undefined) {
          throw new SarahVoiceSessionRejectedError(
            "The LiveKit provisioning intent conflicts with an existing generation",
          );
        }
      });
    } catch (error) {
      if (
        error instanceof SarahVoiceAdmissionRejectedError ||
        error instanceof SarahVoiceSessionRejectedError ||
        error instanceof SarahVoiceLiveKitCapacityError
      ) {
        throw error;
      }
      throw new SarahVoiceStorageError("Sarah LiveKit provisioning intent failed", error);
    }
  };

  const claimLiveKitProvisioningIntent = async (
    input: Readonly<{
      sessionRef: string;
      generation: number;
      provisioningOwnerRef: string;
      staleBeforeIso: string;
      nowIso: string;
    }>,
  ): Promise<boolean> => {
    try {
      const rows = (await sql`
        UPDATE sarah_livekit_provisioning_intents
        SET provisioning_owner_ref = ${input.provisioningOwnerRef},
            provisioning_claimed_at = ${input.nowIso},
            updated_at = ${input.nowIso}
        WHERE session_ref = ${input.sessionRef}
          AND generation = ${input.generation}
          AND state IN ('pending', 'bound')
          AND (
            provisioning_owner_ref IS NULL
            OR provisioning_owner_ref = ${input.provisioningOwnerRef}
            OR provisioning_claimed_at <= ${input.staleBeforeIso}
          )
        RETURNING session_ref
      `) as ReadonlyArray<{ session_ref: string }>;
      return first(rows) !== undefined;
    } catch (error) {
      throw new SarahVoiceStorageError("Sarah LiveKit provisioning claim failed", error);
    }
  };

  const markLiveKitProvisioningIntent = async (
    input: Readonly<{
      sessionRef: string;
      generation: number;
      provisioningOwnerRef: string;
      state: "cleanup_failed" | "cleaned";
      nowIso: string;
    }>,
  ): Promise<SarahVoiceLiveKitProvisioningOutcome> => {
    try {
      // A failed attempt either earns an exponentially later retry or, once the
      // bounded attempts are spent, becomes `cleanup_abandoned`. The decision is
      // made here in one statement so the request path and the scheduled
      // reconciler cannot disagree about when to give up.
      const rows = (await sql`
        UPDATE sarah_livekit_provisioning_intents AS intent
        SET state = CASE
              WHEN ${input.state} = 'cleaned' THEN 'cleaned'
              WHEN intent.cleanup_attempt_count
                >= ${SARAH_LIVEKIT_MAX_PROVISIONING_ATTEMPTS} THEN 'cleanup_abandoned'
              ELSE 'cleanup_failed'
            END,
            cleanup_next_attempt_at = CASE
              WHEN ${input.state} = 'cleaned' THEN NULL
              WHEN intent.cleanup_attempt_count
                >= ${SARAH_LIVEKIT_MAX_PROVISIONING_ATTEMPTS} THEN NULL
              ELSE to_char(
                (
                  ${input.nowIso}::timestamptz
                  + make_interval(secs => LEAST(
                      ${SARAH_LIVEKIT_MAX_CLEANUP_BACKOFF_SECONDS}::double precision,
                      ${SARAH_LIVEKIT_BASE_CLEANUP_BACKOFF_SECONDS}::double precision
                        * power(
                            2,
                            GREATEST(intent.cleanup_attempt_count - 1, 0)
                          )
                    ))
                ) AT TIME ZONE 'UTC',
                'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"'
              )
            END,
            cleanup_abandoned_at = CASE
              WHEN ${input.state} <> 'cleaned'
                AND intent.cleanup_attempt_count
                  >= ${SARAH_LIVEKIT_MAX_PROVISIONING_ATTEMPTS}
                THEN COALESCE(intent.cleanup_abandoned_at, ${input.nowIso})
              ELSE NULL
            END,
            provisioning_owner_ref = NULL,
            provisioning_claimed_at = NULL,
            updated_at = ${input.nowIso}
        WHERE intent.session_ref = ${input.sessionRef}
          AND intent.generation = ${input.generation}
          AND intent.provisioning_owner_ref = ${input.provisioningOwnerRef}
          AND intent.state IN ('pending', 'reconciling', 'cleanup_failed', 'cleaned')
        RETURNING intent.session_ref, intent.state, intent.cleanup_attempt_count
      `) as ReadonlyArray<{
        session_ref: string;
        state: SarahVoiceLiveKitProvisioningIntentState;
        cleanup_attempt_count: number | string;
      }>;
      const row = first(rows);
      if (row === undefined) {
        throw new SarahVoiceSessionRejectedError(
          "The LiveKit provisioning intent is owned by another issuer",
        );
      }
      return {
        state: row.state,
        cleanupAttemptCount: toSafeInteger(row.cleanup_attempt_count, "cleanup_attempt_count"),
      };
    } catch (error) {
      if (error instanceof SarahVoiceSessionRejectedError) throw error;
      throw new SarahVoiceStorageError("Sarah LiveKit provisioning intent update failed", error);
    }
  };

  const claimLiveKitProvisioningIntents = async (
    input: Readonly<{
      staleBeforeIso: string;
      nowIso: string;
      provisioningOwnerRef: string;
      limit?: number;
    }>,
  ): Promise<ReadonlyArray<SarahVoiceLiveKitProvisioningIntent>> => {
    try {
      const boundedLimit = Math.max(1, Math.min(100, Math.trunc(input.limit ?? 100)));
      const rows = await sql.begin(async (tx) => {
        // Lazily retire anything that reached the attempt cap without a
        // terminal mark — an attempt whose process died between the claim and
        // the mark would otherwise sit at the cap in a retryable state forever:
        // no longer claimed, but never visibly given up on either. Same lazy
        // dead-letter shape as the room-binding cleanup and the oa_infra_jobs
        // queue.
        await tx`
          UPDATE sarah_livekit_provisioning_intents
          SET state = 'cleanup_abandoned',
              cleanup_abandoned_at = COALESCE(cleanup_abandoned_at, ${input.nowIso}),
              cleanup_next_attempt_at = NULL,
              provisioning_owner_ref = NULL,
              provisioning_claimed_at = NULL,
              updated_at = ${input.nowIso}
          WHERE state IN ('pending', 'reconciling', 'cleanup_failed')
            AND cleanup_attempt_count >= ${SARAH_LIVEKIT_MAX_PROVISIONING_ATTEMPTS}
        `;
        return (await tx`
          WITH candidates AS (
            SELECT session_ref
            FROM sarah_livekit_provisioning_intents
            WHERE state IN ('pending', 'reconciling', 'cleanup_failed')
              AND updated_at <= ${input.staleBeforeIso}
              AND (
                provisioning_owner_ref IS NULL
                OR provisioning_claimed_at <= ${input.staleBeforeIso}
              )
              AND (
                cleanup_next_attempt_at IS NULL
                OR cleanup_next_attempt_at <= ${input.nowIso}
              )
              AND cleanup_attempt_count < ${SARAH_LIVEKIT_MAX_PROVISIONING_ATTEMPTS}
            ORDER BY created_at
            LIMIT ${boundedLimit}
            FOR UPDATE SKIP LOCKED
          )
          UPDATE sarah_livekit_provisioning_intents AS intent
          SET state = 'reconciling',
              provisioning_owner_ref = ${input.provisioningOwnerRef},
              provisioning_claimed_at = ${input.nowIso},
              cleanup_attempt_count = intent.cleanup_attempt_count + 1,
              updated_at = ${input.nowIso}
          FROM candidates
          WHERE intent.session_ref = candidates.session_ref
          RETURNING intent.session_ref, intent.generation,
            intent.idempotency_key, intent.provisioning_owner_ref,
            intent.cleanup_attempt_count
        `) as ReadonlyArray<{
          session_ref: string;
          generation: number | string;
          idempotency_key: string;
          provisioning_owner_ref: string;
          cleanup_attempt_count: number | string;
        }>;
      });
      return rows.map((row) => ({
        sessionRef: row.session_ref,
        generation: toSafeInteger(row.generation, "generation"),
        idempotencyKey: row.idempotency_key,
        provisioningOwnerRef: row.provisioning_owner_ref,
        cleanupAttemptCount: toSafeInteger(row.cleanup_attempt_count, "cleanup_attempt_count"),
      }));
    } catch (error) {
      throw new SarahVoiceStorageError("Sarah LiveKit provisioning intent claim failed", error);
    }
  };

  const bindLiveKitRoom = async (
    input: Readonly<{
      sessionRef: string;
      ownerUserId: string;
      deviceRef: string;
      threadRef: string;
      generation: number;
      provisioningOwnerRef: string;
      capabilityProfile: SarahVoiceClientProfile;
      admissionRef: string;
      admissionDigest: string;
      roomContext: SarahVoiceLiveKitRoomContext;
      roomRef: string;
      roomEpoch: number;
      participantRef: string;
      sarahParticipantRef: string;
      participantGrantDigest: string;
      joinExpiresAt: string;
      dispatchRef: string;
      sarahPresenceLeaseRef: string;
      workerControlTokenDigest: string;
      publishAllowed: boolean;
      subscribeAllowed: boolean;
      nowIso: string;
    }>,
  ): Promise<void> => {
    try {
      await sql.begin(async (tx) => {
        const intents = (await tx`
          SELECT session_ref
          FROM sarah_livekit_provisioning_intents
          WHERE session_ref = ${input.sessionRef}
            AND generation = ${input.generation}
            AND owner_user_id = ${input.ownerUserId}
            AND device_ref = ${input.deviceRef}
            AND thread_ref = ${input.threadRef}
            AND capability_profile = ${input.capabilityProfile}
            AND admission_ref = ${input.admissionRef}
            AND admission_digest = ${input.admissionDigest}
            AND room_context_kind = ${input.roomContext.kind}
            AND community_ref IS NOT DISTINCT FROM ${
              input.roomContext.kind === "community" ? input.roomContext.communityRef : null
            }
            AND channel_ref IS NOT DISTINCT FROM ${
              input.roomContext.kind === "community" ? input.roomContext.channelRef : null
            }
            AND membership_revision IS NOT DISTINCT FROM ${
              input.roomContext.kind === "community" ? input.roomContext.membershipRevision : null
            }
            AND state IN ('pending', 'bound')
            AND worker_control_token_digest = ${input.workerControlTokenDigest}
            AND provisioning_owner_ref = ${input.provisioningOwnerRef}
          FOR UPDATE
        `) as ReadonlyArray<{ session_ref: string }>;
        if (first(intents) === undefined) {
          throw new SarahVoiceSessionRejectedError(
            "The LiveKit room has no matching provisioning intent",
          );
        }
        const sessions = (await tx`
          SELECT session_ref
          FROM sarah_realtime_voice_sessions
          WHERE session_ref = ${input.sessionRef}
            AND owner_user_id = ${input.ownerUserId}
            AND device_ref = ${input.deviceRef}
            AND thread_ref = ${input.threadRef}
            AND generation = ${input.generation}
            AND client_profile = ${input.capabilityProfile}
            AND state = 'reserved'
            AND session_expires_at > ${input.nowIso}
          FOR UPDATE
        `) as ReadonlyArray<{ session_ref: string }>;
        if (first(sessions) === undefined) {
          throw new SarahVoiceSessionRejectedError(
            "The LiveKit room binding does not match the reserved voice generation",
          );
        }

        const admissions = (await tx`
            SELECT admission_ref
            FROM sarah_voice_admissions
            WHERE admission_ref = ${input.admissionRef}
              AND session_ref = ${input.sessionRef}
              AND owner_user_id = ${input.ownerUserId}
              AND device_ref = ${input.deviceRef}
              AND thread_ref = ${input.threadRef}
              AND generation = ${input.generation}
              AND client_profile = ${input.capabilityProfile}
              AND terms_digest = ${input.admissionDigest}
              AND state = 'consumed'
            FOR SHARE
        `) as ReadonlyArray<{ admission_ref: string }>;
        if (first(admissions) === undefined) {
          throw new SarahVoiceAdmissionRejectedError(
            "The LiveKit room binding does not match the consumed admission",
          );
        }

        // A re-bind of a generation whose owner seat is already taken is a
        // duplicate participant, not a replay. The participant ref is fixed for
        // the generation, so refreshing the grant would hand a second client
        // the identity that is already in the room. A re-bind before the
        // participant arrives, or after its join window closed, is the ordinary
        // same-generation reconnect and stays admitted.
        const seats = (await tx`
          SELECT owner_joined_at
          FROM sarah_livekit_room_bindings
          WHERE session_ref = ${input.sessionRef}
            AND generation = ${input.generation}
            AND state IN ('prepared', 'active')
            AND join_expires_at > ${input.nowIso}
          FOR UPDATE
        `) as ReadonlyArray<{ owner_joined_at: string | null }>;
        const seat = first(seats);
        if (seat !== undefined && seat.owner_joined_at !== null) {
          throw new SarahVoiceDuplicateParticipantError(
            "The LiveKit owner participant identity is already admitted for this generation",
          );
        }
        const bindings = (await tx`
          INSERT INTO sarah_livekit_room_bindings (
            session_ref, owner_user_id, device_ref, thread_ref, generation,
            capability_profile, admission_ref, admission_digest,
            room_context_kind, community_ref, channel_ref, membership_revision,
            room_ref, room_epoch, participant_ref, sarah_participant_ref,
            participant_grant_digest, join_expires_at, dispatch_ref,
            sarah_presence_lease_ref, publish_allowed, subscribe_allowed,
            worker_control_token_digest, state, created_at, updated_at
          ) VALUES (
            ${input.sessionRef}, ${input.ownerUserId}, ${input.deviceRef},
            ${input.threadRef}, ${input.generation}, ${input.capabilityProfile},
            ${input.admissionRef}, ${input.admissionDigest},
            ${input.roomContext.kind},
            ${input.roomContext.kind === "community" ? input.roomContext.communityRef : null},
            ${input.roomContext.kind === "community" ? input.roomContext.channelRef : null},
            ${input.roomContext.kind === "community" ? input.roomContext.membershipRevision : null},
            ${input.roomRef}, ${input.roomEpoch}, ${input.participantRef},
            ${input.sarahParticipantRef}, ${input.participantGrantDigest},
            ${input.joinExpiresAt}, ${input.dispatchRef},
            ${input.sarahPresenceLeaseRef}, ${input.publishAllowed},
            ${input.subscribeAllowed}, ${input.workerControlTokenDigest},
            'prepared', ${input.nowIso}, ${input.nowIso}
          )
          ON CONFLICT (session_ref) DO UPDATE
          SET participant_grant_digest = EXCLUDED.participant_grant_digest,
              join_expires_at = EXCLUDED.join_expires_at,
              updated_at = EXCLUDED.updated_at
          WHERE sarah_livekit_room_bindings.owner_user_id = EXCLUDED.owner_user_id
            AND sarah_livekit_room_bindings.device_ref = EXCLUDED.device_ref
            AND sarah_livekit_room_bindings.thread_ref = EXCLUDED.thread_ref
            AND sarah_livekit_room_bindings.generation = EXCLUDED.generation
            AND sarah_livekit_room_bindings.capability_profile = EXCLUDED.capability_profile
            AND sarah_livekit_room_bindings.admission_ref = EXCLUDED.admission_ref
            AND sarah_livekit_room_bindings.admission_digest = EXCLUDED.admission_digest
            AND sarah_livekit_room_bindings.room_context_kind = EXCLUDED.room_context_kind
            AND sarah_livekit_room_bindings.community_ref IS NOT DISTINCT FROM
              EXCLUDED.community_ref
            AND sarah_livekit_room_bindings.channel_ref IS NOT DISTINCT FROM
              EXCLUDED.channel_ref
            AND sarah_livekit_room_bindings.membership_revision IS NOT DISTINCT FROM
              EXCLUDED.membership_revision
            AND sarah_livekit_room_bindings.room_ref = EXCLUDED.room_ref
            AND sarah_livekit_room_bindings.room_epoch = EXCLUDED.room_epoch
            AND sarah_livekit_room_bindings.participant_ref = EXCLUDED.participant_ref
            AND sarah_livekit_room_bindings.sarah_participant_ref =
              EXCLUDED.sarah_participant_ref
            AND sarah_livekit_room_bindings.dispatch_ref = EXCLUDED.dispatch_ref
            AND sarah_livekit_room_bindings.sarah_presence_lease_ref =
              EXCLUDED.sarah_presence_lease_ref
            AND sarah_livekit_room_bindings.publish_allowed = EXCLUDED.publish_allowed
            AND sarah_livekit_room_bindings.subscribe_allowed = EXCLUDED.subscribe_allowed
            AND sarah_livekit_room_bindings.worker_control_token_digest =
              EXCLUDED.worker_control_token_digest
            AND sarah_livekit_room_bindings.state IN ('prepared', 'active')
          RETURNING session_ref
        `) as ReadonlyArray<{ session_ref: string }>;
        if (first(bindings) === undefined) {
          throw new SarahVoiceSessionRejectedError(
            "The LiveKit room replay conflicts with the bound generation",
          );
        }
        await tx`
          UPDATE sarah_livekit_provisioning_intents
          SET state = 'bound',
              updated_at = ${input.nowIso}
          WHERE session_ref = ${input.sessionRef}
            AND generation = ${input.generation}
            AND state IN ('pending', 'bound')
            AND provisioning_owner_ref = ${input.provisioningOwnerRef}
        `;
      });
    } catch (error) {
      if (
        error instanceof SarahVoiceSessionRejectedError ||
        error instanceof SarahVoiceAdmissionRejectedError ||
        error instanceof SarahVoiceDuplicateParticipantError
      ) {
        throw error;
      }
      if (isUniqueViolation(error)) {
        throw new SarahVoiceSessionRejectedError(
          "The LiveKit room or participant grant was already bound",
        );
      }
      throw new SarahVoiceStorageError("Sarah LiveKit room binding failed", error);
    }
  };

  /**
   * Is the owner seat of this generation already held by a live participant?
   *
   * The session route asks before it provisions, so a second client is refused
   * without minting a grant or touching the room the first client is in.
   * `bindLiveKitRoom` still refuses under the row lock, which is what settles a
   * race between two simultaneous requests.
   */
  const readLiveKitOwnerParticipantAdmitted = async (
    input: Readonly<{
      sessionRef: string;
      generation: number;
      nowIso: string;
    }>,
  ): Promise<boolean> => {
    try {
      const rows = (await sql`
        SELECT owner_joined_at
        FROM sarah_livekit_room_bindings
        WHERE session_ref = ${input.sessionRef}
          AND generation = ${input.generation}
          AND state IN ('prepared', 'active')
          AND join_expires_at > ${input.nowIso}
      `) as ReadonlyArray<{ owner_joined_at: string | null }>;
      return first(rows)?.owner_joined_at != null;
    } catch (error) {
      throw new SarahVoiceStorageError("Sarah LiveKit owner seat read failed", error);
    }
  };

  const completeLiveKitProvisioningIntent = async (
    input: Readonly<{
      sessionRef: string;
      generation: number;
      provisioningOwnerRef: string;
      nowIso: string;
    }>,
  ): Promise<void> => {
    try {
      const rows = (await sql`
        UPDATE sarah_livekit_provisioning_intents AS intent
        SET provisioning_owner_ref = NULL,
            provisioning_claimed_at = NULL,
            updated_at = ${input.nowIso}
        FROM sarah_livekit_room_bindings AS binding
        WHERE intent.session_ref = ${input.sessionRef}
          AND intent.generation = ${input.generation}
          AND intent.state = 'bound'
          AND intent.provisioning_owner_ref = ${input.provisioningOwnerRef}
          AND binding.session_ref = intent.session_ref
          AND binding.generation = intent.generation
          AND binding.worker_job_ref IS NOT NULL
          AND binding.worker_claimed_at IS NOT NULL
        RETURNING intent.session_ref
      `) as ReadonlyArray<{ session_ref: string }>;
      if (first(rows) === undefined) {
        throw new SarahVoiceSessionRejectedError(
          "The LiveKit provisioning intent is not ready for completion",
        );
      }
    } catch (error) {
      if (error instanceof SarahVoiceSessionRejectedError) throw error;
      throw new SarahVoiceStorageError("Sarah LiveKit provisioning completion failed", error);
    }
  };

  /**
   * Record the first admission of one dispatched participant into a bound room.
   *
   * The claiming worker's control-token digest and job ref are the authority:
   * only the worker that holds this generation can report that the participant
   * it was dispatched for appeared in the room. The participant and room refs
   * are read from that binding rather than accepted from the caller, because
   * the binding is what minted them.
   *
   * `owner_joined_at IS NULL` makes the first admission final. A second
   * admission of the same identity is a duplicate participant, not a resume:
   * the participant grant is minted once per generation, so two live clients
   * holding it collide on one LiveKit identity and race the same room, floor,
   * and accounting session. A legitimate resume replays the SAME worker event
   * ref, which the worker-event ledger settles as `replayed` before this is
   * ever reached, and a legitimate reconnect takes a new generation.
   */
  const recordLiveKitParticipantJoin = async (
    input: Readonly<{
      workerControlTokenDigest: string;
      workerJobRef: string;
      sessionRef: string;
      generation: number;
      role: "owner" | "sarah";
      nowIso: string;
    }>,
  ): Promise<void> => {
    try {
      const rows =
        input.role === "owner"
          ? ((await sql`
              UPDATE sarah_livekit_room_bindings
              SET owner_joined_at = ${input.nowIso},
                  state = 'active',
                  updated_at = ${input.nowIso}
              WHERE worker_control_token_digest = ${input.workerControlTokenDigest}
                AND worker_job_ref = ${input.workerJobRef}
                AND session_ref = ${input.sessionRef}
                AND generation = ${input.generation}
                AND owner_joined_at IS NULL
                AND state IN ('prepared', 'active')
                AND join_expires_at > ${input.nowIso}
              RETURNING session_ref
            `) as ReadonlyArray<{ session_ref: string }>)
          : ((await sql`
          UPDATE sarah_livekit_room_bindings
          SET sarah_joined_at = ${input.nowIso},
              state = 'active',
              updated_at = ${input.nowIso}
          WHERE worker_control_token_digest = ${input.workerControlTokenDigest}
            AND worker_job_ref = ${input.workerJobRef}
            AND session_ref = ${input.sessionRef}
            AND generation = ${input.generation}
            AND sarah_joined_at IS NULL
            AND state IN ('prepared', 'active')
            AND join_expires_at > ${input.nowIso}
          RETURNING session_ref
        `) as ReadonlyArray<{ session_ref: string }>);
      if (first(rows) === undefined) {
        const admitted = (await sql`
          SELECT owner_joined_at, sarah_joined_at
          FROM sarah_livekit_room_bindings
          WHERE worker_control_token_digest = ${input.workerControlTokenDigest}
            AND worker_job_ref = ${input.workerJobRef}
            AND session_ref = ${input.sessionRef}
            AND generation = ${input.generation}
            AND state IN ('prepared', 'active')
            AND join_expires_at > ${input.nowIso}
        `) as ReadonlyArray<{
          owner_joined_at: string | null;
          sarah_joined_at: string | null;
        }>;
        const binding = first(admitted);
        const joinedAt =
          input.role === "owner" ? binding?.owner_joined_at : binding?.sarah_joined_at;
        if (joinedAt !== null && joinedAt !== undefined) {
          throw new SarahVoiceDuplicateParticipantError(
            "The LiveKit participant identity is already admitted for this generation",
          );
        }
        throw new SarahVoiceSessionRejectedError(
          "The LiveKit participant is unexpected, revoked, or expired",
        );
      }
    } catch (error) {
      if (
        error instanceof SarahVoiceSessionRejectedError ||
        error instanceof SarahVoiceDuplicateParticipantError
      ) {
        throw error;
      }
      throw new SarahVoiceStorageError("Sarah LiveKit participant join failed", error);
    }
  };

  const claimLiveKitWorkerJob = async (
    input: Readonly<{
      workerControlTokenDigest: string;
      workerRefDigest: string;
      workerJobRef: string;
      workerRoomSid: string;
      sessionRef: string;
      generation: number;
      roomRef: string;
      roomEpoch: number;
      dispatchRef: string;
      participantRef: string;
      sarahParticipantRef: string;
      sarahPresenceLeaseRef: string;
      capabilityProfile: SarahVoiceClientProfile;
      roomContext: SarahVoiceLiveKitRoomContext;
      nowIso: string;
    }>,
  ): Promise<SarahVoiceLiveKitWorkerClaim> => {
    try {
      return await sql.begin(async (tx) => {
        const rows = (await tx`
          SELECT binding.session_ref, binding.generation,
            binding.owner_user_id, binding.capability_profile,
            binding.room_context_kind, binding.community_ref,
            binding.channel_ref, binding.membership_revision,
            binding.admission_digest,
            session.session_expires_at
          FROM sarah_livekit_room_bindings AS binding
          INNER JOIN sarah_realtime_voice_sessions AS session
            ON session.session_ref = binding.session_ref
          WHERE binding.worker_control_token_digest =
              ${input.workerControlTokenDigest}
            AND binding.session_ref = ${input.sessionRef}
            AND binding.generation = ${input.generation}
            AND binding.room_ref = ${input.roomRef}
            AND binding.room_epoch = ${input.roomEpoch}
            AND binding.dispatch_ref = ${input.dispatchRef}
            AND binding.participant_ref = ${input.participantRef}
            AND binding.sarah_participant_ref = ${input.sarahParticipantRef}
            AND binding.sarah_presence_lease_ref =
              ${input.sarahPresenceLeaseRef}
            AND binding.capability_profile = ${input.capabilityProfile}
            AND binding.room_context_kind = ${input.roomContext.kind}
            AND binding.community_ref IS NOT DISTINCT FROM ${
              input.roomContext.kind === "community" ? input.roomContext.communityRef : null
            }
            AND binding.channel_ref IS NOT DISTINCT FROM ${
              input.roomContext.kind === "community" ? input.roomContext.channelRef : null
            }
            AND binding.membership_revision IS NOT DISTINCT FROM ${
              input.roomContext.kind === "community" ? input.roomContext.membershipRevision : null
            }
            AND (
              binding.worker_job_ref IS NULL
              OR binding.worker_job_ref = ${input.workerJobRef}
            )
            AND (
              binding.worker_ref_digest IS NULL
              OR binding.worker_ref_digest = ${input.workerRefDigest}
            )
            AND (
              binding.worker_room_sid IS NULL
              OR binding.worker_room_sid = ${input.workerRoomSid}
            )
            AND binding.state IN ('prepared', 'active')
            AND binding.join_expires_at > ${input.nowIso}
            AND session.state IN ('reserved', 'connected')
            AND session.session_expires_at > ${input.nowIso}
          FOR UPDATE OF binding
        `) as ReadonlyArray<{
          session_ref: string;
          generation: number | string;
          owner_user_id: string;
          capability_profile: SarahVoiceClientProfile;
          room_context_kind: "private" | "community";
          community_ref: string | null;
          channel_ref: string | null;
          membership_revision: string | null;
          admission_digest: string;
          session_expires_at: string;
        }>;
        const row = first(rows);
        if (row === undefined) {
          throw new SarahVoiceSessionRejectedError(
            "The Sarah LiveKit worker job does not match an active binding",
          );
        }
        await tx`
          UPDATE sarah_livekit_room_bindings
          SET worker_job_ref = ${input.workerJobRef},
              worker_ref_digest = ${input.workerRefDigest},
              worker_room_sid = ${input.workerRoomSid},
              worker_claimed_at =
                COALESCE(worker_claimed_at, ${input.nowIso}),
              worker_last_seen_at = ${input.nowIso},
              updated_at = ${input.nowIso}
          WHERE session_ref = ${input.sessionRef}
            AND generation = ${input.generation}
            AND worker_control_token_digest =
              ${input.workerControlTokenDigest}
        `;
        const roomContext: SarahVoiceLiveKitRoomContext =
          row.room_context_kind === "private"
            ? { kind: "private" }
            : {
                kind: "community",
                communityRef: row.community_ref ?? "",
                channelRef: row.channel_ref ?? "",
                membershipRevision: row.membership_revision ?? "",
              };
        if (
          roomContext.kind === "community" &&
          (roomContext.communityRef === "" ||
            roomContext.channelRef === "" ||
            roomContext.membershipRevision === "")
        ) {
          throw new SarahVoiceStorageError(
            "The Sarah LiveKit community binding is incomplete",
            null,
          );
        }
        return {
          sessionRef: row.session_ref,
          generation: toSafeInteger(row.generation, "generation"),
          ownerUserId: row.owner_user_id,
          capabilityProfile: row.capability_profile,
          roomContext,
          admissionDigest: row.admission_digest,
          sessionExpiresAt: row.session_expires_at,
        };
      });
    } catch (error) {
      if (error instanceof SarahVoiceSessionRejectedError) throw error;
      throw new SarahVoiceStorageError("Sarah LiveKit worker claim failed", error);
    }
  };

  const readLiveKitWorkerReadiness = async (
    input: Readonly<{ sessionRef: string; generation: number }>,
  ): Promise<SarahVoiceLiveKitWorkerReadiness> => {
    try {
      const rows = (await sql`
        SELECT session.state AS session_state, binding.state AS binding_state,
          binding.worker_job_ref, binding.worker_claimed_at,
          binding.worker_closed_at, binding.worker_stop_reason
        FROM sarah_livekit_room_bindings AS binding
        INNER JOIN sarah_realtime_voice_sessions AS session
          ON session.session_ref = binding.session_ref
        WHERE binding.session_ref = ${input.sessionRef}
          AND binding.generation = ${input.generation}
      `) as ReadonlyArray<{
        session_state: SarahVoiceSessionState;
        binding_state: SarahVoiceLiveKitBindingState;
        worker_job_ref: string | null;
        worker_claimed_at: string | null;
        worker_closed_at: string | null;
        worker_stop_reason: SarahVoiceLiveKitWorkerStopReason | null;
      }>;
      const row = first(rows);
      if (
        row === undefined ||
        row.worker_closed_at !== null ||
        row.worker_stop_reason !== null ||
        !["reserved", "connected"].includes(row.session_state) ||
        !["prepared", "active"].includes(row.binding_state)
      ) {
        return "closed";
      }
      return row.worker_job_ref !== null && row.worker_claimed_at !== null ? "claimed" : "waiting";
    } catch (error) {
      throw new SarahVoiceStorageError("Sarah LiveKit worker readiness read failed", error);
    }
  };

  const authorizeLiveKitWorkerEvent = async (
    input: Readonly<{
      workerControlTokenDigest: string;
      workerJobRef: string;
      sessionRef: string;
      generation: number;
      workerRoomSid?: string | undefined;
      nowIso: string;
    }>,
  ): Promise<
    | Readonly<{
        roomRef: string;
        sarahParticipantRef: string;
        state: "prepared" | "active";
      }>
    | undefined
  > => {
    try {
      const rows = (await sql`
        UPDATE sarah_livekit_room_bindings AS binding
        SET worker_last_seen_at = ${input.nowIso},
            updated_at = ${input.nowIso}
        FROM sarah_realtime_voice_sessions AS session
        WHERE binding.worker_control_token_digest =
            ${input.workerControlTokenDigest}
          AND binding.worker_job_ref = ${input.workerJobRef}
          AND binding.session_ref = ${input.sessionRef}
          AND binding.generation = ${input.generation}
          AND (
            ${input.workerRoomSid ?? null}::text IS NULL
            OR binding.worker_room_sid = ${input.workerRoomSid ?? null}
          )
          AND binding.state IN (
            'prepared', 'active', 'cleanup_ready', 'cleanup_failed',
            'cleanup_abandoned', 'cleaned'
          )
          AND binding.join_expires_at > ${input.nowIso}
          AND session.session_ref = binding.session_ref
          AND session.state IN ('reserved', 'connected')
          AND session.session_expires_at > ${input.nowIso}
        RETURNING binding.room_ref, binding.sarah_participant_ref,
          binding.state
      `) as ReadonlyArray<{
        room_ref: string;
        sarah_participant_ref: string;
        state: "prepared" | "active";
      }>;
      const row = first(rows);
      return row === undefined
        ? undefined
        : {
            roomRef: row.room_ref,
            sarahParticipantRef: row.sarah_participant_ref,
            state: row.state,
          };
    } catch (error) {
      throw new SarahVoiceStorageError("Sarah LiveKit worker event authorization failed", error);
    }
  };

  const readLiveKitMembershipLease = async (
    input: Readonly<{
      workerControlTokenDigest: string;
      workerJobRef: string;
      sessionRef: string;
      generation: number;
    }>,
  ): Promise<SarahVoiceLiveKitMembershipLease | undefined> => {
    try {
      const rows = (await sql`
        SELECT binding.owner_user_id, binding.room_context_kind,
          binding.community_ref, binding.channel_ref,
          binding.membership_revision,binding.sarah_presence_lease_ref
        FROM sarah_livekit_room_bindings AS binding
        WHERE binding.worker_control_token_digest =
            ${input.workerControlTokenDigest}
          AND binding.worker_job_ref = ${input.workerJobRef}
          AND binding.session_ref = ${input.sessionRef}
          AND binding.generation = ${input.generation}
          AND binding.state IN (
            'prepared', 'active', 'cleanup_ready', 'cleanup_failed',
            'cleanup_abandoned', 'cleaned'
          )
      `) as ReadonlyArray<{
        owner_user_id: string;
        room_context_kind: "private" | "community";
        community_ref: string | null;
        channel_ref: string | null;
        membership_revision: string | null;
        sarah_presence_lease_ref: string;
      }>;
      const row = first(rows);
      if (row === undefined) return undefined;
      if (row.room_context_kind === "private") {
        return {
          ownerUserId: row.owner_user_id,
          sarahPresenceLeaseRef: row.sarah_presence_lease_ref,
          roomContext: { kind: "private" },
        };
      }
      if (
        row.community_ref === null ||
        row.channel_ref === null ||
        row.membership_revision === null
      ) {
        throw new SarahVoiceStorageError(
          "The Sarah LiveKit community membership lease is incomplete",
          null,
        );
      }
      return {
        ownerUserId: row.owner_user_id,
        sarahPresenceLeaseRef: row.sarah_presence_lease_ref,
        roomContext: {
          kind: "community",
          communityRef: row.community_ref,
          channelRef: row.channel_ref,
          membershipRevision: row.membership_revision,
        },
      };
    } catch (error) {
      if (error instanceof SarahVoiceStorageError) throw error;
      throw new SarahVoiceStorageError("Sarah LiveKit membership lease lookup failed", error);
    }
  };

  const closeLiveKitWorkerJob = async (
    input: Readonly<{
      workerControlTokenDigest: string;
      workerJobRef: string;
      sessionRef: string;
      generation: number;
      closeReason: string;
      nowIso: string;
    }>,
  ): Promise<SarahVoiceSessionRecord> => {
    try {
      return await sql.begin(async (tx) => {
        const rows = (await tx`
          SELECT session_ref
          FROM sarah_livekit_room_bindings
          WHERE worker_control_token_digest =
              ${input.workerControlTokenDigest}
            AND worker_job_ref = ${input.workerJobRef}
            AND session_ref = ${input.sessionRef}
            AND generation = ${input.generation}
          FOR UPDATE
        `) as ReadonlyArray<{ session_ref: string }>;
        if (first(rows) === undefined) {
          throw new SarahVoiceSessionRejectedError(
            "The Sarah LiveKit worker close does not match its generation",
          );
        }
        await tx`
          UPDATE sarah_livekit_room_bindings
          SET worker_closed_at = COALESCE(worker_closed_at, ${input.nowIso}),
              worker_close_reason =
                COALESCE(worker_close_reason, ${input.closeReason.slice(0, 256)}),
              worker_last_seen_at = ${input.nowIso},
              updated_at = ${input.nowIso}
          WHERE session_ref = ${input.sessionRef}
            AND generation = ${input.generation}
        `;
        return settleInTransaction(tx, {
          sessionRef: input.sessionRef,
          closeReason: input.closeReason,
          nowIso: input.nowIso,
        });
      });
    } catch (error) {
      if (error instanceof SarahVoiceSessionRejectedError) throw error;
      throw new SarahVoiceStorageError("Sarah LiveKit worker close failed", error);
    }
  };

  const readLiveKitCleanup = async (
    input: Readonly<{ sessionRef: string; generation: number }>,
  ): Promise<SarahVoiceLiveKitCleanup | undefined> => {
    try {
      const rows = (await sql`
        SELECT binding.session_ref, binding.generation, binding.room_ref,
          binding.room_epoch, binding.dispatch_ref,
          binding.sarah_presence_lease_ref
        FROM sarah_livekit_room_bindings AS binding
        INNER JOIN sarah_realtime_voice_sessions AS session
          ON session.session_ref = binding.session_ref
        WHERE binding.session_ref = ${input.sessionRef}
          AND binding.generation = ${input.generation}
          AND binding.state IN ('cleanup_ready', 'cleanup_failed')
          AND session.state IN ('settled', 'released')
          AND session.settlement_receipt_ref IS NOT NULL
      `) as ReadonlyArray<{
        session_ref: string;
        generation: number | string;
        room_ref: string;
        room_epoch: number | string;
        dispatch_ref: string;
        sarah_presence_lease_ref: string;
      }>;
      const row = first(rows);
      return row === undefined
        ? undefined
        : {
            sessionRef: row.session_ref,
            generation: toSafeInteger(row.generation, "generation"),
            roomRef: row.room_ref,
            roomEpoch: toSafeInteger(row.room_epoch, "room_epoch"),
            dispatchRef: row.dispatch_ref,
            sarahPresenceLeaseRef: row.sarah_presence_lease_ref,
          };
    } catch (error) {
      throw new SarahVoiceStorageError("Sarah LiveKit cleanup lookup failed", error);
    }
  };

  const claimLiveKitCleanups = async (input: {
    staleBeforeIso: string;
    nowIso: string;
    limit?: number;
  }): Promise<ReadonlyArray<SarahVoiceLiveKitCleanupClaim>> => {
    try {
      const boundedLimit = Math.max(1, Math.min(100, Math.trunc(input.limit ?? 100)));
      type CleanupClaimRow = Readonly<{
        session_ref: string;
        generation: number | string;
        room_ref: string;
        room_epoch: number | string;
        dispatch_ref: string;
        sarah_presence_lease_ref: string;
        cleanup_attempted_at: string;
        cleanup_attempt_count: number | string;
      }>;
      const rows = await sql.begin(async (tx) => {
        // Lazily retire anything that reached the attempt cap without a
        // terminal mark — an attempt whose process died between the claim and
        // the mark would otherwise sit at the cap in a retryable state
        // forever: no longer claimed, but never visibly given up on either.
        // Same lazy dead-letter shape as the oa_infra_jobs queue.
        await tx`
          UPDATE sarah_livekit_room_bindings
          SET state = 'cleanup_abandoned',
              cleanup_abandoned_at = COALESCE(cleanup_abandoned_at, ${input.nowIso}),
              cleanup_next_attempt_at = NULL,
              updated_at = ${input.nowIso}
          WHERE state IN ('cleanup_ready', 'cleanup_failed')
            AND cleanup_attempt_count >= ${SARAH_LIVEKIT_MAX_CLEANUP_ATTEMPTS}
        `;
        return (await tx`
            WITH candidates AS (
              SELECT binding.session_ref
              FROM sarah_livekit_room_bindings AS binding
              INNER JOIN sarah_realtime_voice_sessions AS session
                ON session.session_ref = binding.session_ref
              WHERE binding.state IN ('cleanup_ready', 'cleanup_failed')
                AND (
                  binding.cleanup_attempted_at IS NULL
                  OR binding.cleanup_attempted_at <= ${input.staleBeforeIso}
                )
                AND (
                  binding.cleanup_next_attempt_at IS NULL
                  OR binding.cleanup_next_attempt_at <= ${input.nowIso}
                )
                AND binding.cleanup_attempt_count
                  < ${SARAH_LIVEKIT_MAX_CLEANUP_ATTEMPTS}
                AND session.state IN ('settled', 'released')
                AND session.settlement_receipt_ref IS NOT NULL
              ORDER BY binding.updated_at, binding.session_ref
              LIMIT ${boundedLimit}
              FOR UPDATE OF binding SKIP LOCKED
            )
            UPDATE sarah_livekit_room_bindings AS binding
            SET cleanup_attempted_at = ${input.nowIso},
                cleanup_attempt_count = binding.cleanup_attempt_count + 1,
                updated_at = ${input.nowIso}
            FROM candidates
            WHERE binding.session_ref = candidates.session_ref
            RETURNING binding.session_ref, binding.generation,
              binding.room_ref, binding.room_epoch, binding.dispatch_ref,
              binding.sarah_presence_lease_ref, binding.cleanup_attempted_at,
              binding.cleanup_attempt_count
          `) as ReadonlyArray<CleanupClaimRow>;
      });
      return rows.map((row) => ({
        sessionRef: row.session_ref,
        generation: toSafeInteger(row.generation, "generation"),
        roomRef: row.room_ref,
        roomEpoch: toSafeInteger(row.room_epoch, "room_epoch"),
        dispatchRef: row.dispatch_ref,
        sarahPresenceLeaseRef: row.sarah_presence_lease_ref,
        cleanupAttemptedAt: row.cleanup_attempted_at,
        cleanupAttemptCount: toSafeInteger(row.cleanup_attempt_count, "cleanup_attempt_count"),
      }));
    } catch (error) {
      throw new SarahVoiceStorageError("Sarah LiveKit cleanup claim failed", error);
    }
  };

  const markLiveKitCleanup = async (
    input: Readonly<{
      sessionRef: string;
      generation: number;
      state: "cleaned" | "cleanup_failed";
      nowIso: string;
    }>,
  ): Promise<SarahVoiceLiveKitCleanupOutcome> => {
    try {
      // A failed attempt either earns an exponentially later retry or, once the
      // bounded attempts are spent, becomes `cleanup_abandoned`. The decision
      // is made here in one statement so the one-shot worker-close path and the
      // scheduled reconciler cannot disagree about when to give up.
      const rows = (await sql`
        UPDATE sarah_livekit_room_bindings AS binding
        SET state = CASE
              WHEN ${input.state} = 'cleaned' THEN 'cleaned'
              WHEN binding.cleanup_attempt_count
                >= ${SARAH_LIVEKIT_MAX_CLEANUP_ATTEMPTS} THEN 'cleanup_abandoned'
              ELSE 'cleanup_failed'
            END,
            cleanup_attempted_at = ${input.nowIso},
            cleanup_next_attempt_at = CASE
              WHEN ${input.state} = 'cleaned' THEN NULL
              WHEN binding.cleanup_attempt_count
                >= ${SARAH_LIVEKIT_MAX_CLEANUP_ATTEMPTS} THEN NULL
              ELSE to_char(
                (
                  ${input.nowIso}::timestamptz
                  + make_interval(secs => LEAST(
                      ${SARAH_LIVEKIT_MAX_CLEANUP_BACKOFF_SECONDS}::double precision,
                      ${SARAH_LIVEKIT_BASE_CLEANUP_BACKOFF_SECONDS}::double precision
                        * power(
                            2,
                            GREATEST(binding.cleanup_attempt_count - 1, 0)
                          )
                    ))
                ) AT TIME ZONE 'UTC',
                'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"'
              )
            END,
            cleanup_abandoned_at = CASE
              WHEN ${input.state} <> 'cleaned'
                AND binding.cleanup_attempt_count
                  >= ${SARAH_LIVEKIT_MAX_CLEANUP_ATTEMPTS}
                THEN COALESCE(binding.cleanup_abandoned_at, ${input.nowIso})
              ELSE NULL
            END,
            cleaned_at = CASE
              WHEN ${input.state} = 'cleaned'
                THEN COALESCE(binding.cleaned_at, ${input.nowIso})
              ELSE NULL
            END,
            updated_at = ${input.nowIso}
        FROM sarah_realtime_voice_sessions AS session
        WHERE binding.session_ref = ${input.sessionRef}
          AND binding.generation = ${input.generation}
          AND session.session_ref = binding.session_ref
          AND session.state IN ('settled', 'released')
          AND session.settlement_receipt_ref IS NOT NULL
          AND (
            binding.state IN ('cleanup_ready', 'cleanup_failed')
            OR (${input.state} = 'cleaned' AND binding.state = 'cleaned')
          )
        RETURNING binding.session_ref, binding.state, binding.cleanup_attempt_count
      `) as ReadonlyArray<{
        session_ref: string;
        state: SarahVoiceLiveKitBindingState;
        cleanup_attempt_count: number | string;
      }>;
      const row = first(rows);
      if (row === undefined) {
        throw new SarahVoiceSessionRejectedError(
          "LiveKit cleanup is not eligible before terminal accounting",
        );
      }
      return {
        state: row.state,
        cleanupAttemptCount: toSafeInteger(row.cleanup_attempt_count, "cleanup_attempt_count"),
      };
    } catch (error) {
      if (error instanceof SarahVoiceSessionRejectedError) throw error;
      throw new SarahVoiceStorageError("Sarah LiveKit cleanup update failed", error);
    }
  };

  const setLiveKitWorkerStopInTransaction = async (
    tx: SyncTransactionSql,
    input: Readonly<{
      sessionRef: string;
      generation: number;
      stopReason: SarahVoiceLiveKitWorkerStopReason;
      closeReason: string;
      nowIso: string;
    }>,
  ): Promise<void> => {
    await tx`
      UPDATE sarah_livekit_room_bindings
      SET worker_stop_reason = COALESCE(worker_stop_reason, ${input.stopReason}),
          worker_stop_close_reason =
            COALESCE(worker_stop_close_reason, ${input.closeReason.slice(0, 256)}),
          worker_stop_requested_at =
            COALESCE(worker_stop_requested_at, ${input.nowIso}),
          worker_stop_deadline_at = COALESCE(
            worker_stop_deadline_at,
            ${plusMillisecondsIso(input.nowIso, SARAH_LIVEKIT_WORKER_DRAIN_TIMEOUT_MS)}
          ),
          updated_at = ${input.nowIso}
      WHERE session_ref = ${input.sessionRef}
        AND generation = ${input.generation}
    `;
  };

  const requestLiveKitWorkerStopInTransaction = async (
    tx: SyncTransactionSql,
    input: Readonly<{
      sessionRef: string;
      generation: number;
      stopReason: SarahVoiceLiveKitWorkerStopReason;
      closeReason: string;
      nowIso: string;
    }>,
  ): Promise<SarahVoiceSessionRecord> => {
    const rows = (await tx`
      SELECT session.session_ref, session.owner_user_id,
        session.owner_actor_ref, session.device_ref, session.thread_ref,
        session.generation, session.disclosure_ref, session.client_profile,
        session.transport_kind, session.credit_mode, session.entitlement_ref,
        session.admission_cohort_ref, session.state, session.reserved_msat,
        session.charged_msat, session.ticket_expires_at,
        session.session_expires_at, session.settlement_receipt_ref
      FROM sarah_livekit_room_bindings AS binding
      INNER JOIN sarah_realtime_voice_sessions AS session
        ON session.session_ref = binding.session_ref
      WHERE binding.session_ref = ${input.sessionRef}
        AND binding.generation = ${input.generation}
      FOR UPDATE OF session
    `) as ReadonlyArray<SessionRow>;
    const row = first(rows);
    if (row === undefined) {
      throw new SarahVoiceSessionRejectedError("The LiveKit room generation is not active");
    }
    const session = toRecord(row);
    if (
      session.state === "accounting_uncertain" ||
      session.state === "settled" ||
      session.state === "released" ||
      session.state === "failed"
    ) {
      return session;
    }
    const bindings = (await tx`
      SELECT session_ref
      FROM sarah_livekit_room_bindings
      WHERE session_ref = ${input.sessionRef}
        AND generation = ${input.generation}
      FOR UPDATE
    `) as ReadonlyArray<{ session_ref: string }>;
    if (first(bindings) === undefined) {
      throw new SarahVoiceSessionRejectedError("The LiveKit room generation is not active");
    }
    await setLiveKitWorkerStopInTransaction(tx, input);
    return session;
  };

  const revokeLiveKitRoom = async (
    input: Readonly<{
      sessionRef: string;
      generation: number;
      stopReason: "membership_revoked" | "operator_stop";
      reason: string;
      nowIso: string;
    }>,
  ): Promise<SarahVoiceSessionRecord> => {
    try {
      return await sql.begin(async (tx) => {
        return requestLiveKitWorkerStopInTransaction(tx, {
          sessionRef: input.sessionRef,
          generation: input.generation,
          stopReason: input.stopReason,
          closeReason: input.reason,
          nowIso: input.nowIso,
        });
      });
    } catch (error) {
      if (error instanceof SarahVoiceSessionRejectedError) throw error;
      throw new SarahVoiceStorageError("Sarah LiveKit room revocation failed", error);
    }
  };

  const requestLiveKitWorkerInterrupt = async (
    input: Readonly<{
      sessionRef: string;
      generation: number;
      nowIso: string;
    }>,
  ): Promise<
    Readonly<{
      interruptSequence: number;
      roomRef: string;
      roomEpoch: number;
      sarahParticipantRef: string;
    }>
  > => {
    try {
      const rows = (await sql`
        UPDATE sarah_livekit_room_bindings AS binding
        SET worker_interrupt_sequence = binding.worker_interrupt_sequence + 1,
            worker_interrupt_requested_at = ${input.nowIso},
            updated_at = ${input.nowIso}
        FROM sarah_realtime_voice_sessions AS session
        WHERE binding.session_ref = ${input.sessionRef}
          AND binding.generation = ${input.generation}
          AND binding.state = 'active'
          AND binding.worker_job_ref IS NOT NULL
          AND binding.worker_closed_at IS NULL
          AND binding.worker_stop_reason IS NULL
          AND session.session_ref = binding.session_ref
          AND session.generation = binding.generation
          AND session.transport_kind = 'livekit_room_v1'
          AND session.state = 'connected'
          AND session.session_expires_at > ${input.nowIso}
        RETURNING binding.worker_interrupt_sequence, binding.room_ref,
          binding.room_epoch, binding.sarah_participant_ref
      `) as ReadonlyArray<{
        worker_interrupt_sequence: number | string;
        room_ref: string;
        room_epoch: number | string;
        sarah_participant_ref: string;
      }>;
      const row = first(rows);
      if (row === undefined) {
        throw new SarahVoiceSessionRejectedError(
          "The Sarah LiveKit generation cannot accept an interrupt",
        );
      }
      return {
        interruptSequence: toSafeInteger(
          row.worker_interrupt_sequence,
          "worker interrupt sequence",
        ),
        roomRef: row.room_ref,
        roomEpoch: toSafeInteger(row.room_epoch, "room epoch"),
        sarahParticipantRef: row.sarah_participant_ref,
      };
    } catch (error) {
      if (error instanceof SarahVoiceSessionRejectedError) throw error;
      throw new SarahVoiceStorageError("Sarah LiveKit interrupt request failed", error);
    }
  };

  const readLiveKitWorkerInterruptApplied = async (
    input: Readonly<{
      sessionRef: string;
      generation: number;
    }>,
  ): Promise<number> => {
    try {
      const rows = (await sql`
        SELECT binding.worker_interrupt_applied_sequence
        FROM sarah_livekit_room_bindings AS binding
        INNER JOIN sarah_realtime_voice_sessions AS session
          ON session.session_ref = binding.session_ref
          AND session.generation = binding.generation
        WHERE binding.session_ref = ${input.sessionRef}
          AND binding.generation = ${input.generation}
          AND binding.state = 'active'
          AND binding.worker_job_ref IS NOT NULL
          AND binding.worker_closed_at IS NULL
          AND session.transport_kind = 'livekit_room_v1'
          AND session.state = 'connected'
      `) as ReadonlyArray<{
        worker_interrupt_applied_sequence: number | string;
      }>;
      const row = first(rows);
      if (row === undefined) {
        throw new SarahVoiceSessionRejectedError(
          "The Sarah LiveKit generation has no active interrupt worker",
        );
      }
      return toSafeInteger(
        row.worker_interrupt_applied_sequence,
        "worker interrupt applied sequence",
      );
    } catch (error) {
      if (error instanceof SarahVoiceSessionRejectedError) throw error;
      throw new SarahVoiceStorageError("Sarah LiveKit interrupt receipt read failed", error);
    }
  };

  const settleLiveKitProvisioningIntent = async (
    input: Readonly<{
      sessionRef: string;
      generation: number;
      provisioningOwnerRef: string;
      closeReason: string;
      nowIso: string;
    }>,
  ): Promise<SarahVoiceSessionRecord> => {
    try {
      return await sql.begin(async (tx) => {
        const intents = (await tx`
          SELECT session_ref
          FROM sarah_livekit_provisioning_intents
          WHERE session_ref = ${input.sessionRef}
            AND generation = ${input.generation}
            AND provisioning_owner_ref = ${input.provisioningOwnerRef}
            AND state IN (
              'pending',
              'bound',
              'reconciling',
              'cleanup_failed',
              'cleaned'
            )
          FOR UPDATE
        `) as ReadonlyArray<{ session_ref: string }>;
        if (first(intents) === undefined) {
          throw new SarahVoiceSessionRejectedError(
            "The LiveKit provisioning generation does not exist",
          );
        }
        return settleInTransaction(tx, {
          sessionRef: input.sessionRef,
          closeReason: input.closeReason,
          nowIso: input.nowIso,
        });
      });
    } catch (error) {
      if (error instanceof SarahVoiceSessionRejectedError) throw error;
      throw new SarahVoiceStorageError("Sarah LiveKit provisioning settlement failed", error);
    }
  };

  const recordUsageInTransaction = async (
    tx: SyncTransactionSql,
    input: Readonly<{
      sessionRef: string;
      generation: number;
      usage: SarahVoiceProviderUsage;
    }>,
  ): Promise<
    Readonly<{
      chargedMsat: number;
      reservedMsat: number;
      creditLimitReached: boolean;
    }>
  > => {
    const sessions = (await tx`
      SELECT generation, state, credit_mode, credit_rate_msat_per_million_tokens
      FROM sarah_realtime_voice_sessions
      WHERE session_ref = ${input.sessionRef}
      FOR UPDATE
    `) as ReadonlyArray<{
      generation: number | string;
      state: SarahVoiceSessionState;
      credit_mode: SarahVoiceCreditMode;
      credit_rate_msat_per_million_tokens: number | string | null;
    }>;
    const session = first(sessions);
    if (
      session === undefined ||
      toSafeInteger(session.generation, "generation") !== input.generation ||
      session.state !== "connected"
    ) {
      throw new SarahVoiceSessionRejectedError(
        "The provider usage does not match the active Sarah voice generation",
      );
    }
    if (session.credit_rate_msat_per_million_tokens === null) {
      throw new SarahVoiceSessionRejectedError(
        "The provider usage does not have a frozen admitted credit rate",
      );
    }
    const creditRateMsatPerMillionTokens = toSafeInteger(
      session.credit_rate_msat_per_million_tokens,
      "credit_rate_msat_per_million_tokens",
    );
    if (creditRateMsatPerMillionTokens <= 0) {
      throw new SarahVoiceSessionRejectedError(
        "The provider usage has an invalid frozen admitted credit rate",
      );
    }
    const chargeNumerator =
      (BigInt(input.usage.inputTokens) + BigInt(input.usage.outputTokens)) *
      BigInt(creditRateMsatPerMillionTokens);
    const chargeMsatBigInt = (chargeNumerator + 999_999n) / 1_000_000n;
    if (chargeMsatBigInt > BigInt(Number.MAX_SAFE_INTEGER)) {
      throw new SarahVoiceSessionRejectedError("The provider usage charge is too large");
    }
    const usage = {
      ...input.usage,
      chargeMsat: session.credit_mode === "owner_waived_unmetered" ? 0 : Number(chargeMsatBigInt),
    };

    const inserted = (await tx`
      INSERT INTO sarah_realtime_voice_usage (
        session_ref, provider_response_ref, input_tokens, output_tokens,
        cached_input_tokens, audio_input_tokens, audio_output_tokens,
        charge_msat, observed_at, usage_kind, provider_status
      ) VALUES (
        ${input.sessionRef}, ${usage.providerResponseRef},
        ${usage.inputTokens}, ${usage.outputTokens},
        ${usage.cachedInputTokens}, ${usage.audioInputTokens},
        ${usage.audioOutputTokens}, ${usage.chargeMsat},
        ${usage.observedAt}, ${usage.usageKind ?? "response"},
        ${usage.providerStatus ?? null}
      )
      ON CONFLICT (session_ref, provider_response_ref) DO NOTHING
      RETURNING session_ref
    `) as ReadonlyArray<{ session_ref: string }>;

    if (first(inserted) === undefined) {
      const replayed = (await tx`
        SELECT input_tokens, output_tokens, cached_input_tokens,
          audio_input_tokens, audio_output_tokens, charge_msat, observed_at,
          usage_kind, provider_status
        FROM sarah_realtime_voice_usage
        WHERE session_ref = ${input.sessionRef}
          AND provider_response_ref = ${usage.providerResponseRef}
        FOR SHARE
      `) as ReadonlyArray<{
        input_tokens: number | string;
        output_tokens: number | string;
        cached_input_tokens: number | string;
        audio_input_tokens: number | string;
        audio_output_tokens: number | string;
        charge_msat: number | string;
        observed_at: string;
        usage_kind: "response" | "transcription";
        provider_status: "completed" | "cancelled" | "failed" | "incomplete" | null;
      }>;
      const replay = first(replayed);
      if (
        replay === undefined ||
        toSafeInteger(replay.input_tokens, "input_tokens") !== usage.inputTokens ||
        toSafeInteger(replay.output_tokens, "output_tokens") !== usage.outputTokens ||
        toSafeInteger(replay.cached_input_tokens, "cached_input_tokens") !==
          usage.cachedInputTokens ||
        toSafeInteger(replay.audio_input_tokens, "audio_input_tokens") !== usage.audioInputTokens ||
        toSafeInteger(replay.audio_output_tokens, "audio_output_tokens") !==
          usage.audioOutputTokens ||
        toSafeInteger(replay.charge_msat, "charge_msat") !== usage.chargeMsat ||
        replay.observed_at !== usage.observedAt ||
        replay.usage_kind !== (usage.usageKind ?? "response") ||
        replay.provider_status !== (usage.providerStatus ?? null)
      ) {
        throw new SarahVoiceSessionRejectedError(
          "The provider response reference was replayed with changed usage",
        );
      }
    } else {
      await tx`
        UPDATE sarah_realtime_voice_sessions
        SET input_tokens = input_tokens + ${usage.inputTokens},
            output_tokens = output_tokens + ${usage.outputTokens},
            cached_input_tokens =
              cached_input_tokens + ${usage.cachedInputTokens},
            audio_input_tokens =
              audio_input_tokens + ${usage.audioInputTokens},
            audio_output_tokens =
              audio_output_tokens + ${usage.audioOutputTokens},
            charged_msat = CASE
              WHEN credit_mode = 'owner_waived_unmetered' THEN 0
              WHEN credit_mode = 'metered' THEN LEAST(
                reserved_msat,
                charged_msat + ${usage.chargeMsat}
              )
              ELSE charged_msat + ${usage.chargeMsat}
            END,
            updated_at = ${usage.observedAt}
        WHERE session_ref = ${input.sessionRef}
          AND generation = ${input.generation}
          AND state = 'connected'
      `;
    }

    const rows = (await tx`
      SELECT reserved_msat, charged_msat, credit_mode
      FROM sarah_realtime_voice_sessions
      WHERE session_ref = ${input.sessionRef}
      FOR UPDATE
    `) as ReadonlyArray<{
      reserved_msat: number | string;
      charged_msat: number | string;
      credit_mode: SarahVoiceCreditMode;
    }>;
    const row = first(rows);
    if (row === undefined) {
      throw new SarahVoiceSessionRejectedError("The voice session does not exist");
    }
    const reservedMsat = toSafeInteger(row.reserved_msat, "reserved_msat");
    const chargedMsat = toSafeInteger(row.charged_msat, "charged_msat");
    return {
      chargedMsat,
      reservedMsat,
      creditLimitReached: row.credit_mode === "metered" && chargedMsat >= reservedMsat,
    };
  };

  const recordUsage = async (
    input: Readonly<{
      sessionRef: string;
      generation: number;
      usage: SarahVoiceProviderUsage;
    }>,
  ): Promise<
    Readonly<{
      chargedMsat: number;
      reservedMsat: number;
      creditLimitReached: boolean;
    }>
  > => {
    try {
      return await sql.begin((tx) => recordUsageInTransaction(tx, input));
    } catch (error) {
      if (error instanceof SarahVoiceSessionRejectedError) throw error;
      throw new SarahVoiceStorageError("Sarah voice usage write failed", error);
    }
  };

  const finalizeUnmeteredAuthorityCaptureInTransaction = async (
    tx: SyncTransactionSql,
    input: Readonly<{
      sessionRef: string;
      generation: number;
      reservedMsat: number;
      chargedMsat: number;
      terminalReceiptRef: string;
      nowIso: string;
      ownerActorRef: string;
    }>,
  ): Promise<void> => {
    const payInRows = (await tx`
      SELECT COUNT(*) AS pay_in_count
      FROM pay_ins
      WHERE context_ref = ${input.sessionRef}
        OR id = ${`sarah:voice:${input.sessionRef}`}
    `) as ReadonlyArray<{ pay_in_count: number | string }>;
    const payInLegRows = (await tx`
      SELECT COUNT(*) AS pay_in_leg_count
      FROM pay_in_legs
      WHERE external_ref = 'sarah_realtime_voice'
        AND pay_in_id = ${`sarah:voice:${input.sessionRef}`}
    `) as ReadonlyArray<{ pay_in_leg_count: number | string }>;
    const payInCount = toSafeInteger(first(payInRows)?.pay_in_count ?? 0, "pay_in_count");
    const payInLegCount = toSafeInteger(
      first(payInLegRows)?.pay_in_leg_count ?? 0,
      "pay_in_leg_count",
    );
    const ledgerMutationCount =
      payInCount +
      payInLegCount +
      (input.reservedMsat === 0 ? 0 : 1) +
      (input.chargedMsat === 0 ? 0 : 1);
    const endLedgerStateDigest = unmeteredLedgerStateDigest({
      sessionRef: input.sessionRef,
      generation: input.generation,
      reservedMsat: input.reservedMsat,
      chargedMsat: input.chargedMsat,
      payInCount,
      payInLegCount,
    });
    const balanceRows = (await tx`
      SELECT balance_msat, held_msat, updated_at
      FROM agent_balances
      WHERE actor_ref = ${input.ownerActorRef}
      FOR SHARE
    `) as ReadonlyArray<{
      balance_msat: number | string;
      held_msat: number | string;
      updated_at: string;
    }>;
    const balance = first(balanceRows);
    const endBalanceStateDigest = unmeteredBalanceStateDigest({
      ownerActorRef: input.ownerActorRef,
      rowPresent: balance !== undefined,
      balanceMsat:
        balance === undefined ? null : toSafeInteger(balance.balance_msat, "balance_msat"),
      heldMsat: balance === undefined ? null : toSafeInteger(balance.held_msat, "held_msat"),
      updatedAt: balance?.updated_at ?? null,
    });
    const captureRows = (await tx`
      SELECT start_ledger_state_digest, start_balance_state_digest, terminal_at
      FROM sarah_voice_unmetered_authority_captures
      WHERE session_ref = ${input.sessionRef}
        AND generation = ${input.generation}
      FOR UPDATE
    `) as ReadonlyArray<{
      start_ledger_state_digest: string;
      start_balance_state_digest: string;
      terminal_at: string | null;
    }>;
    const capture = first(captureRows);
    if (capture === undefined) {
      throw new SarahVoiceStorageError("Unmetered authority capture is missing", null);
    }
    if (capture.terminal_at !== null) return;
    const captureDigest = acceptanceDigest(
      JSON.stringify({
        schema: "openagents.sarah.unmetered-authority-capture.v1",
        authority: "owner_waived_unmetered_v1",
        sessionRef: input.sessionRef,
        generation: input.generation,
        startLedgerStateDigest: capture.start_ledger_state_digest,
        endLedgerStateDigest,
        startBalanceStateDigest: capture.start_balance_state_digest,
        endBalanceStateDigest,
        ledgerMutationCount,
        terminalReceiptRef: input.terminalReceiptRef,
      }),
    );
    await tx`
      UPDATE sarah_voice_unmetered_authority_captures
      SET end_ledger_state_digest = ${endLedgerStateDigest},
          end_balance_state_digest = ${endBalanceStateDigest},
          ledger_mutation_count = ${ledgerMutationCount},
          capture_receipt_ref = ${`sarah_voice_unmetered_authority:${captureDigest}`},
          capture_digest = ${captureDigest},
          terminal_authority_ref = ${input.terminalReceiptRef},
          terminal_at = ${input.nowIso}
      WHERE session_ref = ${input.sessionRef}
        AND generation = ${input.generation}
        AND terminal_at IS NULL
    `;
  };

  const markLiveKitAccountingUncertainInTransaction = async (
    tx: SyncTransactionSql,
    input: Readonly<{
      sessionRef: string;
      generation: number;
      reason: string;
      nowIso: string;
      workerHeartbeatExpiredBeforeIso?: string;
    }>,
  ): Promise<SarahVoiceSessionRecord> => {
    const rows = (await tx`
      SELECT session_ref, owner_user_id, owner_actor_ref, device_ref,
        thread_ref, generation, disclosure_ref, client_profile,
        transport_kind, credit_mode, entitlement_ref, admission_cohort_ref,
        state, reserved_msat, charged_msat, ticket_expires_at,
        session_expires_at, settlement_receipt_ref
      FROM sarah_realtime_voice_sessions
      WHERE session_ref = ${input.sessionRef}
        AND generation = ${input.generation}
      FOR UPDATE
    `) as ReadonlyArray<SessionRow>;
    const row = first(rows);
    if (row === undefined) {
      throw new SarahVoiceSessionRejectedError("The voice session does not exist");
    }
    const current = toRecord(row);
    if (current.state === "accounting_uncertain") return current;
    if (current.state === "settled" || current.state === "released" || current.state === "failed") {
      throw new SarahVoiceSessionRejectedError(
        "Terminal Sarah voice accounting cannot become uncertain",
      );
    }
    const bindings = (await tx`
      SELECT provider_admitted_at, provider_accounting_status,
        worker_stop_reason, worker_last_seen_at
      FROM sarah_livekit_room_bindings
      WHERE session_ref = ${input.sessionRef}
        AND generation = ${input.generation}
      FOR UPDATE
    `) as ReadonlyArray<{
      provider_admitted_at: string | null;
      provider_accounting_status: "pending" | "exact" | "uncertain";
      worker_stop_reason: SarahVoiceLiveKitWorkerStopReason | null;
      worker_last_seen_at: string | null;
    }>;
    const binding = first(bindings);
    if (binding === undefined || binding.provider_admitted_at === null) {
      throw new SarahVoiceSessionRejectedError(
        "Uncertain accounting requires an admitted LiveKit provider",
      );
    }
    if (binding.provider_accounting_status === "exact") {
      if (input.workerHeartbeatExpiredBeforeIso !== undefined) return current;
      throw new SarahVoiceSessionRejectedError("Exact provider accounting cannot become uncertain");
    }
    if (
      input.workerHeartbeatExpiredBeforeIso !== undefined &&
      (binding.worker_stop_reason !== null ||
        binding.worker_last_seen_at === null ||
        binding.worker_last_seen_at > input.workerHeartbeatExpiredBeforeIso)
    ) {
      return current;
    }
    await tx`
      UPDATE sarah_livekit_room_bindings
      SET provider_accounting_status = 'uncertain',
          provider_accounting_terminal_at = NULL,
          provider_accounting_uncertain_at =
            COALESCE(provider_accounting_uncertain_at, ${input.nowIso}),
          provider_accounting_uncertain_reason =
            COALESCE(provider_accounting_uncertain_reason, ${input.reason.slice(0, 256)}),
          updated_at = ${input.nowIso}
      WHERE session_ref = ${input.sessionRef}
        AND generation = ${input.generation}
    `;
    const uncertainRows = (await tx`
      UPDATE sarah_realtime_voice_sessions
      SET state = 'accounting_uncertain',
          ticket_digest = NULL,
          close_reason = ${input.reason.slice(0, 256)},
          updated_at = ${input.nowIso}
      WHERE session_ref = ${input.sessionRef}
        AND generation = ${input.generation}
        AND state IN ('reserved', 'connected')
      RETURNING session_ref, owner_user_id, owner_actor_ref, device_ref,
        thread_ref, generation, disclosure_ref, client_profile,
        transport_kind, credit_mode, entitlement_ref, admission_cohort_ref,
        state, reserved_msat, charged_msat, ticket_expires_at,
        session_expires_at, settlement_receipt_ref
    `) as ReadonlyArray<SessionRow>;
    const uncertain = first(uncertainRows);
    if (uncertain === undefined) {
      throw new SarahVoiceStorageError("The uncertain accounting state was not persisted", null);
    }
    const uncertainRecord = toRecord(uncertain);
    if (uncertainRecord.creditMode === "owner_waived_unmetered") {
      await finalizeUnmeteredAuthorityCaptureInTransaction(tx, {
        sessionRef: uncertainRecord.sessionRef,
        generation: uncertainRecord.generation,
        reservedMsat: uncertainRecord.reservedMsat,
        chargedMsat: uncertainRecord.chargedMsat,
        terminalReceiptRef:
          `sarah_voice_accounting_uncertain:${uncertainRecord.sessionRef}:${uncertainRecord.generation}`,
        nowIso: input.nowIso,
        ownerActorRef: uncertainRecord.ownerActorRef,
      });
    }
    await tx`
      UPDATE sarah_livekit_room_bindings
      SET state = 'cleanup_ready',
          cleanup_attempt_count = 0,
          cleanup_next_attempt_at = NULL,
          cleanup_abandoned_at = NULL,
          updated_at = ${input.nowIso}
      WHERE session_ref = ${uncertainRecord.sessionRef}
        AND generation = ${uncertainRecord.generation}
        AND state IN ('prepared', 'active', 'cleanup_failed')
    `;
    return uncertainRecord;
  };

  const settleInTransaction = async (
    tx: SyncTransactionSql,
    input: Readonly<{
      sessionRef: string;
      closeReason: string;
      nowIso: string;
      exactReconciliation?: boolean | undefined;
    }>,
  ): Promise<SarahVoiceSessionRecord> => {
    const rows = (await tx`
      SELECT session_ref, owner_user_id, owner_actor_ref, device_ref,
        thread_ref, generation, disclosure_ref, client_profile,
        transport_kind, credit_mode,
        entitlement_ref, admission_cohort_ref, state, reserved_msat,
        charged_msat, ticket_expires_at, session_expires_at,
        settlement_receipt_ref, credit_rate_msat_per_million_tokens
      FROM sarah_realtime_voice_sessions
      WHERE session_ref = ${input.sessionRef}
      FOR UPDATE
    `) as ReadonlyArray<
      SessionRow & Readonly<{ credit_rate_msat_per_million_tokens: number | string | null }>
    >;
    const row = first(rows);
    if (row === undefined) {
      throw new SarahVoiceSessionRejectedError("The voice session does not exist");
    }
    const current = toRecord(row);
    if (current.state === "accounting_uncertain" && input.exactReconciliation !== true) {
      throw new SarahVoiceSessionRejectedError(
        "Sarah voice accounting is uncertain and requires explicit reconciliation or owner waiver",
      );
    }
    if (current.state === "connected" && row.credit_rate_msat_per_million_tokens === null) {
      throw new SarahVoiceSessionRejectedError(
        "Sarah voice accounting has no frozen admitted credit rate",
      );
    }
    if (current.state === "settled" || current.state === "released" || current.state === "failed") {
      return current;
    }
    if (current.transportKind === "livekit_room_v1") {
      const bindings = (await tx`
        SELECT provider_admitted_at, provider_accounting_status
        FROM sarah_livekit_room_bindings
        WHERE session_ref = ${current.sessionRef}
          AND generation = ${current.generation}
        FOR UPDATE
      `) as ReadonlyArray<{
        provider_admitted_at: string | null;
        provider_accounting_status: "pending" | "exact" | "uncertain";
      }>;
      const binding = first(bindings);
      if (
        binding?.provider_admitted_at !== null &&
        binding?.provider_admitted_at !== undefined &&
        binding.provider_accounting_status !== "exact"
      ) {
        throw new SarahVoiceSessionRejectedError(
          "An admitted LiveKit provider requires exact terminal accounting before settlement",
        );
      }
    }

    const receiptRef = `sarah_voice_settlement:${current.sessionRef}`;
    if (
      current.creditMode === "staging_owner_entitlement" ||
      current.creditMode === "owner_waived_unmetered"
    ) {
      // The session and usage rows are the settlement evidence. The staging
      // entitlement and owner waiver do not write a payment or change credit.
    } else if (current.chargedMsat > 0) {
      await tx`
        INSERT INTO pay_ins (
          id, pay_in_type, payer_ref, cost_msat, state, rung, context_ref,
          idempotency_key, public_receipt_ref, genesis_id, created_at,
          state_changed_at
        ) VALUES (
          ${`sarah:voice:${current.sessionRef}`}, 'adjustment',
          ${current.ownerActorRef}, ${current.chargedMsat}, 'paid', NULL,
          ${current.sessionRef}, ${`sarah:voice:settle:${current.sessionRef}`},
          ${receiptRef}, NULL, ${input.nowIso}, ${input.nowIso}
        )
        ON CONFLICT (idempotency_key) DO NOTHING
      `;
      const debited = (await tx`
        UPDATE agent_balances
        SET held_msat = held_msat - ${current.reservedMsat},
            balance_msat = balance_msat - ${current.chargedMsat},
            updated_at = ${input.nowIso}
        WHERE actor_ref = ${current.ownerActorRef}
          AND held_msat >= ${current.reservedMsat}
          AND balance_msat >= ${current.chargedMsat}
        RETURNING actor_ref
      `) as ReadonlyArray<{ actor_ref: string }>;
      if (first(debited) === undefined) {
        throw new SarahVoiceStorageError("The reserved balance could not be settled", null);
      }
      await tx`
        INSERT INTO pay_in_legs (
          id, pay_in_id, direction, kind, party_ref, amount_msat,
          resulting_balance_msat, external_ref, refund_of_leg_id, created_at
        )
        SELECT ${`sarah:voice:${current.sessionRef}:balance`},
          ${`sarah:voice:${current.sessionRef}`}, 'in', 'balance',
          ${current.ownerActorRef}, ${current.chargedMsat}, balance_msat,
          'sarah_realtime_voice', NULL, ${input.nowIso}
        FROM agent_balances
        WHERE actor_ref = ${current.ownerActorRef}
        ON CONFLICT (id) DO NOTHING
      `;
    } else {
      const released = (await tx`
        UPDATE agent_balances
        SET held_msat = held_msat - ${current.reservedMsat},
            updated_at = ${input.nowIso}
        WHERE actor_ref = ${current.ownerActorRef}
          AND held_msat >= ${current.reservedMsat}
        RETURNING actor_ref
      `) as ReadonlyArray<{ actor_ref: string }>;
      if (first(released) === undefined) {
        throw new SarahVoiceStorageError("The reserved balance could not be released", null);
      }
    }

    const settledRows = (await tx`
      UPDATE sarah_realtime_voice_sessions
      SET state = ${current.chargedMsat > 0 ? "settled" : "released"},
          ticket_digest = NULL,
          settlement_receipt_ref = ${receiptRef},
          close_reason = ${input.closeReason.slice(0, 256)},
          settled_at = ${input.nowIso},
          updated_at = ${input.nowIso}
      WHERE session_ref = ${current.sessionRef}
      RETURNING session_ref, owner_user_id, owner_actor_ref, device_ref,
        thread_ref, generation, disclosure_ref, client_profile,
        transport_kind, credit_mode,
        entitlement_ref, admission_cohort_ref, state, reserved_msat,
        charged_msat, ticket_expires_at, session_expires_at,
        settlement_receipt_ref
    `) as ReadonlyArray<SessionRow>;
    const settled = first(settledRows);
    if (settled === undefined) {
      throw new SarahVoiceStorageError("The settlement did not return a row", null);
    }
    if (current.creditMode === "owner_waived_unmetered") {
      await finalizeUnmeteredAuthorityCaptureInTransaction(tx, {
        sessionRef: current.sessionRef,
        generation: current.generation,
        reservedMsat: current.reservedMsat,
        chargedMsat: current.chargedMsat,
        terminalReceiptRef: receiptRef,
        nowIso: input.nowIso,
        ownerActorRef: current.ownerActorRef,
      });
    }
    await tx`
      UPDATE sarah_livekit_room_bindings
      SET state = 'cleanup_ready',
          cleanup_attempt_count = 0,
          cleanup_next_attempt_at = NULL,
          cleanup_abandoned_at = NULL,
          updated_at = ${input.nowIso}
      WHERE session_ref = ${current.sessionRef}
        AND state IN ('prepared', 'active', 'cleanup_failed')
    `;
    return toRecord(settled);
  };

  const settle = async (
    input: Readonly<{
      sessionRef: string;
      closeReason: string;
      nowIso: string;
    }>,
  ): Promise<SarahVoiceSessionRecord> => {
    try {
      return await sql.begin((tx) => settleInTransaction(tx, input));
    } catch (error) {
      if (error instanceof SarahVoiceSessionRejectedError) throw error;
      throw new SarahVoiceStorageError("Sarah voice settlement failed", error);
    }
  };

  const waiveLiveKitAccounting = async (
    input: Readonly<{
      waiverRef: string;
      waiverPayloadDigest: string;
      sessionRef: string;
      generation: number;
      providerSessionRefDigest: string;
      operatorActorRef: string;
      reason: string;
      providerEvidenceRefs: ReadonlyArray<string>;
      nowIso: string;
    }>,
  ): Promise<SarahVoiceAccountingWaiverResult> => {
    try {
      if (
        input.waiverRef.length < 1 ||
        input.waiverRef.length > 256 ||
        !/^[0-9a-f]{64}$/u.test(input.waiverPayloadDigest) ||
        !/^[0-9a-f]{64}$/u.test(input.providerSessionRefDigest) ||
        input.reason.length < 1 ||
        input.reason.length > 1_024 ||
        input.providerEvidenceRefs.length < 1 ||
        input.providerEvidenceRefs.length > 16 ||
        new Set(input.providerEvidenceRefs).size !== input.providerEvidenceRefs.length ||
        input.providerEvidenceRefs.some(
          (reference) => reference.length < 1 || reference.length > 512,
        )
      ) {
        throw new SarahVoiceSessionRejectedError("The Sarah voice accounting waiver is invalid");
      }
      const waiverReceiptRef = `sarah_voice_accounting_waiver:${input.waiverPayloadDigest}`;
      return await sql.begin(async (tx) => {
        const sessions = (await tx`
          SELECT session_ref, generation, owner_actor_ref, state, transport_kind,
            credit_mode, reserved_msat, charged_msat
          FROM sarah_realtime_voice_sessions
          WHERE session_ref = ${input.sessionRef}
          FOR UPDATE
        `) as ReadonlyArray<{
          session_ref: string;
          generation: number | string;
          owner_actor_ref: string;
          state: SarahVoiceSessionState;
          transport_kind: SarahVoiceTransportKind;
          credit_mode: SarahVoiceCreditMode;
          reserved_msat: number | string;
          charged_msat: number | string;
        }>;
        const session = first(sessions);
        if (
          session === undefined ||
          toSafeInteger(session.generation, "generation") !== input.generation
        ) {
          throw new SarahVoiceSessionRejectedError(
            "The Sarah voice accounting waiver generation does not exist",
          );
        }

        const priorRows = (await tx`
          SELECT waiver_ref, waiver_receipt_ref, waiver_payload_digest,
            session_ref, generation, operator_actor_ref, waiver_reason,
            provider_evidence_refs_json, provider_session_ref_digest,
            prior_reserved_msat, prior_recorded_charge_msat,
            provider_accounting_status, authority
          FROM sarah_voice_accounting_waivers
          WHERE waiver_ref = ${input.waiverRef}
            OR session_ref = ${input.sessionRef}
          FOR SHARE
        `) as ReadonlyArray<{
          waiver_ref: string;
          waiver_receipt_ref: string;
          waiver_payload_digest: string;
          session_ref: string;
          generation: number | string;
          operator_actor_ref: string;
          waiver_reason: string;
          provider_evidence_refs_json: unknown;
          provider_session_ref_digest: string;
          prior_reserved_msat: number | string;
          prior_recorded_charge_msat: number | string;
          provider_accounting_status: "uncertain";
          authority: "owner_waived_unmetered_v1";
        }>;
        const prior = first(priorRows);
        if (prior !== undefined) {
          if (
            priorRows.length !== 1 ||
            prior.waiver_ref !== input.waiverRef ||
            prior.waiver_receipt_ref !== waiverReceiptRef ||
            prior.waiver_payload_digest !== input.waiverPayloadDigest ||
            prior.session_ref !== input.sessionRef ||
            toSafeInteger(prior.generation, "generation") !== input.generation ||
            prior.operator_actor_ref !== input.operatorActorRef ||
            prior.waiver_reason !== input.reason ||
            prior.provider_session_ref_digest !== input.providerSessionRefDigest ||
            !Array.isArray(prior.provider_evidence_refs_json) ||
            JSON.stringify(prior.provider_evidence_refs_json) !==
              JSON.stringify(input.providerEvidenceRefs) ||
            session.state !== "released" ||
            session.credit_mode !== "owner_waived_unmetered" ||
            toSafeInteger(session.reserved_msat, "reserved_msat") !== 0 ||
            toSafeInteger(session.charged_msat, "charged_msat") !== 0
          ) {
            throw new SarahVoiceSessionRejectedError(
              "The Sarah voice accounting waiver conflicts with prior authority",
            );
          }
          return {
            waiverRef: prior.waiver_ref,
            waiverReceiptRef: prior.waiver_receipt_ref,
            sessionRef: prior.session_ref,
            state: "released",
            releasedHoldMsat: toSafeInteger(prior.prior_reserved_msat, "prior_reserved_msat"),
            recordedChargeWaivedMsat: toSafeInteger(
              prior.prior_recorded_charge_msat,
              "prior_recorded_charge_msat",
            ),
            providerAccountingStatus: prior.provider_accounting_status,
            authority: prior.authority,
            replayed: true,
          };
        }
        if (
          session.transport_kind !== "livekit_room_v1" ||
          session.state !== "accounting_uncertain"
        ) {
          throw new SarahVoiceSessionRejectedError(
            "Only uncertain LiveKit accounting can receive an owner waiver",
          );
        }
        const bindings = (await tx`
          SELECT provider_accounting_status, provider_session_ref_digest
          FROM sarah_livekit_room_bindings
          WHERE session_ref = ${input.sessionRef}
            AND generation = ${input.generation}
          FOR UPDATE
        `) as ReadonlyArray<{
          provider_accounting_status: "pending" | "exact" | "uncertain";
          provider_session_ref_digest: string | null;
        }>;
        const binding = first(bindings);
        if (
          binding?.provider_accounting_status !== "uncertain" ||
          binding.provider_session_ref_digest !== input.providerSessionRefDigest
        ) {
          throw new SarahVoiceSessionRejectedError(
            "The LiveKit provider accounting evidence does not match the waiver",
          );
        }
        const priorReservedMsat = toSafeInteger(session.reserved_msat, "reserved_msat");
        const priorRecordedChargeMsat = toSafeInteger(session.charged_msat, "charged_msat");
        if (priorReservedMsat > 0) {
          const released = (await tx`
            UPDATE agent_balances
            SET held_msat = held_msat - ${priorReservedMsat},
                updated_at = ${input.nowIso}
            WHERE actor_ref = ${session.owner_actor_ref}
              AND held_msat >= ${priorReservedMsat}
            RETURNING actor_ref
          `) as ReadonlyArray<{ actor_ref: string }>;
          if (first(released) === undefined) {
            throw new SarahVoiceStorageError(
              "The waived Sarah voice hold could not be released",
              null,
            );
          }
        }
        await tx`
          INSERT INTO sarah_voice_accounting_waivers (
            waiver_ref, waiver_receipt_ref, waiver_payload_digest, session_ref,
            generation, operator_actor_ref, waiver_reason,
            provider_evidence_refs_json, provider_session_ref_digest,
            prior_reserved_msat, prior_recorded_charge_msat,
            provider_accounting_status, authority, created_at
          ) VALUES (
            ${input.waiverRef}, ${waiverReceiptRef}, ${input.waiverPayloadDigest},
            ${input.sessionRef}, ${input.generation}, ${input.operatorActorRef},
            ${input.reason},
            ${JSON.stringify(input.providerEvidenceRefs)}::text::jsonb,
            ${input.providerSessionRefDigest}, ${priorReservedMsat},
            ${priorRecordedChargeMsat}, 'uncertain',
            'owner_waived_unmetered_v1', ${input.nowIso}
          )
        `;
        await tx`
          UPDATE sarah_realtime_voice_sessions
          SET state = 'released', credit_mode = 'owner_waived_unmetered',
              entitlement_ref = NULL, reserved_msat = 0, charged_msat = 0,
              ticket_digest = NULL, settlement_receipt_ref = ${waiverReceiptRef},
              close_reason = ${`accounting_owner_waived:${input.waiverRef}`.slice(0, 256)},
              settled_at = ${input.nowIso}, updated_at = ${input.nowIso}
          WHERE session_ref = ${input.sessionRef}
            AND generation = ${input.generation}
            AND state = 'accounting_uncertain'
        `;
        return {
          waiverRef: input.waiverRef,
          waiverReceiptRef,
          sessionRef: input.sessionRef,
          state: "released",
          releasedHoldMsat: priorReservedMsat,
          recordedChargeWaivedMsat: priorRecordedChargeMsat,
          providerAccountingStatus: "uncertain",
          authority: "owner_waived_unmetered_v1",
          replayed: false,
        };
      });
    } catch (error) {
      if (
        error instanceof SarahVoiceSessionRejectedError ||
        error instanceof SarahVoiceStorageError
      ) {
        throw error;
      }
      throw new SarahVoiceStorageError("Sarah voice accounting waiver failed", error);
    }
  };

  const reconcileLiveKitAccounting = async (
    input: Readonly<{
      reconciliationRef: string;
      reconciliationPayloadDigest: string;
      sessionRef: string;
      generation: number;
      providerSessionRefDigest: string;
      operatorActorRef: string;
      reason: string;
      providerEvidenceRefs: ReadonlyArray<string>;
      usage: ReadonlyArray<SarahVoiceAccountingReconciliationUsage>;
      nowIso: string;
    }>,
  ): Promise<SarahVoiceAccountingReconciliationResult> => {
    try {
      if (
        !/^[0-9a-f]{64}$/u.test(input.reconciliationPayloadDigest) ||
        !/^[0-9a-f]{64}$/u.test(input.providerSessionRefDigest) ||
        input.providerEvidenceRefs.length < 1 ||
        input.providerEvidenceRefs.length > 16 ||
        new Set(input.providerEvidenceRefs).size !== input.providerEvidenceRefs.length ||
        input.usage.length > 1_024
      ) {
        throw new SarahVoiceSessionRejectedError(
          "The Sarah voice accounting reconciliation is invalid",
        );
      }
      const reconciliationReceiptRef = `sarah_voice_accounting_reconciliation:${input.reconciliationPayloadDigest}`;
      return await sql.begin(async (tx) => {
        const sessions = (await tx`
          SELECT session_ref, generation, state, transport_kind, credit_mode,
            reserved_msat, settlement_receipt_ref,
            credit_rate_msat_per_million_tokens
          FROM sarah_realtime_voice_sessions
          WHERE session_ref = ${input.sessionRef}
          FOR UPDATE
        `) as ReadonlyArray<{
          session_ref: string;
          generation: number | string;
          state: SarahVoiceSessionState;
          transport_kind: SarahVoiceTransportKind;
          credit_mode: SarahVoiceCreditMode;
          reserved_msat: number | string;
          settlement_receipt_ref: string | null;
          credit_rate_msat_per_million_tokens: number | string | null;
        }>;
        const session = first(sessions);
        if (
          session === undefined ||
          toSafeInteger(session.generation, "generation") !== input.generation
        ) {
          throw new SarahVoiceSessionRejectedError(
            "The Sarah voice accounting reconciliation generation does not exist",
          );
        }
        if (session.credit_rate_msat_per_million_tokens === null) {
          throw new SarahVoiceSessionRejectedError(
            "The Sarah voice accounting reconciliation has no frozen admitted credit rate",
          );
        }
        const creditRateMsatPerMillionTokens = toSafeInteger(
          session.credit_rate_msat_per_million_tokens,
          "credit_rate_msat_per_million_tokens",
        );
        if (creditRateMsatPerMillionTokens <= 0) {
          throw new SarahVoiceSessionRejectedError(
            "The Sarah voice accounting reconciliation credit rate is invalid",
          );
        }

        const priorRows = (await tx`
          SELECT reconciliation_ref, reconciliation_receipt_ref, session_ref,
            generation, reconciliation_payload_digest, operator_actor_ref,
            reconciliation_reason, provider_evidence_refs_json,
            credit_rate_msat_per_million_tokens,
            provider_session_ref_digest
          FROM sarah_livekit_accounting_reconciliations
          WHERE reconciliation_ref = ${input.reconciliationRef}
            OR session_ref = ${input.sessionRef}
          FOR SHARE
        `) as ReadonlyArray<{
          reconciliation_ref: string;
          reconciliation_receipt_ref: string;
          session_ref: string;
          generation: number | string;
          reconciliation_payload_digest: string;
          operator_actor_ref: string;
          reconciliation_reason: string;
          provider_evidence_refs_json: unknown;
          credit_rate_msat_per_million_tokens: number | string;
          provider_session_ref_digest: string | null;
        }>;
        const prior = first(priorRows);
        if (prior !== undefined) {
          const priorEvidence = prior.provider_evidence_refs_json;
          if (
            priorRows.length !== 1 ||
            prior.reconciliation_ref !== input.reconciliationRef ||
            prior.reconciliation_receipt_ref !== reconciliationReceiptRef ||
            prior.session_ref !== input.sessionRef ||
            toSafeInteger(prior.generation, "generation") !== input.generation ||
            prior.reconciliation_payload_digest !== input.reconciliationPayloadDigest ||
            prior.operator_actor_ref !== input.operatorActorRef ||
            prior.reconciliation_reason !== input.reason ||
            prior.provider_session_ref_digest !== input.providerSessionRefDigest ||
            !Array.isArray(priorEvidence) ||
            JSON.stringify(priorEvidence) !== JSON.stringify(input.providerEvidenceRefs) ||
            toSafeInteger(
              prior.credit_rate_msat_per_million_tokens,
              "credit_rate_msat_per_million_tokens",
            ) !== creditRateMsatPerMillionTokens ||
            (session.state !== "settled" && session.state !== "released") ||
            session.settlement_receipt_ref === null
          ) {
            throw new SarahVoiceSessionRejectedError(
              "The Sarah voice accounting reconciliation conflicts with prior evidence",
            );
          }
          const terminalRows = (await tx`
            SELECT charged_msat
            FROM sarah_realtime_voice_sessions
            WHERE session_ref = ${input.sessionRef}
          `) as ReadonlyArray<{ charged_msat: number | string }>;
          const terminal = first(terminalRows);
          if (terminal === undefined) {
            throw new SarahVoiceStorageError(
              "The reconciled Sarah voice session disappeared",
              null,
            );
          }
          return {
            reconciliationRef: prior.reconciliation_ref,
            reconciliationReceiptRef: prior.reconciliation_receipt_ref,
            sessionRef: prior.session_ref,
            state: session.state,
            finalChargeMsat: toSafeInteger(terminal.charged_msat, "charged_msat"),
            settlementReceiptRef: session.settlement_receipt_ref,
            replayed: true,
          };
        }
        if (
          session.transport_kind !== "livekit_room_v1" ||
          session.state !== "accounting_uncertain"
        ) {
          throw new SarahVoiceSessionRejectedError(
            "Only uncertain LiveKit accounting can be reconciled",
          );
        }
        const bindings = (await tx`
          SELECT provider_admitted_at, provider_accounting_status,
            provider_session_ref_digest
          FROM sarah_livekit_room_bindings
          WHERE session_ref = ${input.sessionRef}
            AND generation = ${input.generation}
          FOR UPDATE
        `) as ReadonlyArray<{
          provider_admitted_at: string | null;
          provider_accounting_status: "pending" | "exact" | "uncertain";
          provider_session_ref_digest: string | null;
        }>;
        const binding = first(bindings);
        if (
          binding === undefined ||
          binding.provider_admitted_at === null ||
          binding.provider_accounting_status !== "uncertain" ||
          binding.provider_session_ref_digest !== input.providerSessionRefDigest
        ) {
          throw new SarahVoiceSessionRejectedError(
            "The LiveKit provider accounting state is not reconcilable",
          );
        }

        const exactUsage = new Map<
          string,
          SarahVoiceAccountingReconciliationUsage & Readonly<{ chargeMsat: number }>
        >();
        for (const usage of input.usage) {
          const expectedPrefix = usage.usageKind === "response" ? "response:" : "transcription:";
          if (
            !usage.providerResponseRef.startsWith(expectedPrefix) ||
            (usage.usageKind === "response" && usage.providerStatus === undefined) ||
            (usage.usageKind === "transcription" && usage.providerStatus !== undefined) ||
            ![
              usage.inputTokens,
              usage.outputTokens,
              usage.cachedInputTokens,
              usage.audioInputTokens,
              usage.audioOutputTokens,
            ].every((value) => Number.isSafeInteger(value) && value >= 0) ||
            usage.cachedInputTokens > usage.inputTokens ||
            usage.audioInputTokens > usage.inputTokens ||
            usage.audioOutputTokens > usage.outputTokens
          ) {
            throw new SarahVoiceSessionRejectedError("The reconciled provider usage is invalid");
          }
          const chargeNumerator =
            (BigInt(usage.inputTokens) + BigInt(usage.outputTokens)) *
            BigInt(creditRateMsatPerMillionTokens);
          const chargeMsatBigInt = (chargeNumerator + 999_999n) / 1_000_000n;
          if (
            chargeMsatBigInt > BigInt(Number.MAX_SAFE_INTEGER) ||
            exactUsage.has(usage.providerResponseRef)
          ) {
            throw new SarahVoiceSessionRejectedError(
              "The reconciled provider usage is duplicated or too large",
            );
          }
          const chargeMsat = Number(chargeMsatBigInt);
          exactUsage.set(usage.providerResponseRef, { ...usage, chargeMsat });
          // Reconciliation inserts only numeric usage and opaque provider refs.
          // eslint-disable-next-line no-await-in-loop
          await tx`
            INSERT INTO sarah_realtime_voice_usage (
              session_ref, provider_response_ref, input_tokens, output_tokens,
              cached_input_tokens, audio_input_tokens, audio_output_tokens,
              charge_msat, observed_at, usage_kind, provider_status
            ) VALUES (
              ${input.sessionRef}, ${usage.providerResponseRef},
              ${usage.inputTokens}, ${usage.outputTokens},
              ${usage.cachedInputTokens}, ${usage.audioInputTokens},
              ${usage.audioOutputTokens}, ${chargeMsat}, ${input.nowIso},
              ${usage.usageKind}, ${usage.providerStatus ?? null}
            )
            ON CONFLICT (session_ref, provider_response_ref) DO NOTHING
          `;
        }

        const persistedUsage = (await tx`
          SELECT provider_response_ref, input_tokens, output_tokens,
            cached_input_tokens, audio_input_tokens, audio_output_tokens,
            charge_msat, usage_kind, provider_status
          FROM sarah_realtime_voice_usage
          WHERE session_ref = ${input.sessionRef}
          ORDER BY provider_response_ref
          FOR SHARE
        `) as ReadonlyArray<{
          provider_response_ref: string;
          input_tokens: number | string;
          output_tokens: number | string;
          cached_input_tokens: number | string;
          audio_input_tokens: number | string;
          audio_output_tokens: number | string;
          charge_msat: number | string;
          usage_kind: "response" | "transcription";
          provider_status: "completed" | "cancelled" | "failed" | "incomplete" | null;
        }>;
        if (persistedUsage.length !== exactUsage.size) {
          throw new SarahVoiceSessionRejectedError(
            "The reconciliation does not contain the complete provider usage set",
          );
        }
        for (const persisted of persistedUsage) {
          const expected = exactUsage.get(persisted.provider_response_ref);
          if (
            expected === undefined ||
            toSafeInteger(persisted.input_tokens, "input_tokens") !== expected.inputTokens ||
            toSafeInteger(persisted.output_tokens, "output_tokens") !== expected.outputTokens ||
            toSafeInteger(persisted.cached_input_tokens, "cached_input_tokens") !==
              expected.cachedInputTokens ||
            toSafeInteger(persisted.audio_input_tokens, "audio_input_tokens") !==
              expected.audioInputTokens ||
            toSafeInteger(persisted.audio_output_tokens, "audio_output_tokens") !==
              expected.audioOutputTokens ||
            toSafeInteger(persisted.charge_msat, "charge_msat") !== expected.chargeMsat ||
            persisted.usage_kind !== expected.usageKind ||
            persisted.provider_status !== (expected.providerStatus ?? null)
          ) {
            throw new SarahVoiceSessionRejectedError(
              "The reconciliation conflicts with previously recorded provider usage",
            );
          }
        }

        const sum = (
          select: (
            usage: SarahVoiceAccountingReconciliationUsage & Readonly<{ chargeMsat: number }>,
          ) => number,
          field: string,
        ): number => {
          const total = [...exactUsage.values()].reduce(
            (accumulator, usage) => accumulator + BigInt(select(usage)),
            0n,
          );
          if (total > BigInt(Number.MAX_SAFE_INTEGER)) {
            throw new SarahVoiceSessionRejectedError(`The reconciled ${field} total is too large`);
          }
          return Number(total);
        };
        const inputTokens = sum((usage) => usage.inputTokens, "input token");
        const outputTokens = sum((usage) => usage.outputTokens, "output token");
        const cachedInputTokens = sum((usage) => usage.cachedInputTokens, "cached input token");
        const audioInputTokens = sum((usage) => usage.audioInputTokens, "audio input token");
        const audioOutputTokens = sum((usage) => usage.audioOutputTokens, "audio output token");
        const exactChargeMsat = sum((usage) => usage.chargeMsat, "charge");
        const finalChargeMsat = Math.min(
          toSafeInteger(session.reserved_msat, "reserved_msat"),
          exactChargeMsat,
        );

        await tx`
          INSERT INTO sarah_livekit_accounting_reconciliations (
            reconciliation_ref, reconciliation_receipt_ref, session_ref,
            generation, reconciliation_payload_digest, operator_actor_ref,
            reconciliation_reason, provider_evidence_refs_json,
            credit_rate_msat_per_million_tokens,
            provider_session_ref_digest, created_at
          ) VALUES (
            ${input.reconciliationRef}, ${reconciliationReceiptRef},
            ${input.sessionRef}, ${input.generation},
            ${input.reconciliationPayloadDigest}, ${input.operatorActorRef},
            ${input.reason},
            ${JSON.stringify(input.providerEvidenceRefs)}::text::jsonb,
            ${creditRateMsatPerMillionTokens},
            ${input.providerSessionRefDigest}, ${input.nowIso}
          )
        `;
        await tx`
          UPDATE sarah_livekit_room_bindings
          SET provider_accounting_status = 'exact',
              provider_accounting_terminal_at = ${input.nowIso},
              provider_accounting_uncertain_at = NULL,
              provider_accounting_uncertain_reason = NULL,
              updated_at = ${input.nowIso}
          WHERE session_ref = ${input.sessionRef}
            AND generation = ${input.generation}
        `;
        await tx`
          UPDATE sarah_realtime_voice_sessions
          SET input_tokens = ${inputTokens},
              output_tokens = ${outputTokens},
              cached_input_tokens = ${cachedInputTokens},
              audio_input_tokens = ${audioInputTokens},
              audio_output_tokens = ${audioOutputTokens},
              charged_msat = ${finalChargeMsat},
              updated_at = ${input.nowIso}
          WHERE session_ref = ${input.sessionRef}
            AND generation = ${input.generation}
            AND state = 'accounting_uncertain'
        `;
        const settled = await settleInTransaction(tx, {
          sessionRef: input.sessionRef,
          closeReason: `accounting_reconciled:${input.reconciliationRef}`,
          nowIso: input.nowIso,
          exactReconciliation: true,
        });
        if (
          (settled.state !== "settled" && settled.state !== "released") ||
          settled.settlementReceiptRef === null
        ) {
          throw new SarahVoiceStorageError(
            "The reconciled Sarah voice session did not settle",
            null,
          );
        }
        return {
          reconciliationRef: input.reconciliationRef,
          reconciliationReceiptRef,
          sessionRef: input.sessionRef,
          state: settled.state,
          finalChargeMsat: settled.chargedMsat,
          settlementReceiptRef: settled.settlementReceiptRef,
          replayed: false,
        };
      });
    } catch (error) {
      if (
        error instanceof SarahVoiceSessionRejectedError ||
        error instanceof SarahVoiceStorageError
      ) {
        throw error;
      }
      throw new SarahVoiceStorageError("Sarah voice accounting reconciliation failed", error);
    }
  };

  const readSettlement = async (
    input: Readonly<{ sessionRef: string; ownerUserId: string }>,
  ): Promise<SarahVoiceSettlementProjection | undefined> => {
    try {
      const rows = (await sql`
        SELECT session.session_ref, session.generation, session.state, session.credit_mode,
          session.charged_msat, session.reserved_msat,
          session.settlement_receipt_ref, session.reservation_ref,
          session.admission_cohort_ref,
          session.input_tokens, session.output_tokens,
          session.cached_input_tokens, session.audio_input_tokens,
          session.audio_output_tokens, session.accounting_escalation_ref,
          session.accounting_escalated_at,
          binding.provider_accounting_uncertain_reason,
          binding.worker_job_ref, binding.provider_session_ref_digest,
          binding.provider_configuration_digest, binding.capability_profile,
          binding.admission_digest, binding.room_context_kind,
          binding.community_ref, binding.channel_ref,
          binding.membership_revision, binding.sarah_participant_ref,
          binding.provider_accounting_status, binding.worker_closed_at,
          binding.provider_admitted_at,
          waiver.waiver_receipt_ref, waiver.waiver_payload_digest,
          waiver.provider_evidence_refs_json,
          waiver.provider_accounting_status AS waiver_provider_accounting_status,
          waiver.authority AS waiver_authority,
          capture.generation AS capture_generation,
          capture.start_ledger_state_digest, capture.end_ledger_state_digest,
          capture.start_balance_state_digest, capture.end_balance_state_digest,
          capture.ledger_mutation_count, capture.capture_receipt_ref,
          capture.capture_digest,
          CASE
            WHEN session.credit_mode = 'metered'
              THEN COALESCE(
                GREATEST(balance.balance_msat - balance.held_msat, 0),
                0
              )
            ELSE NULL
          END AS spendable_remaining_credit_msat
        FROM sarah_realtime_voice_sessions AS session
        LEFT JOIN agent_balances AS balance
          ON balance.actor_ref = session.owner_actor_ref
        LEFT JOIN sarah_livekit_room_bindings AS binding
          ON binding.session_ref = session.session_ref
        LEFT JOIN sarah_voice_accounting_waivers AS waiver
          ON waiver.session_ref = session.session_ref
        LEFT JOIN sarah_voice_unmetered_authority_captures AS capture
          ON capture.session_ref = session.session_ref
        WHERE session.session_ref = ${input.sessionRef}
          AND session.owner_user_id = ${input.ownerUserId}
          AND session.state IN ('accounting_uncertain', 'settled', 'released')
      `) as ReadonlyArray<{
        session_ref: string;
        generation: number | string;
        state: "accounting_uncertain" | "settled" | "released";
        credit_mode: SarahVoiceCreditMode;
        charged_msat: number | string;
        reserved_msat: number | string;
        settlement_receipt_ref: string | null;
        reservation_ref: string;
        admission_cohort_ref: string | null;
        input_tokens: number | string;
        output_tokens: number | string;
        cached_input_tokens: number | string;
        audio_input_tokens: number | string;
        audio_output_tokens: number | string;
        accounting_escalation_ref: string | null;
        accounting_escalated_at: string | null;
        provider_accounting_uncertain_reason: string | null;
        worker_job_ref: string | null;
        provider_session_ref_digest: string | null;
        provider_configuration_digest: string | null;
        capability_profile: string | null;
        admission_digest: string | null;
        room_context_kind: "private" | "community" | null;
        community_ref: string | null;
        channel_ref: string | null;
        membership_revision: string | null;
        sarah_participant_ref: string | null;
        provider_accounting_status: "pending" | "exact" | "uncertain" | null;
        worker_closed_at: string | null;
        provider_admitted_at: string | null;
        waiver_receipt_ref: string | null;
        waiver_payload_digest: string | null;
        provider_evidence_refs_json: unknown | null;
        waiver_provider_accounting_status: "uncertain" | null;
        waiver_authority: "owner_waived_unmetered_v1" | null;
        capture_generation: number | string | null;
        start_ledger_state_digest: string | null;
        end_ledger_state_digest: string | null;
        start_balance_state_digest: string | null;
        end_balance_state_digest: string | null;
        ledger_mutation_count: number | string | null;
        capture_receipt_ref: string | null;
        capture_digest: string | null;
        spendable_remaining_credit_msat: number | string | null;
      }>;
      const row = first(rows);
      if (row === undefined) return undefined;
      if (row.state === "accounting_uncertain") {
        let failureEvidence: SarahVoiceLiveKitFailureEvidence | undefined;
        if (
          row.sarah_participant_ref === "principal.sarah" &&
          row.worker_job_ref !== null &&
          row.provider_session_ref_digest !== null &&
          row.provider_configuration_digest !== null &&
          row.provider_accounting_status === "uncertain" &&
          row.provider_admitted_at !== null
        ) {
          const usageRows = (await sql`
            SELECT provider_response_ref, usage_kind, provider_status,
              input_tokens, output_tokens, cached_input_tokens,
              audio_input_tokens, audio_output_tokens
            FROM sarah_realtime_voice_usage
            WHERE session_ref = ${row.session_ref}
            ORDER BY usage_kind, provider_response_ref
          `) as ReadonlyArray<{
            provider_response_ref: string;
            usage_kind: "response" | "transcription";
            provider_status: "completed" | "cancelled" | "failed" | "incomplete" | null;
            input_tokens: number | string;
            output_tokens: number | string;
            cached_input_tokens: number | string;
            audio_input_tokens: number | string;
            audio_output_tokens: number | string;
          }>;
          const workerJobRows = (await sql`
            SELECT COUNT(DISTINCT worker_job_ref) AS worker_job_count
            FROM sarah_livekit_worker_events
            WHERE session_ref = ${row.session_ref}
          `) as ReadonlyArray<{ worker_job_count: number | string }>;
          failureEvidence = {
            principal: "principal.sarah",
            generation: toSafeInteger(row.generation, "generation"),
            identityDigests: {
              job: acceptanceDigest(row.worker_job_ref),
              providerSession: row.provider_session_ref_digest,
              providerConfiguration: row.provider_configuration_digest,
              hold: acceptanceDigest(row.reservation_ref),
              usage: acceptanceDigest(
                JSON.stringify(
                  usageRows.map((usage) => [usage.usage_kind, usage.provider_response_ref]),
                ),
              ),
            },
            usage: {
              inputTokens: toSafeInteger(row.input_tokens, "input_tokens"),
              outputTokens: toSafeInteger(row.output_tokens, "output_tokens"),
              cachedInputTokens: toSafeInteger(row.cached_input_tokens, "cached_input_tokens"),
              audioInputTokens: toSafeInteger(row.audio_input_tokens, "audio_input_tokens"),
              audioOutputTokens: toSafeInteger(row.audio_output_tokens, "audio_output_tokens"),
              recordedChargeMsat: toSafeInteger(row.charged_msat, "charged_msat"),
              responseCount: usageRows.filter((usage) => usage.usage_kind === "response").length,
              transcriptionCount: usageRows.filter((usage) => usage.usage_kind === "transcription")
                .length,
              cancelledResponseCount: usageRows.filter(
                (usage) => usage.usage_kind === "response" && usage.provider_status === "cancelled",
              ).length,
            },
            providerAccountingStatus: "uncertain",
            workerJobCount: toSafeInteger(
              first(workerJobRows)?.worker_job_count ?? 0,
              "worker_job_count",
            ),
            providerSessionCount: 1,
            providerAdmittedAt: row.provider_admitted_at,
            workerClosedAt: row.worker_closed_at,
            holdPreserved: toSafeInteger(row.reserved_msat, "reserved_msat") > 0,
            noHoldCreated: toSafeInteger(row.reserved_msat, "reserved_msat") === 0,
          };
        }
        const unmeteredAuthorityCapture =
          row.credit_mode === "owner_waived_unmetered" &&
          row.capture_generation !== null &&
          row.start_ledger_state_digest !== null &&
          row.end_ledger_state_digest !== null &&
          row.start_balance_state_digest !== null &&
          row.end_balance_state_digest !== null &&
          row.ledger_mutation_count !== null &&
          row.capture_receipt_ref !== null &&
          row.capture_digest !== null
            ? {
                schema: "openagents.sarah.unmetered-authority-capture.v1" as const,
                authority: "owner_waived_unmetered_v1" as const,
                generation: toSafeInteger(row.capture_generation, "capture_generation"),
                sessionRefDigest: acceptanceDigest(row.session_ref),
                startLedgerStateDigest: row.start_ledger_state_digest,
                endLedgerStateDigest: row.end_ledger_state_digest,
                startBalanceStateDigest: row.start_balance_state_digest,
                endBalanceStateDigest: row.end_balance_state_digest,
                ledgerMutationCount: toSafeInteger(
                  row.ledger_mutation_count,
                  "ledger_mutation_count",
                ),
                captureReceiptRef: row.capture_receipt_ref,
                captureDigest: row.capture_digest,
              }
            : undefined;
        return {
          sessionRef: row.session_ref,
          state: row.state,
          creditMode: row.credit_mode,
          recordedChargeMsat: toSafeInteger(row.charged_msat, "charged_msat"),
          reservedMsat: toSafeInteger(row.reserved_msat, "reserved_msat"),
          holdPreserved: toSafeInteger(row.reserved_msat, "reserved_msat") > 0,
          noHoldCreated: toSafeInteger(row.reserved_msat, "reserved_msat") === 0,
          reason: row.provider_accounting_uncertain_reason ?? "provider_accounting_uncertain",
          ...(row.accounting_escalation_ref === null || row.accounting_escalated_at === null
            ? {}
            : {
                accountingEscalation: {
                  escalationRef: row.accounting_escalation_ref,
                  escalatedAt: row.accounting_escalated_at,
                },
              }),
          ...(failureEvidence === undefined ? {} : { failureEvidence }),
          ...(unmeteredAuthorityCapture === undefined
            ? {}
            : { unmeteredAuthorityCapture }),
        };
      }
      if (row.settlement_receipt_ref === null) {
        throw new SarahVoiceStorageError("Terminal settlement receipt is missing", null);
      }
      let acceptanceEvidence: SarahVoiceLiveKitAcceptanceEvidence | undefined;
      if (
        row.sarah_participant_ref === "principal.sarah" &&
        row.admission_cohort_ref === "sarah_voice_cohort:alpha_v1" &&
        row.worker_job_ref !== null &&
        row.provider_session_ref_digest !== null &&
        row.provider_configuration_digest !== null &&
        row.capability_profile !== null &&
        row.admission_digest !== null &&
        row.room_context_kind !== null &&
        row.provider_accounting_status === "exact" &&
        row.worker_closed_at !== null &&
        row.provider_admitted_at !== null
      ) {
        const usageRows = (await sql`
          SELECT provider_response_ref, usage_kind, provider_status,
            input_tokens, output_tokens, cached_input_tokens,
            audio_input_tokens, audio_output_tokens, charge_msat
          FROM sarah_realtime_voice_usage
          WHERE session_ref = ${row.session_ref}
          ORDER BY usage_kind, provider_response_ref
        `) as ReadonlyArray<{
          provider_response_ref: string;
          usage_kind: "response" | "transcription";
          provider_status: "completed" | "cancelled" | "failed" | "incomplete" | null;
          input_tokens: number | string;
          output_tokens: number | string;
          cached_input_tokens: number | string;
          audio_input_tokens: number | string;
          audio_output_tokens: number | string;
          charge_msat: number | string;
        }>;
        const workerJobRows = (await sql`
          SELECT COUNT(DISTINCT worker_job_ref) AS worker_job_count
          FROM sarah_livekit_worker_events
          WHERE session_ref = ${row.session_ref}
        `) as ReadonlyArray<{ worker_job_count: number | string }>;
        const contextAuthority = JSON.stringify([
          row.room_context_kind,
          row.community_ref,
          row.channel_ref,
          row.membership_revision,
          row.admission_digest,
        ]);
        acceptanceEvidence = {
          principal: "principal.sarah",
          identityDigests: {
            job: acceptanceDigest(row.worker_job_ref),
            providerSession: row.provider_session_ref_digest,
            providerConfiguration: row.provider_configuration_digest,
            context: acceptanceDigest(contextAuthority),
            capability: acceptanceDigest(row.capability_profile),
            hold: acceptanceDigest(row.reservation_ref),
            usage: acceptanceDigest(
              JSON.stringify(
                usageRows.map((usage) => [usage.usage_kind, usage.provider_response_ref]),
              ),
            ),
            settlement: acceptanceDigest(row.settlement_receipt_ref),
          },
          usage: {
            inputTokens: toSafeInteger(row.input_tokens, "input_tokens"),
            outputTokens: toSafeInteger(row.output_tokens, "output_tokens"),
            cachedInputTokens: toSafeInteger(row.cached_input_tokens, "cached_input_tokens"),
            audioInputTokens: toSafeInteger(row.audio_input_tokens, "audio_input_tokens"),
            audioOutputTokens: toSafeInteger(row.audio_output_tokens, "audio_output_tokens"),
            chargeMsat: toSafeInteger(row.charged_msat, "charged_msat"),
            responseCount: usageRows.filter((usage) => usage.usage_kind === "response").length,
            transcriptionCount: usageRows.filter((usage) => usage.usage_kind === "transcription")
              .length,
            cancelledResponseCount: usageRows.filter(
              (usage) => usage.usage_kind === "response" && usage.provider_status === "cancelled",
            ).length,
          },
          providerAccountingStatus: "exact",
          workerJobCount: toSafeInteger(
            first(workerJobRows)?.worker_job_count ?? 0,
            "worker_job_count",
          ),
          providerSessionCount: 1,
          workerClosedAt: row.worker_closed_at,
          providerAdmittedAt: row.provider_admitted_at,
        };
      }
      let accountingWaiver:
        | NonNullable<
            Extract<
              SarahVoiceSettlementProjection,
              Readonly<{ state: "settled" | "released" }>
            >["accountingWaiver"]
          >
        | undefined;
      if (
        row.waiver_authority === "owner_waived_unmetered_v1" &&
        row.waiver_provider_accounting_status === "uncertain" &&
        row.waiver_receipt_ref !== null &&
        row.waiver_payload_digest !== null &&
        Array.isArray(row.provider_evidence_refs_json)
      ) {
        accountingWaiver = {
          authority: row.waiver_authority,
          providerAccountingStatus: row.waiver_provider_accounting_status,
          waiverReceiptRef: row.waiver_receipt_ref,
          waiverPayloadDigest: row.waiver_payload_digest,
          providerEvidenceDigest: acceptanceDigest(JSON.stringify(row.provider_evidence_refs_json)),
        };
      }
      let unmeteredAuthorityCapture: SarahVoiceUnmeteredAuthorityCapture | undefined;
      if (
        row.credit_mode === "owner_waived_unmetered" &&
        row.capture_generation !== null &&
        row.start_ledger_state_digest !== null &&
        row.end_ledger_state_digest !== null &&
        row.start_balance_state_digest !== null &&
        row.end_balance_state_digest !== null &&
        row.ledger_mutation_count !== null &&
        row.capture_receipt_ref !== null &&
        row.capture_digest !== null
      ) {
        unmeteredAuthorityCapture = {
          schema: "openagents.sarah.unmetered-authority-capture.v1",
          authority: "owner_waived_unmetered_v1",
          generation: toSafeInteger(row.capture_generation, "capture_generation"),
          sessionRefDigest: acceptanceDigest(row.session_ref),
          startLedgerStateDigest: row.start_ledger_state_digest,
          endLedgerStateDigest: row.end_ledger_state_digest,
          startBalanceStateDigest: row.start_balance_state_digest,
          endBalanceStateDigest: row.end_balance_state_digest,
          ledgerMutationCount: toSafeInteger(
            row.ledger_mutation_count,
            "ledger_mutation_count",
          ),
          captureReceiptRef: row.capture_receipt_ref,
          captureDigest: row.capture_digest,
        };
      }
      return {
        sessionRef: row.session_ref,
        state: row.state,
        creditMode: row.credit_mode,
        finalChargeMsat: toSafeInteger(row.charged_msat, "charged_msat"),
        spendableRemainingCreditMsat:
          row.spendable_remaining_credit_msat === null
            ? null
            : toSafeInteger(row.spendable_remaining_credit_msat, "spendable_remaining_credit_msat"),
        settlementReceiptRef: row.settlement_receipt_ref,
        ...(acceptanceEvidence === undefined ? {} : { acceptanceEvidence }),
        ...(accountingWaiver === undefined ? {} : { accountingWaiver }),
        ...(unmeteredAuthorityCapture === undefined
          ? {}
          : { unmeteredAuthorityCapture }),
      };
    } catch (error) {
      throw new SarahVoiceStorageError("Sarah voice settlement lookup failed", error);
    }
  };

  const revokeAlphaCohort = async (
    input: Readonly<{
      cohortRef: string;
      actorRef: string;
      reason: string;
      nowIso: string;
    }>,
  ): Promise<number> => {
    try {
      return await sql.begin(async (tx) => {
        await tx`
          SELECT session.session_ref
          FROM sarah_realtime_voice_sessions AS session
          WHERE session.admission_cohort_ref = ${input.cohortRef}
            AND session.state IN ('reserved', 'connected')
          ORDER BY session.session_ref
          FOR UPDATE OF session
        `;
        await tx`
          SELECT binding.session_ref, binding.generation
          FROM sarah_livekit_room_bindings AS binding
          INNER JOIN sarah_realtime_voice_sessions AS session
            ON session.session_ref = binding.session_ref
          WHERE session.admission_cohort_ref = ${input.cohortRef}
            AND session.state IN ('reserved', 'connected')
            AND binding.state IN ('prepared', 'active')
          ORDER BY binding.session_ref, binding.generation
          FOR UPDATE OF binding
        `;
        const rows = (await tx`
          UPDATE sarah_voice_alpha_memberships
          SET state = 'revoked',
              revoked_at = ${input.nowIso},
              revocation_actor_ref = ${input.actorRef},
              revocation_reason = ${input.reason},
              updated_at = ${input.nowIso}
          WHERE cohort_ref = ${input.cohortRef}
            AND state = 'active'
          RETURNING membership_ref
        `) as ReadonlyArray<{ membership_ref: string }>;
        for (const row of rows) {
          // Membership references are unique, so this event is idempotent across retries.
          // eslint-disable-next-line no-await-in-loop
          await tx`
            INSERT INTO sarah_voice_alpha_membership_audit (
              event_ref, membership_ref, cohort_ref, action, actor_ref,
              reason, source, occurred_at
            ) VALUES (
              ${`${row.membership_ref}:revoked`}, ${row.membership_ref},
              ${input.cohortRef}, 'revoked', ${input.actorRef}, ${input.reason},
              'operator_api', ${input.nowIso}
            )
            ON CONFLICT (event_ref) DO NOTHING
          `;
        }
        await tx`
          UPDATE sarah_livekit_room_bindings AS binding
          SET worker_stop_reason =
                COALESCE(binding.worker_stop_reason, 'membership_revoked'),
              worker_stop_close_reason = COALESCE(
                binding.worker_stop_close_reason,
                ${input.reason.slice(0, 256)}
              ),
              worker_stop_requested_at =
                COALESCE(binding.worker_stop_requested_at, ${input.nowIso}),
              worker_stop_deadline_at = COALESCE(
                binding.worker_stop_deadline_at,
                ${plusMillisecondsIso(input.nowIso, SARAH_LIVEKIT_WORKER_DRAIN_TIMEOUT_MS)}
              ),
              updated_at = ${input.nowIso}
          FROM sarah_realtime_voice_sessions AS session
          WHERE session.session_ref = binding.session_ref
            AND session.admission_cohort_ref = ${input.cohortRef}
            AND session.state IN ('reserved', 'connected')
            AND binding.state IN ('prepared', 'active')
            AND binding.worker_stop_reason IS NULL
        `;
        return rows.length;
      });
    } catch (error) {
      throw new SarahVoiceStorageError("Sarah voice alpha cohort revocation failed", error);
    }
  };

  const requestLiveKitProviderDisconnectFault = async (
    input: Readonly<{
      requestRef: string;
      sessionRef: string;
      generation: number;
      providerSessionRefDigest: string;
      operatorActorRef: string;
      nowIso: string;
    }>,
  ): Promise<SarahVoiceLiveKitProviderDisconnectFaultResult> => {
    try {
      if (
        input.requestRef.length < 1 ||
        input.requestRef.length > 256 ||
        input.sessionRef.length < 1 ||
        input.sessionRef.length > 256 ||
        !Number.isSafeInteger(input.generation) ||
        input.generation < 1 ||
        !/^[0-9a-f]{64}$/u.test(input.providerSessionRefDigest) ||
        input.operatorActorRef.length < 1 ||
        input.operatorActorRef.length > 256
      ) {
        throw new SarahVoiceSessionRejectedError(
          "The Sarah provider-disconnect acceptance request is invalid",
        );
      }
      return await sql.begin(async (tx) => {
        const priorRows = (await tx`
          SELECT request_ref, session_ref, generation,
            provider_session_ref_digest, operator_actor_ref, applied_at
          FROM sarah_livekit_provider_disconnect_faults
          WHERE request_ref = ${input.requestRef}
            OR (session_ref = ${input.sessionRef} AND generation = ${input.generation})
          FOR UPDATE
        `) as ReadonlyArray<{
          request_ref: string;
          session_ref: string;
          generation: number | string;
          provider_session_ref_digest: string;
          operator_actor_ref: string;
          applied_at: string | null;
        }>;
        const prior = first(priorRows);
        if (prior !== undefined) {
          if (
            priorRows.length !== 1 ||
            prior.request_ref !== input.requestRef ||
            prior.session_ref !== input.sessionRef ||
            toSafeInteger(prior.generation, "generation") !== input.generation ||
            prior.provider_session_ref_digest !== input.providerSessionRefDigest ||
            prior.operator_actor_ref !== input.operatorActorRef
          ) {
            throw new SarahVoiceSessionRejectedError(
              "The Sarah provider-disconnect acceptance request conflicts with prior authority",
            );
          }
          return {
            requestRef: prior.request_ref,
            sessionRef: prior.session_ref,
            generation: input.generation,
            providerSessionRefDigest: prior.provider_session_ref_digest,
            state: prior.applied_at === null ? "requested" : "applied",
            replayed: true,
          };
        }

        const targets = (await tx`
          SELECT session.session_ref, binding.provider_session_ref_digest
          FROM sarah_realtime_voice_sessions AS session
          INNER JOIN sarah_livekit_room_bindings AS binding
            ON binding.session_ref = session.session_ref
            AND binding.generation = session.generation
          WHERE session.session_ref = ${input.sessionRef}
            AND session.generation = ${input.generation}
            AND session.transport_kind = 'livekit_room_v1'
            AND session.admission_cohort_ref = ${SARAH_VOICE_ALPHA_COHORT_REF}
            AND session.state = 'connected'
            AND session.session_expires_at > ${input.nowIso}
            AND binding.state = 'active'
            AND binding.worker_job_ref IS NOT NULL
            AND binding.worker_closed_at IS NULL
            AND binding.worker_stop_reason IS NULL
            AND binding.provider_admitted_at IS NOT NULL
            AND binding.provider_accounting_status = 'pending'
            AND binding.provider_session_ref_digest = ${input.providerSessionRefDigest}
          FOR UPDATE OF session, binding
        `) as ReadonlyArray<{
          session_ref: string;
          provider_session_ref_digest: string;
        }>;
        if (targets.length !== 1) {
          throw new SarahVoiceSessionRejectedError(
            "The Sarah provider-disconnect target is not one active accepted generation",
          );
        }
        await tx`
          INSERT INTO sarah_livekit_provider_disconnect_faults (
            request_ref, session_ref, generation, provider_session_ref_digest,
            operator_actor_ref, requested_at
          ) VALUES (
            ${input.requestRef}, ${input.sessionRef}, ${input.generation},
            ${input.providerSessionRefDigest}, ${input.operatorActorRef}, ${input.nowIso}
          )
        `;
        return {
          requestRef: input.requestRef,
          sessionRef: input.sessionRef,
          generation: input.generation,
          providerSessionRefDigest: input.providerSessionRefDigest,
          state: "requested",
          replayed: false,
        };
      });
    } catch (error) {
      if (error instanceof SarahVoiceSessionRejectedError) throw error;
      throw new SarahVoiceStorageError(
        "Sarah provider-disconnect acceptance request failed",
        error,
      );
    }
  };

  type WorkerEventReceiptRow = Readonly<{
    worker_job_ref: string;
    worker_control_token_digest: string;
    event_kind: SarahVoiceLiveKitWorkerEvent["eventKind"];
    event_payload_digest: string;
    stop_reason: SarahVoiceLiveKitWorkerStopReason | null;
    observed_at: string;
  }>;

  const readWorkerInterruptSequence = async (
    tx: SyncTransactionSql,
    input: Readonly<{
      sessionRef: string;
      generation: number;
      eventKind: SarahVoiceLiveKitWorkerEvent["eventKind"];
    }>,
  ): Promise<number | undefined> => {
    if (input.eventKind !== "lease_check") return undefined;
    const rows = (await tx`
      SELECT worker_interrupt_sequence
      FROM sarah_livekit_room_bindings
      WHERE session_ref = ${input.sessionRef}
        AND generation = ${input.generation}
      FOR SHARE
    `) as ReadonlyArray<{ worker_interrupt_sequence: number | string }>;
    const row = first(rows);
    if (row === undefined) {
      throw new SarahVoiceSessionRejectedError("The Sarah LiveKit worker binding disappeared");
    }
    return toSafeInteger(row.worker_interrupt_sequence, "worker interrupt sequence");
  };

  const readWorkerProviderDisconnectFault = async (
    tx: SyncTransactionSql,
    input: Readonly<{
      sessionRef: string;
      generation: number;
      eventKind: SarahVoiceLiveKitWorkerEvent["eventKind"];
    }>,
  ): Promise<
    | Readonly<{
        requestRef: string;
        providerSessionRefDigest: string;
      }>
    | undefined
  > => {
    if (input.eventKind !== "lease_check") return undefined;
    const rows = (await tx`
      SELECT request_ref, provider_session_ref_digest
      FROM sarah_livekit_provider_disconnect_faults
      WHERE session_ref = ${input.sessionRef}
        AND generation = ${input.generation}
        AND applied_at IS NULL
      FOR SHARE
    `) as ReadonlyArray<{
      request_ref: string;
      provider_session_ref_digest: string;
    }>;
    if (rows.length > 1) {
      throw new SarahVoiceStorageError(
        "The Sarah provider-disconnect generation has overlapping directives",
        null,
      );
    }
    const row = first(rows);
    return row === undefined
      ? undefined
      : {
          requestRef: row.request_ref,
          providerSessionRefDigest: row.provider_session_ref_digest,
        };
  };

  const readWorkerEventReceipt = async (
    tx: SyncTransactionSql,
    input: SarahVoiceLiveKitWorkerEvent,
  ): Promise<SarahVoiceLiveKitWorkerEventResult | undefined> => {
    const rows = (await tx`
      SELECT worker_job_ref, worker_control_token_digest, event_kind,
        event_payload_digest, stop_reason, observed_at
      FROM sarah_livekit_worker_events
      WHERE session_ref = ${input.sessionRef}
        AND generation = ${input.generation}
        AND event_ref = ${input.eventRef}
      FOR SHARE
    `) as ReadonlyArray<WorkerEventReceiptRow>;
    const row = first(rows);
    if (row === undefined) return undefined;
    if (
      row.worker_job_ref !== input.workerJobRef ||
      row.worker_control_token_digest !== input.workerControlTokenDigest ||
      row.event_kind !== input.eventKind ||
      row.event_payload_digest !== input.eventPayloadDigest
    ) {
      throw new SarahVoiceSessionRejectedError(
        "The Sarah LiveKit worker event reference was replayed with changed facts",
      );
    }
    const interruptSequence = await readWorkerInterruptSequence(tx, input);
    const providerDisconnectFault = await readWorkerProviderDisconnectFault(tx, input);
    return {
      observedAt: row.observed_at,
      replayed: true,
      ...(interruptSequence === undefined ? {} : { interruptSequence }),
      ...(providerDisconnectFault === undefined ? {} : { providerDisconnectFault }),
      ...(row.stop_reason === null ? {} : { stopReason: row.stop_reason }),
    };
  };

  const insertWorkerEventReceipt = async (
    tx: SyncTransactionSql,
    input: SarahVoiceLiveKitWorkerEvent,
  ): Promise<SarahVoiceLiveKitWorkerEventResult> => {
    const inserted = (await tx`
      INSERT INTO sarah_livekit_worker_events (
        session_ref, generation, event_ref, worker_job_ref,
        worker_control_token_digest, event_kind, event_payload_digest,
        stop_reason, observed_at
      ) VALUES (
        ${input.sessionRef}, ${input.generation}, ${input.eventRef},
        ${input.workerJobRef}, ${input.workerControlTokenDigest},
        ${input.eventKind}, ${input.eventPayloadDigest}, NULL, ${input.nowIso}
      )
      ON CONFLICT (session_ref, generation, event_ref) DO NOTHING
      RETURNING observed_at
    `) as ReadonlyArray<{ observed_at: string }>;
    const row = first(inserted);
    if (row !== undefined) {
      const interruptSequence = await readWorkerInterruptSequence(tx, input);
      const providerDisconnectFault = await readWorkerProviderDisconnectFault(tx, input);
      return {
        observedAt: row.observed_at,
        replayed: false,
        ...(interruptSequence === undefined ? {} : { interruptSequence }),
        ...(providerDisconnectFault === undefined ? {} : { providerDisconnectFault }),
      };
    }
    const replay = await readWorkerEventReceipt(tx, input);
    if (replay === undefined) {
      throw new SarahVoiceStorageError("The Sarah LiveKit worker event receipt disappeared", null);
    }
    return replay;
  };

  const setWorkerEventStopReason = async (
    tx: SyncTransactionSql,
    input: SarahVoiceLiveKitWorkerEvent,
    stopReason: SarahVoiceLiveKitWorkerStopReason,
  ): Promise<void> => {
    await tx`
      UPDATE sarah_livekit_worker_events
      SET stop_reason = ${stopReason}
      WHERE session_ref = ${input.sessionRef}
        AND generation = ${input.generation}
        AND event_ref = ${input.eventRef}
        AND event_payload_digest = ${input.eventPayloadDigest}
    `;
  };

  const applyLiveKitWorkerEvent = async (
    input: SarahVoiceLiveKitWorkerEvent,
  ): Promise<SarahVoiceLiveKitWorkerEventResult> => {
    try {
      return await sql.begin(async (tx) => {
        const replay = await readWorkerEventReceipt(tx, input);
        if (replay !== undefined) return replay;

        const sessions = (await tx`
          SELECT session.state, session.session_expires_at,
            session.owner_user_id, session.credit_mode,
            session.entitlement_ref, session.admission_cohort_ref
          FROM sarah_realtime_voice_sessions AS session
          INNER JOIN sarah_livekit_room_bindings AS binding
            ON binding.session_ref = session.session_ref
          WHERE binding.worker_control_token_digest =
              ${input.workerControlTokenDigest}
            AND binding.worker_job_ref = ${input.workerJobRef}
            AND binding.session_ref = ${input.sessionRef}
            AND binding.generation = ${input.generation}
            AND (
              ${input.eventKind === "worker_connected" ? input.workerRoomSid : null}::text IS NULL
              OR binding.worker_room_sid =
                ${input.eventKind === "worker_connected" ? input.workerRoomSid : null}
            )
          FOR UPDATE OF session
        `) as ReadonlyArray<{
          state: SarahVoiceSessionState;
          session_expires_at: string;
          owner_user_id: string;
          credit_mode: SarahVoiceCreditMode;
          entitlement_ref: string | null;
          admission_cohort_ref: string | null;
        }>;
        const session = first(sessions);
        if (session === undefined) {
          throw new SarahVoiceSessionRejectedError(
            "The Sarah LiveKit worker event does not match its claimed generation",
          );
        }

        const bindings = (await tx`
          SELECT binding.room_ref, binding.sarah_participant_ref,
            binding.state AS binding_state, binding.join_expires_at,
            binding.sarah_joined_at, binding.worker_closed_at,
            binding.worker_stop_reason, binding.worker_stop_close_reason,
            binding.provider_session_ref_digest,
            binding.provider_configuration_digest,
            binding.provider_admitted_at, binding.provider_accounting_status,
            binding.worker_interrupt_sequence,
            binding.worker_interrupt_applied_sequence
          FROM sarah_livekit_room_bindings AS binding
          WHERE binding.worker_control_token_digest =
              ${input.workerControlTokenDigest}
            AND binding.worker_job_ref = ${input.workerJobRef}
            AND binding.session_ref = ${input.sessionRef}
            AND binding.generation = ${input.generation}
            AND (
              ${input.eventKind === "worker_connected" ? input.workerRoomSid : null}::text IS NULL
              OR binding.worker_room_sid =
                ${input.eventKind === "worker_connected" ? input.workerRoomSid : null}
            )
          FOR UPDATE
        `) as ReadonlyArray<{
          room_ref: string;
          sarah_participant_ref: string;
          binding_state: SarahVoiceLiveKitBindingState;
          join_expires_at: string;
          sarah_joined_at: string | null;
          worker_closed_at: string | null;
          worker_stop_reason: SarahVoiceLiveKitWorkerStopReason | null;
          worker_stop_close_reason: string | null;
          provider_session_ref_digest: string | null;
          provider_configuration_digest: string | null;
          provider_admitted_at: string | null;
          provider_accounting_status: "pending" | "exact" | "uncertain";
          worker_interrupt_sequence: number | string;
          worker_interrupt_applied_sequence: number | string;
        }>;
        const binding = first(bindings);
        if (binding === undefined) {
          throw new SarahVoiceSessionRejectedError(
            "The Sarah LiveKit worker event does not match its claimed generation",
          );
        }

        let workerStopReason = binding.worker_stop_reason;
        let workerStopCloseReason = binding.worker_stop_close_reason;
        if (input.eventKind !== "close" && workerStopReason === null) {
          const currentMembership =
            session.credit_mode !== "staging_owner_entitlement"
              ? ((await tx`
                  SELECT membership.membership_ref AS lease_ref
                  FROM sarah_voice_alpha_memberships AS membership
                  INNER JOIN users
                    ON users.id = membership.owner_user_id
                  WHERE membership.owner_user_id = ${session.owner_user_id}
                    AND membership.cohort_ref =
                      ${session.admission_cohort_ref}
                    AND membership.state = 'active'
                    AND membership.admitted_at <= ${input.nowIso}
                    AND users.status = 'active'
                    AND users.deleted_at IS NULL
                  FOR SHARE OF membership, users
                `) as ReadonlyArray<{ lease_ref: string }>)
              : ((await tx`
                  SELECT entitlement.entitlement_ref AS lease_ref
                  FROM sarah_voice_credit_entitlements AS entitlement
                  INNER JOIN users
                    ON users.id = entitlement.owner_user_id
                  WHERE entitlement.owner_user_id = ${session.owner_user_id}
                    AND entitlement.entitlement_ref =
                      ${session.entitlement_ref}
                    AND entitlement.environment = 'staging'
                    AND entitlement.state = 'active'
                    AND entitlement.activated_at <= ${input.nowIso}
                    AND entitlement.expires_at > ${input.nowIso}
                    AND users.status = 'active'
                    AND users.deleted_at IS NULL
                  FOR SHARE OF entitlement, users
                `) as ReadonlyArray<{ lease_ref: string }>);
          if (first(currentMembership) === undefined) {
            workerStopReason = "membership_revoked";
            workerStopCloseReason = "membership_revoked";
            await setLiveKitWorkerStopInTransaction(tx, {
              sessionRef: input.sessionRef,
              generation: input.generation,
              stopReason: workerStopReason,
              closeReason: workerStopCloseReason,
              nowIso: input.nowIso,
            });
          }
        }
        if (
          input.eventKind !== "close" &&
          workerStopReason === null &&
          input.eventKind === "lease_check" &&
          binding.sarah_joined_at === null &&
          binding.join_expires_at <= input.nowIso
        ) {
          workerStopReason = "session_expired";
          workerStopCloseReason = "provider_admission_expired";
          await setLiveKitWorkerStopInTransaction(tx, {
            sessionRef: input.sessionRef,
            generation: input.generation,
            stopReason: workerStopReason,
            closeReason: workerStopCloseReason,
            nowIso: input.nowIso,
          });
        }
        if (
          input.eventKind !== "close" &&
          workerStopReason === null &&
          session.state === "connected" &&
          session.session_expires_at <= input.nowIso
        ) {
          workerStopReason = "session_expired";
          workerStopCloseReason = "session_expired";
          await setLiveKitWorkerStopInTransaction(tx, {
            sessionRef: input.sessionRef,
            generation: input.generation,
            stopReason: workerStopReason,
            closeReason: workerStopCloseReason,
            nowIso: input.nowIso,
          });
        }

        if (input.eventKind === "close") {
          const receipt = await insertWorkerEventReceipt(tx, input);
          if (receipt.replayed) return receipt;
          if (binding.worker_closed_at !== null) {
            throw new SarahVoiceSessionRejectedError(
              "The Sarah LiveKit worker emitted a second terminal event",
            );
          }
          const closeReason = workerStopCloseReason ?? input.closeReason;
          await tx`
            UPDATE sarah_livekit_room_bindings
            SET worker_closed_at = ${receipt.observedAt},
                worker_close_reason = ${closeReason.slice(0, 256)},
                worker_last_seen_at = ${receipt.observedAt},
                provider_accounting_status = ${input.accountingStatus},
                provider_accounting_terminal_at = CASE
                  WHEN ${input.accountingStatus} = 'exact' THEN ${receipt.observedAt}
                  ELSE NULL
                END,
                provider_accounting_uncertain_at = CASE
                  WHEN ${input.accountingStatus} = 'uncertain' THEN ${receipt.observedAt}
                  ELSE NULL
                END,
                provider_accounting_uncertain_reason = CASE
                  WHEN ${input.accountingStatus} = 'uncertain'
                    THEN ${closeReason.slice(0, 256)}
                  ELSE NULL
                END,
                updated_at = ${receipt.observedAt}
            WHERE session_ref = ${input.sessionRef}
              AND generation = ${input.generation}
          `;
          if (input.accountingStatus === "uncertain") {
            await markLiveKitAccountingUncertainInTransaction(tx, {
              sessionRef: input.sessionRef,
              generation: input.generation,
              reason: closeReason,
              nowIso: receipt.observedAt,
            });
            return receipt;
          }
          await settleInTransaction(tx, {
            sessionRef: input.sessionRef,
            closeReason,
            nowIso: receipt.observedAt,
          });
          return receipt;
        }

        if (
          (input.eventKind === "worker_connected" ||
            input.eventKind === "provider_admitted" ||
            input.eventKind === "lease_check") &&
          workerStopReason !== null
        ) {
          const receipt = await insertWorkerEventReceipt(tx, input);
          if (receipt.replayed) return receipt;
          await setWorkerEventStopReason(tx, input, workerStopReason);
          return { ...receipt, stopReason: workerStopReason };
        }

        const bindingAdmitted =
          input.eventKind === "worker_connected" || input.eventKind === "provider_admitted"
            ? (binding.binding_state === "prepared" || binding.binding_state === "active") &&
              binding.sarah_joined_at === null &&
              binding.join_expires_at > input.nowIso
            : input.eventKind === "lease_check" &&
                binding.binding_state === "prepared" &&
                binding.sarah_joined_at === null
              ? binding.join_expires_at > input.nowIso && session.state === "reserved"
              : binding.binding_state === "active" &&
                binding.sarah_joined_at !== null &&
                session.state === "connected";
        const active =
          bindingAdmitted &&
          (session.state === "reserved" || session.state === "connected") &&
          (session.session_expires_at > input.nowIso || workerStopReason !== null) &&
          binding.worker_closed_at === null;
        if (!active) {
          const receipt = await insertWorkerEventReceipt(tx, input);
          if (receipt.replayed) return receipt;
          await setWorkerEventStopReason(tx, input, "membership_revoked");
          return {
            ...receipt,
            stopReason: "membership_revoked",
          };
        }

        const receipt = await insertWorkerEventReceipt(tx, input);
        if (receipt.replayed) return receipt;

        if (input.eventKind === "worker_connected") {
          if (binding.sarah_joined_at !== null) {
            throw new SarahVoiceSessionRejectedError(
              "The Sarah LiveKit worker connected event conflicts with the joined participant",
            );
          }
          await tx`
            UPDATE sarah_livekit_room_bindings
            SET worker_last_seen_at = ${receipt.observedAt},
                updated_at = ${receipt.observedAt}
            WHERE session_ref = ${input.sessionRef}
              AND generation = ${input.generation}
          `;
          return receipt;
        }

        if (input.eventKind === "provider_admitted") {
          if (
            binding.provider_admitted_at !== null ||
            binding.provider_session_ref_digest !== null ||
            binding.provider_configuration_digest !== null
          ) {
            throw new SarahVoiceSessionRejectedError(
              "The Sarah LiveKit provider emitted a second admission",
            );
          }
          const sessions = (await tx`
            UPDATE sarah_realtime_voice_sessions
            SET state = 'connected',
                connected_at = COALESCE(connected_at, ${receipt.observedAt}),
                updated_at = ${receipt.observedAt}
            WHERE session_ref = ${input.sessionRef}
              AND generation = ${input.generation}
              AND transport_kind = 'livekit_room_v1'
              AND state IN ('reserved', 'connected')
              AND session_expires_at > ${receipt.observedAt}
            RETURNING session_ref
          `) as ReadonlyArray<{ session_ref: string }>;
          if (first(sessions) === undefined) {
            throw new SarahVoiceSessionRejectedError(
              "The Sarah LiveKit worker cannot connect this voice generation",
            );
          }
          const joined = (await tx`
            UPDATE sarah_livekit_room_bindings
            SET sarah_joined_at = ${receipt.observedAt},
                provider_session_ref_digest = ${input.providerSessionRefDigest},
                provider_configuration_digest = ${input.providerConfigurationDigest},
                provider_admitted_at = ${receipt.observedAt},
                state = 'active',
                worker_last_seen_at = ${receipt.observedAt},
                updated_at = ${receipt.observedAt}
            WHERE session_ref = ${input.sessionRef}
              AND generation = ${input.generation}
              AND room_ref = ${binding.room_ref}
              AND sarah_participant_ref = ${binding.sarah_participant_ref}
              AND sarah_joined_at IS NULL
              AND state IN ('prepared', 'active')
              AND join_expires_at > ${receipt.observedAt}
            RETURNING session_ref
          `) as ReadonlyArray<{ session_ref: string }>;
          if (first(joined) === undefined) {
            throw new SarahVoiceSessionRejectedError(
              "The Sarah LiveKit worker participant cannot join this generation",
            );
          }
          return receipt;
        }

        if (input.eventKind === "interrupt_applied") {
          const requestedSequence = toSafeInteger(
            binding.worker_interrupt_sequence,
            "worker interrupt sequence",
          );
          const appliedSequence = toSafeInteger(
            binding.worker_interrupt_applied_sequence,
            "worker interrupt applied sequence",
          );
          if (
            input.interruptSequence <= appliedSequence ||
            input.interruptSequence > requestedSequence
          ) {
            throw new SarahVoiceSessionRejectedError(
              "The Sarah LiveKit worker interrupt receipt is out of sequence",
            );
          }
          await tx`
            UPDATE sarah_livekit_room_bindings
            SET worker_interrupt_applied_sequence = ${input.interruptSequence},
                worker_interrupt_applied_at = ${receipt.observedAt},
                worker_last_seen_at = ${receipt.observedAt},
                updated_at = ${receipt.observedAt}
            WHERE session_ref = ${input.sessionRef}
              AND generation = ${input.generation}
          `;
          return receipt;
        }

        if (input.eventKind === "provider_disconnect_fault_applied") {
          if (
            binding.provider_admitted_at === null ||
            binding.provider_session_ref_digest !== input.providerSessionRefDigest
          ) {
            throw new SarahVoiceSessionRejectedError(
              "The Sarah provider-disconnect receipt does not match provider authority",
            );
          }
          const applied = (await tx`
            UPDATE sarah_livekit_provider_disconnect_faults
            SET applied_at = ${receipt.observedAt},
                worker_job_ref = ${input.workerJobRef}
            WHERE request_ref = ${input.requestRef}
              AND session_ref = ${input.sessionRef}
              AND generation = ${input.generation}
              AND provider_session_ref_digest = ${input.providerSessionRefDigest}
              AND applied_at IS NULL
            RETURNING request_ref
          `) as ReadonlyArray<{ request_ref: string }>;
          if (first(applied) === undefined) {
            throw new SarahVoiceSessionRejectedError(
              "The Sarah provider-disconnect directive is absent or already applied",
            );
          }
          return receipt;
        }

        await tx`
          UPDATE sarah_livekit_room_bindings
          SET worker_last_seen_at = ${receipt.observedAt},
              updated_at = ${receipt.observedAt}
          WHERE session_ref = ${input.sessionRef}
            AND generation = ${input.generation}
        `;
        if (input.eventKind === "lease_check") {
          if (workerStopReason === null) return receipt;
          await setWorkerEventStopReason(tx, input, workerStopReason);
          return { ...receipt, stopReason: workerStopReason };
        }

        const usageResult = await recordUsageInTransaction(tx, {
          sessionRef: input.sessionRef,
          generation: input.generation,
          usage: {
            ...input.usage,
            observedAt: receipt.observedAt,
          },
        });
        if (workerStopReason !== null) {
          await setWorkerEventStopReason(tx, input, workerStopReason);
          return { ...receipt, stopReason: workerStopReason };
        }
        if (!usageResult.creditLimitReached) return receipt;
        await setLiveKitWorkerStopInTransaction(tx, {
          sessionRef: input.sessionRef,
          generation: input.generation,
          stopReason: "hold_exhausted",
          closeReason: "livekit_worker_hold_exhausted",
          nowIso: receipt.observedAt,
        });
        await setWorkerEventStopReason(tx, input, "hold_exhausted");
        return {
          ...receipt,
          stopReason: "hold_exhausted",
        };
      });
    } catch (error) {
      if (error instanceof SarahVoiceSessionRejectedError) throw error;
      throw new SarahVoiceStorageError("Sarah LiveKit worker event apply failed", error);
    }
  };

  const readLiveKitProviderAdmission = async (
    input: Readonly<{ sessionRef: string; generation: number }>,
  ): Promise<
    | Readonly<{ state: "waiting" | "closed" }>
    | Readonly<{
        state: "admitted";
        providerSessionRefDigest: string;
        providerConfigurationDigest: string;
        admittedAt: string;
      }>
  > => {
    try {
      const rows = (await sql`
        SELECT provider_session_ref_digest, provider_configuration_digest,
          provider_admitted_at, binding.state AS binding_state,
          binding.worker_closed_at, binding.worker_stop_reason,
          session.state AS session_state
        FROM sarah_livekit_room_bindings AS binding
        INNER JOIN sarah_realtime_voice_sessions AS session
          ON session.session_ref = binding.session_ref
        WHERE binding.session_ref = ${input.sessionRef}
          AND binding.generation = ${input.generation}
      `) as ReadonlyArray<{
        provider_session_ref_digest: string | null;
        provider_configuration_digest: string | null;
        provider_admitted_at: string | null;
        binding_state: SarahVoiceLiveKitBindingState;
        worker_closed_at: string | null;
        worker_stop_reason: SarahVoiceLiveKitWorkerStopReason | null;
        session_state: SarahVoiceSessionState;
      }>;
      const row = first(rows);
      if (
        row === undefined ||
        row.worker_closed_at !== null ||
        row.worker_stop_reason !== null ||
        ["settled", "released", "failed"].includes(row.session_state) ||
        ["cleanup_ready", "cleanup_failed", "cleanup_abandoned", "cleaned"].includes(
          row.binding_state,
        )
      ) {
        return { state: "closed" };
      }
      if (
        row.provider_session_ref_digest !== null &&
        row.provider_configuration_digest !== null &&
        row.provider_admitted_at !== null &&
        row.binding_state === "active" &&
        row.session_state === "connected"
      ) {
        return {
          state: "admitted",
          providerSessionRefDigest: row.provider_session_ref_digest,
          providerConfigurationDigest: row.provider_configuration_digest,
          admittedAt: row.provider_admitted_at,
        };
      }
      return { state: "waiting" };
    } catch (error) {
      throw new SarahVoiceStorageError("Sarah LiveKit provider admission read failed", error);
    }
  };

  type LiveKitToolProposalRow = Readonly<{
    proposal_ref: string;
    proposal_digest: string;
    worker_job_ref: string;
    worker_control_token_digest: string;
    worker_event_ref: string;
    provider_call_ref: string;
    command_payload_digest: string;
    command: unknown;
    confirmation_required: boolean;
    state: "proposed" | "declined" | "execute_sent" | "outcome";
    outcome_ref: string | null;
    outcome_ok: boolean | null;
    outcome_summary: string | null;
    expires_at: string;
  }>;

  const decodeStoredLiveKitEditorCommand = (value: unknown): SarahVoiceLiveKitEditorCommand => {
    try {
      const command = validateSarahEditorCommandTarget(decodeSarahEditorCommand(value));
      if (command._tag === "open_path") {
        throw new Error("editor_open_path_not_allowed");
      }
      return command;
    } catch (error) {
      throw new SarahVoiceStorageError("The Sarah LiveKit tool command is invalid", error);
    }
  };

  const toLiveKitToolProposal = (row: LiveKitToolProposalRow): SarahVoiceLiveKitToolProposal => {
    const expiresAtMs = Date.parse(row.expires_at);
    if (!Number.isSafeInteger(expiresAtMs) || expiresAtMs < 0) {
      throw new SarahVoiceStorageError(
        "The Sarah LiveKit tool proposal expiry is invalid",
        row.expires_at,
      );
    }
    return {
      proposalRef: row.proposal_ref,
      proposalDigest: row.proposal_digest,
      command: decodeStoredLiveKitEditorCommand(row.command),
      confirmationRequired: row.confirmation_required,
      expiresAtMs,
    };
  };

  const proposeLiveKitTool = async (
    input: Readonly<{
      workerControlTokenDigest: string;
      workerJobRef: string;
      sessionRef: string;
      generation: number;
      workerEventRef: string;
      providerCallRef: string;
      commandPayloadDigest: string;
      proposalRef: string;
      proposalDigest: string;
      command: SarahVoiceLiveKitEditorCommand;
      confirmationRequired: boolean;
      nowIso: string;
      expiresAt: string;
    }>,
  ): Promise<SarahVoiceLiveKitToolProposal> => {
    try {
      if (input.confirmationRequired !== sarahEditorCommandRequiresConfirmation(input.command)) {
        throw new SarahVoiceSessionRejectedError(
          "The Sarah LiveKit tool confirmation law is invalid",
        );
      }
      return await sql.begin(async (tx) => {
        const existingRows = (await tx`
          SELECT proposal_ref, proposal_digest, worker_job_ref,
            worker_control_token_digest, worker_event_ref, provider_call_ref,
            command_payload_digest, command, confirmation_required, state,
            outcome_ref, outcome_ok,
            outcome_summary, expires_at
          FROM sarah_livekit_tool_proposals
          WHERE session_ref = ${input.sessionRef}
            AND generation = ${input.generation}
            AND worker_event_ref = ${input.workerEventRef}
          FOR UPDATE
        `) as ReadonlyArray<LiveKitToolProposalRow>;
        const existing = first(existingRows);
        if (existing !== undefined) {
          if (
            existing.worker_job_ref !== input.workerJobRef ||
            existing.worker_control_token_digest !== input.workerControlTokenDigest ||
            existing.provider_call_ref !== input.providerCallRef ||
            existing.command_payload_digest !== input.commandPayloadDigest ||
            existing.confirmation_required !== input.confirmationRequired
          ) {
            throw new SarahVoiceSessionRejectedError(
              "The Sarah LiveKit tool event was replayed with changed facts",
            );
          }
          return toLiveKitToolProposal(existing);
        }

        const admitted = (await tx`
          SELECT session.session_expires_at
          FROM sarah_livekit_room_bindings AS binding
          INNER JOIN sarah_realtime_voice_sessions AS session
            ON session.session_ref = binding.session_ref
          WHERE binding.worker_control_token_digest =
              ${input.workerControlTokenDigest}
            AND binding.worker_job_ref = ${input.workerJobRef}
            AND binding.session_ref = ${input.sessionRef}
            AND binding.generation = ${input.generation}
            AND binding.room_context_kind = 'private'
            AND binding.capability_profile = 'omega_editor'
            AND binding.state = 'active'
            AND binding.worker_closed_at IS NULL
            AND binding.worker_stop_reason IS NULL
            AND session.state = 'connected'
            AND session.transport_kind = 'livekit_room_v1'
            AND session.client_profile = 'omega_editor'
            AND session.session_expires_at > ${input.nowIso}
            AND ${input.expiresAt} > ${input.nowIso}
          FOR UPDATE OF binding, session
        `) as ReadonlyArray<{ session_expires_at: string }>;
        if (first(admitted) === undefined) {
          throw new SarahVoiceSessionRejectedError(
            "The Sarah LiveKit generation has no private tool authority",
          );
        }

        const inserted = (await tx`
          INSERT INTO sarah_livekit_tool_proposals (
            session_ref, generation, proposal_ref, proposal_digest,
            worker_job_ref, worker_control_token_digest, worker_event_ref,
            provider_call_ref, command_payload_digest, command,
            confirmation_required, state,
            outcome_ref, outcome_ok, outcome_summary, created_at, expires_at,
            decision_at, outcome_at
          ) VALUES (
            ${input.sessionRef}, ${input.generation}, ${input.proposalRef},
            ${input.proposalDigest}, ${input.workerJobRef},
            ${input.workerControlTokenDigest}, ${input.workerEventRef},
            ${input.providerCallRef}, ${input.commandPayloadDigest},
            ${JSON.stringify(input.command)}::text::jsonb,
            ${input.confirmationRequired},
            ${input.confirmationRequired ? "proposed" : "execute_sent"},
            NULL, NULL, NULL, ${input.nowIso}, ${input.expiresAt},
            ${input.confirmationRequired ? null : input.nowIso}, NULL
          )
          RETURNING proposal_ref, proposal_digest, worker_job_ref,
            worker_control_token_digest, worker_event_ref, provider_call_ref,
            command_payload_digest, command, confirmation_required, state,
            outcome_ref, outcome_ok,
            outcome_summary, expires_at
        `) as ReadonlyArray<LiveKitToolProposalRow>;
        const row = first(inserted);
        if (row === undefined) {
          throw new SarahVoiceStorageError(
            "The Sarah LiveKit tool proposal did not return a row",
            null,
          );
        }
        return toLiveKitToolProposal(row);
      });
    } catch (error) {
      if (error instanceof SarahVoiceSessionRejectedError) throw error;
      if (error instanceof SarahVoiceStorageError) throw error;
      throw new SarahVoiceStorageError("Sarah LiveKit tool proposal failed", error);
    }
  };

  const readLiveKitToolProposals = async (
    input: Readonly<{
      sessionRef: string;
      generation: number;
      nowIso: string;
    }>,
  ): Promise<ReadonlyArray<SarahVoiceLiveKitToolProposal>> => {
    try {
      const rows = (await sql`
        SELECT proposal.proposal_ref, proposal.proposal_digest,
          proposal.worker_job_ref, proposal.worker_control_token_digest,
          proposal.worker_event_ref, proposal.provider_call_ref,
          proposal.command_payload_digest, proposal.command,
          proposal.confirmation_required, proposal.state,
          proposal.outcome_ref, proposal.outcome_ok, proposal.outcome_summary,
          proposal.expires_at
        FROM sarah_livekit_tool_proposals AS proposal
        INNER JOIN sarah_livekit_room_bindings AS binding
          ON binding.session_ref = proposal.session_ref
          AND binding.generation = proposal.generation
        INNER JOIN sarah_realtime_voice_sessions AS session
          ON session.session_ref = proposal.session_ref
        WHERE proposal.session_ref = ${input.sessionRef}
          AND proposal.generation = ${input.generation}
          AND proposal.state IN ('proposed', 'execute_sent')
          AND proposal.expires_at > ${input.nowIso}
          AND binding.room_context_kind = 'private'
          AND binding.capability_profile = 'omega_editor'
          AND binding.state = 'active'
          AND binding.worker_closed_at IS NULL
          AND binding.worker_stop_reason IS NULL
          AND session.state = 'connected'
          AND session.transport_kind = 'livekit_room_v1'
          AND session.session_expires_at > ${input.nowIso}
        ORDER BY proposal.created_at ASC
        LIMIT 16
      `) as ReadonlyArray<LiveKitToolProposalRow>;
      return rows.map(toLiveKitToolProposal);
    } catch (error) {
      if (error instanceof SarahVoiceStorageError) throw error;
      throw new SarahVoiceStorageError("Sarah LiveKit tool proposal read failed", error);
    }
  };

  const decideLiveKitTool = async (
    input: Readonly<{
      sessionRef: string;
      generation: number;
      proposalRef: string;
      proposalDigest: string;
      decision: "confirm" | "decline";
      nowIso: string;
    }>,
  ): Promise<SarahVoiceLiveKitToolProposal | undefined> => {
    try {
      return await sql.begin(async (tx) => {
        const rows = (await tx`
          SELECT proposal.proposal_ref, proposal.proposal_digest,
            proposal.worker_job_ref, proposal.worker_control_token_digest,
            proposal.worker_event_ref, proposal.provider_call_ref,
            proposal.command_payload_digest, proposal.command,
            proposal.confirmation_required, proposal.state,
            proposal.outcome_ref, proposal.outcome_ok,
            proposal.outcome_summary, proposal.expires_at
          FROM sarah_livekit_tool_proposals AS proposal
          INNER JOIN sarah_livekit_room_bindings AS binding
            ON binding.session_ref = proposal.session_ref
            AND binding.generation = proposal.generation
          INNER JOIN sarah_realtime_voice_sessions AS session
            ON session.session_ref = proposal.session_ref
          WHERE proposal.session_ref = ${input.sessionRef}
            AND proposal.generation = ${input.generation}
            AND proposal.proposal_ref = ${input.proposalRef}
            AND proposal.proposal_digest = ${input.proposalDigest}
            AND binding.room_context_kind = 'private'
            AND binding.capability_profile = 'omega_editor'
            AND binding.state = 'active'
            AND binding.worker_closed_at IS NULL
            AND binding.worker_stop_reason IS NULL
            AND session.state = 'connected'
            AND session.transport_kind = 'livekit_room_v1'
            AND session.session_expires_at > ${input.nowIso}
          FOR UPDATE OF proposal
        `) as ReadonlyArray<LiveKitToolProposalRow>;
        const row = first(rows);
        if (row === undefined || row.expires_at <= input.nowIso) {
          throw new SarahVoiceSessionRejectedError(
            "The Sarah LiveKit tool decision is invalid or expired",
          );
        }
        if (!row.confirmation_required) {
          throw new SarahVoiceSessionRejectedError(
            "The Sarah LiveKit tool does not accept a confirmation decision",
          );
        }
        if (
          (input.decision === "confirm" && row.state === "execute_sent") ||
          (input.decision === "decline" && row.state === "declined")
        ) {
          return input.decision === "confirm" ? toLiveKitToolProposal(row) : undefined;
        }
        if (row.state !== "proposed") {
          throw new SarahVoiceSessionRejectedError(
            "The Sarah LiveKit tool decision conflicts with prior state",
          );
        }
        await tx`
          UPDATE sarah_livekit_tool_proposals
          SET state = ${input.decision === "confirm" ? "execute_sent" : "declined"},
              decision_at = ${input.nowIso}
          WHERE session_ref = ${input.sessionRef}
            AND generation = ${input.generation}
            AND proposal_ref = ${input.proposalRef}
        `;
        return input.decision === "confirm" ? toLiveKitToolProposal(row) : undefined;
      });
    } catch (error) {
      if (error instanceof SarahVoiceSessionRejectedError) throw error;
      if (error instanceof SarahVoiceStorageError) throw error;
      throw new SarahVoiceStorageError("Sarah LiveKit tool decision failed", error);
    }
  };

  const recordLiveKitToolOutcome = async (
    input: Readonly<{
      sessionRef: string;
      generation: number;
      proposalRef: string;
      proposalDigest: string;
      outcomeRef: string;
      ok: boolean;
      summary: string;
      nowIso: string;
    }>,
  ): Promise<void> => {
    try {
      await sql.begin(async (tx) => {
        const rows = (await tx`
          SELECT proposal_ref, proposal_digest, worker_job_ref,
            worker_control_token_digest, worker_event_ref, provider_call_ref,
            command_payload_digest, command, confirmation_required, state,
            outcome_ref, outcome_ok,
            outcome_summary, expires_at
          FROM sarah_livekit_tool_proposals
          WHERE session_ref = ${input.sessionRef}
            AND generation = ${input.generation}
            AND proposal_ref = ${input.proposalRef}
            AND proposal_digest = ${input.proposalDigest}
          FOR UPDATE
        `) as ReadonlyArray<LiveKitToolProposalRow>;
        const row = first(rows);
        if (row === undefined) {
          throw new SarahVoiceSessionRejectedError(
            "The Sarah LiveKit tool outcome does not match a proposal",
          );
        }
        if (row.state === "outcome") {
          if (
            row.outcome_ref !== input.outcomeRef ||
            row.outcome_ok !== input.ok ||
            row.outcome_summary !== input.summary
          ) {
            throw new SarahVoiceSessionRejectedError(
              "The Sarah LiveKit tool outcome was replayed with changed facts",
            );
          }
          return;
        }
        if (row.state !== "execute_sent") {
          throw new SarahVoiceSessionRejectedError(
            "The Sarah LiveKit tool outcome arrived before approved execution",
          );
        }
        await tx`
          UPDATE sarah_livekit_tool_proposals
          SET state = 'outcome',
              outcome_ref = ${input.outcomeRef},
              outcome_ok = ${input.ok},
              outcome_summary = ${input.summary},
              outcome_at = ${input.nowIso}
          WHERE session_ref = ${input.sessionRef}
            AND generation = ${input.generation}
            AND proposal_ref = ${input.proposalRef}
        `;
      });
    } catch (error) {
      if (error instanceof SarahVoiceSessionRejectedError) throw error;
      throw new SarahVoiceStorageError("Sarah LiveKit tool outcome failed", error);
    }
  };

  const readLiveKitToolState = async (
    input: Readonly<{
      workerControlTokenDigest: string;
      workerJobRef: string;
      sessionRef: string;
      generation: number;
      proposalRef: string;
      proposalDigest: string;
      nowIso: string;
    }>,
  ): Promise<SarahVoiceLiveKitToolState> => {
    try {
      return await sql.begin(async (tx) => {
        await tx`
          UPDATE sarah_livekit_tool_proposals AS proposal
          SET state = 'declined',
              decision_at = COALESCE(decision_at, ${input.nowIso})
          FROM sarah_livekit_room_bindings AS binding,
            sarah_realtime_voice_sessions AS session
          WHERE proposal.session_ref = ${input.sessionRef}
            AND proposal.generation = ${input.generation}
            AND proposal.proposal_ref = ${input.proposalRef}
            AND proposal.proposal_digest = ${input.proposalDigest}
            AND proposal.worker_control_token_digest =
              ${input.workerControlTokenDigest}
            AND proposal.worker_job_ref = ${input.workerJobRef}
            AND proposal.state = 'proposed'
            AND binding.session_ref = proposal.session_ref
            AND binding.generation = proposal.generation
            AND session.session_ref = proposal.session_ref
            AND (
              proposal.expires_at <= ${input.nowIso}
              OR binding.state <> 'active'
              OR binding.worker_closed_at IS NOT NULL
              OR binding.worker_stop_reason IS NOT NULL
              OR session.state <> 'connected'
              OR session.session_expires_at <= ${input.nowIso}
            )
        `;
        const rows = (await tx`
          SELECT proposal_ref, proposal_digest, worker_job_ref,
            worker_control_token_digest, worker_event_ref, provider_call_ref,
            command_payload_digest, command, confirmation_required, state,
            outcome_ref, outcome_ok,
            outcome_summary, expires_at
          FROM sarah_livekit_tool_proposals
          WHERE session_ref = ${input.sessionRef}
            AND generation = ${input.generation}
            AND proposal_ref = ${input.proposalRef}
            AND proposal_digest = ${input.proposalDigest}
            AND worker_control_token_digest =
              ${input.workerControlTokenDigest}
            AND worker_job_ref = ${input.workerJobRef}
          FOR SHARE
        `) as ReadonlyArray<LiveKitToolProposalRow>;
        const row = first(rows);
        if (row === undefined) {
          throw new SarahVoiceSessionRejectedError(
            "The Sarah LiveKit tool state does not match its worker generation",
          );
        }
        if (row.state === "proposed") return { state: "waiting_decision" };
        if (row.state === "declined") return { state: "declined" };
        if (row.state === "execute_sent") return { state: "execute_sent" };
        if (row.outcome_ref === null || row.outcome_ok === null || row.outcome_summary === null) {
          throw new SarahVoiceStorageError("The Sarah LiveKit tool outcome row is incomplete", row);
        }
        return {
          state: "outcome",
          outcomeRef: row.outcome_ref,
          ok: row.outcome_ok,
          summary: row.outcome_summary,
        };
      });
    } catch (error) {
      if (error instanceof SarahVoiceSessionRejectedError) throw error;
      if (error instanceof SarahVoiceStorageError) throw error;
      throw new SarahVoiceStorageError("Sarah LiveKit tool state read failed", error);
    }
  };

  /**
   * Escalates overdue provider-accounting holds without settling them.
   *
   * The durable escalation removes only the per-owner voice-concurrency lock.
   * It does not change the session state, recorded charge, provider-accounting
   * status, or held credit. Exact provider evidence is still required by
   * `reconcileLiveKitAccounting` before any money moves.
   */
  const escalateStuckAccountingUncertainHolds = async (input: {
    readonly nowIso: string;
    readonly stuckAfterMs?: number | undefined;
    readonly batchSize?: number | undefined;
  }): Promise<SarahVoiceAccountingEscalationResult> => {
    const nowMs = Date.parse(input.nowIso);
    if (!Number.isFinite(nowMs)) {
      throw new SarahVoiceSessionRejectedError("An accounting escalation needs a valid instant");
    }
    const stuckAfterMs = input.stuckAfterMs ?? SARAH_VOICE_ACCOUNTING_UNCERTAIN_STUCK_MS;
    const batchSize = input.batchSize ?? SARAH_VOICE_ACCOUNTING_ESCALATION_BATCH_SIZE;
    if (!Number.isFinite(stuckAfterMs) || stuckAfterMs < 0) {
      throw new SarahVoiceSessionRejectedError(
        "An accounting escalation needs a non-negative bound",
      );
    }
    if (!Number.isSafeInteger(batchSize) || batchSize < 1 || batchSize > 1_000) {
      throw new SarahVoiceSessionRejectedError(
        "An accounting escalation needs a batch size from 1 through 1000",
      );
    }
    const stuckBeforeIso = new Date(nowMs - stuckAfterMs).toISOString();
    try {
      return await sql.begin(async (tx) => {
        const rows = (await tx`
          SELECT session.session_ref, session.generation, session.owner_user_id,
            COALESCE(binding.provider_accounting_uncertain_at, session.updated_at)
              AS accounting_uncertain_at
          FROM sarah_realtime_voice_sessions AS session
          LEFT JOIN sarah_livekit_room_bindings AS binding
            ON binding.session_ref = session.session_ref
            AND binding.generation = session.generation
          WHERE session.state = 'accounting_uncertain'
            AND session.credit_mode <> 'owner_waived_unmetered'
            AND session.accounting_escalated_at IS NULL
            AND COALESCE(binding.provider_accounting_uncertain_at, session.updated_at)
                <= ${stuckBeforeIso}
          ORDER BY accounting_uncertain_at, session.session_ref
          LIMIT ${batchSize}
          FOR UPDATE OF session SKIP LOCKED
        `) as ReadonlyArray<{
          session_ref: string;
          generation: number | string;
          owner_user_id: string;
          accounting_uncertain_at: string;
        }>;
        const owners = new Set<string>();
        let escalatedCount = 0;
        let oldestMs: number | undefined;
        for (const row of rows) {
          const generation = toSafeInteger(row.generation, "generation");
          const escalationRef = `sarah_voice_accounting_escalation:${acceptanceDigest(
            JSON.stringify([row.session_ref, generation, row.accounting_uncertain_at]),
          )}`;
          // Serialize one immutable escalation receipt onto the session row.
          // eslint-disable-next-line no-await-in-loop
          const escalated = (await tx`
            UPDATE sarah_realtime_voice_sessions
            SET accounting_escalated_at = ${input.nowIso},
                accounting_escalation_ref = ${escalationRef}
            WHERE session_ref = ${row.session_ref}
              AND generation = ${generation}
              AND state = 'accounting_uncertain'
              AND credit_mode <> 'owner_waived_unmetered'
              AND accounting_escalated_at IS NULL
            RETURNING owner_user_id
          `) as ReadonlyArray<{ owner_user_id: string }>;
          if (first(escalated) === undefined) continue;
          escalatedCount += 1;
          owners.add(row.owner_user_id);
          const uncertainMs = Date.parse(row.accounting_uncertain_at);
          if (Number.isFinite(uncertainMs)) {
            oldestMs = oldestMs === undefined ? uncertainMs : Math.min(oldestMs, uncertainMs);
          }
        }
        return {
          escalated: escalatedCount,
          owners: owners.size,
          oldestAgeMs: oldestMs === undefined ? 0 : Math.max(0, nowMs - oldestMs),
        };
      });
    } catch (error) {
      if (error instanceof SarahVoiceSessionRejectedError) throw error;
      throw new SarahVoiceStorageError("Sarah voice accounting escalation failed", error);
    }
  };

  /**
   * Counts `accounting_uncertain` holds that have outlived
   * `SARAH_VOICE_ACCOUNTING_UNCERTAIN_STUCK_MS`.
   *
   * Reads only. It exists because nothing anywhere reported this state: an
   * unreconciled hold occupies its owner's concurrency slot indefinitely, and
   * `sweepExpired` — which is what *opens* the state — has no branch that can
   * close it, so the per-minute sweep stays green while voice is denied. This
   * makes that condition visible without changing it.
   *
   * The result carries no session, owner, provider, room, or job identifier.
   * An operator who sees the alarm queries for the specific rows; an alarm that
   * ships identifiers into every log line to save that one query is not worth
   * the exposure.
   */
  const readStuckAccountingUncertainHolds = async (input: {
    readonly nowIso: string;
    readonly stuckAfterMs?: number | undefined;
  }): Promise<Readonly<{ stuck: number; owners: number; oldestAgeMs: number }>> => {
    const nowMs = Date.parse(input.nowIso);
    if (!Number.isFinite(nowMs)) {
      throw new SarahVoiceSessionRejectedError("A stuck-hold scan needs a valid instant");
    }
    const stuckAfterMs = input.stuckAfterMs ?? SARAH_VOICE_ACCOUNTING_UNCERTAIN_STUCK_MS;
    if (!Number.isFinite(stuckAfterMs) || stuckAfterMs < 0) {
      throw new SarahVoiceSessionRejectedError("A stuck-hold scan needs a non-negative bound");
    }
    const stuckBeforeIso = new Date(nowMs - stuckAfterMs).toISOString();
    const rows = (await sql`
      SELECT COUNT(*) AS stuck,
        COUNT(DISTINCT session.owner_user_id) AS owners,
        MIN(COALESCE(binding.provider_accounting_uncertain_at, session.updated_at))
          AS oldest_uncertain_at
      FROM sarah_realtime_voice_sessions AS session
      LEFT JOIN sarah_livekit_room_bindings AS binding
        ON binding.session_ref = session.session_ref
        AND binding.generation = session.generation
      WHERE session.state = 'accounting_uncertain'
        AND session.credit_mode <> 'owner_waived_unmetered'
        AND COALESCE(binding.provider_accounting_uncertain_at, session.updated_at)
            <= ${stuckBeforeIso}
    `) as ReadonlyArray<{
      stuck: number | string;
      owners: number | string;
      oldest_uncertain_at: string | null;
    }>;
    const row = first(rows);
    // `COALESCE(SUM(...),0)` and friends come back as strings from a bigint
    // aggregate. A `typeof === "number"` guard on one of these is exactly how
    // the free-tier spend ceiling read zero forever while looking healthy.
    const stuck = row === undefined ? 0 : Number(row.stuck);
    const owners = row === undefined ? 0 : Number(row.owners);
    const oldestMs =
      row?.oldest_uncertain_at === null || row?.oldest_uncertain_at === undefined
        ? undefined
        : Date.parse(row.oldest_uncertain_at);
    return {
      stuck: Number.isFinite(stuck) ? stuck : 0,
      owners: Number.isFinite(owners) ? owners : 0,
      oldestAgeMs:
        oldestMs === undefined || !Number.isFinite(oldestMs) ? 0 : Math.max(0, nowMs - oldestMs),
    };
  };

  const sweepExpired = async (nowIso: string): Promise<number> => {
    const workerHeartbeatExpiredBeforeIso = new Date(
      Date.parse(nowIso) - SARAH_LIVEKIT_WORKER_HEARTBEAT_TIMEOUT_MS,
    ).toISOString();
    await sql`
      UPDATE sarah_voice_admissions
      SET state = 'expired'
      WHERE state = 'active'
        AND expires_at <= ${nowIso}
    `;
    const rows = (await sql`
      SELECT session.session_ref, session.generation, session.state,
        session.transport_kind, binding.worker_stop_reason,
        binding.worker_stop_close_reason, binding.worker_stop_deadline_at,
        binding.worker_last_seen_at,
        binding.provider_admitted_at, binding.provider_accounting_status,
        session.credit_rate_msat_per_million_tokens
      FROM sarah_realtime_voice_sessions AS session
      LEFT JOIN sarah_livekit_room_bindings AS binding
        ON binding.session_ref = session.session_ref
      WHERE (
          session.state = 'reserved'
          AND session.ticket_expires_at <= ${nowIso}
        )
        OR (
          session.state = 'connected'
          AND (
            session.session_expires_at <= ${nowIso}
            OR binding.worker_stop_deadline_at <= ${nowIso}
            OR (
              session.transport_kind = 'livekit_room_v1'
              AND binding.worker_stop_reason IS NULL
              AND binding.worker_job_ref IS NOT NULL
              AND binding.worker_last_seen_at <= ${workerHeartbeatExpiredBeforeIso}
            )
          )
        )
      ORDER BY CASE
        WHEN session.state = 'reserved' THEN session.ticket_expires_at
        ELSE COALESCE(
          binding.worker_stop_deadline_at,
          session.session_expires_at
        )
      END
      LIMIT 100
    `) as ReadonlyArray<{
      session_ref: string;
      generation: number | string;
      state: "reserved" | "connected";
      transport_kind: SarahVoiceTransportKind;
      worker_stop_reason: SarahVoiceLiveKitWorkerStopReason | null;
      worker_stop_close_reason: string | null;
      worker_stop_deadline_at: string | null;
      worker_last_seen_at: string | null;
      provider_admitted_at: string | null;
      provider_accounting_status: "pending" | "exact" | "uncertain" | null;
      credit_rate_msat_per_million_tokens: number | string | null;
    }>;
    let processed = 0;
    const failures: unknown[] = [];
    for (const row of rows) {
      try {
        if (
          row.state === "connected" &&
          row.transport_kind === "livekit_room_v1" &&
          row.worker_stop_reason === null
        ) {
          const workerUnavailable =
            row.worker_last_seen_at !== null &&
            row.worker_last_seen_at <= workerHeartbeatExpiredBeforeIso;
          if (
            workerUnavailable &&
            row.provider_admitted_at !== null &&
            row.provider_accounting_status !== "exact"
          ) {
            // The expired heartbeat proves this worker cannot honor a graceful
            // stop request. Waiting through a drain deadline would only delay
            // the same uncertainty boundary while provider usage remains
            // unknowable.
            // eslint-disable-next-line no-await-in-loop
            await sql.begin((tx) =>
              markLiveKitAccountingUncertainInTransaction(tx, {
                sessionRef: row.session_ref,
                generation: toSafeInteger(row.generation, "generation"),
                reason: "livekit_worker_heartbeat_expired",
                nowIso,
                workerHeartbeatExpiredBeforeIso,
              }),
            );
          } else {
            // eslint-disable-next-line no-await-in-loop
            await sql.begin((tx) =>
              requestLiveKitWorkerStopInTransaction(tx, {
                sessionRef: row.session_ref,
                generation: toSafeInteger(row.generation, "generation"),
                stopReason: workerUnavailable ? "worker_unavailable" : "session_expired",
                closeReason: workerUnavailable
                  ? "livekit_worker_heartbeat_expired"
                  : "session_expired",
                nowIso,
              }),
            );
          }
          processed += 1;
          continue;
        }
        if (
          row.state === "connected" &&
          row.transport_kind === "livekit_room_v1" &&
          (row.worker_stop_deadline_at === null || row.worker_stop_deadline_at > nowIso)
        ) {
          continue;
        }
        if (
          row.state === "connected" &&
          row.transport_kind === "livekit_room_v1" &&
          row.provider_admitted_at !== null &&
          row.provider_accounting_status !== "exact"
        ) {
          // A dead worker cannot attest that every billable provider response
          // reached response.done. Keep the full hold until an operator
          // reconciles provider truth instead of settling a partial ledger.
          // eslint-disable-next-line no-await-in-loop
          await sql.begin((tx) =>
            markLiveKitAccountingUncertainInTransaction(tx, {
              sessionRef: row.session_ref,
              generation: toSafeInteger(row.generation, "generation"),
              reason: row.worker_stop_close_reason ?? "livekit_worker_accounting_unavailable",
              nowIso,
            }),
          );
          processed += 1;
          continue;
        }
        if (row.state === "connected" && row.credit_rate_msat_per_million_tokens === null) {
          // eslint-disable-next-line no-await-in-loop
          await sql`
            UPDATE sarah_realtime_voice_sessions
            SET state = 'accounting_uncertain',
                ticket_digest = NULL,
                close_reason = 'legacy_accounting_authority_unavailable',
                updated_at = ${nowIso}
            WHERE session_ref = ${row.session_ref}
              AND generation = ${row.generation}
              AND state = 'connected'
              AND credit_rate_msat_per_million_tokens IS NULL
          `;
          processed += 1;
          continue;
        }
        // Run one settlement at a time to protect the shared database pool.
        // eslint-disable-next-line no-await-in-loop
        await settle({
          sessionRef: row.session_ref,
          closeReason:
            row.worker_stop_close_reason ??
            (row.state === "reserved" ? "ticket_expired" : "session_expired"),
          nowIso,
        });
        processed += 1;
      } catch (error) {
        failures.push(error);
      }
    }
    if (failures.length > 0) {
      throw new AggregateError(
        failures,
        `Sarah voice expiry processed ${processed} row(s) and failed ${failures.length}`,
      );
    }
    return processed;
  };

  return {
    applyLiveKitWorkerEvent,
    authorizeLiveKitWorkerEvent,
    bindLiveKitRoom,
    claimLiveKitProvisioningIntent,
    claimLiveKitCleanups,
    claimLiveKitWorkerJob,
    closeLiveKitWorkerJob,
    completeLiveKitProvisioningIntent,
    connect,
    ensureStagingOwnerEntitlement,
    issueAdmission,
    markLiveKitCleanup,
    markLiveKitProvisioningIntent,
    prepareLiveKitProvisioningIntent,
    readActiveAlphaMembership,
    readActiveStagingOwnerEntitlement,
    readLiveKitCleanup,
    readLiveKitMembershipLease,
    readLiveKitOwnerParticipantAdmitted,
    readLiveKitProviderAdmission,
    readLiveKitToolProposals,
    readLiveKitToolState,
    readLiveKitWorkerReadiness,
    claimLiveKitProvisioningIntents,
    readSettlement,
    readStuckAccountingUncertainHolds,
    readSpendableCredit,
    recordLiveKitParticipantJoin,
    recordUsage,
    recordLiveKitToolOutcome,
    reconcileLiveKitAccounting,
    waiveLiveKitAccounting,
    reserve,
    revokeAlphaCohort,
    revokeLiveKitRoom,
    requestLiveKitWorkerInterrupt,
    requestLiveKitProviderDisconnectFault,
    readLiveKitWorkerInterruptApplied,
    decideLiveKitTool,
    escalateStuckAccountingUncertainHolds,
    proposeLiveKitTool,
    settle,
    settleLiveKitProvisioningIntent,
    sweepExpired,
  } as const;
};
