import { describe, expect, test } from "vitest";

import { MutationResult, PushResponse } from "@openagentsinc/khala-sync";
import type { MutatorRegistry, SyncSql } from "@openagentsinc/khala-sync-server";

import { seedCloudCodingRuntimeTurn } from "./cloud-coding-runtime-seed";

describe("cloud coding runtime seed", () => {
  test("creates the owner thread, repository binding, message, and runtime turn in order", async () => {
    const calls: Array<Record<string, unknown>> = [];
    const sql = (() => Promise.resolve([])) as unknown as SyncSql;

    await seedCloudCodingRuntimeTurn({
      branch: "main",
      executePushImpl: async (input) => {
        calls.push(input as unknown as Record<string, unknown>);
        return new PushResponse({
          lastMutationId: 4,
          protocolVersion: 1,
          results: input.request.mutations.map(
            (mutation) =>
              new MutationResult({
                mutationId: mutation.mutationId,
                status: "applied" as const,
              }),
          ),
        });
      },
      nowIso: "2026-07-24T15:00:00.000Z",
      objective: "Stage the Omega qualification file.",
      ownerUserId: "owner-1",
      registry: {} as MutatorRegistry,
      repositoryFullName: "OpenAgentsInc/openagents",
      sql,
      threadRef: "thread.cloud-coding.ccs-1",
      turnId: "ccs-1",
    });

    expect(calls).toHaveLength(1);
    const request = calls[0]?.request as {
      mutations: ReadonlyArray<{ name: string; argsJson: string }>;
    };
    expect(request.mutations.map((mutation) => mutation.name)).toEqual([
      "chat.createThread",
      "chat.bindThreadRepo",
      "chat.appendMessage",
      "runtime.startTurn",
    ]);
    expect(JSON.parse(request.mutations[3]!.argsJson)).toMatchObject({
      kind: "turn.start",
      target: { lane: "managed_cloud" },
      threadId: "thread.cloud-coding.ccs-1",
      turnId: "ccs-1",
    });
  });

  test("accepts an existing turn only when its owner and thread match", async () => {
    const sql = (() =>
      Promise.resolve([
        { owner_user_id: "owner-1", thread_id: "thread.cloud-coding.ccs-1" },
      ])) as unknown as SyncSql;
    let pushed = false;

    await seedCloudCodingRuntimeTurn({
      branch: "main",
      executePushImpl: async () => {
        pushed = true;
        throw new Error("unexpected push");
      },
      nowIso: "2026-07-24T15:00:00.000Z",
      objective: "Stage the Omega qualification file.",
      ownerUserId: "owner-1",
      registry: {} as MutatorRegistry,
      repositoryFullName: "OpenAgentsInc/openagents",
      sql,
      threadRef: "thread.cloud-coding.ccs-1",
      turnId: "ccs-1",
    });

    expect(pushed).toBe(false);
  });
});
