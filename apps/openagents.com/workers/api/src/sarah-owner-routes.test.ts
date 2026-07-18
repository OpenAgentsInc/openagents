import type { SyncSql } from "@openagentsinc/khala-sync-server";
import {
  ROOT_AUTHORITY_PROFILE_REF,
  ROOT_AUTHORITY_REVISION,
  SARAH_AUTHORITY_PROFILE_REF,
  SARAH_AUTHORITY_REVISION,
  SARAH_CAPABILITIES,
  SARAH_PRINCIPAL_SCHEMA,
} from "@openagentsinc/sarah";
import { Effect } from "effect";
import { describe, expect, test } from "vitest";

import { materializeHttpResult } from "./http/responses";
import {
  SARAH_OWNER_PATH,
  authorizeSarahOperation,
  ensureSarahPrincipal,
  hasSarahThreadAuthority,
  makeSarahOwnerRoutes,
  sarahThreadRefForOwner,
} from "./sarah-owner-routes";

const contractId = "openagents_mobile.sarah_owner_orchestrator.v1";
const connectionString = "postgresql://fixture/private";

type Env = Readonly<{ KHALA_SYNC_DB?: Readonly<{ connectionString: string }> }>;

const makeHarness = (ownerUserId?: string) => {
  let ended = 0;
  const routes = makeSarahOwnerRoutes<Env>({
    authenticateOwner: async () =>
      ownerUserId === undefined ? undefined : { userId: ownerUserId },
    makeSqlClient: async () => ({
      sql: (async () => []) as unknown as SyncSql,
      end: async () => {
        ended += 1;
      },
    }),
    ensurePrincipal: async (_sql, owner) => ({
      schema: SARAH_PRINCIPAL_SCHEMA,
      principalRef: "principal.sarah",
      displayName: "Sarah",
      role: "Owner orchestrator",
      threadRef: await sarahThreadRefForOwner(owner),
      authorityProfileRef: SARAH_AUTHORITY_PROFILE_REF,
      authorityRevision: SARAH_AUTHORITY_REVISION,
      rootAuthorityProfileRef: ROOT_AUTHORITY_PROFILE_REF,
      rootAuthorityRevision: ROOT_AUTHORITY_REVISION,
      memory: "durable_cited",
      capabilities: SARAH_CAPABILITIES,
    }),
  });
  const run = (method = "POST") =>
    routes
      .handle(
        new Request(`https://openagents.com${SARAH_OWNER_PATH}`, { method }),
        { KHALA_SYNC_DB: { connectionString } },
        {} as ExecutionContext,
      )
      .pipe(Effect.map(materializeHttpResult), Effect.runPromise);
  return { run, ended: () => ended };
};

