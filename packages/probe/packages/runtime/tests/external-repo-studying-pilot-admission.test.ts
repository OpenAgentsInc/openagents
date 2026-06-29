import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  buildOpenAgentsExternalRepoStudyPilot,
  buildOpenAgentsExternalRepoStudyPilotAdmission,
  openAgentsExternalRepoStudyPilotAdmissionHash,
  type BuildOpenAgentsExternalRepoStudyPilotResult,
  type CustomerPrivateHoldoutCommitment,
} from "../src";

const externalRepoFiles = {
  "AGENTS.md": "# Widget Agent Contract\n\nRead INVARIANTS before source changes.\n",
  "INVARIANTS.md": "# Widget Invariants\n\n- API edits must preserve typed JSON responses.\n",
  "README.md": "# Widget Service\n\nSmall external service used for repo-studying pilot admission coverage.\n",
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
    commit: "7777777777777777777777777777777777777777",
    committedAt: "2026-06-19T00:00:00.000Z",
    subjectDigest: "sha256:925486e11092a7c8371a9509453f7e6dcff00677e04a4a3db83872142549498e",
    subjectPreview: "Add widget service typed response",
  },
] as const;

const repo = "ExampleCorp/widget-service" as const;
const commit = "7777777777777777777777777777777777777777" as const;
const generatedAt = "2026-06-19T00:00:00.000Z" as const;

// A REFS-ONLY holdout commitment. No private task text, gold answers, rubric, or
// evidence excerpts ever cross this boundary.
const holdout: CustomerPrivateHoldoutCommitment = {
  splitRef: "split.customer.examplecorp.private_holdout.v0",
  datasetRef: "dataset.customer.examplecorp.private_holdout.v0",
  checksumRef: "sha256:" + "b".repeat(64),
  rowCount: 8,
  holdoutPassCount: 6,
  holdoutPassRateLiftBps: 2_200,
};

const contributor = {
  contributorRef: "contributor.pylon.448ba824.v0",
  pilotTermsAccepted: true,
} as const;

const customer = { customerRef: "customer.examplecorp.v0" } as const;

const SECRET_STRINGS = [
  "gold_answer",
  "rubric_claim",
  "evidence_excerpt",
  "hidden task text",
];

