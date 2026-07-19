import { mkdirSync, mkdtempSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { afterEach, describe, expect, test } from "vite-plus/test";

import {
  checkDocumentClaims,
  driftDispositionKey,
  loadPolicy,
  repositoryRoot,
  runDriftOracles,
} from "../src/index.ts";

const roots: string[] = [];
afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true });
});

const fixture = (): string => {
  const root = mkdtempSync(join(tmpdir(), "assure-repo-drift-"));
  roots.push(root);
  mkdirSync(join(root, "packages", "x"), { recursive: true });
  writeFileSync(join(root, "packages", "x", "index.ts"), "export const x = 1\n");
  writeFileSync(join(root, "package.json"), JSON.stringify({ scripts: { check: "vp lint" } }));
  return root;
};

const topLevel = (root: string) => new Set(readdirSync(root));

describe("checkDocumentClaims — path oracle", () => {
  test("passes an existing repo-root path", () => {
    const root = fixture();
    const findings = checkDocumentClaims(
      root,
      "DOC.md",
      "See `packages/x/index.ts`.",
      ["check"],
      topLevel(root),
    );
    expect(findings.filter((f) => f.verdict === "broken")).toEqual([]);
  });

  test("flags a missing file under a real top-level dir as broken", () => {
    const root = fixture();
    const findings = checkDocumentClaims(
      root,
      "DOC.md",
      "See `packages/x/gone.ts`.",
      ["check"],
      topLevel(root),
    );
    expect(findings.some((f) => f.kind === "path" && f.verdict === "broken")).toBe(true);
  });

  test("treats a context-relative path (first segment not a repo-root entry) as unverifiable, not broken", () => {
    const root = fixture();
    const findings = checkDocumentClaims(
      root,
      "DOC.md",
      "See `routes/download.tsx`.",
      ["check"],
      topLevel(root),
    );
    expect(findings.some((f) => f.verdict === "broken")).toBe(false);
    expect(findings.some((f) => f.verdict === "unverifiable")).toBe(true);
  });

  test("ignores npm package specifiers and URLs", () => {
    const root = fixture();
    const findings = checkDocumentClaims(
      root,
      "DOC.md",
      "`@openagentsinc/product-spec@0.1.0` and `https://x/y/z`.",
      ["check"],
      topLevel(root),
    );
    expect(findings).toEqual([]);
  });
});

describe("checkDocumentClaims — command oracle", () => {
  test("passes an existing root script and flags a missing one", () => {
    const root = fixture();
    const findings = checkDocumentClaims(
      root,
      "DOC.md",
      "Run `pnpm run check` then `pnpm run nope`.",
      ["check"],
      topLevel(root),
    );
    const broken = findings.filter((f) => f.kind === "command" && f.verdict === "broken");
    expect(broken.length).toBe(1);
    expect(broken[0]!.claim).toBe("pnpm run nope");
  });
});

describe("runDriftOracles dispositions", () => {
  test("a dispositioned broken finding is not counted as open", () => {
    const root = fixture();
    writeFileSync(join(root, "DOC.md"), "Broken `packages/x/gone.ts`.");
    const key = driftDispositionKey({ file: "DOC.md", claim: "packages/x/gone.ts" });
    const report = runDriftOracles(root, ["DOC.md"], { [key]: "known" });
    expect(report.summary.broken).toBe(1);
    expect(report.summary.dispositioned).toBe(1);
    expect(report.summary.brokenUndispositioned).toBe(0);
  });

  test("without a disposition the finding is open", () => {
    const root = fixture();
    writeFileSync(join(root, "DOC.md"), "Broken `packages/x/gone.ts`.");
    const report = runDriftOracles(root, ["DOC.md"], {});
    expect(report.summary.brokenUndispositioned).toBe(1);
  });
});

describe("runDriftOracles over the real governed set is side-effect free and gated", () => {
  test("governed docs check with no open broken claims", () => {
    const root = repositoryRoot();
    const policy = loadPolicy(root);
    const report = runDriftOracles(
      root,
      [...policy.governedDocuments, "docs/assure-repo/README.md", "packages/assure-repo/README.md"],
      policy.driftDispositions,
    );
    expect(report.summary.brokenUndispositioned).toBe(0);
  });
});
