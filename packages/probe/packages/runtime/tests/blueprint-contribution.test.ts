import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  PROBE_BLUEPRINT_CONTRIBUTION_NO_AUTHORITY,
  PROBE_STUDYBENCH_CONTRIBUTION_CAPABILITY_FAMILY,
  isProbeStudybenchBlueprintContributionKind,
  probeBlueprintContributionBlockerRefs,
  probeBlueprintContributionCanEnterReleaseGate,
  probeBlueprintContributionHasRuntimeAuthority,
  probeBlueprintContributionRuntimeEligibility,
  probeBlueprintContributionTargetRefs,
  validateProbeBlueprintContributionDraft,
  type ProbeBlueprintContributionDraft,
} from "../src";

const contribution = (
  overrides: Partial<ProbeBlueprintContributionDraft> = {},
): ProbeBlueprintContributionDraft => ({
  authority: PROBE_BLUEPRINT_CONTRIBUTION_NO_AUTHORITY,
  backendProjectionAdapterRefs: ["adapter.probe.apple_fm.blueprint_tools.v1"],
  capabilityFamily: "program_signature",
  capabilitySummaryRef: "capability.probe.tool_menu.package.summary.v1",
  contentRedacted: true,
  contextPackageRefs: ["context_package.probe.repo.readonly.v1"],
  contributionKind: "signature_contribution",
  contributorRefs: ["contributor.openagents.probe"],
  dogfoodScopeRef: "dogfood.probe.assignment_only.v1",
  fixtureRefs: ["fixture.probe.tool_menu.decode.v1"],
  id: "probe_blueprint_contribution.tool_menu.v1",
  intendedProgramFamily: "action_planning",
  noProductionRuntimeAuthority: true,
  outcomeTemplateRefs: [],
  paymentAttributionRefs: [
    "payment_attribution.probe.tool_menu.promoted_ref.v1",
  ],
  promotionRef: null,
  proposedModuleVersionRefs: ["module_version.probe.tool_menu.seed.v1"],
  proposedProgramSignatureRefs: [
    "program_signature.probe.tool_menu.project.v1",
  ],
  proposedProgramTypeRefs: ["program_type.probe.tool_menu.project"],
  rejectionRef: null,
  releaseGateRefs: ["release_gate.probe.tool_menu.seed.v1"],
  retainedFailureRefs: ["failure.probe.tool_menu.fixture_retained.v1"],
  reviewStatus: "approved",
  riskClass: "medium",
  selfPromotionAttempt: false,
  sourceRefs: ["source_ref.probe.contribution.audit.v1"],
  status: "approved_for_release_gate",
  toolPackageRefs: ["tool_package.probe.readonly_repo_tools.v1"],
  uiBindingRefs: [],
  ...overrides,
});

