import { describe, expect, test } from "vite-plus/test";
import { Effect, Result } from "effect";

import {
  CodeShareBuilder,
  CodeShareBuilderLayer,
  CodeShareDigestService,
  compileCodeShareBundle,
} from "./ide-code-share-builder.js";
import { verifyCodeShareBundleIntegrity } from "./ide-code-share-bundle.js";

// A dependency-free deterministic 256-bit digest injected as the content port.
const testDigest = (value: string): string => {
  const seeds = [
    0x811c9dc5, 0x01000193, 0xdeadbeef, 0xcafebabe, 0xa5a5a5a5, 0x5a5a5a5a, 0x12345678, 0x9abcdef0,
  ];
  let out = "";
  for (const seed of seeds) {
    let hash = seed >>> 0;
    for (let index = 0; index < value.length; index += 1) {
      hash ^= value.charCodeAt(index);
      hash = Math.imul(hash, 0x01000193) >>> 0;
    }
    out += hash.toString(16).padStart(8, "0");
  }
  return out;
};
const port = { digest: testDigest };

const candidate = (overrides: Partial<Record<string, unknown>> = {}) => ({
  entryRef: "entry.1",
  pathLabel: "src/one.ts",
  languageRef: "language.typescript",
  sourceGeneration: 4,
  classification: "text" as const,
  allowlisted: true,
  content: "export const one = 1\n",
  ...overrides,
});

const request = (candidates: ReadonlyArray<unknown>) => ({
  bundleRef: "bundle.public.1",
  creatorRef: "creator.1",
  rendererVersion: "renderer.khala.1",
  source: {
    sessionRef: "session.1",
    projectRef: "project.1",
    worktreeRef: "worktree.1",
    placementRef: "placement.owner-local.1",
    sourceGeneration: 4,
  },
  candidates,
  retainDays: 30,
  lifetimeSeconds: 604_800,
  createdAt: "2026-07-21T00:00:00.000Z",
});

const buildOrThrow = (candidates: ReadonlyArray<unknown>) => {
  const result = compileCodeShareBundle(request(candidates), port);
  if (Result.isFailure(result)) {
    throw new Error(`unexpected build failure: ${result.failure.reason}`);
  }
  return result.success;
};

describe("CodeShareBuilder publishes only allowlisted text", () => {
  test("publishes an allowlisted text candidate and passes integrity", () => {
    const bundle = buildOrThrow([candidate()]);
    expect(bundle.entryCount).toBe(1);
    expect(bundle.omittedCount).toBe(0);
    expect(verifyCodeShareBundleIntegrity(bundle, port)).toBe(true);
    expect(bundle.entries[0].contentDigest).toBe(testDigest("export const one = 1\n"));
  });

  test("omits a non-allowlisted candidate as not_allowlisted", () => {
    const bundle = buildOrThrow([candidate({ entryRef: "entry.2", allowlisted: false })]);
    expect(bundle.entryCount).toBe(0);
    expect(bundle.omissions).toEqual([{ omittedRef: "entry.2", reason: "not_allowlisted" }]);
    expect(bundle.truncated).toBe(true);
  });

  test("omits binary, ignored, hidden, and private classifications", () => {
    const bundle = buildOrThrow([
      candidate({ entryRef: "entry.b", classification: "binary" }),
      candidate({ entryRef: "entry.i", classification: "ignored" }),
      candidate({ entryRef: "entry.h", classification: "hidden" }),
      candidate({ entryRef: "entry.p", classification: "private" }),
    ]);
    expect(bundle.entryCount).toBe(0);
    expect(bundle.omissions.map((omission) => omission.reason).sort()).toEqual([
      "binary",
      "hidden",
      "ignored",
      "private",
    ]);
  });

  test("omits an allowlisted candidate that hides a credential", () => {
    const bundle = buildOrThrow([
      candidate({ entryRef: "secret", content: "const k = ghp_0123456789abcdefABCDEF\n" }),
    ]);
    expect(bundle.entryCount).toBe(0);
    expect(bundle.omissions).toEqual([{ omittedRef: "secret", reason: "forbidden_material" }]);
    // The published surface never carries the forbidden bytes.
    expect(verifyCodeShareBundleIntegrity(bundle, port)).toBe(true);
  });
});

