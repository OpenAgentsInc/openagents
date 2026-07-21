import { Effect, Ref } from "effect";
import { describe, expect, test } from "vite-plus/test";

import { makeLocalSandboxProvider } from "./local-sandbox-provider.ts";
import type { HarnessSandboxSession } from "./sandbox.ts";

describe("harness sandbox provider — session workspace", () => {
  test("createSession composes the working directory under the base and applies a bootstrap file", async () => {
    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const provider = makeLocalSandboxProvider({ baseDirectory: "/harness" });
        const session = yield* provider.createSession({
          sessionId: "s1",
          identity: "id-a",
          bootstrap: {
            identity: "id-a",
            files: [{ path: "bridge.txt", content: "ready" }],
          },
        });
        const bridge = yield* session.readTextFile({ path: "bridge.txt" });
        return { workingDirectory: session.workingDirectory, bridge };
      }),
    );

    // The framework composes `<base>/<sessionId>`.
    expect(result.workingDirectory).toBe("/harness/s1");
    // The bootstrap file is written into the workspace and readable afterward.
    expect(result.bridge).toBe("ready");
  });

  test("writeTextFile / readTextFile round-trip", async () => {
    const content = await Effect.runPromise(
      Effect.gen(function* () {
        const provider = makeLocalSandboxProvider();
        const session = yield* provider.createSession({ sessionId: "s1" });
        yield* session.writeTextFile({ path: "notes/plan.md", content: "step one" });
        return yield* session.readTextFile({ path: "notes/plan.md" });
      }),
    );
    expect(content).toBe("step one");
  });
});

describe("harness sandbox provider — snapshot identity", () => {
  test("onFirstCreate runs exactly once per identity, and again for a different identity", async () => {
    const runs = await Effect.runPromise(
      Effect.gen(function* () {
        const provider = makeLocalSandboxProvider();
        const counter = yield* Ref.make(0);
        const onFirstCreate = (_session: HarnessSandboxSession) =>
          Ref.update(counter, (n) => n + 1);

        // Two createSession calls with the SAME identity — onFirstCreate runs once.
        yield* provider.createSession({ sessionId: "s1", identity: "id-a", onFirstCreate });
        const afterSameIdentity = yield* Ref.get(counter);
        yield* provider.createSession({ sessionId: "s2", identity: "id-a", onFirstCreate });
        const afterSecondSameIdentity = yield* Ref.get(counter);

        // A DIFFERENT identity runs onFirstCreate again.
        yield* provider.createSession({ sessionId: "s3", identity: "id-b", onFirstCreate });
        const afterOtherIdentity = yield* Ref.get(counter);

        return { afterSameIdentity, afterSecondSameIdentity, afterOtherIdentity };
      }),
    );

    expect(runs.afterSameIdentity).toBe(1);
    // The second same-identity create must NOT re-run onFirstCreate.
    expect(runs.afterSecondSameIdentity).toBe(1);
    // A fresh identity re-runs it exactly once more.
    expect(runs.afterOtherIdentity).toBe(2);
  });
});

describe("harness sandbox provider — fail-closed capability posture", () => {
  test("getPortUrl is absent on the local double (no port capability)", async () => {
    const hasPortUrl = await Effect.runPromise(
      Effect.gen(function* () {
        const provider = makeLocalSandboxProvider();
        const session = yield* provider.createSession({ sessionId: "s1" });
        return session.getPortUrl !== undefined;
      }),
    );
    // Absence of the optional method IS the "no port capability" signal.
    expect(hasPortUrl).toBe(false);
  });
});

describe("harness sandbox provider — resume rehydrates the same workspace", () => {
  test("a file written before resume is readable after resumeSession", async () => {
    const content = await Effect.runPromise(
      Effect.gen(function* () {
        const provider = makeLocalSandboxProvider();
        const created = yield* provider.createSession({ sessionId: "s1" });
        yield* created.writeTextFile({ path: "state.txt", content: "persisted" });

        // resumeSession is present on this provider and rebinds the same vfs.
        const resumed = yield* provider.resumeSession!({ sessionId: "s1" });
        return yield* resumed.readTextFile({ path: "state.txt" });
      }),
    );
    expect(content).toBe("persisted");
  });
});
