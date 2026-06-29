import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  buildOpenAgentsExternalRepoStudyPilot,
  openAgentsExternalRepoStudyProductSurfaceHash,
} from "../src";

const externalRepoFiles = {
  "AGENTS.md": "# Widget Agent Contract\n\nRead INVARIANTS before source changes.\n",
  "INVARIANTS.md": "# Widget Invariants\n\n- API edits must preserve typed JSON responses.\n",
  "README.md": "# Widget Service\n\nSmall external service used for repo-studying pilot coverage.\n",
  "docs/architecture.md": "# Architecture\n\nThe service owner for HTTP behavior is src/server.ts.\n",
  "docs/playbook.md": "# Edit Playbook\n\nChange src/server.ts and run bun test for API behavior.\n",
  "docs/repo-studying.md": "# Repo Studying\n\nStudy packet refs must stay separate from private customer material.\n",
  "docs/retained-failures.md": "# Retained Failures\n\nDo not reintroduce the old untyped response object.\n",
  "package.json": "{\"name\":\"widget-service\",\"scripts\":{\"test\":\"bun test\"}}\n",
  "src/server.ts": "export const response = () => ({ ok: true, version: 1 });\n",
  "tests/server.test.ts": "import { response } from '../src/server';\nif (!response().ok) throw new Error('expected ok');\n",
};

const commitHistory = [
  {
    commit: "9999999999999999999999999999999999999999",
    committedAt: "2026-06-18T00:00:00.000Z",
    subjectDigest: "sha256:925486e11092a7c8371a9509453f7e6dcff00677e04a4a3db83872142549498e",
    subjectPreview: "Add widget service typed response",
  },
] as const;

async function withExternalFixture<A>(run: (root: string) => Promise<A>): Promise<A> {
  const root = await mkdtemp(join(tmpdir(), "external-repo-study-"));

  try {
    for (const [path, content] of Object.entries(externalRepoFiles)) {
      const absolutePath = join(root, path);
      await mkdir(dirname(absolutePath), { recursive: true });
      await writeFile(absolutePath, content, "utf8");
    }

    return await run(root);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}

describe("external repo studying product pilot", () => {
  test("runs S1-S5 style studying pipeline on a non-openagents repo", async () => {
    await withExternalFixture(async (rootDir) => {
      const result = await Effect.runPromise(
        buildOpenAgentsExternalRepoStudyPilot({
          commit: "9999999999999999999999999999999999999999",
          commitHistory,
          editSitePath: "src/server.ts",
          generatedAt: "2026-06-18T00:00:00.000Z",
          repo: "ExampleCorp/widget-service",
          rootDir,
        }),
      );

      expect(result.manifest.repo).toBe("ExampleCorp/widget-service");
      expect(result.manifest.repo).not.toBe("OpenAgentsInc/openagents");
      expect(result.packet.repo).toBe("ExampleCorp/widget-service");
      expect(result.packet.sections.map((section) => section.kind).sort()).toEqual([
        "edit_playbook",
        "invariant_map",
        "retained_failure_fixture",
        "source_map",
        "test_command_catalog",
        "trap_catalog",
        "typed_ref_glossary",
      ]);
      expect(result.graph.repo).toBe("ExampleCorp/widget-service");
      expect(result.graph.edges.some((edge) => edge.kind === "edit_site_respects_invariant")).toBe(true);
      expect(result.graph.edges.some((edge) => edge.kind === "edit_site_commit_context")).toBe(true);
      expect(result.graph.edges.some((edge) => edge.kind === "code_explained_by_audit")).toBe(true);
      expect(result.graph.edges.some((edge) => edge.kind === "code_warned_by_rejected_lineage")).toBe(true);
      expect(result.verification.correctnessGatePassed).toBe(true);
      expect(result.evalReport.comparison.studiedBeatsBaseline).toBe(true);
      expect(result.coderContext.editSitePath).toBe("src/server.ts");
      expect(result.coderContext.invariantNodeRefs.length).toBeGreaterThan(0);
      expect(result.coderContext.introducingCommitNodeRefs.length).toBeGreaterThan(0);
      expect(result.coderContext.auditNodeRefs.length).toBeGreaterThan(0);
      expect(result.productSurface).toMatchObject({
        customerPublicClaimAllowed: false,
        marketplacePackageAllowed: false,
        payoutEligible: false,
        repo: "ExampleCorp/widget-service",
        state: "pilot_ready",
        studiedBeatsBaseline: true,
      });
      expect(result.productSurface.productSurfaceHash).toBe(
        openAgentsExternalRepoStudyProductSurfaceHash(result.productSurface),
      );
      expect(result.productSurface.evidenceRefs).toEqual(
        expect.arrayContaining([
          result.manifest.manifestRef,
          result.packet.packetRef,
          result.graph.graphRef,
          result.verification.verificationRef,
          result.evalReport.reportRef,
          result.coderContext.contextPackRef,
        ]),
      );
      expect(`${result.productSurface.safeCopy} ${result.productSurface.unsafeCopyRefs.join(" ")}`).not.toMatch(
        /customer repo studying is live|trained repo expert|payout eligible/i,
      );
    });
  });

  test("rejects the dogfood repo so S7 evidence must use an external target", async () => {
    await withExternalFixture(async (rootDir) => {
      await expect(
        Effect.runPromise(
          buildOpenAgentsExternalRepoStudyPilot({
            commit: "9999999999999999999999999999999999999999",
            commitHistory,
            editSitePath: "src/server.ts",
            repo: "OpenAgentsInc/openagents",
            rootDir,
          }),
        ),
      ).rejects.toMatchObject({
        path: "externalRepoStudy.repo",
        reason: "pilot repo must not be OpenAgentsInc/openagents",
      });
    });
  });
});
