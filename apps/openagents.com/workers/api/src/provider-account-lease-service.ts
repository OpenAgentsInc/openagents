import type { IdentityAuthMirror } from "./identity-auth-domain-store";
import type { ProviderAccountProvider } from "./provider-account-domain";
import {
  type ProviderAccountFailoverFailureClass,
  classifyProviderAccountFailover,
} from "./provider-account-failover-policy";
import { PROVIDER_ACCOUNT_LEASE_POLICY_VERSION } from "./provider-account-lease-policy";
import { compactRandomId } from "./runtime-primitives";

export type ProviderAccountLease = Readonly<{
  leaseId: string;
  leaseRef: string;
  providerAccountId: string;
  providerAccountRef: string;
  accountLabel: string | null;
  requestedAction: string;
  runId: string | null;
  assignmentId: string | null;
  orderId: string | null;
  selectedByPolicyVersion: typeof PROVIDER_ACCOUNT_LEASE_POLICY_VERSION;
  selectionReason: string;
  selectedByActor: string;
  activeLeaseCountBeforeSelection: number;
  operatorPriority: number;
  startedAt: string;
  expiresAt: string;
  lastTouchedAt: string;
  status: "active";
}>;

export type ProviderAccountLeaseListItem = Readonly<{
  leaseRef: string;
  providerAccountRef: string;
  accountLabel: string | null;
  requestedAction: string;
  runId: string | null;
  assignmentId: string | null;
  orderId: string | null;
  startedAt: string;
  expiresAt: string;
  lastTouchedAt: string | null;
  status: string;
}>;

export type ProviderAccountLeaseSelectionExplanation = Readonly<{
  status: "selected" | "none";
  providerAccountRef: string | null;
  accountLabel: string | null;
  selectedByPolicyVersion: typeof PROVIDER_ACCOUNT_LEASE_POLICY_VERSION;
  selectionReason: string;
  activeLeaseCount: number | null;
  leaseLimit: number | null;
  operatorPriority: number | null;
}>;

export type ActiveProviderAccountLease = Readonly<{
  leaseRef: string;
  provider: ProviderAccountProvider;
  providerAccountId: string;
  providerAccountRef: string;
  requestedAction: string;
  runId: string | null;
  assignmentId: string | null;
  orderId: string | null;
  expiresAt: string;
  status: "active";
  userId: string;
}>;

export type ProviderAccountFailoverReceipt = Readonly<{
  receiptId: string;
  runId: string | null;
  assignmentId: string | null;
  orderId: string | null;
  requestedAction: string;
  previousLeaseRef: string | null;
  previousProviderAccountRef: string | null;
  nextLeaseRef: string | null;
  nextProviderAccountRef: string | null;
  failureClass: ProviderAccountFailoverFailureClass;
  accountStateAction: string;
  cooldownUntil: string | null;
  outcome: "retrying" | "blocked";
  attemptNumber: number;
  maxAttempts: number;
  customerSafeStatus: string;
  operatorSummary: string;
  customerSafeSummary: string | null;
  policyVersion: typeof PROVIDER_ACCOUNT_LEASE_POLICY_VERSION;
  createdAt: string;
}>;

export type ProviderAccountFailoverResult = Readonly<{
  action: ReturnType<typeof classifyProviderAccountFailover>;
  nextLease: ProviderAccountLease | null;
  outcome: "retrying" | "blocked";
  previousLease: ActiveProviderAccountLease;
  receipt: ProviderAccountFailoverReceipt;
}>;

