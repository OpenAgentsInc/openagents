import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  buildOpenAgentsCustomerPrivateValidation,
  buildOpenAgentsExternalRepoStudyPilot,
  openAgentsCustomerPrivateValidationVerdictHash,
  planCustomerPrivateValidationDelivery,
  type BuildOpenAgentsExternalRepoStudyPilotResult,
  type CustomerPrivateHoldoutCommitment,
} from "../src";

const customerRepoFiles = {
  "AGENTS.md": "# Customer Agent Contract\n\nRead INVARIANTS before source changes.\n",
  "INVARIANTS.md": "# Customer Invariants\n\n- API edits must preserve typed JSON responses.\n",
  "README.md": "# Customer Service\n\nPrivate customer service used for customer-private validation coverage.\n",
  "docs/architecture.md": "# Architecture\n\nThe service owner for HTTP behavior is src/server.ts.\n",
  "docs/playbook.md": "# Edit Playbook\n\nChange src/server.ts and run bun test for API behavior.\n",
  "docs/repo-studying.md": "# Repo Studying\n\nStudy packet refs must stay separate from private customer material.\n",
  "docs/retained-failures.md": "# Retained Failures\n\nDo not reintroduce the old untyped response object.\n",
  "package.json": "{\"name\":\"customer-service\",\"scripts\":{\"test\":\"bun test\"}}\n",
  "src/server.ts": "export const response = () => ({ ok: true, version: 1 });\n",
  "tests/server.test.ts": "import { response } from '../src/server';\nif (!response().ok) throw new Error('expected ok');\n",
};

const commitHistory = [
  {
    commit: "8888888888888888888888888888888888888888",
    committedAt: "2026-06-19T00:00:00.000Z",
    subjectDigest: "sha256:925486e11092a7c8371a9509453f7e6dcff00677e04a4a3db83872142549498e",
    subjectPreview: "Add customer service typed response",
  },
] as const;

const repo = "CustomerCorp/customer-service" as const;
const commit = "8888888888888888888888888888888888888888" as const;
const generatedAt = "2026-06-19T00:00:00.000Z" as const;

// A REFS-ONLY holdout commitment. No private task text, gold answers, rubric, or
// evidence excerpts ever cross this boundary.
const holdout: CustomerPrivateHoldoutCommitment = {
  splitRef: "split.customer.customercorp.private_holdout.v0",
  datasetRef: "dataset.customer.customercorp.private_holdout.v0",
  checksumRef: "sha256:" + "a".repeat(64),
  rowCount: 8,
  holdoutPassCount: 6,
  holdoutPassRateLiftBps: 2_200,
};

const SECRET_STRINGS = [
  "gold_answer",
  "private_holdout",
  "rubric_claim",
  "evidence_excerpt",
  "hidden task text",
];

async function withCustomerFixture<A>(
  run: (pilot: BuildOpenAgentsExternalRepoStudyPilotResult) => Promise<A>,
): Promise<A> {
  const root = await mkdtemp(join(tmpdir(), "customer-private-validation-"));
  try {
    for (const [path, content] of Object.entries(customerRepoFiles)) {
      const absolutePath = join(root, path);
      await mkdir(dirname(absolutePath), { recursive: true });
      await writeFile(absolutePath, content, "utf8");
    }
    const pilot = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyPilot({
        commit,
        commitHistory,
        editSitePath: "src/server.ts",
        generatedAt,
        repo,
        rootDir: root,
      }),
    );
    return await run(pilot);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}