describe("non-widening law: published set is a forbidden-free subset of the allowlist", () => {
  test("across a mixed candidate set, published refs are allowlisted and forbidden-free", () => {
    const candidates = [
      candidate({ entryRef: "entry.a", allowlisted: true, classification: "text" }),
      candidate({ entryRef: "entry.b", allowlisted: false, classification: "text" }),
      candidate({ entryRef: "entry.c", allowlisted: true, classification: "binary" }),
      candidate({
        entryRef: "entry.d",
        allowlisted: true,
        classification: "text",
        content: "password = supersecretvalue\n",
      }),
      candidate({ entryRef: "entry.e", allowlisted: true, classification: "text" }),
    ];
    const bundle = buildOrThrow(candidates);
    const publishedRefs = bundle.entries.map((entry) => entry.entryRef);
    const allowlistedRefs = candidates
      .filter((item) => item.allowlisted)
      .map((item) => item.entryRef);
    // Every published ref was allowlisted at the authoritative source.
    for (const ref of publishedRefs) {
      expect(allowlistedRefs).toContain(ref);
    }
    // The non-allowlisted candidate is never published, even though it was text.
    expect(publishedRefs).not.toContain("entry.b");
    // The forbidden candidate is never published, even though it was allowlisted text.
    expect(publishedRefs).not.toContain("entry.d");
    expect([...publishedRefs].sort()).toEqual(["entry.a", "entry.e"]);
    expect(verifyCodeShareBundleIntegrity(bundle, port)).toBe(true);
  });

  test("a fully forbidden or non-allowlisted set publishes an empty, honest bundle", () => {
    const bundle = buildOrThrow([
      candidate({ entryRef: "entry.a", allowlisted: false }),
      candidate({ entryRef: "entry.b", content: "secret: aws_credentials_here\n" }),
    ]);
    expect(bundle.entries).toEqual([]);
    expect(bundle.omittedCount).toBe(2);
    expect(verifyCodeShareBundleIntegrity(bundle, port)).toBe(true);
  });
});

describe("CodeShareBuilder failures", () => {
  test("fails invalid_request on a malformed candidate", () => {
    const result = compileCodeShareBundle(request([{ entryRef: "x", notAField: true }]), port);
    expect(Result.isFailure(result)).toBe(true);
    if (Result.isFailure(result)) {
      expect(result.failure.reason).toBe("invalid_request");
    }
  });

  test("fails bundle_too_large when total bytes exceed the cap", () => {
    const big = "a".repeat(200_000) + "\n";
    const candidates = Array.from({ length: 45 }, (_, index) =>
      candidate({ entryRef: `entry.${index}`, pathLabel: `src/f${index}.ts`, content: big }),
    );
    const result = compileCodeShareBundle(request(candidates), port);
    expect(Result.isFailure(result)).toBe(true);
    if (Result.isFailure(result)) {
      expect(result.failure.reason).toBe("bundle_too_large");
    }
  });
});

describe("CodeShareBuilder Effect layer", () => {
  test("builds through the injected digest service", async () => {
    const bundle = await Effect.runPromise(
      Effect.gen(function* () {
        const builder = yield* CodeShareBuilder;
        return yield* builder.build(request([candidate()]));
      }).pipe(
        Effect.provide(CodeShareBuilderLayer),
        Effect.provideService(CodeShareDigestService, { digest: testDigest }),
      ),
    );
    expect(bundle.entryCount).toBe(1);
    expect(verifyCodeShareBundleIntegrity(bundle, port)).toBe(true);
  });
});
