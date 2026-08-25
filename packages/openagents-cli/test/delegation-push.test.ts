import { Effect, Redacted } from "effect";
import { chmodSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { pushDelegated, redactSecret, validateRefspec } from "../src/delegation-push.js";

const CANARY = "oa_assignment_canary_12345";

const makeFakeGit = (opts: {
  readonly fail?: boolean;
  readonly stderr?: string;
}): { readonly dir: string; readonly log: string; readonly originalPath: string } => {
  const dir = mkdtempSync(join(tmpdir(), "oa-fake-git-"));
  const log = join(dir, "git.log");
  const script = `#!/bin/sh
CANARY="${CANARY}"
for arg in "$@"; do
  if [ "$arg" = "get-url" ]; then
    echo "https://openagents.com/owner/repo.git"
    exit 0
  fi
  if [ "$arg" = "push" ]; then
    for a in "$@"; do
      printf "%s\\n" "$a" >> ${JSON.stringify(log)}
    done
    printf "%s\\n" "---" >> ${JSON.stringify(log)}
    if [ -n "${opts.stderr ?? ""}" ]; then
      printf "%s\\n" "${opts.stderr ?? ""}" >&2
    fi
    exit ${opts.fail ? 1 : 0}
  fi
done
exit 1
`;
  writeFileSync(join(dir, "git"), script, { mode: 0o755 });
  chmodSync(join(dir, "git"), 0o755);
  return { dir, log, originalPath: process.env.PATH ?? "" };
};

const setPath = (dir: string): void => {
  process.env.PATH = `${dir}:${process.env.PATH ?? ""}`;
};

const restorePath = (original: string): void => {
  process.env.PATH = original;
};

describe("delegated push refspec validation", () => {
  it("allows the assigned branch by short name, full ref, or matching src:dst", async () => {
    await expect(
      Effect.runPromise(validateRefspec("feature-1", "feature-1")),
    ).resolves.toBeUndefined();
    await expect(
      Effect.runPromise(validateRefspec("refs/heads/feature-1", "feature-1")),
    ).resolves.toBeUndefined();
    await expect(
      Effect.runPromise(validateRefspec("refs/heads/feature-1:refs/heads/feature-1", "feature-1")),
    ).resolves.toBeUndefined();
    await expect(
      Effect.runPromise(validateRefspec("feature-1:feature-1", "feature-1")),
    ).resolves.toBeUndefined();
  });

  it("refuses an unauthorized branch, a mismatched src:dst, and invalid forms", async () => {
    await expect(Effect.runPromise(validateRefspec("main", "feature-1"))).rejects.toThrow(
      /not the assigned branch/,
    );
    await expect(
      Effect.runPromise(validateRefspec("refs/heads/main", "feature-1")),
    ).rejects.toThrow(/not the assigned branch/);
    await expect(
      Effect.runPromise(validateRefspec("refs/heads/feature-1:refs/heads/main", "feature-1")),
    ).rejects.toThrow(/not the assigned branch/);
    await expect(Effect.runPromise(validateRefspec("refs/tags/v1", "feature-1"))).rejects.toThrow(
      /not the assigned branch/,
    );
    await expect(Effect.runPromise(validateRefspec("", "feature-1"))).rejects.toThrow(/empty/);
    await expect(Effect.runPromise(validateRefspec("+feature-1", "feature-1"))).rejects.toThrow(
      /Force or option/,
    );
    await expect(Effect.runPromise(validateRefspec("feature-1:", "feature-1"))).rejects.toThrow(
      /empty destination/,
    );
  });

  it("refuses a multi-ref push", async () => {
    await expect(Effect.runPromise(validateRefspec("feature-1 main", "feature-1"))).rejects.toThrow(
      /Multi-ref/,
    );
    await expect(Effect.runPromise(validateRefspec("feature-1,main", "feature-1"))).rejects.toThrow(
      /Multi-ref/,
    );
  });
});

describe("delegated push credential redaction", () => {
  it("redacts known credential patterns", () => {
    expect(redactSecret("token oa_assignment_canary_12345 here")).not.toContain(CANARY);
    expect(redactSecret("token oa_assignment_canary_12345 here")).toContain("[REDACTED]");
    const auth = redactSecret("Authorization: Bearer abc.def");
    expect(auth).not.toContain("abc.def");
    expect(auth).toContain("[REDACTED]");
    expect(redactSecret("api-key=secret")).toBe("[REDACTED]");
    expect(redactSecret("password=s3cr3t")).toBe("[REDACTED]");
  });
});

describe("delegated push lifecycle", () => {
  const pushDir = () => mkdtempSync(join(tmpdir(), "oa-delegation-test-root-"));

  const cleanupTemp = (): void => {
    for (const name of readdirSync(tmpdir())) {
      if (name.startsWith("oa-delegation-push-") || name.startsWith("oa-delegation-test-root-")) {
        try {
          rmSync(join(tmpdir(), name), { recursive: true, force: true });
        } catch {
          // ignore
        }
      }
    }
  };

  let fake: ReturnType<typeof makeFakeGit> | undefined;

  beforeEach(() => {
    cleanupTemp();
  });

  afterEach(() => {
    if (fake !== undefined) {
      restorePath(fake.originalPath);
      rmSync(fake.dir, { recursive: true, force: true });
      fake = undefined;
    }
    cleanupTemp();
  });

  it("pushes the assigned branch and cleans up the credential temp directory", async () => {
    fake = makeFakeGit({ fail: false });
    setPath(fake.dir);
    const directory = pushDir();
    await expect(
      Effect.runPromise(
        pushDelegated({
          directory,
          remote: "origin",
          refspec: "feature-1",
          repository: "owner/repo",
          branch: "feature-1",
          credential: Redacted.make(CANARY),
          origin: "https://openagents.com",
        }),
      ),
    ).resolves.toBeUndefined();
    const log = fake.log;
    const logged = readLog(log);
    expect(logged.join("\n")).not.toContain(CANARY);
    expect(logged).toContain("origin");
    expect(logged).toContain("feature-1");
    expect(logged).toContain("push");
    expect(logged.some((line) => line.startsWith("credential."))).toBe(true);
    expect(readdirSync(tmpdir()).some((n) => n.startsWith("oa-delegation-push-"))).toBe(false);
  });

  it("fails distinguishably for a ref mismatch before any git push", async () => {
    fake = makeFakeGit({ fail: false });
    setPath(fake.dir);
    const directory = pushDir();
    await expect(
      Effect.runPromise(
        pushDelegated({
          directory,
          remote: "origin",
          refspec: "main",
          repository: "owner/repo",
          branch: "feature-1",
          credential: Redacted.make(CANARY),
          origin: "https://openagents.com",
        }),
      ),
    ).rejects.toThrow(/not the assigned branch/);
  });

  it("fails distinguishably when the remote repository does not match the assignment", async () => {
    fake = makeFakeGit({ fail: false });
    setPath(fake.dir);
    const directory = pushDir();
    await expect(
      Effect.runPromise(
        pushDelegated({
          directory,
          remote: "origin",
          refspec: "feature-1",
          repository: "wrong/repo",
          branch: "feature-1",
          credential: Redacted.make(CANARY),
          origin: "https://openagents.com",
        }),
      ),
    ).rejects.toThrow(/not the assigned/);
  });

  it("fails with a redacted message and cleans up after a git push failure", async () => {
    fake = makeFakeGit({
      fail: true,
      stderr: `error: invalid token ${CANARY}`,
    });
    setPath(fake.dir);
    const directory = pushDir();
    let error: unknown;
    try {
      await Effect.runPromise(
        pushDelegated({
          directory,
          remote: "origin",
          refspec: "feature-1",
          repository: "owner/repo",
          branch: "feature-1",
          credential: Redacted.make(CANARY),
          origin: "https://openagents.com",
        }),
      );
    } catch (cause) {
      error = cause;
    }
    expect(error).toBeInstanceOf(Error);
    const message = error instanceof Error ? error.message : String(error);
    expect(message).toContain("git push failed");
    expect(message).not.toContain(CANARY);
    expect(message).toContain("[REDACTED]");
    expect(readdirSync(tmpdir()).some((n) => n.startsWith("oa-delegation-push-"))).toBe(false);
  });
});

const readLog = (path: string): ReadonlyArray<string> => {
  try {
    return readFileSync(path, "utf8")
      .split("\n")
      .filter((line) => line !== "");
  } catch {
    return [];
  }
};
