import type { SyncSql, SyncTransactionSql } from "./sql.js";

export type SarahVoiceSessionState = "reserved" | "connected" | "settled" | "released" | "failed";
export type SarahVoiceClientProfile = "omega_editor" | "mobile_voice_only";
export type SarahVoiceCreditMode = "metered" | "staging_owner_entitlement";

export type SarahVoiceCreditEntitlement = Readonly<{
  entitlementRef: string;
  ownerUserId: string;
  expiresAt: string;
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
  creditMode: SarahVoiceCreditMode;
  entitlementRef: string | null;
  state: SarahVoiceSessionState;
  reservedMsat: number;
  chargedMsat: number;
  ticketExpiresAt: string;
  sessionExpiresAt: string;
  settlementReceiptRef: string | null;
}>;

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

export class SarahVoiceInsufficientCreditError extends Error {
  override readonly name = "SarahVoiceInsufficientCreditError";
}

export class SarahVoiceConcurrentSessionError extends Error {
  override readonly name = "SarahVoiceConcurrentSessionError";
}

export class SarahVoiceSessionRejectedError extends Error {
  override readonly name = "SarahVoiceSessionRejectedError";
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
  credit_mode: SarahVoiceCreditMode;
  entitlement_ref: string | null;
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
  creditMode: row.credit_mode,
  entitlementRef: row.entitlement_ref,
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
      creditMode: SarahVoiceCreditMode;
      entitlementRef: string | null;
      reservedMsat: number;
      ticketExpiresAt: string;
      sessionExpiresAt: string;
      nowIso: string;
    }>,
  ): Promise<SarahVoiceSessionRecord> => {
    try {
      return await sql.begin(async (tx) => {
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

        if (input.creditMode === "metered") {
          const balances = (await tx`
            INSERT INTO agent_balances (
              actor_ref, balance_msat, held_msat, usd_credit_msat, created_at, updated_at
            ) VALUES (
              ${input.ownerActorRef},
              ${Math.max(1_000_000_000, input.reservedMsat)},
              ${input.reservedMsat},
              1_000_000_000,
              ${input.nowIso},
              ${input.nowIso}
            )
            ON CONFLICT (actor_ref) DO UPDATE
            SET balance_msat = GREATEST(
                  agent_balances.balance_msat,
                  agent_balances.held_msat + ${input.reservedMsat} + 1_000_000_000
                ),
                held_msat = agent_balances.held_msat + ${input.reservedMsat},
                updated_at = ${input.nowIso}
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

        const rows = (await tx`
          INSERT INTO sarah_realtime_voice_sessions (
            session_ref, reservation_ref, owner_user_id, owner_actor_ref,
            device_ref, thread_ref, generation, ticket_digest, disclosure_ref,
            client_profile, credit_mode, entitlement_ref, state, reserved_msat,
            charged_msat, ticket_expires_at,
            session_expires_at, created_at, updated_at
          ) VALUES (
            ${input.sessionRef}, ${input.reservationRef}, ${input.ownerUserId},
            ${input.ownerActorRef}, ${input.deviceRef}, ${input.threadRef},
            ${input.generation}, ${input.ticketDigest}, ${input.disclosureRef},
            ${input.clientProfile}, ${input.creditMode}, ${input.entitlementRef},
            'reserved', ${input.reservedMsat}, 0, ${input.ticketExpiresAt},
            ${input.sessionExpiresAt}, ${input.nowIso}, ${input.nowIso}
          )
          RETURNING session_ref, owner_user_id, owner_actor_ref, device_ref,
            thread_ref, generation, disclosure_ref, client_profile, credit_mode,
            entitlement_ref, state, reserved_msat,
            charged_msat, ticket_expires_at, session_expires_at,
            settlement_receipt_ref
        `) as ReadonlyArray<SessionRow>;
        const row = first(rows);
        if (row === undefined) {
          throw new SarahVoiceStorageError("The reservation did not return a row", null);
        }
        return toRecord(row);
      });
    } catch (error) {
      if (
        error instanceof SarahVoiceInsufficientCreditError ||
        error instanceof SarahVoiceSessionRejectedError
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
            thread_ref, generation, disclosure_ref, client_profile, credit_mode,
            entitlement_ref, state, reserved_msat,
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

  const recordUsage = async (
    input: Readonly<{
      sessionRef: string;
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

        if (first(inserted) !== undefined) {
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
        thread_ref, generation, disclosure_ref, client_profile, credit_mode,
        entitlement_ref, state, reserved_msat,
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
        thread_ref, generation, disclosure_ref, client_profile, credit_mode,
        entitlement_ref, state, reserved_msat,
        charged_msat, ticket_expires_at, session_expires_at,
        settlement_receipt_ref
    `) as ReadonlyArray<SessionRow>;
    const settled = first(settledRows);
    if (settled === undefined) {
      throw new SarahVoiceStorageError("The settlement did not return a row", null);
    }
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

  const sweepExpired = async (nowIso: string): Promise<number> => {
    const rows = (await sql`
      SELECT session_ref
      FROM sarah_realtime_voice_sessions
      WHERE state IN ('reserved', 'connected')
        AND session_expires_at <= ${nowIso}
      ORDER BY session_expires_at
      LIMIT 100
    `) as ReadonlyArray<{ session_ref: string }>;
    let settled = 0;
    for (const row of rows) {
      // Run one settlement at a time to protect the shared database pool.
      // eslint-disable-next-line no-await-in-loop
      await settle({
        sessionRef: row.session_ref,
        closeReason: "session_expired",
        nowIso,
      });
      settled += 1;
    }
    return settled;
  };

  return {
    connect,
    ensureStagingOwnerEntitlement,
    readActiveStagingOwnerEntitlement,
    recordUsage,
    reserve,
    settle,
    sweepExpired,
  } as const;
};
