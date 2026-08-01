import type { ForensicBudget } from "@openagentsinc/forensic-contract";
import { Effect } from "effect";
import { describe, expect, test } from "vite-plus/test";

import {
  FORENSIC_MANAGED_SANDBOX_PATH,
  assertForensicBudgetBelowPrompt,
  makeForensicManagedSandbox,
  makeForensicManagedSandboxRoutes,
} from "./forensic-managed-sandbox";
import type { BoxV1Policy } from "./managed-sandbox-box-v1-routes";
import type { ManagedSandboxBroker } from "./managed-sandbox-broker";

const budget: ForensicBudget = {
  maxTimeSeconds: 900,
  maxTokens: 20_000,
  maxCostMicros: 10_000,
  maxConcurrency: 1,
  maxArtifactBytes: 10_000_000,
  maxNetworkBytes: 100_000_000,
};

const usage = {
  elapsedSeconds: 10,
  tokens: 100,
  costMicros: 20,
  artifactBytes: 30,
  networkBytes: 40,
  activeTurns: 0,
  costMeasured: true,
};

const policy: BoxV1Policy = {
  target: {
    targetRef: "target.gcp.managed-sandbox.us-central1",
    targetClass: "openagents_managed",
    provider: "google_cloud",
    adapterRef: "adapter.oa-codex-control.gce.v1",
    region: "us-central1",
    isolation: "gce_vm",
    dataPosture: "openagents_managed_region",
  },
  imageDigest: `sha256:${"a".repeat(64)}`,
  profileRef: "profile.sbx.gce.e2-small.v1",
  defaultTtlSeconds: 900,
  maxTtlSeconds: 900,
  maxActiveBoxes: 1,
  maxCostMicros: 10_000,
  maxCpuMillis: 900_000,
  maxNetworkBytes: 100_000_000,
  maxArtifactBytes: 10_000_000,
};

describe("forensic managed sandbox", () => {
  test("admits only when every below-prompt budget has remaining measured capacity", async () => {
    await expect(
      Effect.runPromise(assertForensicBudgetBelowPrompt(budget, usage)),
    ).resolves.toBeUndefined();

    for (const refused of [
      { ...usage, costMeasured: false },
      { ...usage, activeTurns: 1 },
      { ...usage, elapsedSeconds: budget.maxTimeSeconds },
      { ...usage, tokens: budget.maxTokens },
      { ...usage, costMicros: budget.maxCostMicros },
      { ...usage, artifactBytes: budget.maxArtifactBytes },
      { ...usage, networkBytes: budget.maxNetworkBytes },
    ]) {
      await expect(
        Effect.runPromise(assertForensicBudgetBelowPrompt(budget, refused)),
      ).rejects.toBeDefined();
    }
  });

  test("refuses a foreign target before invoking the broker", async () => {
    let brokerInvoked = false;
    const broker = {
      execute: () => {
        brokerInvoked = true;
        return Effect.die("broker must not run");
      },
      list: () => Effect.succeed([]),
    } as ManagedSandboxBroker;
    const service = makeForensicManagedSandbox({
      broker,
      policy: {
        ...policy,
        target: { ...policy.target, provider: "box" as "google_cloud" },
      },
      profileDigest: `sha256:${"b".repeat(64)}`,
      resolveBudget: () => Effect.succeed(budget),
    });

    await expect(
      Effect.runPromise(
        service.admit({
          commandRef: "command.forensic.create.fixture",
          idempotencyRef: "idempotency.forensic.create.fixture",
          requestedByRef: "principal.forensic.fixture",
          ownerRef: "owner.forensic.fixture",
          tenantRef: "tenant.forensic.fixture",
          workUnitRef: "work.forensic.fixture",
          attachmentRef: "attachment.forensic.fixture",
          placementRef: "placement.forensic.fixture",
          regionRef: "region.google-cloud.us-central1",
          budgetRef: "budget.forensic.fixture",
          requestedAt: "2026-08-01T10:00:00.000Z",
          expiresAt: "2026-08-01T10:15:00.000Z",
          budget,
        }),
      ),
    ).rejects.toBeDefined();
    expect(brokerInvoked).toBe(false);
  });

  test("does not expose resume, stop, checkpoint, or restore operations", () => {
    const service = makeForensicManagedSandbox({
      broker: {
        execute: () => Effect.die("unused"),
        list: () => Effect.succeed([]),
      },
      policy,
      profileDigest: `sha256:${"b".repeat(64)}`,
      resolveBudget: () => Effect.succeed(budget),
    });
    expect(Object.keys(service).sort()).toEqual(["admit", "cancel", "delete", "dispatch"]);
  });

  test("the live route is authenticated, default-off, and exposes one exact path", async () => {
    const routes = makeForensicManagedSandboxRoutes({
      authenticateOwner: async () => ({ userId: "owner.forensic.fixture" }),
      enabled: () => false,
      policy: () => Effect.die("policy must not run while disabled"),
      profileDigest: () => undefined,
      store: () => {
        throw new Error("store must not run while disabled");
      },
      runtime: () => Effect.die("runtime must not run while disabled"),
    });
    const response = await Effect.runPromise(
      routes.handle(
        new Request(`https://api.openagents.com${FORENSIC_MANAGED_SANDBOX_PATH}`, {
          method: "POST",
          body: JSON.stringify({ _tag: "Admit" }),
        }),
        {},
        {} as ExecutionContext,
      ),
    );
    expect(response.status).toBe(503);
    await expect(response.json()).resolves.toMatchObject({ error: "runtime_not_admitted" });
  });
});
