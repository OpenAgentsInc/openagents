import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  buildOpenAgentsRepoCorpusManifest,
  extractOpenAgentsRepoCorpusEvidenceSpan,
  openAgentsRepoCorpusEvidenceSpanHash,
} from "../src";

async function withFixture<A>(run: (root: string) => Promise<A>): Promise<A> {
  const root = await mkdtemp(join(tmpdir(), "openagents-repo-corpus-"));

  try {
    await mkdir(join(root, "docs", "launch"), { recursive: true });
    await mkdir(join(root, "packages", "probe", "src"), { recursive: true });
    await mkdir(join(root, ".secrets"), { recursive: true });
    await mkdir(join(root, "node_modules", "left-pad"), { recursive: true });
    await mkdir(join(root, "dist"), { recursive: true });

    await writeFile(
      join(root, "docs", "launch", "roadmap.md"),
      "Title\nBoundary one\nBoundary two\nBoundary three\n",
      "utf8",
    );
    await writeFile(join(root, "packages", "probe", "src", "index.ts"), "export const probe = true;\n", "utf8");
    await writeFile(join(root, ".secrets", "operator.env"), "MDK_ACCESS_TOKEN=secret\n", "utf8");
    await writeFile(join(root, "node_modules", "left-pad", "index.js"), "module.exports = '';\n", "utf8");
    await writeFile(join(root, "dist", "bundle.js"), "console.log('bundle');\n", "utf8");
    await writeFile(join(root, "release.dmg"), "binary-ish\n", "utf8");
    await writeFile(join(root, "docs", "launch", "provider-secret-boundary.md"), "Public-safe title only.\n", "utf8");

    return await run(root);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}

function buildManifest(root: string) {
  return Effect.runPromise(
    buildOpenAgentsRepoCorpusManifest({
      commit: "sha256:fixture",
      generatedAt: "2026-06-17T00:00:00.000Z",
      repo: "OpenAgentsInc/openagents",
      rootDir: root,
    }),
  );
}

describe("OpenAgents repo corpus manifest", () => {
  test("builds a stable deterministic manifest with excluded paths removed", async () => {
    await withFixture(async (root) => {
      const first = await buildManifest(root);
      const second = await buildManifest(root);
      const paths = first.entries.map((entry) => entry.path);

      expect(first.manifestHash).toBe(second.manifestHash);
      expect(paths).toEqual(["docs/launch/roadmap.md", "packages/probe/src/index.ts"]);
      expect(paths.some((path) => path.includes(".secrets"))).toBe(false);
      expect(paths.some((path) => path.includes("node_modules"))).toBe(false);
      expect(paths.some((path) => path.includes("release.dmg"))).toBe(false);
      expect(paths.some((path) => path.includes("provider-secret"))).toBe(false);
      expect(first.entries[0]?.sha256.startsWith("sha256:")).toBe(true);
    });
  });

  test("extracts line-numbered evidence spans with stable span hashes", async () => {
    await withFixture(async (root) => {
      const manifest = await buildManifest(root);
      const span = await Effect.runPromise(
        extractOpenAgentsRepoCorpusEvidenceSpan({
          endLine: 3,
          manifest,
          path: "docs/launch/roadmap.md",
          rootDir: root,
          spanId: "s1",
          startLine: 2,
        }),
      );
      const repeated = await Effect.runPromise(
        extractOpenAgentsRepoCorpusEvidenceSpan({
          endLine: 3,
          manifest,
          path: "docs/launch/roadmap.md",
          rootDir: root,
          spanId: "s1",
          startLine: 2,
        }),
      );

      expect(span.evidence.excerpt).toBe("0002: Boundary one\n0003: Boundary two");
      expect(span.spanHash).toBe(repeated.spanHash);
      expect(span.spanHash).toBe(openAgentsRepoCorpusEvidenceSpanHash(manifest.manifestRef, span.evidence));
    });
  });

  test("fails closed for non-admitted files and invalid line ranges", async () => {
    await withFixture(async (root) => {
      const manifest = await buildManifest(root);

      await expect(
        Effect.runPromise(
          extractOpenAgentsRepoCorpusEvidenceSpan({
            endLine: 1,
            manifest,
            path: ".secrets/operator.env",
            rootDir: root,
            startLine: 1,
          }),
        ),
      ).rejects.toMatchObject({
        _tag: "ProbeBenchmarkContractError",
        path: "repoCorpusEvidenceSpan.path",
      });

      await expect(
        Effect.runPromise(
          extractOpenAgentsRepoCorpusEvidenceSpan({
            endLine: 99,
            manifest,
            path: "docs/launch/roadmap.md",
            rootDir: root,
            startLine: 1,
          }),
        ),
      ).rejects.toMatchObject({
        _tag: "ProbeBenchmarkContractError",
        path: "repoCorpusEvidenceSpan.endLine",
      });
    });
  });

  test("rejects unsafe evidence excerpts from admitted files", async () => {
    await withFixture(async (root) => {
      await writeFile(join(root, "docs", "launch", "unsafe.md"), "Do not publish access_token values.\n", "utf8");
      const manifest = await buildManifest(root);

      await expect(
        Effect.runPromise(
          extractOpenAgentsRepoCorpusEvidenceSpan({
            endLine: 1,
            manifest,
            path: "docs/launch/unsafe.md",
            rootDir: root,
            startLine: 1,
          }),
        ),
      ).rejects.toMatchObject({
        _tag: "ProbeBenchmarkContractError",
      });
    });
  });
});
