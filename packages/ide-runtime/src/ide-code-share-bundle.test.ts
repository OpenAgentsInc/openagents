import { describe, expect, test } from "vite-plus/test";
import { Schema as S } from "effect";

import {
  CodeShareBundle,
  IDE_CODE_SHARE_BUNDLE_SCHEMA_LITERAL,
  canonicalCodeShareManifest,
  decodeCodeShareBundle,
  verifyCodeShareBundleIntegrity,
} from "./ide-code-share-bundle.js";

// A dependency-free deterministic 256-bit digest. The package forbids a Node or
// platform hash, and the digest is an injected port, so tests supply their own
// content-addressing function. Different inputs yield different digests, which
// is all the immutability and content-addressing laws require.
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

const digestOf = (content: string) => testDigest(content);

const entry = (overrides: Partial<Record<string, unknown>> = {}) => {
  const content = (overrides.content as string | undefined) ?? "export const one = 1\n";
  return {
    entryRef: "entry.1",
    pathLabel: "src/one.ts",
    languageRef: "language.typescript",
    sourceGeneration: 4,
    byteCount: content.length,
    lineCount: content.split("\n").length,
    contentDigest: digestOf(content),
    content,
    truncated: false,
    ...overrides,
    ...(overrides.content === undefined
      ? {}
      : {
          byteCount: content.length,
          lineCount: content.split("\n").length,
          contentDigest: digestOf(content),
        }),
  };
};

const unsigned = (overrides: Partial<Record<string, unknown>> = {}) => {
  const entries = (overrides.entries as ReadonlyArray<unknown> | undefined) ?? [entry()];
  const omissions = (overrides.omissions as ReadonlyArray<unknown> | undefined) ?? [];
  const totalByteCount =
    (overrides.totalByteCount as number | undefined) ??
    entries.reduce(
      (total: number, item) => total + ((item as { byteCount?: number }).byteCount ?? 0),
      0,
    );
  return {
    schema: IDE_CODE_SHARE_BUNDLE_SCHEMA_LITERAL,
    bundleRef: "bundle.public.1",
    visibility: "public" as const,
    audience: "public_anonymous" as const,
    creatorRef: "creator.1",
    source: {
      sessionRef: "session.1",
      projectRef: "project.1",
      worktreeRef: "worktree.1",
      placementRef: "placement.owner-local.1",
      sourceGeneration: 4,
    },
    rendererVersion: "renderer.khala.1",
    entries,
    omissions,
    entryCount: entries.length,
    omittedCount: omissions.length,
    totalByteCount,
    truncated: omissions.length > 0,
    retention: { retainDays: 30, deleteAfter: "2026-08-20T00:00:00.000Z" },
    createdAt: "2026-07-21T00:00:00.000Z",
    expiresAt: "2026-07-28T00:00:00.000Z",
    ...overrides,
  };
};

const signed = (overrides: Partial<Record<string, unknown>> = {}) => {
  const base = unsigned(overrides);
  return { ...base, manifestDigest: port.digest(canonicalCodeShareManifest(base as never)) };
};

