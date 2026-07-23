import { afterEach, beforeEach, describe, expect, test } from "vitest";

import { makeProviderAccountLeaseService } from "./provider-account-lease-service";
import { IDENTITY_AUTH_DOMAIN_D1_SCHEMA, makeSqliteD1, type SqliteD1 } from "./test/sqlite-d1";

const now = "2026-07-23T04:40:00.000Z";
const expiresAt = "2026-07-23T04:55:00.000Z";

const insertAccount = async (
  sqlite: SqliteD1,
  input: {
    id: string;
    userId: string;
    provider: "chatgpt_codex" | "google_gemini";
    providerAccountRef: string;
    operatorPriority: number;
  },
): Promise<void> => {
  await sqlite.db
    .prepare(
      `INSERT INTO provider_accounts
        (id,
         user_id,
         team_id,
         provider,
         auth_mode,
         status,
         health,
         provider_account_ref,
         secret_ref,
         account_label,
         plan_type,
         connected_at,
         disconnected_at,
         denied_at,
         last_status_at,
         metadata_json,
         created_at,
         updated_at,
         deleted_at,
         operator_priority,
         lease_limit,
         low_credit_flag)
       VALUES (?, ?, NULL, ?, 'oauth', 'connected', 'healthy', ?, ?, ?, NULL,
               ?, NULL, NULL, ?, NULL, ?, ?, NULL, ?, 1, 0)`,
    )
    .bind(
      input.id,
      input.userId,
      input.provider,
      input.providerAccountRef,
      `secret://${input.providerAccountRef}`,
      input.providerAccountRef,
      now,
      now,
      now,
      now,
      input.operatorPriority,
    )
    .run();
};

