import type { SyncSql, SyncTransactionSql } from "./sql.js";

export type SarahVoiceSessionState = "reserved" | "connected" | "settled" | "released" | "failed";
export type SarahVoiceClientProfile =
  | "omega_editor"
  | "mobile_voice_only"
  | "mobile_command_center";
export type SarahVoiceCreditMode = "metered" | "staging_owner_entitlement";
export type SarahVoiceTransportKind = "custom_wss_v1" | "livekit_room_v1";

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
  providerResponseRef: string;
  inputTokens: number;
  outputTokens: number;
  cachedInputTokens: number;
  audioInputTokens: number;
  audioOutputTokens: number;
  chargeMsat: number;
  observedAt: string;
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

export type SarahVoiceLiveKitProvisioningIntent = Readonly<{
  sessionRef: string;
  generation: number;
  idempotencyKey: string;
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
              connected_at = ${input.nowIso},
              updated_at = ${input.nowIso}
          WHERE session_ref = ${input.sessionRef}
            AND ticket_digest = ${input.ticketDigest}
            AND state = 'reserved'
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
      roomContext: SarahVoiceLiveKitRoomContext;
      nowIso: string;
    }>,
  ): Promise<void> => {
    try {
      await sql.begin(async (tx) => {
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
        await tx`
          INSERT INTO sarah_livekit_provisioning_intents (
            session_ref, generation, idempotency_key, owner_user_id,
            device_ref, thread_ref, capability_profile, admission_ref,
            admission_digest, room_context_kind, community_ref, channel_ref,
            membership_revision, state, created_at, updated_at
          ) VALUES (
            ${input.sessionRef}, ${input.generation}, ${input.idempotencyKey},
            ${input.ownerUserId}, ${input.deviceRef}, ${input.threadRef},
            ${input.capabilityProfile}, ${input.admissionRef},
            ${input.admissionDigest}, ${input.roomContext.kind},
            ${input.roomContext.kind === "community" ? input.roomContext.communityRef : null},
            ${input.roomContext.kind === "community" ? input.roomContext.channelRef : null},
            ${input.roomContext.kind === "community" ? input.roomContext.membershipRevision : null},
            'pending', ${input.nowIso}, ${input.nowIso}
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
        error instanceof SarahVoiceSessionRejectedError
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
            state, created_at, updated_at
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
            ${input.subscribeAllowed}, 'prepared', ${input.nowIso},
            ${input.nowIso}
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

  const revokeLiveKitRoom = async (
    input: Readonly<{
      sessionRef: string;
      generation: number;
      reason: string;
      nowIso: string;
    }>,
  ): Promise<SarahVoiceSessionRecord> => {
    try {
      return await sql.begin(async (tx) => {
        const rows = (await tx`
          SELECT session_ref
          FROM sarah_livekit_room_bindings
          WHERE session_ref = ${input.sessionRef}
            AND generation = ${input.generation}
            AND state IN (
              'prepared',
              'active',
              'cleanup_ready',
              'cleanup_failed',
              'cleaned'
            )
          FOR UPDATE
        `) as ReadonlyArray<{ session_ref: string }>;
        if (first(rows) === undefined) {
          throw new SarahVoiceSessionRejectedError("The LiveKit room generation is not active");
        }
        return settleInTransaction(tx, {
          sessionRef: input.sessionRef,
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
      return await sql.begin(async (tx) => {
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
            charge_msat, observed_at
          ) VALUES (
            ${input.sessionRef}, ${input.usage.providerResponseRef},
            ${input.usage.inputTokens}, ${input.usage.outputTokens},
            ${input.usage.cachedInputTokens}, ${input.usage.audioInputTokens},
            ${input.usage.audioOutputTokens}, ${input.usage.chargeMsat},
            ${input.usage.observedAt}
          )
          ON CONFLICT (session_ref, provider_response_ref) DO NOTHING
          RETURNING session_ref
        `) as ReadonlyArray<{ session_ref: string }>;

        if (first(inserted) === undefined) {
          const replayed = (await tx`
            SELECT input_tokens, output_tokens, cached_input_tokens,
              audio_input_tokens, audio_output_tokens, charge_msat, observed_at
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
            replay.observed_at !== input.usage.observedAt
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
      });
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
        return rows.length;
      });
    } catch (error) {
      throw new SarahVoiceStorageError("Sarah voice alpha cohort revocation failed", error);
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
      SELECT session_ref, state
      FROM sarah_realtime_voice_sessions
      WHERE (state = 'reserved' AND ticket_expires_at <= ${nowIso})
         OR (state = 'connected' AND session_expires_at <= ${nowIso})
      ORDER BY CASE
        WHEN state = 'reserved' THEN ticket_expires_at
        ELSE session_expires_at
      END
      LIMIT 100
    `) as ReadonlyArray<{
      session_ref: string;
      state: "reserved" | "connected";
    }>;
    let settled = 0;
    for (const row of rows) {
      // Run one settlement at a time to protect the shared database pool.
      // eslint-disable-next-line no-await-in-loop
      await settle({
        sessionRef: row.session_ref,
        closeReason: row.state === "reserved" ? "ticket_expired" : "session_expired",
        nowIso,
      });
      settled += 1;
    }
    return settled;
  };

  return {
    bindLiveKitRoom,
    connect,
    ensureStagingOwnerEntitlement,
    issueAdmission,
    markLiveKitCleanup,
    markLiveKitProvisioningIntent,
    prepareLiveKitProvisioningIntent,
    readActiveAlphaMembership,
    readActiveStagingOwnerEntitlement,
    readLiveKitCleanup,
    claimLiveKitProvisioningIntents,
    readSettlement,
    readSpendableCredit,
    recordLiveKitParticipantJoin,
    recordUsage,
    reserve,
    revokeAlphaCohort,
    revokeLiveKitRoom,
    settle,
    settleLiveKitProvisioningIntent,
    sweepExpired,
  } as const;
};
