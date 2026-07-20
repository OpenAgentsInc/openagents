import { describe, expect, test } from "vite-plus/test";
import { Schema as S } from "effect";

import {
  IDE_REVIEW_PROJECTION_SCHEMA_LITERAL,
  IdeReviewItem,
  IdeReviewProjection,
  MAX_IDE_REVIEW_EXCERPT_CHARS,
  MAX_IDE_REVIEW_ITEMS,
} from "./ide-review-projection.js";

const decode = (input: unknown) =>
  S.decodeUnknownSync(IdeReviewProjection)(input, { onExcessProperty: "error" });

const fixture = () => ({
  schema: IDE_REVIEW_PROJECTION_SCHEMA_LITERAL,
  projectionRef: "projection.ide.review.1",
  audience: "owner_authenticated" as const,
  source: {
    sessionRef: "session.1",
    projectRef: "project.1",
    worktreeRef: "worktree.1",
    attachmentRef: "attachment.1",
    placementRef: "placement.owner-local.1",
    attachmentGeneration: 7,
    projectGeneration: 11,
    serviceGeneration: 13,
    evidenceGeneration: 17,
  },
  freshness: {
    state: "live" as const,
    observedAt: "2026-07-20T17:00:00.000Z",
    sourceSequence: 42,
  },
  availability: "ready" as const,
  items: [
    {
      _tag: "TreeNode" as const,
      nodeRef: "node.src.index",
      parentNodeRef: "node.src",
      displayName: "index.ts",
      nodeKind: "file" as const,
      availability: "ready" as const,
    },
    {
      _tag: "Problem" as const,
      problemRef: "problem.1",
      documentRef: "document.1",
      range: {
        rangeRef: "range.problem.1",
        startLine: 8,
        startColumn: 3,
        endLine: 8,
        endColumn: 14,
        documentGeneration: 5,
      },
      severity: "error" as const,
      codeRef: "diagnostic.TS2322",
      detail: "Type number is not assignable to type string.",
    },
  ],
  omittedCount: 0,
  truncated: false,
  generatedAt: "2026-07-20T17:00:00.000Z",
  expiresAt: "2026-07-20T17:05:00.000Z",
});

describe("IdeReviewProjection", () => {
  test("decodes one bounded mobile/web projection with generation-bound opaque refs", () => {
    const projection = decode(fixture());

    expect(projection.schema).toBe(IDE_REVIEW_PROJECTION_SCHEMA_LITERAL);
    expect(projection.source.attachmentGeneration).toBe(7);
    expect(IdeReviewItem.guards.TreeNode(projection.items[0])).toBe(true);
    expect(IdeReviewItem.guards.Problem(projection.items[1])).toBe(true);
  });

  test.each([
    ["raw root", { rootPath: "/Users/owner/work/project" }],
    ["environment", { environment: { API_KEY: "secret" } }],
    ["credential", { credential: "sk-live-0123456789abcdef" }],
    ["terminal", { terminal: { raw: "cat .env" } }],
    ["process", { processId: 4242 }],
    ["native handle", { nativeHandle: "pty:19" }],
    ["mutable capability", { bearerToken: "capability-secret" }],
  ])("rejects an excess %s field", (_name, forbidden) => {
    expect(() => decode({ ...fixture(), ...forbidden })).toThrow();
  });

  test.each([
    "/Users/owner/work/project/index.ts",
    "C:\\Users\\owner\\project\\index.ts",
    "token=secret-value",
    "AKIAIOSFODNN7EXAMPLE",
    "-----BEGIN PRIVATE KEY-----",
  ])("rejects recognizable forbidden material inside an allowed text field", (text) => {
    const value = fixture();
    const first = value.items[0];
    if (first === undefined || !IdeReviewItem.guards.TreeNode(first)) {
      throw new Error("fixture must start with a tree node");
    }
    value.items[0] = { ...first, displayName: text };
    expect(() => decode(value)).toThrow();
  });

  test("rejects unrestricted excerpts and unbounded result pages", () => {
    const oversizedExcerpt = {
      ...fixture(),
      items: [
        {
          _tag: "Excerpt",
          excerptRef: "excerpt.1",
          documentRef: "document.1",
          range: {
            rangeRef: "range.excerpt.1",
            startLine: 1,
            startColumn: 1,
            endLine: 1,
            endColumn: 2,
            documentGeneration: 1,
          },
          languageRef: "language.typescript",
          text: "x".repeat(MAX_IDE_REVIEW_EXCERPT_CHARS + 1),
          truncated: true,
        },
      ],
    };
    expect(() => decode(oversizedExcerpt)).toThrow();

    const tooManyItems = {
      ...fixture(),
      items: Array.from({ length: MAX_IDE_REVIEW_ITEMS + 1 }, (_, index) => ({
        _tag: "Task",
        taskRef: `task.${index}`,
        status: "queued",
        evidenceRef: `evidence.${index}`,
      })),
    };
    expect(() => decode(tooManyItems)).toThrow();
  });

  test("rejects path-shaped refs and public audience claims", () => {
    expect(() =>
      decode({
        ...fixture(),
        source: { ...fixture().source, projectRef: "/Users/owner/project" },
      }),
    ).toThrow();
    expect(() => decode({ ...fixture(), audience: "public" })).toThrow();
  });

  test("requires exact gap, range, and expiry truth", () => {
    expect(() =>
      decode({
        ...fixture(),
        freshness: {
          state: "gap",
          observedAt: "2026-07-20T17:00:00.000Z",
          sourceSequence: 42,
        },
      }),
    ).toThrow();

    const invalidRange = fixture();
    const problem = invalidRange.items[1];
    if (problem === undefined || !IdeReviewItem.guards.Problem(problem)) {
      throw new Error("fixture must contain a problem");
    }
    invalidRange.items[1] = {
      ...problem,
      range: { ...problem.range, endLine: 7 },
    };
    expect(() => decode(invalidRange)).toThrow();

    expect(() =>
      decode({
        ...fixture(),
        expiresAt: "2026-07-20T16:59:59.000Z",
      }),
    ).toThrow();
  });
});