async function withPilot<A>(
  run: (pilot: BuildOpenAgentsExternalRepoStudyPilotResult) => Promise<A>,
): Promise<A> {
  const root = await mkdtemp(join(tmpdir(), "external-repo-study-admission-"));
  try {
    for (const [path, content] of Object.entries(externalRepoFiles)) {
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

describe("external repo studying pilot customer-private admission", () => {
  test("admits an external study privately for a customer and holds it inert (flag default-OFF)", async () => {
    await withPilot(async (pilot) => {
      const { admission, validationVerdict } = await Effect.runPromise(
        buildOpenAgentsExternalRepoStudyPilotAdmission({
          contributor,
          customer,
          generatedAt,
          holdout,
          pilot,
        }),
      );

      // The private validation engine was REUSED (not rebuilt).
      expect(validationVerdict.schemaRef).toBe(
        "openagents.customer_private_repo_study_validation.v0",
      );
      expect(validationVerdict.state).toBe("validated_held");
      expect(admission.validationVerdictRef).toBe(validationVerdict.verdictRef);
      expect(admission.validationVerdictHash).toBe(validationVerdict.verdictHash);

      // Private admission gate passed but is held inert by default-OFF flag.
      expect(admission.privateValidationPassed).toBe(true);
      expect(admission.contributorTermsAccepted).toBe(true);
      expect(admission.state).toBe("admittable_held");
      expect(admission.admissionGate.state).toBe("inert_disabled");
      expect(admission.admissionGate.flagName).toBe(
        "EXTERNAL_REPO_STUDY_PILOT_ADMISSION_ENABLED",
      );

      // Inert/no-claim guarantees.
      expect(admission.admitted).toBe(false);
      expect(admission.effectsApplied).toBe(false);
      expect(admission.admissionGate.effectsApplied).toBe(false);
      expect(admission.wouldAdmitWhenArmed).toBe(false);
      expect(admission.customerPublicClaimAllowed).toBe(false);
      expect(admission.marketplacePackageAllowed).toBe(false);
      expect(admission.payoutEligible).toBe(false);
      expect(admission.sourceBoundary).toBe("customer_refs_withheld");

      // Refs-only evidence and stable hash.
      expect(admission.repo).toBe(repo);
      expect(admission.repo).not.toBe("OpenAgentsInc/openagents");
      expect(admission.evidenceRefs).toEqual(
        expect.arrayContaining([
          pilot.productSurface.productSurfaceRef,
          validationVerdict.verdictRef,
          holdout.splitRef,
          contributor.contributorRef,
        ]),
      );
      expect(admission.admissionHash).toBe(
        openAgentsExternalRepoStudyPilotAdmissionHash(admission),
      );
      expect(
        `${admission.safeCopy} ${admission.unsafeCopyRefs.join(" ")}`,
      ).not.toMatch(/customer repo studying is live|payout eligible/i);
    });
  });

  test("would-admit-when-armed only once flag + owner signoff are present, still inert", async () => {
    await withPilot(async (pilot) => {
      const { admission } = await Effect.runPromise(
        buildOpenAgentsExternalRepoStudyPilotAdmission({
          admissionFlagArmed: true,
          contributor,
          customer,
          generatedAt,
          holdout,
          ownerSignoffPresent: true,
          pilot,
        }),
      );

      expect(admission.admissionGate.state).toBe("armed_ready");
      expect(admission.wouldAdmitWhenArmed).toBe(true);
      // Even armed + ready, no real effect is applied by this module.
      expect(admission.admitted).toBe(false);
      expect(admission.effectsApplied).toBe(false);
      expect(admission.admissionGate.effectsApplied).toBe(false);
    });
  });

  test("armed without owner signoff blocks the admission gate", async () => {
    await withPilot(async (pilot) => {
      const { admission } = await Effect.runPromise(
        buildOpenAgentsExternalRepoStudyPilotAdmission({
          admissionFlagArmed: true,
          contributor,
          customer,
          generatedAt,
          holdout,
          pilot,
        }),
      );

      expect(admission.admissionGate.state).toBe("armed_blocked");
      expect(admission.admissionGate.blockedReasonRefs).toContain(
        "admission.blocked.owner_signoff_missing",
      );
      expect(admission.wouldAdmitWhenArmed).toBe(false);
      expect(admission.admitted).toBe(false);
    });
  });

  test("blocks admission when the contributor has not accepted pilot terms", async () => {
    await withPilot(async (pilot) => {
      const { admission } = await Effect.runPromise(
        buildOpenAgentsExternalRepoStudyPilotAdmission({
          contributor: { contributorRef: contributor.contributorRef },
          customer,
          generatedAt,
          holdout,
          pilot,
        }),
      );

      expect(admission.contributorTermsAccepted).toBe(false);
      expect(admission.state).toBe("blocked");
      expect(admission.blockerRefs).toContain(
        "blocker.external_repo_study_pilot_admission.contributor_terms_not_accepted",
      );
    });
  });

  test("blocks admission when the private holdout shows no lift", async () => {
    await withPilot(async (pilot) => {
      const { admission, validationVerdict } = await Effect.runPromise(
        buildOpenAgentsExternalRepoStudyPilotAdmission({
          contributor,
          customer,
          generatedAt,
          holdout: { ...holdout, holdoutPassRateLiftBps: 0, holdoutPassCount: 0 },
          pilot,
        }),
      );

      expect(validationVerdict.state).toBe("blocked");
      expect(admission.privateValidationPassed).toBe(false);
      expect(admission.state).toBe("blocked");
      expect(admission.blockerRefs).toContain(
        "blocker.external_repo_study_pilot_admission.private_validation_not_passed",
      );
    });
  });

  test("never leaks private holdout row content into the public admission projection", async () => {
    await withPilot(async (pilot) => {
      const { admission } = await Effect.runPromise(
        buildOpenAgentsExternalRepoStudyPilotAdmission({
          contributor,
          customer,
          generatedAt,
          holdout,
          pilot,
        }),
      );
      const serialized = JSON.stringify(admission);
      for (const secret of SECRET_STRINGS) {
        expect(serialized).not.toContain(secret);
      }
    });
  });
});
