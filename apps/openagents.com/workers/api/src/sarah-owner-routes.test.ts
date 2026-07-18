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
      principal: { threadRef: string; memory: string; rootAuthorityRevision: number };
    };
    expect(first.status).toBe(200);
    expect(body.principal).toMatchObject({ memory: "durable_cited", rootAuthorityRevision: 3 });
    expect(body.principal.threadRef).toMatch(/^thread\.sarah\.[0-9a-f]{24}$/);
    expect(JSON.stringify(await second.json())).toContain(body.principal.threadRef);
    expect(harness.ended()).toBe(2);
  });

  test("requires a persisted admitted receipt before a Sarah-shaped thread gains authority", async () => {
    const ownerUserId = "owner.fixture.123";
    const threadRef = await sarahThreadRefForOwner(ownerUserId);
    const withoutReceipt = (async () => []) as unknown as SyncSql;
    const withReceipt = (async () => [
      { receipt_ref: "receipt.authority.sarah.fixture" },
    ]) as unknown as SyncSql;
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
});
