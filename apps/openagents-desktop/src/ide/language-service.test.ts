import { Effect, Exit } from "effect";
import { describe, expect, test } from "vite-plus/test";

import {
  IdeLanguageStopRequestSchema,
  type IdeLanguageCancelRequest,
  type IdeLanguageRequest,
} from "./language-contract.ts";
import { ideLanguageProviderFixture, makeIdeLanguageRequestFixture, makeIdeLanguageResultFixture } from "./language-fixture.ts";
import { makeIdeLanguageService, type IdeLanguageProvider } from "./language-service.ts";

const stopRequest = IdeLanguageStopRequestSchema.make({
  schemaVersion: "openagents.desktop.ide-language-stop.v1",
  grantRef: "workspace.grant.service",
  reason: "project_closed",
});

describe("IdeLanguageService", () => {
  test("starts lazily, returns exact provider evidence, and tears down", async () => {
    let starts = 0;
    let stops = 0;
    const provider: IdeLanguageProvider = {
      start: async () => { starts += 1; return ideLanguageProviderFixture(); },
      request: async request => makeIdeLanguageResultFixture(request),
      cancel: async () => {},
      stop: async () => { stops += 1; },
    };
    const service = await Effect.runPromise(makeIdeLanguageService(provider));
    expect((await Effect.runPromise(service.snapshot()))._tag).toBe("Unconfigured");
    expect(starts).toBe(0);
    const request = makeIdeLanguageRequestFixture("lazy");
    const result = await Effect.runPromise(service.request(request));
    expect(result).toMatchObject({
      requestRef: request.requestRef,
      documentGeneration: request.documentGeneration,
      documentVersion: request.documentVersion,
      evidenceTier: "project_local",
    });
    expect(starts).toBe(1);
    const stopped = await Effect.runPromise(service.stop(stopRequest));
    expect(stopped).toMatchObject({ _tag: "Stopped", activeRequests: 0, queuedRequests: 0 });
    expect(stops).toBe(1);
  });

  test("supersedes an older same-document capability and strips its late items", async () => {
    const pending = new Map<string, { request: IdeLanguageRequest; resolve: (value: unknown) => void }>();
    const cancellations: IdeLanguageCancelRequest[] = [];
    const provider: IdeLanguageProvider = {
      start: async () => ideLanguageProviderFixture(),
      request: request => new Promise(resolve => pending.set(String(request.requestRef), { request, resolve })),
      cancel: async request => { cancellations.push(request); },
      stop: async () => {},
    };
    const service = await Effect.runPromise(makeIdeLanguageService(provider));
    const older = makeIdeLanguageRequestFixture("older");
    const newer = { ...makeIdeLanguageRequestFixture("newer"), documentRef: older.documentRef };
    const olderRun = Effect.runPromise(service.request(older));
    while (!pending.has(String(older.requestRef))) await new Promise(resolve => setTimeout(resolve, 0));
    const newerRun = Effect.runPromise(service.request(newer));
    while (!pending.has(String(newer.requestRef))) await new Promise(resolve => setTimeout(resolve, 0));
    pending.get(String(newer.requestRef))!.resolve(makeIdeLanguageResultFixture(newer));
    pending.get(String(older.requestRef))!.resolve(makeIdeLanguageResultFixture(older));
    const [olderResult, newerResult] = await Promise.all([olderRun, newerRun]);
    expect(cancellations).toContainEqual(expect.objectContaining({ requestRef: older.requestRef, reason: "superseded" }));
    expect(olderResult).toMatchObject({ state: { _tag: "Stale" }, items: [] });
    expect(newerResult.state._tag).toBe("Complete");
  });

  test("reports malformed providers and bounded timeouts as typed failures", async () => {
    const malformed = await Effect.runPromise(makeIdeLanguageService({
      start: async () => ideLanguageProviderFixture(),
      request: async () => ({ nope: true }),
      cancel: async () => {},
      stop: async () => {},
    }));
    const malformedExit = await Effect.runPromiseExit(malformed.request(makeIdeLanguageRequestFixture("malformed")));
    expect(Exit.isFailure(malformedExit)).toBe(true);
    if (Exit.isFailure(malformedExit)) expect(String(malformedExit.cause)).toContain("MalformedResult");

    let cancelled = false;
    const timed = await Effect.runPromise(makeIdeLanguageService({
      start: async () => ideLanguageProviderFixture(),
      request: async () => await new Promise(() => {}),
      cancel: async () => { cancelled = true; },
      stop: async () => {},
    }));
    const request = { ...makeIdeLanguageRequestFixture("timeout"), timeoutMs: 50 };
    const timeoutExit = await Effect.runPromiseExit(timed.request(request));
    expect(Exit.isFailure(timeoutExit)).toBe(true);
    if (Exit.isFailure(timeoutExit)) expect(String(timeoutExit.cause)).toContain("TimedOut");
    expect(cancelled).toBe(true);
  });

  test("increments the service generation on supervised restart", async () => {
    let starts = 0;
    const provider: IdeLanguageProvider = {
      start: async () => { starts += 1; return ideLanguageProviderFixture(); },
      request: async request => makeIdeLanguageResultFixture(request),
      cancel: async () => {},
      stop: async () => {},
    };
    const service = await Effect.runPromise(makeIdeLanguageService(provider));
    await Effect.runPromise(service.request(makeIdeLanguageRequestFixture("restart")));
    const restarted = await Effect.runPromise(service.restart("fixture crash"));
    expect(restarted).toMatchObject({ _tag: "Ready", serviceGeneration: 2, restartCount: 1 });
    expect(starts).toBe(2);
  });
});