describe("Probe Blueprint contribution release gates", () => {
  test("allows release-gate entry only for reviewed non-authoritative contributions with targets", async () => {
    const draft = contribution();

    await expect(
      Effect.runPromise(validateProbeBlueprintContributionDraft(draft)),
    ).resolves.toEqual(draft);
    expect(probeBlueprintContributionHasRuntimeAuthority(draft)).toBe(false);
    expect(probeBlueprintContributionCanEnterReleaseGate(draft)).toBe(true);
    expect(probeBlueprintContributionTargetRefs(draft)).toContain(
      "program_signature.probe.tool_menu.project.v1",
    );
    expect(probeBlueprintContributionTargetRefs(draft)).toContain(
      "tool_package.probe.readonly_repo_tools.v1",
    );
  });

  test("rejects runtime authority on contribution drafts", async () => {
    const authoritative = contribution({
      authority: {
        ...PROBE_BLUEPRINT_CONTRIBUTION_NO_AUTHORITY,
        canDispatchRuntime: true,
      },
    });

    expect(probeBlueprintContributionHasRuntimeAuthority(authoritative)).toBe(
      true,
    );
    expect(probeBlueprintContributionCanEnterReleaseGate(authoritative)).toBe(
      false,
    );
    expect(probeBlueprintContributionBlockerRefs(authoritative)).toContain(
      "blocker.probe_blueprint_contribution.runtime_authority_present",
    );
    await expect(
      Effect.runPromise(validateProbeBlueprintContributionDraft(authoritative)),
    ).rejects.toMatchObject({
      _tag: "ProbeBlueprintContributionUnsafe",
    });
  });

  test("requires approval, fixtures, release gates, and target refs", () => {
    const blocked = contribution({
      fixtureRefs: [],
      releaseGateRefs: [],
      retainedFailureRefs: [],
      reviewStatus: "pending",
      status: "submitted",
      proposedModuleVersionRefs: [],
      proposedProgramSignatureRefs: [],
      proposedProgramTypeRefs: [],
      contextPackageRefs: [],
      outcomeTemplateRefs: [],
      toolPackageRefs: [],
      uiBindingRefs: [],
      backendProjectionAdapterRefs: [],
    });

    expect(probeBlueprintContributionCanEnterReleaseGate(blocked)).toBe(false);
    expect(probeBlueprintContributionBlockerRefs(blocked)).toEqual([
      "blocker.probe_blueprint_contribution.fixture_refs_missing",
      "blocker.probe_blueprint_contribution.not_release_gate_ready",
      "blocker.probe_blueprint_contribution.release_gate_refs_missing",
      "blocker.probe_blueprint_contribution.retained_failure_refs_missing",
      "blocker.probe_blueprint_contribution.review_not_approved",
      "blocker.probe_blueprint_contribution.target_ref_missing",
    ]);
  });

  test("blocks optimizer self-promotion attempts", async () => {
    const selfPromoting = contribution({
      selfPromotionAttempt: true,
    });

    expect(probeBlueprintContributionCanEnterReleaseGate(selfPromoting)).toBe(
      false,
    );
    expect(probeBlueprintContributionBlockerRefs(selfPromoting)).toContain(
      "blocker.probe_blueprint_contribution.self_promotion_attempt",
    );
    await expect(
      Effect.runPromise(validateProbeBlueprintContributionDraft(selfPromoting)),
    ).rejects.toMatchObject({
      _tag: "ProbeBlueprintContributionUnsafe",
    });
  });

  test("distinguishes candidate dogfood use from production runtime use", () => {
    const candidate = contribution();
    const promoted = contribution({
      promotionRef: "promotion.probe.tool_menu.v1",
      status: "promoted",
    });

    expect(
      probeBlueprintContributionRuntimeEligibility(candidate, {
        assignmentAllowsCandidate: false,
      }),
    ).toMatchObject({
      candidateRuntimeAllowed: false,
      productionRuntimeAllowed: false,
    });
    expect(
      probeBlueprintContributionRuntimeEligibility(candidate, {
        assignmentAllowsCandidate: true,
      }),
    ).toMatchObject({
      candidateRuntimeAllowed: true,
      productionRuntimeAllowed: false,
    });
    expect(
      probeBlueprintContributionRuntimeEligibility(promoted, {
        assignmentAllowsCandidate: false,
      }),
    ).toMatchObject({
      candidateRuntimeAllowed: false,
      productionRuntimeAllowed: true,
    });
  });

  test("rejects private contribution material and raw timestamps", async () => {
    await expect(
      Effect.runPromise(
        validateProbeBlueprintContributionDraft(
          contribution({
            sourceRefs: ["raw_prompt.do_not_publish"],
          }),
        ),
      ),
    ).rejects.toMatchObject({ _tag: "ProbeBlueprintContributionUnsafe" });

    await expect(
      Effect.runPromise(
        validateProbeBlueprintContributionDraft(
          contribution({
            retainedFailureRefs: ["failure.probe.2026-06-07T00:00:00.raw"],
          }),
        ),
      ),
    ).rejects.toMatchObject({ _tag: "ProbeBlueprintContributionUnsafe" });
  });

  test("keeps payment and attribution on promoted refs without raw payment material", async () => {
    const packageContribution = contribution({
      capabilityFamily: "tool_package",
      contributionKind: "developer_package_contribution",
      paymentAttributionRefs: [
        "payment_attribution.promoted_package.probe_tooling.v1",
      ],
      proposedProgramSignatureRefs: [],
      proposedProgramTypeRefs: [],
      toolPackageRefs: ["tool_package.probe.repo_read_tools.v1"],
    });

    await expect(
      Effect.runPromise(
        validateProbeBlueprintContributionDraft(packageContribution),
      ),
    ).resolves.toEqual(packageContribution);
    expect(JSON.stringify(packageContribution)).not.toContain("invoice");
    expect(JSON.stringify(packageContribution)).not.toContain("preimage");
    expect(probeBlueprintContributionTargetRefs(packageContribution)).toContain(
      "tool_package.probe.repo_read_tools.v1",
    );
  });

  test("maps StudyBench contribution kinds to evidence-only Blueprint capability families", async () => {
    const kinds = [
      "studybench.task_authoring.v0",
      "studybench.evidence_span_extraction.v0",
      "studybench.rubric_authoring.v0",
      "studybench.rubric_judging.v0",
      "repo_study_packet.v0",
    ] as const;

    for (const contributionKind of kinds) {
      expect(isProbeStudybenchBlueprintContributionKind(contributionKind)).toBe(
        true,
      );

      const draft = contribution({
        capabilityFamily:
          PROBE_STUDYBENCH_CONTRIBUTION_CAPABILITY_FAMILY[contributionKind],
        contributionKind,
        dogfoodScopeRef: "dogfood.openagents.studybench.authoring.v0",
        fixtureRefs: [
          `fixture.openagents_studybench.${contributionKind}.retained.v0`,
        ],
        id: `probe_blueprint_contribution.${contributionKind}`,
        paymentAttributionRefs: [],
        retainedFailureRefs: [
          `failure.openagents_studybench.${contributionKind}.retained.v0`,
        ],
        sourceRefs: ["source_ref.openagents_studybench.public_retained.v0"],
      });

      await expect(
        Effect.runPromise(validateProbeBlueprintContributionDraft(draft)),
      ).resolves.toEqual(draft);
      expect(probeBlueprintContributionHasRuntimeAuthority(draft)).toBe(false);
      expect(probeBlueprintContributionCanEnterReleaseGate(draft)).toBe(true);
    }
  });

  test("requires retained failure refs before StudyBench contribution release-gate readiness", async () => {
    const draft = contribution({
      capabilityFamily: "context_package",
      contributionKind: "studybench.task_authoring.v0",
      paymentAttributionRefs: [],
      retainedFailureRefs: [],
    });

    await expect(
      Effect.runPromise(validateProbeBlueprintContributionDraft(draft)),
    ).resolves.toEqual(draft);
    expect(probeBlueprintContributionCanEnterReleaseGate(draft)).toBe(false);
    expect(probeBlueprintContributionBlockerRefs(draft)).toContain(
      "blocker.probe_blueprint_contribution.retained_failure_refs_missing",
    );
  });

  test("rejects StudyBench contribution kind capability mismatches", async () => {
    await expect(
      Effect.runPromise(
        validateProbeBlueprintContributionDraft(
          contribution({
            capabilityFamily: "tool_package",
            contributionKind: "studybench.task_authoring.v0",
            paymentAttributionRefs: [],
          }),
        ),
      ),
    ).rejects.toMatchObject({
      _tag: "ProbeBlueprintContributionUnsafe",
      path: "capabilityFamily",
    });
  });
});
