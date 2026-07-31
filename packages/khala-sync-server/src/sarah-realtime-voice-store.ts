import {
  decodeSarahEditorCommand,
  sarahEditorCommandRequiresConfirmation,
  type SarahEditorCommand,
  validateSarahEditorCommandTarget,
} from "@openagentsinc/audio-contract";
import type { SyncSql, SyncTransactionSql } from "./sql.js";

export type SarahVoiceSessionState = "reserved" | "connected" | "settled" | "released" | "failed";
export type SarahVoiceClientProfile =
  | "omega_editor"
  | "mobile_voice_only"
  | "mobile_command_center";
export type SarahVoiceCreditMode = "metered" | "staging_owner_entitlement";
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

export type SarahVoiceSettlementProjection = Readonly<{
  sessionRef: string;
  state: "settled" | "released";
  creditMode: SarahVoiceCreditMode;
  finalChargeMsat: number;
  spendableRemainingCreditMsat: number | null;
  settlementReceiptRef: string;
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
  Readonly<{ admissionExpiresAt: string | undefined }>;

export type SarahVoiceUsage = Readonly<{
  usageKind?: "response" | "transcription";
  providerResponseRef: string;
  inputTokens: number;
  outputTokens: number;
  cachedInputTokens: number;
  audioInputTokens: number;
  audioOutputTokens: number;
  chargeMsat: number;
  observedAt: string;
}>;

export type SarahVoiceLiveKitWorkerStopReason =
  | "hold_exhausted"
  | "membership_revoked"
  | "operator_stop"
  | "session_expired";

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
        eventKind: "lease_check";
      }>)
  | (SarahVoiceLiveKitWorkerEventCommon &
      Readonly<{
        eventKind: "response_usage" | "transcription_usage";
        usage: Omit<SarahVoiceUsage, "observedAt">;
      }>)
  | (SarahVoiceLiveKitWorkerEventCommon &
      Readonly<{
        eventKind: "close";
        closeReason: string;
      }>);