describe(`contract ${contractId}`, () => {
  test("requires the existing authenticated human owner boundary", async () => {
    expect((await makeHarness().run()).status).toBe(401);
  });

  test("returns one opaque stable thread and closes storage", async () => {
    const harness = makeHarness("owner.fixture.123");
    const first = await harness.run();
    const second = await harness.run("GET");
    const body = (await first.json()) as {
      principal: {
        threadRef: string;
        memory: string;
        rootAuthorityRevision: number;
      };
    };
    expect(first.status).toBe(200);
    expect(body.principal).toMatchObject({
      memory: "durable_cited",
      rootAuthorityRevision: 5,
    });
    expect(body.principal.threadRef).toMatch(/^thread\.sarah\.[0-9a-f]{24}$/);
    expect(JSON.stringify(await second.json())).toContain(body.principal.threadRef);
    expect(harness.ended()).toBe(2);
  });

  test("requires a persisted admitted receipt before a Sarah-shaped thread gains authority", async () => {
    const ownerUserId = "owner.fixture.123";
    const threadRef = await sarahThreadRefForOwner(ownerUserId);
    const withoutReceipt = (async (strings: TemplateStringsArray) =>
      strings.join("?").includes("FROM users AS owner_user")
        ? [{ email: "chris@openagents.com" }]
        : []) as unknown as SyncSql;
    const withReceipt = (async (strings: TemplateStringsArray) =>
      strings.join("?").includes("FROM users AS owner_user")
        ? [{ email: "chris@openagents.com" }]
        : [{ receipt_ref: "receipt.authority.sarah.fixture" }]) as unknown as SyncSql;
    expect(await hasSarahThreadAuthority(withoutReceipt, ownerUserId, threadRef)).toBe(false);
    expect(await hasSarahThreadAuthority(withReceipt, ownerUserId, threadRef)).toBe(true);
    expect(
      await hasSarahThreadAuthority(
        withReceipt,
        ownerUserId,
        "thread.sarah.ffffffffffffffffffffffff",
      ),
    ).toBe(false);
  });

  test("refuses a historical Sarah receipt when its current identity is not the admitted owner", async () => {
    const ownerUserId = "owner.fixture.not-admin";
    const threadRef = await sarahThreadRefForOwner(ownerUserId);
    const sql = (async (strings: TemplateStringsArray) =>
      strings.join("?").includes("FROM users AS owner_user")
        ? [{ email: "someone-else@example.com" }]
        : [{ receipt_ref: "receipt.authority.sarah.historical" }]) as unknown as SyncSql;

    expect(await hasSarahThreadAuthority(sql, ownerUserId, threadRef)).toBe(false);
  });

  test("parses the authority evidence parameter through text before storing jsonb", async () => {
    const ownerUserId = "owner.fixture.123";
    const threadRef = await sarahThreadRefForOwner(ownerUserId);
    const statements: Array<string> = [];
    const sql = (async (strings: TemplateStringsArray, ..._values: ReadonlyArray<unknown>) => {
      const statement = strings.join("?");
      statements.push(statement);
      return statement.includes("FROM khala_sync_chat_threads") ? [{ thread_id: threadRef }] : [];
    }) as unknown as SyncSql;

    await ensureSarahPrincipal(sql, ownerUserId);

    const receiptInsert = statements.find((statement) =>
      statement.includes("INSERT INTO sarah_authority_decision_receipts"),
    );
    expect(receiptInsert).toContain("::text::jsonb");
  });

  test("upgrades a previously admitted Sarah thread to the current authority revision", async () => {
    const ownerUserId = "owner.fixture.123";
    const threadRef = await sarahThreadRefForOwner(ownerUserId);
    const insertedValues: Array<ReadonlyArray<unknown>> = [];
    const sql = (async (strings: TemplateStringsArray, ...values: ReadonlyArray<unknown>) => {
      const statement = strings.join("?");
      if (statement.includes("FROM users AS owner_user")) {
        return [{ email: "chris@openagents.com" }];
      }
      if (statement.includes("profile_revision") && statement.includes("SELECT receipt_ref")) {
        return [];
      }
      if (statement.includes("SELECT receipt_ref")) {
        return [{ receipt_ref: "receipt.authority.sarah.bootstrap.prior" }];
      }
      if (statement.includes("FROM khala_sync_chat_threads")) {
        return [{ thread_id: threadRef }];
      }
      if (statement.includes("INSERT INTO sarah_authority_decision_receipts")) {
        insertedValues.push(values);
      }
      return [];
    }) as unknown as SyncSql;

    expect(await hasSarahThreadAuthority(sql, ownerUserId, threadRef)).toBe(true);
    expect(insertedValues.flat()).toContain(SARAH_AUTHORITY_REVISION);
    expect(insertedValues.flat()).toContain(
      `receipt.authority.sarah.bootstrap.${threadRef.slice(-24)}.rev${SARAH_AUTHORITY_REVISION}`,
    );
  });

  test("receipts an exact admitted Sarah operation before the target broker runs", async () => {
    const ownerUserId = "owner.fixture.123";
    const threadRef = await sarahThreadRefForOwner(ownerUserId);
    const insertedValues: Array<ReadonlyArray<unknown>> = [];
    const sql = (async (strings: TemplateStringsArray, ...values: ReadonlyArray<unknown>) => {
      const statement = strings.join("?");
      if (statement.includes("FROM users AS owner_user")) {
        return [{ email: "chris@openagents.com" }];
      }
      if (statement.includes("SELECT receipt_ref")) {
        return [{ receipt_ref: "receipt.authority.sarah.current" }];
      }
      if (statement.includes("INSERT INTO sarah_authority_decision_receipts")) {
        insertedValues.push(values);
      }
      return [];
    }) as unknown as SyncSql;

    const result = await Effect.runPromise(
      authorizeSarahOperation(sql, {
        action: "dispatch_owner_capacity_coding_workers",
        ownerUserId,
        resource: "owner_linked_pylon_coding_capacity",
        threadRef,
        triggerRef: "turn.fixture.tool.call.fixture",
      }),
    );
    expect(result).toMatchObject({ allowed: true });
    expect(insertedValues.flat()).toContain("dispatch_owner_capacity_coding_workers");
    expect(insertedValues.flat()).toContain(SARAH_AUTHORITY_REVISION);
  });

  test("admits Sarah's exact owner-private terminal-history harness review broker", async () => {
    const ownerUserId = "owner.fixture.123";
    const threadRef = await sarahThreadRefForOwner(ownerUserId);
    const insertedValues: Array<ReadonlyArray<unknown>> = [];
    const sql = (async (strings: TemplateStringsArray, ...values: ReadonlyArray<unknown>) => {
      const statement = strings.join("?");
      if (statement.includes("FROM users AS owner_user")) {
        return [{ email: "chris@openagents.com" }];
      }
      if (statement.includes("SELECT receipt_ref")) {
        return [{ receipt_ref: "receipt.authority.sarah.current" }];
      }
      if (statement.includes("INSERT INTO sarah_authority_decision_receipts")) {
        insertedValues.push(values);
      }
      return [];
    }) as unknown as SyncSql;

    const result = await Effect.runPromise(
      authorizeSarahOperation(sql, {
        action: "review_own_terminal_history_and_propose_harness",
        ownerUserId,
        resource: "owner_private_sarah_harness",
        threadRef,
        triggerRef: "turn.fixture.tool.call.harness_review",
      }),
    );

    expect(result).toMatchObject({ allowed: true });
    expect(insertedValues.flat()).toContain(
      "review_own_terminal_history_and_propose_harness",
    );
    expect(insertedValues.flat()).toContain(
      JSON.stringify(["resource:owner_private_sarah_harness"]),
    );
    expect(insertedValues.flat()).toContain(SARAH_AUTHORITY_REVISION);
  });
});
