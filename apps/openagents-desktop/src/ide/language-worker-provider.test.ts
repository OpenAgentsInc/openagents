import { describe, expect, test } from "vite-plus/test";

import { makeIdeLanguageRequestFixture, makeIdeLanguageResultFixture } from "./language-fixture.ts";
import { makeIdeLanguageWorkerProvider } from "./language-worker-provider.ts";
import type {
  IdePortableMutationAuthority,
  IdePortableMutationPermit,
} from "./portable-mutation-authority.ts";

class FakeLanguageUtilityWorker {
  readonly messages: unknown[] = [];
  terminations = 0;
  private messageListener: ((value: unknown) => void) | null = null;
  private errorListener: ((error: Error) => void) | null = null;
  private exitListener: ((code: number) => void) | null = null;

  postMessage(value: unknown): void {
    this.messages.push(value);
  }

  onMessage(listener: (value: unknown) => void): void {
    this.messageListener = listener;
  }

  onceError(listener: (error: Error) => void): void {
    this.errorListener = listener;
  }

  onceExit(listener: (code: number) => void): void {
    this.exitListener = listener;
  }

  async terminate(): Promise<number> {
    this.terminations += 1;
    return 0;
  }

  emitMessage(value: unknown): void {
    this.messageListener?.(value);
  }
}

const permit: IdePortableMutationPermit = {
  _tag: "Portable",
  key: "portable:workspace.grant.language:session:context:attachment:7:target",
  grantRef: "workspace.grant.language",
  sessionRef: "session",
  workContextRef: "context",
  attachmentRef: "attachment",
  generation: 7,
  targetRef: "target",
};

const mutableAuthority = (initiallyCurrent: boolean) => {
  let current = initiallyCurrent;
  const authority: IdePortableMutationAuthority = {
    authorize: (grantRef) =>
      grantRef === permit.grantRef && current
        ? { _tag: "Permitted", permit }
        : { _tag: "Refused", reason: "attachment_ambiguous" },
    reauthorize: (candidate) => current && candidate.key === permit.key,
  };
  return {
    authority,
    revoke: () => {
      current = false;
    },
  };
};

describe("IDE language worker portable authority", () => {
  test("refuses initial authority without spawning a worker", async () => {
    const gate = mutableAuthority(false);
    let spawns = 0;
    const provider = makeIdeLanguageWorkerProvider(
      "/fixture",
      new URL("file:///fixture/language-worker.js"),
      permit.grantRef,
      gate.authority,
      () => {
        spawns += 1;
        return new FakeLanguageUtilityWorker();
      },
    );

    await expect(provider.start()).rejects.toThrow("authority was refused");
    expect(spawns).toBe(0);
  });

  test("tears down exactly once when authority changes during spawn", async () => {
    const gate = mutableAuthority(true);
    const worker = new FakeLanguageUtilityWorker();
    const provider = makeIdeLanguageWorkerProvider(
      "/fixture",
      new URL("file:///fixture/language-worker.js"),
      permit.grantRef,
      gate.authority,
      () => {
        gate.revoke();
        return worker;
      },
    );

    await expect(provider.start()).rejects.toThrow("authority changed during worker spawn");
    await Promise.resolve();
    expect(worker.terminations).toBe(1);
    expect(worker.messages).toEqual([{ kind: "stop" }]);
  });

  test("suppresses a late result after revocation and tears down the worker", async () => {
    const gate = mutableAuthority(true);
    const worker = new FakeLanguageUtilityWorker();
    const provider = makeIdeLanguageWorkerProvider(
      "/fixture",
      new URL("file:///fixture/language-worker.js"),
      permit.grantRef,
      gate.authority,
      () => worker,
    );

    const start = provider.start();
    worker.emitMessage({ kind: "ready", providerVersion: "6.0.3" });
    await start;
    const request = makeIdeLanguageRequestFixture("portable-revoked");
    const result = provider.request(request);
    await Promise.resolve();
    gate.revoke();
    worker.emitMessage({ kind: "result", result: makeIdeLanguageResultFixture(request) });

    await expect(result).rejects.toThrow("authority changed before a worker event");
    await Promise.resolve();
    expect(worker.terminations).toBe(1);
    expect(
      worker.messages.filter(
        (message) =>
          typeof message === "object" &&
          message !== null &&
          "kind" in message &&
          message.kind === "stop",
      ),
    ).toHaveLength(1);
  });
});
