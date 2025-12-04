import * as BunContext from "@effect/platform-bun/BunContext";
import * as FileSystem from "@effect/platform/FileSystem";
import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import { appendUsageRecord, readUsageRecords, summarizeUsage } from "./store.js";
import type { UsageRecord } from "./types.js";

const runWithBun = <A, E>(
  program: Effect.Effect<A, E, FileSystem.FileSystem>,
) => Effect.runPromise(program.pipe(Effect.provide(BunContext.layer)));

const sampleRecord = (overrides: Partial<UsageRecord> = {}): UsageRecord => ({
  sessionId: "s-1",
  projectId: "openagents",
  timestamp: "2024-01-01T00:00:00.000Z",
  idempotencyKey: undefined,
  inputTokens: 10,
  outputTokens: 5,
  cacheReadTokens: 2,
  cacheCreationTokens: 1,
  totalCostUsd: 0.01,
  agent: "claude-code",
  subtasks: 1,
  durationMs: 1000,
  ...overrides,
});

describe("usage store", () => {
  test("append and read usage records", async () => {
    const records = await runWithBun(
      Effect.gen(function* () {
        const fs = yield* FileSystem.FileSystem;
        const dir = yield* fs.makeTempDirectory({ prefix: "usage-store" });

        yield* appendUsageRecord({ rootDir: dir, record: sampleRecord({ sessionId: "s-1" }) });
        yield* appendUsageRecord({ rootDir: dir, record: sampleRecord({ sessionId: "s-2", inputTokens: 20 }) });

        return yield* readUsageRecords(dir);
      }),
    );

    expect(records).toHaveLength(2);
    expect(records[0].sessionId).toBe("s-1");
    expect(records[1].inputTokens).toBe(20);
  });

  test("summarize usage by day", async () => {
    const summary = await runWithBun(
      Effect.gen(function* () {
        const fs = yield* FileSystem.FileSystem;
        const dir = yield* fs.makeTempDirectory({ prefix: "usage-summary" });

        yield* appendUsageRecord({ rootDir: dir, record: sampleRecord({ sessionId: "s-1", timestamp: "2024-01-01T00:00:00.000Z" }) });
        yield* appendUsageRecord({ rootDir: dir, record: sampleRecord({ sessionId: "s-2", timestamp: "2024-01-01T12:00:00.000Z", inputTokens: 30 }) });
        yield* appendUsageRecord({ rootDir: dir, record: sampleRecord({ sessionId: "s-3", timestamp: "2024-01-02T00:00:00.000Z", inputTokens: 5 }) });

        return yield* summarizeUsage({ rootDir: dir, period: "day" });
      }),
    );

    expect(summary.byPeriod["2024-01-01"].sessions).toBe(2);
    expect(summary.byPeriod["2024-01-01"].inputTokens).toBe(40);
    expect(summary.byPeriod["2024-01-02"].inputTokens).toBe(5);
    expect(summary.overall.sessions).toBe(3);
  });

  test("deduplicates by idempotency key", async () => {
    const records = await runWithBun(
      Effect.gen(function* () {
        const fs = yield* FileSystem.FileSystem;
        const dir = yield* fs.makeTempDirectory({ prefix: "usage-dedupe" });

        const record = sampleRecord({ idempotencyKey: "key-1" });
        yield* appendUsageRecord({ rootDir: dir, record });
        yield* appendUsageRecord({ rootDir: dir, record });

        return yield* readUsageRecords(dir);
      }),
    );

    expect(records).toHaveLength(1);
    expect(records[0].idempotencyKey).toBe("key-1");
  });

  test("auto-generated idempotency key prevents duplicate append", async () => {
    const records = await runWithBun(
      Effect.gen(function* () {
        const fs = yield* FileSystem.FileSystem;
        const dir = yield* fs.makeTempDirectory({ prefix: "usage-auto-dedupe" });

        const record = sampleRecord({ idempotencyKey: undefined });
        yield* appendUsageRecord({ rootDir: dir, record });
        yield* appendUsageRecord({ rootDir: dir, record });

        return yield* readUsageRecords(dir);
      }),
    );

    expect(records).toHaveLength(1);
    expect(records[0].idempotencyKey).toBeDefined();
  });
});
