import { Effect } from "effect";
import { describe, expect, it } from "vite-plus/test";

import {
  ALL_WORK_BUILDERS,
  ALL_WORK_FIXTURE_FACTORIES,
  ALL_WORK_PRODUCTION_METHODS,
  makeAllWorkClient,
} from "../src/client.generated.ts";

describe("generated All Work TypeScript client", () => {
  it("exports implemented methods, strict builders, and fresh fixture factories", async () => {
    expect(Object.keys(ALL_WORK_PRODUCTION_METHODS)).toContain("work.index.subscribe");
    expect(Object.keys(ALL_WORK_PRODUCTION_METHODS)).not.toContain("view.read");
    const first = ALL_WORK_FIXTURE_FACTORIES["valid/work-summary.json"]();
    const second = ALL_WORK_FIXTURE_FACTORIES["valid/work-summary.json"]();
    expect(first).toEqual(second);
    expect(first).not.toBe(second);
    expect(() => ALL_WORK_BUILDERS.WorkIndexReadRequest({ unknown: true })).toThrow();

    const client = makeAllWorkClient(() =>
      Effect.succeed({
        ok: true,
        result: ALL_WORK_BUILDERS.WorkIndexReadResult({
          items: [first],
          nextCursor: null,
          completeness: { state: "complete", cursor: null, gapRefs: [] },
          generatedAt: "2026-08-03T13:00:00Z",
        }),
      }),
    );
    const result = await Effect.runPromise(client.workIndexRead({}));
    expect(result.items[0]?.workRef).toBe(first.workRef);
  });

  it("returns a typed retryable protocol error and rejects malformed success payloads", async () => {
    const failed = makeAllWorkClient(() =>
      Effect.succeed({
        ok: false,
        error: {
          code: "unavailable",
          message: "The reference process is unavailable.",
          retryable: true,
          minimumVersion: null,
        },
      }),
    );
    const protocolError = await Effect.runPromise(failed.workIndexRead({}).pipe(Effect.flip));
    expect(protocolError).toMatchObject({
      stage: "protocol",
      protocolError: { code: "unavailable", retryable: true },
    });

    const malformed = makeAllWorkClient(() => Effect.succeed({ ok: true, result: {} }));
    const responseError = await Effect.runPromise(malformed.workIndexRead({}).pipe(Effect.flip));
    expect(responseError).toMatchObject({ stage: "response" });
  });
});