describe("customer-private repo-study validation", () => {
  test("validates a packet privately against a committed holdout and holds it inert", async () => {
    await withCustomerFixture(async (pilot) => {
      const verdict = await Effect.runPromise(
        buildOpenAgentsCustomerPrivateValidation({
          evalReport: pilot.evalReport,
          generatedAt,
          graph: pilot.graph,
          holdout,
          packet: pilot.packet,
          repo,
          verification: pilot.verification,
        }),
      );

      // Inert/no-claim guarantees.
      expect(verdict.deliverable).toBe(false);
      expect(verdict.customerPublicClaimAllowed).toBe(false);
      expect(verdict.marketplacePackageAllowed).toBe(false);
      expect(verdict.payoutEligible).toBe(false);
      expect(verdict.deliveryGate.effectsApplied).toBe(false);
      expect(verdict.deliveryGate.state).toBe("inert_disabled");
      expect(verdict.sourceBoundary).toBe("customer_refs_withheld");

      // Validation evidence is refs/hashes/counts only.
      expect(verdict.holdoutSplitRef).toBe(holdout.splitRef);
      expect(verdict.holdoutRowCount).toBe(holdout.rowCount);
      expect(verdict.holdoutPassRateLiftBps).toBe(holdout.holdoutPassRateLiftBps);
      expect(verdict.verdictHash).toBe(
        openAgentsCustomerPrivateValidationVerdictHash(verdict),
      );
    });
  });

  test("never leaks private holdout content into the public verdict projection", async () => {
    await withCustomerFixture(async (pilot) => {
      const verdict = await Effect.runPromise(
        buildOpenAgentsCustomerPrivateValidation({
          evalReport: pilot.evalReport,
          generatedAt,
          graph: pilot.graph,
          holdout,
          packet: pilot.packet,
          repo,
          verification: pilot.verification,
        }),
      );
      const serialized = JSON.stringify(verdict);
      // The verdict references the holdout but must not embed hidden content.
      // (split/dataset/checksum refs are explicit refs, not row content.)
      for (const secret of SECRET_STRINGS) {
        if (secret === "private_holdout") {
          // split/dataset refs legitimately contain "private_holdout" as a ref token;
          // the rule is no ROW CONTENT, which is covered by the other markers.
          continue;
        }
        expect(serialized.includes(secret)).toBe(false);
      }
    });
  });

  test("rejects a holdout commitment that carries raw content instead of a digest ref", async () => {
    await withCustomerFixture(async (pilot) => {
      const result = await Effect.runPromiseExit(
        buildOpenAgentsCustomerPrivateValidation({
          evalReport: pilot.evalReport,
          generatedAt,
          graph: pilot.graph,
          holdout: {
            ...holdout,
            checksumRef: "the answer is 42 and the gold_answer text leaks here",
          },
          packet: pilot.packet,
          repo,
          verification: pilot.verification,
        }),
      );
      expect(result._tag).toBe("Failure");
    });
  });

  test("delivery stays inert even when the flag is armed with owner signoff", async () => {
    await withCustomerFixture(async (pilot) => {
      const verdict = await Effect.runPromise(
        buildOpenAgentsCustomerPrivateValidation({
          deliveryFlagArmed: true,
          evalReport: pilot.evalReport,
          generatedAt,
          graph: pilot.graph,
          holdout,
          ownerSignoffPresent: true,
          packet: pilot.packet,
          repo,
          verification: pilot.verification,
        }),
      );

      // Even armed + signed, the verdict applies no real effect and stays
      // non-deliverable from this module.
      expect(verdict.deliverable).toBe(false);
      expect(verdict.deliveryGate.effectsApplied).toBe(false);

      const plan = planCustomerPrivateValidationDelivery({
        customerRef: "customer.customercorp.v0",
        verdict,
      });
      expect(plan.deliverable).toBe(false);
      expect(plan.effectsApplied).toBe(false);
      // When every gate passes, wouldDeliverWhenArmed reflects the policy verdict
      // but no real delivery is performed here.
      if (verdict.state === "validated_held") {
        expect(plan.gateState).toBe("armed_ready");
        expect(plan.wouldDeliverWhenArmed).toBe(true);
      }
    });
  });

  test("blocks the verdict when the private-holdout lift is missing", async () => {
    await withCustomerFixture(async (pilot) => {
      const verdict = await Effect.runPromise(
        buildOpenAgentsCustomerPrivateValidation({
          evalReport: pilot.evalReport,
          generatedAt,
          graph: pilot.graph,
          holdout: { ...holdout, holdoutPassRateLiftBps: 0, holdoutPassCount: 0 },
          packet: pilot.packet,
          repo,
          verification: pilot.verification,
        }),
      );
      expect(verdict.state).toBe("blocked");
      expect(verdict.holdoutLiftMet).toBe(false);
      expect(verdict.blockerRefs).toContain(
        "blocker.customer_private_validation.private_holdout_lift_missing",
      );
      expect(verdict.deliverable).toBe(false);
    });
  });
});