export type ProviderAccountLeaseService = Readonly<{
  acquire: (
    input: Readonly<{
      userId: string;
      requiredProvider: ProviderAccountProvider | null;
      requestedAction: string;
      runId: string | null;
      assignmentId: string | null;
      orderId: string | null;
      now: string;
      expiresAt: string;
      selectedByActor: string;
      source: string;
    }>,
  ) => Promise<ProviderAccountLease | undefined>;
  explainSelection: (
    userId: string,
    now: string,
  ) => Promise<ProviderAccountLeaseSelectionExplanation>;
  failover: (
    input: Readonly<{
      userId: string;
      previousLeaseRef: string;
      failureClass: ProviderAccountFailoverFailureClass;
      requestedAction: string;
      attemptNumber: number;
      maxAttempts: number;
      now: string;
      expiresAt: string;
      runId: string | null;
      assignmentId: string | null;
      orderId: string | null;
      selectedByActor: string;
      source: string;
    }>,
  ) => Promise<ProviderAccountFailoverResult | undefined>;
  findActive: (
    input: Readonly<{ leaseRef: string; now: string; userId: string }>,
  ) => Promise<ActiveProviderAccountLease | undefined>;
  listActive: (userId: string, now: string) => Promise<ReadonlyArray<ProviderAccountLeaseListItem>>;
  listFailoverReceipts: (input: {
    userId: string;
    runId: string | null;
    assignmentId: string | null;
    orderId: string | null;
    limit: number;
  }) => Promise<ReadonlyArray<ProviderAccountFailoverReceipt>>;
  release: (input: {
    userId: string;
    leaseRef: string;
    now: string;
    status: "released" | "succeeded" | "failed";
    terminalOutcome: string;
    failureClass: string | null;
  }) => Promise<boolean>;
  touch: (input: {
    userId: string;
    leaseRef: string;
    now: string;
    expiresAt: string;
  }) => Promise<boolean>;
}>;

type LeaseInsertRow = Readonly<{
  id: string;
  lease_ref: string;
  provider_account_id: string;
  provider_account_ref: string;
  requested_action: string;
  run_id: string | null;
  assignment_id: string | null;
  order_id: string | null;
  selected_by_policy_version: typeof PROVIDER_ACCOUNT_LEASE_POLICY_VERSION;
  selection_reason: string;
  selected_by_actor: string;
  active_lease_count_before_selection: number;
  operator_priority: number;
  started_at: string;
  expires_at: string;
  last_touched_at: string;
  status: "active";
}>;

type ActiveLeaseRow = Readonly<{
  lease_ref: string;
  provider: ProviderAccountProvider;
  provider_account_id: string;
  provider_account_ref: string;
  requested_action: string;
  run_id: string | null;
  assignment_id: string | null;
  order_id: string | null;
  expires_at: string;
  status: "active";
  user_id: string;
}>;

const leaseSelectionReason = (activeLeaseCount: number, operatorPriority: number): string =>
  `Selected connected healthy account with ${activeLeaseCount} active lease(s), priority ${operatorPriority}, and no cooldown, reconnect marker, or low-credit flag.`;

const expireStaleLeases = async (db: D1Database, now: string): Promise<void> => {
  // The key-less expiry update is not mirrored on the hot path. The normal
  // restart backfill makes these rows converge to the identity mirror.
  await db
    .prepare(
      `UPDATE provider_account_leases
          SET status = 'expired',
              terminal_outcome = 'expired_before_release'
        WHERE status = 'active'
          AND expires_at <= ?`,
    )
    .bind(now)
    .run();
};

const mirrorLeaseRef = async (
  mirror: IdentityAuthMirror | undefined,
  leaseRef: string,
): Promise<void> => {
  if (mirror !== undefined) {
    await mirror.mirrorRowsWhere("provider_account_leases", ["lease_ref"], [leaseRef]);
  }
};