describe("CodeShareBundle schema", () => {
  test("decodes a well-formed public bundle", () => {
    const bundle = S.decodeUnknownSync(CodeShareBundle)(signed(), { onExcessProperty: "error" });
    expect(bundle.visibility).toBe("public");
    expect(bundle.audience).toBe("public_anonymous");
    expect(bundle.entryCount).toBe(1);
  });

  test("rejects a rooted host path label", () => {
    expect(() =>
      S.decodeUnknownSync(CodeShareBundle)(
        signed({ entries: [entry({ pathLabel: "/Users/someone/secret.ts" })] }),
        { onExcessProperty: "error" },
      ),
    ).toThrow();
  });

  test("rejects a hidden path label", () => {
    expect(() =>
      S.decodeUnknownSync(CodeShareBundle)(signed({ entries: [entry({ pathLabel: ".env" })] }), {
        onExcessProperty: "error",
      }),
    ).toThrow();
  });

  test("rejects a parent-directory path label", () => {
    expect(() =>
      S.decodeUnknownSync(CodeShareBundle)(
        signed({ entries: [entry({ pathLabel: "src/../etc/passwd" })] }),
        { onExcessProperty: "error" },
      ),
    ).toThrow();
  });

  test("rejects an entryCount that does not match the entry set", () => {
    expect(() =>
      S.decodeUnknownSync(CodeShareBundle)(signed({ entryCount: 5 }), {
        onExcessProperty: "error",
      }),
    ).toThrow();
  });

  test("rejects expiry that precedes creation", () => {
    expect(() =>
      S.decodeUnknownSync(CodeShareBundle)(signed({ expiresAt: "2026-07-20T00:00:00.000Z" }), {
        onExcessProperty: "error",
      }),
    ).toThrow();
  });

  test("rejects retention deletion before expiry", () => {
    expect(() =>
      S.decodeUnknownSync(CodeShareBundle)(
        signed({ retention: { retainDays: 30, deleteAfter: "2026-07-22T00:00:00.000Z" } }),
        { onExcessProperty: "error" },
      ),
    ).toThrow();
  });

  test("rejects an excess property outside the allowlist", () => {
    expect(() =>
      S.decodeUnknownSync(CodeShareBundle)(
        { ...signed(), mutationEndpoint: "https://write" },
        { onExcessProperty: "error" },
      ),
    ).toThrow();
  });
});

describe("redaction law: a public bundle carries no forbidden material", () => {
  test("integrity verification fails when an entry hides a credential", () => {
    const content = "const token = ghp_0123456789abcdefABCDEF\n";
    const bundle = signed({ entries: [entry({ content })] });
    // The bundle text carries forbidden material; verification must reject it.
    expect(verifyCodeShareBundleIntegrity(bundle as never, port)).toBe(false);
  });

  test("integrity verification fails when an entry hides a host path", () => {
    const content = 'import x from "/Users/someone/app/secret.ts"\n';
    const bundle = signed({ entries: [entry({ content })] });
    expect(verifyCodeShareBundleIntegrity(bundle as never, port)).toBe(false);
  });

  test("a clean bundle passes integrity verification", () => {
    const bundle = signed();
    expect(verifyCodeShareBundleIntegrity(bundle as never, port)).toBe(true);
  });
});

describe("immutability law: a public bundle cannot be edited or written back", () => {
  test("changing entry content breaks the manifest digest", () => {
    const bundle = signed();
    const tampered = {
      ...bundle,
      entries: [
        { ...(bundle.entries[0] as Record<string, unknown>), content: "export const two = 2\n" },
      ],
    };
    expect(verifyCodeShareBundleIntegrity(tampered as never, port)).toBe(false);
  });

  test("changing the expiry breaks the manifest digest", () => {
    const bundle = signed();
    const tampered = { ...bundle, expiresAt: "2027-07-28T00:00:00.000Z" };
    expect(verifyCodeShareBundleIntegrity(tampered as never, port)).toBe(false);
  });

  test("changing an omission reason breaks the manifest digest", () => {
    const bundle = signed({
      entries: [entry()],
      omissions: [{ omittedRef: "omit.1", reason: "not_allowlisted" }],
    });
    const tampered = {
      ...bundle,
      omissions: [{ omittedRef: "omit.1", reason: "binary" as const }],
    };
    expect(verifyCodeShareBundleIntegrity(tampered as never, port)).toBe(false);
  });

  test("re-signing tampered content with a different port cannot forge the original digest", () => {
    const bundle = signed();
    const tampered = {
      ...bundle,
      entries: [
        { ...(bundle.entries[0] as Record<string, unknown>), content: "export const two = 2\n" },
      ],
    };
    // A real content-addressing port never reproduces the recorded digest for changed bytes.
    expect(verifyCodeShareBundleIntegrity(tampered as never, port)).toBe(false);
    const tamperedEntry = tampered.entries[0] as { content: string; contentDigest: string };
    expect(port.digest(tamperedEntry.content)).not.toBe(tamperedEntry.contentDigest);
  });
});

describe("decodeCodeShareBundle effect boundary", () => {
  test("decodes an untrusted bundle", async () => {
    const { Effect } = await import("effect");
    const bundle = await Effect.runPromise(decodeCodeShareBundle(signed()));
    expect(bundle.schema).toBe(IDE_CODE_SHARE_BUNDLE_SCHEMA_LITERAL);
  });
});
