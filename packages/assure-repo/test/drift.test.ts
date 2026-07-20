import { execFileSync } from "node:child_process";
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

describe("checkDocumentClaims — gitignored paths are environment-specific", () => {
  test("a gitignored path is unverifiable, not broken", () => {
    const root = mkdtempSync(join(tmpdir(), "assure-repo-drift-git-"));
    roots.push(root);
    execFileSync("git", ["-C", root, "init", "-q"]);
    mkdirSync(join(root, "secrets"), { recursive: true });
    writeFileSync(join(root, ".gitignore"), "secrets/\n");
    writeFileSync(join(root, "package.json"), JSON.stringify({ scripts: {} }));
    // secrets/token.env is gitignored and absent from a fresh checkout.
    const findings = checkDocumentClaims(
      root,
      "DOC.md",
      "Read `secrets/token.env`.",
      [],
      new Set(readdirSync(root)),
    );
    const secret = findings.find((f) => f.claim === "secrets/token.env");
    expect(secret?.verdict).toBe("unverifiable");
    expect(secret?.detail).toContain("gitignored");
  });
});

describe("checkDocumentClaims — markdown-link oracle", () => {
  test("passes a relative link that resolves to an existing file", () => {
    const root = fixture();
    // Linking file is at packages/x/DOC.md; `./index.ts` resolves to the sibling.
    const findings = checkDocumentClaims(
      root,
      "packages/x/DOC.md",
      "See [the module](./index.ts).",
      ["check"],
      topLevel(root),
    );
    expect(findings.filter((f) => f.kind === "link")).toEqual([]);
  });

  test("flags a relative link to a missing file as broken", () => {
    const root = fixture();
    const findings = checkDocumentClaims(
      root,
      "packages/x/DOC.md",
      "See [gone](./gone.ts).",
      ["check"],
      topLevel(root),
    );
    const link = findings.find((f) => f.kind === "link");
    expect(link?.verdict).toBe("broken");
    expect(link?.claim).toBe("./gone.ts");
  });

  test("resolves a parent-relative link against the linking file's directory", () => {
    const root = fixture();
    const findings = checkDocumentClaims(
      root,
      "packages/x/DOC.md",
      "See [pkg json](../../package.json).",
      ["check"],
      topLevel(root),
    );
    expect(findings.filter((f) => f.kind === "link")).toEqual([]);
  });

  test("treats a directory-shaped missing link target as unverifiable, not broken", () => {
    const root = fixture();
    const findings = checkDocumentClaims(
      root,
      "packages/x/DOC.md",
      "See [the area](../y).",
      ["check"],
      topLevel(root),
    );
    const link = findings.find((f) => f.kind === "link");
    expect(link?.verdict).toBe("unverifiable");
  });

  test("ignores external, mail, and pure-anchor links", () => {
    const root = fixture();
    const findings = checkDocumentClaims(
      root,
      "packages/x/DOC.md",
      "[web](https://x/y) [mail](mailto:a@b.c) [top](#heading) [word](nothingish)",
      ["check"],
      topLevel(root),
    );
    expect(findings.filter((f) => f.kind === "link")).toEqual([]);
  });
});

describe("checkDocumentClaims — non-path references are excluded", () => {
  test("git refs and known GitHub org slugs are not path findings", () => {
    const root = fixture();
    const findings = checkDocumentClaims(
      root,
      "DOC.md",
      "Reset to `origin/main`; the repo is `OpenAgentsInc/openagents`.",
      ["check"],
      topLevel(root),
    );
    expect(findings).toEqual([]);
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
