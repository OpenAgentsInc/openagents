import { describe, expect, test } from "vite-plus/test";
import { Effect, Result } from "effect";

import { compileCodeShareBundle } from "./ide-code-share-builder.js";
import {
  CodeShareCommandOutcome,
  CodeShareViewOutcome,
  CodeShareViewer,
  CodeShareViewerLayer,
  rejectCodeShareCommand,
  resolveCodeShareBundle,
} from "./ide-code-share-viewer.js";
import { CodeShareDigestService } from "./ide-code-share-builder.js";

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

const bundle = () => {
  const result = compileCodeShareBundle(
    {
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
      candidates: [
        {
          entryRef: "entry.1",
          pathLabel: "src/one.ts",
          languageRef: "language.typescript",
          sourceGeneration: 4,
          classification: "text",
          allowlisted: true,
          content: "export const one = 1\n",
        },
      ],
      retainDays: 30,
      lifetimeSeconds: 604_800,
      createdAt: "2026-07-21T00:00:00.000Z",
    },
    port,
  );
  if (Result.isFailure(result)) {
    throw new Error(`build failed: ${result.failure.reason}`);
  }
  return result.success;
};

// createdAt + 7d = 2026-07-28; deleteAfter = +30d = 2026-08-27.

describe("CodeShareViewer renders a fixed read-only snapshot", () => {
  test("renders a valid bundle within its lifetime", () => {
    const outcome = resolveCodeShareBundle(bundle(), "2026-07-22T00:00:00.000Z", false, port);
    expect(outcome._tag).toBe("Rendered");
    if (CodeShareViewOutcome.guards.Rendered(outcome)) {
      expect(outcome.bundle.entryCount).toBe(1);
    }
  });

  test("reports tampered when a frozen entry is edited", () => {
    const original = bundle();
    const tampered = {
      ...original,
      entries: [{ ...original.entries[0], content: "export const two = 2\n" }],
    };
    const outcome = resolveCodeShareBundle(tampered, "2026-07-22T00:00:00.000Z", false, port);
    expect(outcome._tag).toBe("Unavailable");
    if (CodeShareViewOutcome.guards.Unavailable(outcome)) {
      expect(outcome.reason).toBe("tampered");
    }
  });

  test("reports revoked when the origin revokes the share", () => {
    const outcome = resolveCodeShareBundle(bundle(), "2026-07-22T00:00:00.000Z", true, port);
    expect(outcome._tag).toBe("Unavailable");
    if (CodeShareViewOutcome.guards.Unavailable(outcome)) {
      expect(outcome.reason).toBe("revoked");
    }
  });

  test("reports expired after the expiry", () => {
    const outcome = resolveCodeShareBundle(bundle(), "2026-07-29T00:00:00.000Z", false, port);
    expect(outcome._tag).toBe("Unavailable");
    if (CodeShareViewOutcome.guards.Unavailable(outcome)) {
      expect(outcome.reason).toBe("expired");
    }
  });

  test("reports deleted after the retention deletion time", () => {
    const outcome = resolveCodeShareBundle(bundle(), "2026-09-01T00:00:00.000Z", false, port);
    expect(outcome._tag).toBe("Unavailable");
    if (CodeShareViewOutcome.guards.Unavailable(outcome)) {
      expect(outcome.reason).toBe("deleted");
    }
  });

  test("reports invalid_bundle on undecodable input", () => {
    const outcome = resolveCodeShareBundle(
      { not: "a bundle" },
      "2026-07-22T00:00:00.000Z",
      false,
      port,
    );
    expect(outcome._tag).toBe("Unavailable");
    if (CodeShareViewOutcome.guards.Unavailable(outcome)) {
      expect(outcome.reason).toBe("invalid_bundle");
    }
  });
});

describe("no-write-back law: a public share never accepts a command", () => {
  test("the command outcome type has exactly the rejected case", () => {
    const outcome = rejectCodeShareCommand();
    expect(outcome._tag).toBe("Rejected");
    expect(outcome.reason).toBe("public_share_read_only");
    expect(Object.keys(CodeShareCommandOutcome.cases)).toEqual(["Rejected"]);
  });

  test("the viewer service rejects every command payload", async () => {
    const outcome = await Effect.runPromise(
      Effect.gen(function* () {
        const viewer = yield* CodeShareViewer;
        return yield* viewer.command({ intent: "approve", proposalRef: "proposal.1" });
      }).pipe(
        Effect.provide(CodeShareViewerLayer),
        Effect.provideService(CodeShareDigestService, { digest: testDigest }),
      ),
    );
    expect(outcome._tag).toBe("Rejected");
    expect(outcome.reason).toBe("public_share_read_only");
  });

  test("the viewer service renders through the injected digest port", async () => {
    const outcome = await Effect.runPromise(
      Effect.gen(function* () {
        const viewer = yield* CodeShareViewer;
        return yield* viewer.resolve(bundle(), "2026-07-22T00:00:00.000Z", false);
      }).pipe(
        Effect.provide(CodeShareViewerLayer),
        Effect.provideService(CodeShareDigestService, { digest: testDigest }),
      ),
    );
    expect(outcome._tag).toBe("Rendered");
  });
});