export const makeProviderAccountLeaseService = (dependencies: {
  db: D1Database;
  mirror?: IdentityAuthMirror | undefined;
}): ProviderAccountLeaseService => {
  const { db, mirror } = dependencies;

  const acquire: ProviderAccountLeaseService["acquire"] = async (input) => {
    await expireStaleLeases(db, input.now);

    const leaseId = compactRandomId("provider_account_lease");
    const leaseRef = compactRandomId("provider-account-lease_ref");
    const row = await db
      .prepare(
        `INSERT INTO provider_account_leases
          (id,
           lease_ref,
           provider_account_id,
           user_id,
           team_id,
           provider,
           provider_account_ref,
           requested_action,
           run_id,
           assignment_id,
           order_id,
           selected_by_policy_version,
           selection_reason,
           selected_by_actor,
           status,
           started_at,
           expires_at,
           last_touched_at,
           released_at,
           terminal_outcome,
           failure_class,
           metadata_json)
         SELECT
           ?,
           ?,
           pa.id,
           pa.user_id,
           pa.team_id,
           pa.provider,
           pa.provider_account_ref,
           ?,
           ?,
           ?,
           ?,
           ?,
           printf(
             'Selected connected healthy account with %d active lease(s), priority %d, and no cooldown, reconnect marker, or low-credit flag.',
             COUNT(active_leases.id),
             pa.operator_priority
           ),
           ?,
           'active',
           ?,
           ?,
           ?,
           NULL,
           NULL,
           NULL,
           json_object(
             'source', ?,
             'providerAccountRef', pa.provider_account_ref,
             'activeLeaseCountBeforeSelection', COUNT(active_leases.id),
             'operatorPriority', pa.operator_priority
           )
         FROM provider_accounts pa
         LEFT JOIN provider_account_leases active_leases
           ON active_leases.provider_account_id = pa.id
          AND active_leases.status = 'active'
          AND active_leases.expires_at > ?
         WHERE pa.user_id = ?
           AND (? IS NULL OR pa.provider = ?)
           AND pa.status = 'connected'
           AND pa.health = 'healthy'
           AND pa.secret_ref IS NOT NULL
           AND pa.deleted_at IS NULL
           AND COALESCE(pa.low_credit_flag, 0) = 0
           AND pa.reauth_required_reason IS NULL
           AND (pa.cooldown_until IS NULL OR pa.cooldown_until <= ?)
         GROUP BY pa.id
         HAVING COUNT(active_leases.id) < COALESCE(pa.lease_limit, 1)
         ORDER BY
           COUNT(active_leases.id) ASC,
           pa.operator_priority ASC,
           COALESCE(pa.last_selected_at, pa.connected_at, pa.created_at) ASC,
           pa.provider_account_ref ASC
         LIMIT 1
         RETURNING
           id,
           lease_ref,
           provider_account_id,
           provider_account_ref,
           requested_action,
           run_id,
           assignment_id,
           order_id,
           selected_by_policy_version,
           selection_reason,
           selected_by_actor,
           CAST(json_extract(metadata_json, '$.activeLeaseCountBeforeSelection') AS INTEGER)
             AS active_lease_count_before_selection,
           CAST(json_extract(metadata_json, '$.operatorPriority') AS INTEGER)
             AS operator_priority,
           started_at,
           expires_at,
           last_touched_at,
           status`,
      )
      .bind(
        leaseId,
        leaseRef,
        input.requestedAction,
        input.runId,
        input.assignmentId,
        input.orderId,
        PROVIDER_ACCOUNT_LEASE_POLICY_VERSION,
        input.selectedByActor,
        input.now,
        input.expiresAt,
        input.now,
        input.source,
        input.now,
        input.userId,
        input.requiredProvider ?? null,
        input.requiredProvider ?? null,
        input.now,
      )
      .first<LeaseInsertRow>();

    if (row === null) {
      return undefined;
    }

    await db
      .prepare(
        `UPDATE provider_accounts
            SET last_selected_at = ?,
                updated_at = ?
          WHERE id = ?
            AND user_id = ?`,
      )
      .bind(input.now, input.now, row.provider_account_id, input.userId)
      .run();

    if (mirror !== undefined) {
      await mirror.mirrorRowsByKey("provider_account_leases", [[row.id]]);
      await mirror.mirrorRowsByKey("provider_accounts", [[row.provider_account_id]]);
    }

    const accountLabelRow = await db
      .prepare(
        `SELECT COALESCE(operator_label, account_label) AS account_label
         FROM provider_accounts
         WHERE id = ?
           AND user_id = ?`,
      )
      .bind(row.provider_account_id, input.userId)
      .first<Readonly<{ account_label: string | null }>>();

    return {
      leaseId: row.id,
      leaseRef: row.lease_ref,
      providerAccountId: row.provider_account_id,
      providerAccountRef: row.provider_account_ref,
      accountLabel: accountLabelRow?.account_label ?? null,
      requestedAction: row.requested_action,
      runId: row.run_id,
      assignmentId: row.assignment_id,
      orderId: row.order_id,
      selectedByPolicyVersion: row.selected_by_policy_version,
      selectedByActor: row.selected_by_actor,
      selectionReason:
        row.selection_reason ??
        leaseSelectionReason(row.active_lease_count_before_selection, row.operator_priority),
      activeLeaseCountBeforeSelection: row.active_lease_count_before_selection,
      operatorPriority: row.operator_priority,
      startedAt: row.started_at,
      expiresAt: row.expires_at,
      lastTouchedAt: row.last_touched_at,
      status: row.status,
    };
  };

  const findActive: ProviderAccountLeaseService["findActive"] = async (input) => {
    await expireStaleLeases(db, input.now);

    const row = await db
      .prepare(
        `SELECT lease_ref,
                provider,
                provider_account_id,
                provider_account_ref,
                requested_action,
                run_id,
                assignment_id,
                order_id,
                expires_at,
                status,
                user_id
         FROM provider_account_leases
         WHERE lease_ref = ?
           AND user_id = ?
           AND status = 'active'
           AND expires_at > ?`,
      )
      .bind(input.leaseRef, input.userId, input.now)
      .first<ActiveLeaseRow>();

    return row === null
      ? undefined
      : {
          leaseRef: row.lease_ref,
          provider: row.provider,
          providerAccountId: row.provider_account_id,
          providerAccountRef: row.provider_account_ref,
          requestedAction: row.requested_action,
          runId: row.run_id,
          assignmentId: row.assignment_id,
          orderId: row.order_id,
          expiresAt: row.expires_at,
          status: row.status,
          userId: row.user_id,
        };
  };

  const listActive: ProviderAccountLeaseService["listActive"] = async (userId, now) => {
    await expireStaleLeases(db, now);
    const rows = await db
      .prepare(
        `SELECT l.lease_ref,
                l.provider_account_ref,
                COALESCE(pa.operator_label, pa.account_label) AS account_label,
                l.requested_action,
                l.run_id,
                l.assignment_id,
                l.order_id,
                l.started_at,
                l.expires_at,
                l.last_touched_at,
                l.status
         FROM provider_account_leases l
         JOIN provider_accounts pa ON pa.id = l.provider_account_id
         WHERE l.user_id = ?
           AND pa.user_id = ?
           AND l.status = 'active'
           AND l.expires_at > ?
         ORDER BY l.started_at DESC
         LIMIT 100`,
      )
      .bind(userId, userId, now)
      .all<
        Readonly<{
          lease_ref: string;
          provider_account_ref: string;
          account_label: string | null;
          requested_action: string;
          run_id: string | null;
          assignment_id: string | null;
          order_id: string | null;
          started_at: string;
          expires_at: string;
          last_touched_at: string | null;
          status: string;
        }>
      >();

    return rows.results.map((row) => ({
      accountLabel: row.account_label,
      assignmentId: row.assignment_id,
      expiresAt: row.expires_at,
      lastTouchedAt: row.last_touched_at,
      leaseRef: row.lease_ref,
      orderId: row.order_id,
      providerAccountRef: row.provider_account_ref,
      requestedAction: row.requested_action,
      runId: row.run_id,
      startedAt: row.started_at,
      status: row.status,
    }));
  };

  const explainSelection: ProviderAccountLeaseService["explainSelection"] = async (userId, now) => {
    await expireStaleLeases(db, now);
    const row = await db
      .prepare(
        `SELECT pa.provider_account_ref,
                  COALESCE(pa.operator_label, pa.account_label) AS account_label,
                  COUNT(active_leases.id) AS active_lease_count,
                  COALESCE(pa.lease_limit, 1) AS lease_limit,
                  pa.operator_priority
           FROM provider_accounts pa
           LEFT JOIN provider_account_leases active_leases
             ON active_leases.provider_account_id = pa.id
            AND active_leases.status = 'active'
            AND active_leases.expires_at > ?
           WHERE pa.user_id = ?
             AND pa.provider = 'chatgpt_codex'
             AND pa.status = 'connected'
             AND pa.health = 'healthy'
             AND pa.secret_ref IS NOT NULL
             AND pa.deleted_at IS NULL
             AND COALESCE(pa.low_credit_flag, 0) = 0
             AND pa.reauth_required_reason IS NULL
             AND (pa.cooldown_until IS NULL OR pa.cooldown_until <= ?)
           GROUP BY pa.id
           HAVING COUNT(active_leases.id) < COALESCE(pa.lease_limit, 1)
           ORDER BY
             COUNT(active_leases.id) ASC,
             pa.operator_priority ASC,
             COALESCE(pa.last_selected_at, pa.connected_at, pa.created_at) ASC,
             pa.provider_account_ref ASC
           LIMIT 1`,
      )
      .bind(now, userId, now)
      .first<
        Readonly<{
          provider_account_ref: string;
          account_label: string | null;
          active_lease_count: number;
          lease_limit: number;
          operator_priority: number;
        }>
      >();

    if (row === null) {
      return {
        status: "none",
        providerAccountRef: null,
        accountLabel: null,
        selectedByPolicyVersion: PROVIDER_ACCOUNT_LEASE_POLICY_VERSION,
        selectionReason:
          "No connected healthy ChatGPT/Codex account is currently eligible for lease.",
        activeLeaseCount: null,
        leaseLimit: null,
        operatorPriority: null,
      };
    }

    return {
      status: "selected",
      providerAccountRef: row.provider_account_ref,
      accountLabel: row.account_label,
      selectedByPolicyVersion: PROVIDER_ACCOUNT_LEASE_POLICY_VERSION,
      selectionReason: leaseSelectionReason(row.active_lease_count, row.operator_priority),
      activeLeaseCount: row.active_lease_count,
      leaseLimit: row.lease_limit,
      operatorPriority: row.operator_priority,
    };
  };

  const touch: ProviderAccountLeaseService["touch"] = async (input) => {
    const result = await db
      .prepare(
        `UPDATE provider_account_leases
            SET last_touched_at = ?,
                expires_at = ?
          WHERE lease_ref = ?
            AND user_id = ?
            AND status = 'active'`,
      )
      .bind(input.now, input.expiresAt, input.leaseRef, input.userId)
      .run();
    const changed = (result.meta?.changes ?? 0) > 0;
    if (changed) {
      await mirrorLeaseRef(mirror, input.leaseRef);
    }
    return changed;
  };

  const release: ProviderAccountLeaseService["release"] = async (input) => {
    const result = await db
      .prepare(
        `UPDATE provider_account_leases
            SET status = ?,
                released_at = ?,
                terminal_outcome = ?,
                failure_class = ?
          WHERE lease_ref = ?
            AND user_id = ?
            AND status = 'active'`,
      )
      .bind(
        input.status,
        input.now,
        input.terminalOutcome,
        input.failureClass,
        input.leaseRef,
        input.userId,
      )
      .run();
    const changed = (result.meta?.changes ?? 0) > 0;
    if (changed) {
      await mirrorLeaseRef(mirror, input.leaseRef);
    }
    return changed;
  };

  const applyFailoverAccountState = async (
    input: Readonly<{
      lease: ActiveProviderAccountLease;
      failureClass: ProviderAccountFailoverFailureClass;
      now: string;
      userId: string;
    }>,
  ): Promise<ReturnType<typeof classifyProviderAccountFailover>> => {
    const action = classifyProviderAccountFailover(input.failureClass, input.now);
    const failed = await db
      .prepare(
        `UPDATE provider_account_leases
            SET status = 'failed',
                released_at = ?,
                terminal_outcome = ?,
                failure_class = ?
          WHERE lease_ref = ?
            AND user_id = ?
            AND status = 'active'`,
      )
      .bind(input.now, input.failureClass, input.failureClass, input.lease.leaseRef, input.userId)
      .run();

    if ((failed.meta?.changes ?? 0) === 0) {
      throw new Error("provider_account_lease_failover_lost_ownership");
    }
    await mirrorLeaseRef(mirror, input.lease.leaseRef);

    if (action.accountStateAction !== "do_not_poison_account") {
      await db
        .prepare(
          `UPDATE provider_accounts
              SET health = COALESCE(?, health),
                  status = CASE
                    WHEN ? = 'requires_reauth' THEN 'unhealthy'
                    WHEN ? = 'unhealthy' THEN 'unhealthy'
                    ELSE status
                  END,
                  low_credit_flag = ?,
                  cooldown_until = ?,
                  recent_failure_class = ?,
                  last_failed_launch_at = ?,
                  reauth_required_reason = CASE
                    WHEN ? = 'requires_reauth' THEN ?
                    ELSE reauth_required_reason
                  END,
                  refill_note = CASE
                    WHEN ? = 1 THEN 'Refill or rotate this ChatGPT/Codex account before reuse.'
                    ELSE refill_note
                  END,
                  last_status_at = ?,
                  updated_at = ?
            WHERE id = ?
              AND user_id = ?`,
        )
        .bind(
          action.health,
          action.health,
          action.health,
          action.lowCredit ? 1 : 0,
          action.cooldownUntil,
          action.recentFailureClass,
          input.now,
          action.health,
          input.failureClass,
          action.lowCredit ? 1 : 0,
          input.now,
          input.now,
          input.lease.providerAccountId,
          input.userId,
        )
        .run();
    } else {
      await db
        .prepare(
          `UPDATE provider_accounts
              SET recent_failure_class = ?,
                  last_failed_launch_at = ?,
                  updated_at = ?
            WHERE id = ?
              AND user_id = ?`,
        )
        .bind(
          action.recentFailureClass,
          input.now,
          input.now,
          input.lease.providerAccountId,
          input.userId,
        )
        .run();
    }
    if (mirror !== undefined) {
      await mirror.mirrorRowsByKey("provider_accounts", [[input.lease.providerAccountId]]);
    }
    return action;
  };

  const recordFailoverReceipt = async (input: {
    action: ReturnType<typeof classifyProviderAccountFailover>;
    attemptNumber: number;
    maxAttempts: number;
    now: string;
    outcome: "retrying" | "blocked";
    previousLease: ActiveProviderAccountLease;
    nextLease: ProviderAccountLease | null;
    requestedAction: string;
    runId: string | null;
    assignmentId: string | null;
    orderId: string | null;
    source: string;
  }): Promise<ProviderAccountFailoverReceipt> => {
    const receiptId = compactRandomId("provider_account_failover_receipt");
    const customerSafeStatus =
      input.outcome === "blocked"
        ? "Work is blocked until another eligible account is available."
        : input.action.customerSafeStatus;
    const operatorSummary =
      input.outcome === "blocked"
        ? `Provider account failover blocked after ${input.attemptNumber}/${input.maxAttempts} attempt(s); no eligible ChatGPT/Codex account was available.`
        : `Provider account failover retrying after ${input.action.failureClass}; next account lease was created.`;
    const customerSafeSummary =
      input.outcome === "blocked"
        ? "Work is waiting for operator capacity before it can continue."
        : "Work is retrying through another connected execution account.";

    await db
      .prepare(
        `INSERT INTO provider_account_failover_receipts
          (id,
           run_id,
           assignment_id,
           order_id,
           requested_action,
           previous_lease_ref,
           previous_provider_account_ref,
           next_lease_ref,
           next_provider_account_ref,
           failure_class,
           account_state_action,
           cooldown_until,
           outcome,
           attempt_number,
           max_attempts,
           customer_safe_status,
           policy_version,
           operator_summary,
           customer_safe_summary,
           created_at,
           metadata_json)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        receiptId,
        input.runId,
        input.assignmentId,
        input.orderId,
        input.requestedAction,
        input.previousLease.leaseRef,
        input.previousLease.providerAccountRef,
        input.nextLease?.leaseRef ?? null,
        input.nextLease?.providerAccountRef ?? null,
        input.action.failureClass,
        input.action.accountStateAction,
        input.action.cooldownUntil,
        input.outcome,
        input.attemptNumber,
        input.maxAttempts,
        customerSafeStatus,
        PROVIDER_ACCOUNT_LEASE_POLICY_VERSION,
        operatorSummary,
        customerSafeSummary,
        input.now,
        JSON.stringify({
          accountStateAction: input.action.accountStateAction,
          cooldownUntil: input.action.cooldownUntil,
          failureClass: input.action.failureClass,
          outcome: input.outcome,
          policyVersion: PROVIDER_ACCOUNT_LEASE_POLICY_VERSION,
          source: input.source,
        }),
      )
      .run();

    if (mirror !== undefined) {
      await mirror.mirrorRowsByKey("provider_account_failover_receipts", [[receiptId]]);
    }

    return {
      receiptId,
      runId: input.runId,
      assignmentId: input.assignmentId,
      orderId: input.orderId,
      requestedAction: input.requestedAction,
      previousLeaseRef: input.previousLease.leaseRef,
      previousProviderAccountRef: input.previousLease.providerAccountRef,
      nextLeaseRef: input.nextLease?.leaseRef ?? null,
      nextProviderAccountRef: input.nextLease?.providerAccountRef ?? null,
      failureClass: input.action.failureClass,
      accountStateAction: input.action.accountStateAction,
      cooldownUntil: input.action.cooldownUntil,
      outcome: input.outcome,
      attemptNumber: input.attemptNumber,
      maxAttempts: input.maxAttempts,
      customerSafeStatus,
      operatorSummary,
      customerSafeSummary,
      policyVersion: PROVIDER_ACCOUNT_LEASE_POLICY_VERSION,
      createdAt: input.now,
    };
  };

  const failover: ProviderAccountLeaseService["failover"] = async (input) => {
    const previousLease = await findActive({
      leaseRef: input.previousLeaseRef,
      now: input.now,
      userId: input.userId,
    });
    if (previousLease === undefined) {
      return undefined;
    }

    const action = await applyFailoverAccountState({
      failureClass: input.failureClass,
      lease: previousLease,
      now: input.now,
      userId: input.userId,
    });
    const exhausted = input.attemptNumber >= input.maxAttempts;
    const nextLease =
      exhausted || !action.retryAllowed
        ? null
        : ((await acquire({
            assignmentId: input.assignmentId ?? previousLease.assignmentId,
            expiresAt: input.expiresAt,
            now: input.now,
            orderId: input.orderId ?? previousLease.orderId,
            requiredProvider: previousLease.provider,
            requestedAction: input.requestedAction,
            runId: input.runId ?? previousLease.runId,
            selectedByActor: input.selectedByActor,
            source: input.source,
            userId: input.userId,
          })) ?? null);
    const outcome = nextLease === null ? "blocked" : "retrying";
    const receipt = await recordFailoverReceipt({
      action,
      assignmentId: input.assignmentId ?? previousLease.assignmentId,
      attemptNumber: input.attemptNumber,
      maxAttempts: input.maxAttempts,
      nextLease,
      now: input.now,
      orderId: input.orderId ?? previousLease.orderId,
      outcome,
      previousLease,
      requestedAction: input.requestedAction,
      runId: input.runId ?? previousLease.runId,
      source: input.source,
    });

    return { action, nextLease, outcome, previousLease, receipt };
  };

  const listFailoverReceipts: ProviderAccountLeaseService["listFailoverReceipts"] = async (
    input,
  ) => {
    const rows = await db
      .prepare(
        `SELECT r.id,
                  r.run_id,
                  r.assignment_id,
                  r.order_id,
                  r.requested_action,
                  r.previous_lease_ref,
                  r.previous_provider_account_ref,
                  r.next_lease_ref,
                  r.next_provider_account_ref,
                  r.failure_class,
                  r.account_state_action,
                  r.cooldown_until,
                  r.outcome,
                  r.attempt_number,
                  r.max_attempts,
                  r.customer_safe_status,
                  r.policy_version,
                  r.operator_summary,
                  r.customer_safe_summary,
                  r.created_at
             FROM provider_account_failover_receipts r
             LEFT JOIN provider_account_leases previous_lease
               ON previous_lease.lease_ref = r.previous_lease_ref
             LEFT JOIN provider_account_leases next_lease
               ON next_lease.lease_ref = r.next_lease_ref
            WHERE COALESCE(previous_lease.user_id, next_lease.user_id) = ?
              AND (? IS NULL OR r.run_id = ?)
              AND (? IS NULL OR r.assignment_id = ?)
              AND (? IS NULL OR r.order_id = ?)
            ORDER BY r.created_at DESC
            LIMIT ?`,
      )
      .bind(
        input.userId,
        input.runId,
        input.runId,
        input.assignmentId,
        input.assignmentId,
        input.orderId,
        input.orderId,
        input.limit,
      )
      .all<
        Readonly<{
          id: string;
          run_id: string | null;
          assignment_id: string | null;
          order_id: string | null;
          requested_action: string;
          previous_lease_ref: string | null;
          previous_provider_account_ref: string | null;
          next_lease_ref: string | null;
          next_provider_account_ref: string | null;
          failure_class: ProviderAccountFailoverFailureClass;
          account_state_action: string;
          cooldown_until: string | null;
          outcome: "retrying" | "blocked";
          attempt_number: number;
          max_attempts: number;
          customer_safe_status: string;
          policy_version: typeof PROVIDER_ACCOUNT_LEASE_POLICY_VERSION;
          operator_summary: string;
          customer_safe_summary: string | null;
          created_at: string;
        }>
      >();

    return rows.results.map((row) => ({
      receiptId: row.id,
      runId: row.run_id,
      assignmentId: row.assignment_id,
      orderId: row.order_id,
      requestedAction: row.requested_action,
      previousLeaseRef: row.previous_lease_ref,
      previousProviderAccountRef: row.previous_provider_account_ref,
      nextLeaseRef: row.next_lease_ref,
      nextProviderAccountRef: row.next_provider_account_ref,
      failureClass: row.failure_class,
      accountStateAction: row.account_state_action,
      cooldownUntil: row.cooldown_until,
      outcome: row.outcome,
      attemptNumber: row.attempt_number,
      maxAttempts: row.max_attempts,
      customerSafeStatus: row.customer_safe_status,
      operatorSummary: row.operator_summary,
      customerSafeSummary: row.customer_safe_summary,
      policyVersion: row.policy_version,
      createdAt: row.created_at,
    }));
  };

  return {
    acquire,
    explainSelection,
    failover,
    findActive,
    listActive,
    listFailoverReceipts,
    release,
    touch,
  };
};