export type SarahVoiceLiveKitWorkerEventResult = Readonly<{
  observedAt: string;
  replayed: boolean;
  stopReason?: SarahVoiceLiveKitWorkerStopReason;
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
  Readonly<{ cleanupAttemptedAt: string }>;

export type SarahVoiceLiveKitProvisioningIntent = Readonly<{
  sessionRef: string;
  generation: number;
  idempotencyKey: string;
}>;

export type SarahVoiceLiveKitWorkerClaim = Readonly<{
  sessionRef: string;
  generation: number;
  ownerUserId: string;
  capabilityProfile: SarahVoiceClientProfile;
  roomContext: SarahVoiceLiveKitRoomContext;
  sessionExpiresAt: string;
}>;

export type SarahVoiceLiveKitMembershipLease = Readonly<{
  ownerUserId: string;
  roomContext: SarahVoiceLiveKitRoomContext;
}>;

export const SARAH_LIVEKIT_WORKER_DRAIN_TIMEOUT_MS = 15_000;

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
      termsDigest: string;
      spendableRemainingCreditMsat: number | null;
      nowIso: string;
      expiresAt: string;
    }>,
  ): Promise<SarahVoiceAdmissionRecord> => {
    try {
      const rows = (await sql`
        INSERT INTO sarah_voice_admissions (
          admission_ref, owner_user_id, device_ref, thread_ref, session_ref,
          generation, disclosure_ref, client_profile, admission_cohort_ref,
          credit_mode, terms_digest, spendable_remaining_credit_msat, state,
          issued_at, expires_at, consumed_at
        ) VALUES (
          ${input.admissionRef}, ${input.ownerUserId}, ${input.deviceRef},
          ${input.threadRef}, ${input.sessionRef}, ${input.generation},
          ${input.disclosureRef}, ${input.clientProfile},
          ${input.admissionCohortRef}, ${input.creditMode},
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
      reservedMsat: number;
      ticketExpiresAt: string;
      sessionExpiresAt: string;
      nowIso: string;
      admissionBinding?: Readonly<{
        admissionRef: string;
        termsDigest: string;
        spendableRemainingCreditMsat: number | null;
      }>;
    }>,
  ): Promise<SarahVoiceReservationRecord> => {
    try {
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
          SELECT generation, state
          FROM sarah_realtime_voice_sessions
          WHERE owner_user_id = ${input.ownerUserId}
            AND thread_ref = ${input.threadRef}
          ORDER BY generation DESC, created_at DESC
          LIMIT 1
          FOR UPDATE
        `) as ReadonlyArray<{
          generation: number | string;
          state: SarahVoiceSessionState;
        }>;
        const priorSession = first(priorSessions);
        if (priorSession !== undefined) {
          const priorGeneration = toSafeInteger(priorSession.generation, "generation");
          if (priorSession.state !== "settled" && priorSession.state !== "released") {
            throw new SarahVoiceConcurrentSessionError(
              "The prior Sarah voice generation has not completed accounting",
            );
          }
          if (input.generation <= priorGeneration) {
            throw new SarahVoiceSessionRejectedError("The Sarah voice generation must advance");
          }
        }

        if (input.creditMode === "metered") {
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
          if (input.admissionBinding !== undefined) {
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
            admission_cohort_ref, charged_msat, ticket_expires_at,
            session_expires_at, created_at, updated_at
          ) VALUES (
            ${input.sessionRef}, ${input.reservationRef}, ${input.ownerUserId},
            ${input.ownerActorRef}, ${input.deviceRef}, ${input.threadRef},
            ${input.generation}, ${input.ticketDigest}, ${input.disclosureRef},
            ${input.clientProfile}, ${input.transportKind ?? "custom_wss_v1"},
            ${input.creditMode}, ${input.entitlementRef},
            'reserved', ${input.reservedMsat}, ${input.admissionCohortRef}, 0,
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
        return { ...toRecord(row), admissionExpiresAt };
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

  const markLiveKitProvisioningIntent = async (
    input: Readonly<{
      sessionRef: string;
      generation: number;
      state: "cleanup_failed" | "cleaned";
      nowIso: string;
    }>,
  ): Promise<void> => {
    try {
      await sql`
        UPDATE sarah_livekit_provisioning_intents
        SET state = ${input.state}, updated_at = ${input.nowIso}
        WHERE session_ref = ${input.sessionRef}
          AND generation = ${input.generation}
          AND state IN ('pending', 'reconciling', 'cleanup_failed', 'cleaned')
      `;
    } catch (error) {
      throw new SarahVoiceStorageError("Sarah LiveKit provisioning intent update failed", error);
    }
  };

  const claimLiveKitProvisioningIntents = async (
    input: Readonly<{
      staleBeforeIso: string;
      nowIso: string;
      limit?: number;
    }>,
  ): Promise<ReadonlyArray<SarahVoiceLiveKitProvisioningIntent>> => {
    try {
      const boundedLimit = Math.max(1, Math.min(100, Math.trunc(input.limit ?? 100)));
      const rows = await sql.begin(
        async (tx) =>
          (await tx`
          WITH candidates AS (
            SELECT session_ref
            FROM sarah_livekit_provisioning_intents
            WHERE state IN ('pending', 'reconciling', 'cleanup_failed')
              AND updated_at <= ${input.staleBeforeIso}
            ORDER BY created_at
            LIMIT ${boundedLimit}
            FOR UPDATE SKIP LOCKED
          )
          UPDATE sarah_livekit_provisioning_intents AS intent
          SET state = 'reconciling', updated_at = ${input.nowIso}
          FROM candidates
          WHERE intent.session_ref = candidates.session_ref
          RETURNING intent.session_ref, intent.generation,
            intent.idempotency_key
        `) as ReadonlyArray<{
            session_ref: string;
            generation: number | string;
            idempotency_key: string;
          }>,
      );
      return rows.map((row) => ({
        sessionRef: row.session_ref,
        generation: toSafeInteger(row.generation, "generation"),
        idempotencyKey: row.idempotency_key,
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
            AND state = 'pending'
            AND worker_control_token_digest = ${input.workerControlTokenDigest}
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

        await tx`
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
        `;
        await tx`
          UPDATE sarah_livekit_provisioning_intents
          SET state = 'bound', updated_at = ${input.nowIso}
          WHERE session_ref = ${input.sessionRef}
            AND generation = ${input.generation}
            AND state = 'pending'
        `;
      });
    } catch (error) {
      if (
        error instanceof SarahVoiceSessionRejectedError ||
        error instanceof SarahVoiceAdmissionRejectedError
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

  const recordLiveKitParticipantJoin = async (
    input: Readonly<{
      sessionRef: string;
      generation: number;
      roomRef: string;
      participantRef: string;
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
              WHERE session_ref = ${input.sessionRef}
                AND generation = ${input.generation}
                AND room_ref = ${input.roomRef}
                AND participant_ref = ${input.participantRef}
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
          WHERE session_ref = ${input.sessionRef}
            AND generation = ${input.generation}
            AND room_ref = ${input.roomRef}
            AND sarah_participant_ref = ${input.participantRef}
            AND sarah_joined_at IS NULL
            AND state IN ('prepared', 'active')
            AND join_expires_at > ${input.nowIso}
          RETURNING session_ref
        `) as ReadonlyArray<{ session_ref: string }>);
      if (first(rows) === undefined) {
        throw new SarahVoiceSessionRejectedError(
          "The LiveKit participant is unexpected, duplicated, revoked, or expired",
        );
      }
    } catch (error) {
      if (error instanceof SarahVoiceSessionRejectedError) throw error;
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
          sessionExpiresAt: row.session_expires_at,
        };
      });
    } catch (error) {
      if (error instanceof SarahVoiceSessionRejectedError) throw error;
      throw new SarahVoiceStorageError("Sarah LiveKit worker claim failed", error);
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
          AND binding.state IN ('prepared', 'active')
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
          binding.membership_revision
        FROM sarah_livekit_room_bindings AS binding
        WHERE binding.worker_control_token_digest =
            ${input.workerControlTokenDigest}
          AND binding.worker_job_ref = ${input.workerJobRef}
          AND binding.session_ref = ${input.sessionRef}
          AND binding.generation = ${input.generation}
          AND binding.state IN ('prepared', 'active')
      `) as ReadonlyArray<{
        owner_user_id: string;
        room_context_kind: "private" | "community";
        community_ref: string | null;
        channel_ref: string | null;
        membership_revision: string | null;
      }>;
      const row = first(rows);
      if (row === undefined) return undefined;
      if (row.room_context_kind === "private") {
        return {
          ownerUserId: row.owner_user_id,
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
      }>;
      const rows = await sql.begin(
        async (tx) =>
          (await tx`
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
                AND session.state IN ('settled', 'released')
                AND session.settlement_receipt_ref IS NOT NULL
              ORDER BY binding.updated_at, binding.session_ref
              LIMIT ${boundedLimit}
              FOR UPDATE OF binding SKIP LOCKED
            )
            UPDATE sarah_livekit_room_bindings AS binding
            SET cleanup_attempted_at = ${input.nowIso},
                updated_at = ${input.nowIso}
            FROM candidates
            WHERE binding.session_ref = candidates.session_ref
            RETURNING binding.session_ref, binding.generation,
              binding.room_ref, binding.room_epoch, binding.dispatch_ref,
              binding.sarah_presence_lease_ref, binding.cleanup_attempted_at
          `) as ReadonlyArray<CleanupClaimRow>,
      );
      return rows.map((row) => ({
        sessionRef: row.session_ref,
        generation: toSafeInteger(row.generation, "generation"),
        roomRef: row.room_ref,
        roomEpoch: toSafeInteger(row.room_epoch, "room_epoch"),
        dispatchRef: row.dispatch_ref,
        sarahPresenceLeaseRef: row.sarah_presence_lease_ref,
        cleanupAttemptedAt: row.cleanup_attempted_at,
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
  ): Promise<void> => {
    try {
      const rows = (await sql`
        UPDATE sarah_livekit_room_bindings AS binding
        SET state = ${input.state},
            cleanup_attempted_at = ${input.nowIso},
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
        RETURNING binding.session_ref
      `) as ReadonlyArray<{ session_ref: string }>;
      if (first(rows) === undefined) {
        throw new SarahVoiceSessionRejectedError(
          "LiveKit cleanup is not eligible before terminal accounting",
        );
      }
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
    if (session.state === "settled" || session.state === "released" || session.state === "failed") {
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

  const settleLiveKitProvisioningIntent = async (
    input: Readonly<{
      sessionRef: string;
      generation: number;
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
            AND state IN (
              'pending',
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
      usage: SarahVoiceUsage;
    }>,
  ): Promise<
    Readonly<{
      chargedMsat: number;
      reservedMsat: number;
      creditLimitReached: boolean;
    }>
  > => {
    const sessions = (await tx`
      SELECT generation, state
      FROM sarah_realtime_voice_sessions
      WHERE session_ref = ${input.sessionRef}
      FOR UPDATE
    `) as ReadonlyArray<{
      generation: number | string;
      state: SarahVoiceSessionState;
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

    const inserted = (await tx`
      INSERT INTO sarah_realtime_voice_usage (
        session_ref, provider_response_ref, input_tokens, output_tokens,
        cached_input_tokens, audio_input_tokens, audio_output_tokens,
        charge_msat, observed_at, usage_kind
      ) VALUES (
        ${input.sessionRef}, ${input.usage.providerResponseRef},
        ${input.usage.inputTokens}, ${input.usage.outputTokens},
        ${input.usage.cachedInputTokens}, ${input.usage.audioInputTokens},
        ${input.usage.audioOutputTokens}, ${input.usage.chargeMsat},
        ${input.usage.observedAt}, ${input.usage.usageKind ?? "response"}
      )
      ON CONFLICT (session_ref, provider_response_ref) DO NOTHING
      RETURNING session_ref
    `) as ReadonlyArray<{ session_ref: string }>;

    if (first(inserted) === undefined) {
      const replayed = (await tx`
        SELECT input_tokens, output_tokens, cached_input_tokens,
          audio_input_tokens, audio_output_tokens, charge_msat, observed_at,
          usage_kind
        FROM sarah_realtime_voice_usage
        WHERE session_ref = ${input.sessionRef}
          AND provider_response_ref = ${input.usage.providerResponseRef}
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
      }>;
      const replay = first(replayed);
      if (
        replay === undefined ||
        toSafeInteger(replay.input_tokens, "input_tokens") !== input.usage.inputTokens ||
        toSafeInteger(replay.output_tokens, "output_tokens") !== input.usage.outputTokens ||
        toSafeInteger(replay.cached_input_tokens, "cached_input_tokens") !==
          input.usage.cachedInputTokens ||
        toSafeInteger(replay.audio_input_tokens, "audio_input_tokens") !==
          input.usage.audioInputTokens ||
        toSafeInteger(replay.audio_output_tokens, "audio_output_tokens") !==
          input.usage.audioOutputTokens ||
        toSafeInteger(replay.charge_msat, "charge_msat") !== input.usage.chargeMsat ||
        replay.observed_at !== input.usage.observedAt ||
        replay.usage_kind !== (input.usage.usageKind ?? "response")
      ) {
        throw new SarahVoiceSessionRejectedError(
          "The provider response reference was replayed with changed usage",
        );
      }
    } else {
      await tx`
        UPDATE sarah_realtime_voice_sessions
        SET input_tokens = input_tokens + ${input.usage.inputTokens},
            output_tokens = output_tokens + ${input.usage.outputTokens},
            cached_input_tokens =
              cached_input_tokens + ${input.usage.cachedInputTokens},
            audio_input_tokens =
              audio_input_tokens + ${input.usage.audioInputTokens},
            audio_output_tokens =
              audio_output_tokens + ${input.usage.audioOutputTokens},
            charged_msat = CASE
              WHEN credit_mode = 'metered' THEN LEAST(
                reserved_msat,
                charged_msat + ${input.usage.chargeMsat}
              )
              ELSE charged_msat + ${input.usage.chargeMsat}
            END,
            updated_at = ${input.usage.observedAt}
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
      usage: SarahVoiceUsage;
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

  const settleInTransaction = async (
    tx: SyncTransactionSql,
    input: Readonly<{
      sessionRef: string;
      closeReason: string;
      nowIso: string;
    }>,
  ): Promise<SarahVoiceSessionRecord> => {
    const rows = (await tx`
      SELECT session_ref, owner_user_id, owner_actor_ref, device_ref,
        thread_ref, generation, disclosure_ref, client_profile,
        transport_kind, credit_mode,
        entitlement_ref, admission_cohort_ref, state, reserved_msat,
        charged_msat, ticket_expires_at, session_expires_at,
        settlement_receipt_ref
      FROM sarah_realtime_voice_sessions
      WHERE session_ref = ${input.sessionRef}
      FOR UPDATE
    `) as ReadonlyArray<SessionRow>;
    const row = first(rows);
    if (row === undefined) {
      throw new SarahVoiceSessionRejectedError("The voice session does not exist");
    }
    const current = toRecord(row);
    if (current.state === "settled" || current.state === "released" || current.state === "failed") {
      return current;
    }

    const receiptRef = `sarah_voice_settlement:${current.sessionRef}`;
    if (current.creditMode === "staging_owner_entitlement") {
      // The session and usage rows are the settlement evidence. The staging
      // entitlement does not write a payment or change the credit balance.
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
    await tx`
      UPDATE sarah_livekit_room_bindings
      SET state = 'cleanup_ready',
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

  const readSettlement = async (
    input: Readonly<{ sessionRef: string; ownerUserId: string }>,
  ): Promise<SarahVoiceSettlementProjection | undefined> => {
    try {
      const rows = (await sql`
        SELECT session.session_ref, session.state, session.credit_mode,
          session.charged_msat, session.settlement_receipt_ref,
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
        WHERE session.session_ref = ${input.sessionRef}
          AND session.owner_user_id = ${input.ownerUserId}
          AND session.state IN ('settled', 'released')
          AND session.settlement_receipt_ref IS NOT NULL
      `) as ReadonlyArray<{
        session_ref: string;
        state: "settled" | "released";
        credit_mode: SarahVoiceCreditMode;
        charged_msat: number | string;
        settlement_receipt_ref: string;
        spendable_remaining_credit_msat: number | string | null;
      }>;
      const row = first(rows);
      if (row === undefined) return undefined;
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

  type WorkerEventReceiptRow = Readonly<{
    worker_job_ref: string;
    worker_control_token_digest: string;
    event_kind: SarahVoiceLiveKitWorkerEvent["eventKind"];
    event_payload_digest: string;
    stop_reason: SarahVoiceLiveKitWorkerStopReason | null;
    observed_at: string;
  }>;

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
    return {
      observedAt: row.observed_at,
      replayed: true,
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
      return { observedAt: row.observed_at, replayed: false };
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
            binding.worker_stop_reason, binding.worker_stop_close_reason
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
          binding_state: "prepared" | "active" | "cleanup_ready" | "cleanup_failed" | "cleaned";
          join_expires_at: string;
          sarah_joined_at: string | null;
          worker_closed_at: string | null;
          worker_stop_reason: SarahVoiceLiveKitWorkerStopReason | null;
          worker_stop_close_reason: string | null;
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
            session.credit_mode === "metered"
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
                updated_at = ${receipt.observedAt}
            WHERE session_ref = ${input.sessionRef}
              AND generation = ${input.generation}
          `;
          await settleInTransaction(tx, {
            sessionRef: input.sessionRef,
            closeReason,
            nowIso: receipt.observedAt,
          });
          return receipt;
        }

        if (input.eventKind === "worker_connected" && workerStopReason !== null) {
          const receipt = await insertWorkerEventReceipt(tx, input);
          if (receipt.replayed) return receipt;
          await setWorkerEventStopReason(tx, input, workerStopReason);
          return { ...receipt, stopReason: workerStopReason };
        }

        const bindingAdmitted =
          input.eventKind === "worker_connected"
            ? (binding.binding_state === "prepared" || binding.binding_state === "active") &&
              binding.sarah_joined_at === null &&
              binding.join_expires_at > input.nowIso
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

  const sweepExpired = async (nowIso: string): Promise<number> => {
    await sql`
      UPDATE sarah_voice_admissions
      SET state = 'expired'
      WHERE state = 'active'
        AND expires_at <= ${nowIso}
    `;
    const rows = (await sql`
      SELECT session.session_ref, session.generation, session.state,
        session.transport_kind, binding.worker_stop_reason,
        binding.worker_stop_close_reason, binding.worker_stop_deadline_at
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
    }>;
    let processed = 0;
    for (const row of rows) {
      if (
        row.state === "connected" &&
        row.transport_kind === "livekit_room_v1" &&
        row.worker_stop_reason === null
      ) {
        // eslint-disable-next-line no-await-in-loop
        await sql.begin((tx) =>
          requestLiveKitWorkerStopInTransaction(tx, {
            sessionRef: row.session_ref,
            generation: toSafeInteger(row.generation, "generation"),
            stopReason: "session_expired",
            closeReason: "session_expired",
            nowIso,
          }),
        );
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
    }
    return processed;
  };

  return {
    applyLiveKitWorkerEvent,
    authorizeLiveKitWorkerEvent,
    bindLiveKitRoom,
    claimLiveKitCleanups,
    claimLiveKitWorkerJob,
    closeLiveKitWorkerJob,
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
    readLiveKitToolProposals,
    readLiveKitToolState,
    claimLiveKitProvisioningIntents,
    readSettlement,
    readSpendableCredit,
    recordLiveKitParticipantJoin,
    recordUsage,
    recordLiveKitToolOutcome,
    reserve,
    revokeAlphaCohort,
    revokeLiveKitRoom,
    decideLiveKitTool,
    proposeLiveKitTool,
    settle,
    settleLiveKitProvisioningIntent,
    sweepExpired,
  } as const;
};
