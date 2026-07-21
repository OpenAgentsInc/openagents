import { rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { Effect } from "effect";
import { describe, expect, test } from "vite-plus/test";

import { makeLocalProcessSandboxProvider } from "./local-process-sandbox-provider.ts";

const baseFor = (name: string) => join(tmpdir(), `harness-local-sandbox-${name}`);

describe("HARN-07 local-process sandbox provider (real fs + child_process)", () => {
  test("composes the session working directory and round-trips a real file", async () => {
    const baseDir = baseFor("roundtrip");
    await rm(baseDir, { recursive: true, force: true });
    const out = await Effect.runPromise(
      Effect.gen(function* () {
        const provider = makeLocalProcessSandboxProvider({ baseDir });
        const session = yield* provider.createSession({ sessionId: "s1" });
        yield* session.writeTextFile({ path: "notes/hello.txt", content: "hi harness" });
        const read = yield* session.readTextFile({ path: "notes/hello.txt" });
        return {
          workingDirectory: session.workingDirectory,
          read,
          hasPort: session.getPortUrl !== undefined,
        };
      }),
    );
    expect(out.workingDirectory).toBe(join(baseDir, "s1"));
    expect(out.read).toBe("hi harness");
    // No port capability on the local provider.
    expect(out.hasPort).toBe(false);
    await rm(baseDir, { recursive: true, force: true });
  });

  test("runs a real host command and reports stdout + a nonzero exit honestly", async () => {
    const baseDir = baseFor("run");
    await rm(baseDir, { recursive: true, force: true });
    const out = await Effect.runPromise(
      Effect.gen(function* () {
        const provider = makeLocalProcessSandboxProvider({ baseDir });
        const session = yield* provider.createSession({ sessionId: "s1" });
        const ok = yield* session.run({ command: "echo harness-ran" });
        const bad = yield* session.run({ command: "false" });
        return { ok, bad };
      }),
    );
    expect(out.ok.stdout.trim()).toBe("harness-ran");
    expect(out.ok.exitCode).toBe(0);
    expect(out.bad.exitCode).not.toBe(0);
    await rm(baseDir, { recursive: true, force: true });
  });

  test("applies a bootstrap once per identity", async () => {
    const baseDir = baseFor("bootstrap");
    await rm(baseDir, { recursive: true, force: true });
    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const provider = makeLocalProcessSandboxProvider({ baseDir });
        let firstCreateCalls = 0;
        const bootstrap = {
          identity: "img-a",
          files: [{ path: "bridge.txt", content: "bridge-ready" }],
        };
        const onFirstCreate = () =>
          Effect.sync(() => {
            firstCreateCalls += 1;
          });

        const s1 = yield* provider.createSession({
          sessionId: "s1",
          identity: "img-a",
          bootstrap,
          onFirstCreate,
        });
        const bridge = yield* s1.readTextFile({ path: "bridge.txt" });
        // Same identity again: onFirstCreate must NOT run a second time.
        yield* provider.createSession({
          sessionId: "s2",
          identity: "img-a",
          bootstrap,
          onFirstCreate,
        });
        // A different identity runs it again.
        yield* provider.createSession({
          sessionId: "s3",
          identity: "img-b",
          bootstrap,
          onFirstCreate,
        });
        return { bridge, firstCreateCalls };
      }),
    );
    expect(result.bridge).toBe("bridge-ready");
    expect(result.firstCreateCalls).toBe(2); // img-a once, img-b once
    await rm(baseDir, { recursive: true, force: true });
  });

  test("refuses a path that escapes the session workspace", async () => {
    const baseDir = baseFor("escape");
    await rm(baseDir, { recursive: true, force: true });
    const exit = await Effect.runPromiseExit(
      Effect.gen(function* () {
        const provider = makeLocalProcessSandboxProvider({ baseDir });
        const session = yield* provider.createSession({ sessionId: "s1" });
        yield* session.writeTextFile({ path: "../escape.txt", content: "nope" });
      }),
    );
    expect(exit._tag).toBe("Failure");
    await rm(baseDir, { recursive: true, force: true });
  });
});