describe("provider account lease service", () => {
  let sqlite: SqliteD1;

  beforeEach(() => {
    sqlite = makeSqliteD1();
    sqlite.exec(IDENTITY_AUTH_DOMAIN_D1_SCHEMA);
  });

  afterEach(() => {
    sqlite.close();
  });

  test("atomically selects an exact-owner provider account and records caller provenance", async () => {
    await insertAccount(sqlite, {
      id: "account_owner_codex",
      operatorPriority: 10,
      provider: "chatgpt_codex",
      providerAccountRef: "provider-owner-codex",
      userId: "owner-a",
    });
    await insertAccount(sqlite, {
      id: "account_owner_gemini",
      operatorPriority: 1,
      provider: "google_gemini",
      providerAccountRef: "provider-owner-gemini",
      userId: "owner-a",
    });
    await insertAccount(sqlite, {
      id: "account_other_codex",
      operatorPriority: 1,
      provider: "chatgpt_codex",
      providerAccountRef: "provider-other-codex",
      userId: "owner-b",
    });

    const service = makeProviderAccountLeaseService({ db: sqlite.db });
    const lease = await service.acquire({
      assignmentId: "assignment-1",
      expiresAt,
      now,
      orderId: null,
      requiredProvider: "chatgpt_codex",
      requestedAction: "coding",
      runId: "run-1",
      selectedByActor: "sarah_cloud_dispatch",
      source: "sarah_cloud_coding_session",
      userId: "owner-a",
    });

    expect(lease).toMatchObject({
      activeLeaseCountBeforeSelection: 0,
      operatorPriority: 10,
      providerAccountRef: "provider-owner-codex",
      selectedByActor: "sarah_cloud_dispatch",
    });

    const persisted = await sqlite.db
      .prepare(
        `SELECT user_id, provider, selected_by_actor, metadata_json
           FROM provider_account_leases
          WHERE lease_ref = ?`,
      )
      .bind(lease?.leaseRef)
      .first<{
        user_id: string;
        provider: string;
        selected_by_actor: string;
        metadata_json: string;
      }>();

    expect(persisted).toMatchObject({
      provider: "chatgpt_codex",
      selected_by_actor: "sarah_cloud_dispatch",
      user_id: "owner-a",
    });
    expect(JSON.parse(persisted?.metadata_json ?? "{}")).toMatchObject({
      activeLeaseCountBeforeSelection: 0,
      operatorPriority: 10,
      providerAccountRef: "provider-owner-codex",
      source: "sarah_cloud_coding_session",
    });
  });

  test("does not touch or release another owner lease", async () => {
    await insertAccount(sqlite, {
      id: "account_owner",
      operatorPriority: 1,
      provider: "chatgpt_codex",
      providerAccountRef: "provider-owner",
      userId: "owner-a",
    });
    const service = makeProviderAccountLeaseService({ db: sqlite.db });
    const lease = await service.acquire({
      assignmentId: null,
      expiresAt,
      now,
      orderId: null,
      requiredProvider: "chatgpt_codex",
      requestedAction: "coding",
      runId: null,
      selectedByActor: "test",
      source: "test",
      userId: "owner-a",
    });

    expect(lease).toBeDefined();
    expect(
      await service.touch({
        expiresAt: "2026-07-23T05:10:00.000Z",
        leaseRef: lease?.leaseRef ?? "",
        now,
        userId: "owner-b",
      }),
    ).toBe(false);
    expect(
      await service.release({
        failureClass: null,
        leaseRef: lease?.leaseRef ?? "",
        now,
        status: "released",
        terminalOutcome: "released",
        userId: "owner-b",
      }),
    ).toBe(false);
    expect(
      await service.findActive({
        leaseRef: lease?.leaseRef ?? "",
        now,
        userId: "owner-a",
      }),
    ).toBeDefined();
  });

  test("applies typed failover state and leases the next bounded account for the same owner", async () => {
    await insertAccount(sqlite, {
      id: "account_first",
      operatorPriority: 1,
      provider: "chatgpt_codex",
      providerAccountRef: "provider-first",
      userId: "owner-a",
    });
    await insertAccount(sqlite, {
      id: "account_second",
      operatorPriority: 2,
      provider: "chatgpt_codex",
      providerAccountRef: "provider-second",
      userId: "owner-a",
    });
    await insertAccount(sqlite, {
      id: "account_other",
      operatorPriority: 1,
      provider: "chatgpt_codex",
      providerAccountRef: "provider-other",
      userId: "owner-b",
    });
    const service = makeProviderAccountLeaseService({ db: sqlite.db });
    const first = await service.acquire({
      assignmentId: "assignment-1",
      expiresAt,
      now,
      orderId: "order-1",
      requiredProvider: "chatgpt_codex",
      requestedAction: "coding",
      runId: "run-1",
      selectedByActor: "test",
      source: "test",
      userId: "owner-a",
    });

    const result = await service.failover({
      assignmentId: null,
      attemptNumber: 1,
      expiresAt,
      failureClass: "rate_limited",
      maxAttempts: 3,
      now,
      orderId: null,
      previousLeaseRef: first?.leaseRef ?? "",
      requestedAction: "coding",
      runId: null,
      selectedByActor: "sarah_cloud_dispatch",
      source: "sarah_cloud_coding_failover",
      userId: "owner-a",
    });

    expect(result).toMatchObject({
      action: {
        failureClass: "rate_limited",
      },
      nextLease: {
        providerAccountRef: "provider-second",
      },
      outcome: "retrying",
      previousLease: {
        providerAccountRef: "provider-first",
      },
      receipt: {
        assignmentId: "assignment-1",
        orderId: "order-1",
        runId: "run-1",
      },
    });

    const firstAccount = await sqlite.db
      .prepare(
        `SELECT recent_failure_class, cooldown_until
           FROM provider_accounts
          WHERE id = 'account_first'`,
      )
      .first<{
        recent_failure_class: string | null;
        cooldown_until: string | null;
      }>();
    expect(firstAccount?.recent_failure_class).toBe("rate_limited");
    expect(firstAccount?.cooldown_until).not.toBeNull();
  });

  test("records a bounded terminal audit receipt and quarantines an invalid token", async () => {
    await insertAccount(sqlite, {
      id: "account_terminal",
      operatorPriority: 1,
      provider: "chatgpt_codex",
      providerAccountRef: "provider-terminal",
      userId: "owner-a",
    });
    const service = makeProviderAccountLeaseService({ db: sqlite.db });
    const lease = await service.acquire({
      assignmentId: "assignment-terminal",
      expiresAt,
      now,
      orderId: null,
      requiredProvider: "chatgpt_codex",
      requestedAction: "agent_computer_codex_turn",
      runId: "run-terminal",
      selectedByActor: "sarah_managed_cloud_dispatch",
      source: "managed_cloud_runtime_dispatch",
      userId: "owner-a",
    });

    const result = await service.failover({
      assignmentId: "assignment-terminal",
      attemptNumber: 1,
      expiresAt,
      failureClass: "token_invalidated",
      maxAttempts: 1,
      now,
      orderId: null,
      previousLeaseRef: lease?.leaseRef ?? "",
      requestedAction: "agent_computer_codex_turn",
      runId: "run-terminal",
      selectedByActor: "sarah_managed_cloud_dispatch",
      source: "managed_cloud_runtime_terminal_failover",
      userId: "owner-a",
    });

    expect(result).toMatchObject({
      action: {
        accountStateAction: "requires_reauth",
        failureClass: "token_invalidated",
      },
      nextLease: null,
      outcome: "blocked",
      receipt: {
        attemptNumber: 1,
        failureClass: "token_invalidated",
        maxAttempts: 1,
        outcome: "blocked",
      },
    });
    const receipts = await service.listFailoverReceipts({
      assignmentId: "assignment-terminal",
      limit: 10,
      orderId: null,
      runId: "run-terminal",
      userId: "owner-a",
    });
    expect(receipts).toHaveLength(1);
    expect(receipts[0]).toMatchObject({
      failureClass: "token_invalidated",
      previousLeaseRef: lease?.leaseRef,
    });
    const account = await sqlite.db
      .prepare(
        `SELECT health, reauth_required_reason, recent_failure_class
           FROM provider_accounts
          WHERE id = 'account_terminal'`,
      )
      .first<{
        health: string;
        reauth_required_reason: string | null;
        recent_failure_class: string | null;
      }>();
    expect(account).toEqual({
      health: "requires_reauth",
      reauth_required_reason: "token_invalidated",
      recent_failure_class: "token_invalidated",
    });
  });
});
