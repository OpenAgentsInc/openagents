import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  classifyProbeRequestedEffect,
  guardProgramRunDirectEffect,
  makeProbeActionSubmissionProposal,
  validateProbeActionSubmissionProposal,
  type ProbeActionSubmissionProposal,
  type ProbeRequestedEffectKind,
} from "../src";

const externalEffects: ReadonlyArray<ProbeRequestedEffectKind> = [
  "create_pull_request",
  "deploy",
  "send_email",
  "post_public_claim",
  "spend_money",
  "legal_sensitive_commitment",
  "mutate_source_backed_business_fact",
];

const proposalInput = (effectKind: ProbeRequestedEffectKind = "create_pull_request") => ({
  actorRef: "actor.probe.test",
  approvalPolicyRef: "policy.blueprint.action_submission.proposals_only.v1",
  assignmentRef: "assignment.probe.test",
  contextPackRefs: ["context_pack.openagents.thread_1"],
  effectKind,
  evidenceRefs: ["evidence.program_run.summary_ref", "evidence.diff.redacted_ref"],
  moduleVersionId: "module_version.probe.tool_menu.seed.v1",
  observedAt: "2026-06-07T00:00:00.000Z",
  programRunRef: "program_run.probe.apple_fm.test",
  programSignatureId: "program_signature.probe.tool_menu.project.v1",
  programTypeId: "program_type.probe.tool_menu.project",
  sourceAuthorityRefs: ["source_authority.repo.openagents.probe"],
  summaryRef: "summary.action_submission.create_pr.test",
  toolRefs: ["tool.probe.propose_action_submission"],
  typedIntent: {
    targetRef: "repo.openagents.probe",
    titleRef: "intent.title.create_pr.test",
    diffRef: "artifact.diff.redacted_ref",
  },
});

describe("Probe Blueprint Action Submission boundary", () => {
  test("classifies external write-side effects as Action Submission required", () => {
    for (const effectKind of externalEffects) {
      const classification = classifyProbeRequestedEffect(effectKind);

      expect(classification.actionSubmissionRequired).toBe(true);
      expect(classification.directProgramRunExecutionAllowed).toBe(false);
      expect(classification.reasonRef).toContain(effectKind);
    }

    expect(classifyProbeRequestedEffect("local_sandbox_file_edit")).toMatchObject({
      actionSubmissionRequired: false,
      directProgramRunExecutionAllowed: true,
    });
  });

  test("denies direct Program Run authority for external writes", async () => {
    for (const effectKind of externalEffects) {
      await expect(Effect.runPromise(guardProgramRunDirectEffect(effectKind))).rejects.toMatchObject({
        _tag: "ProbeDirectExternalEffectDenied",
        actionSubmissionRequired: true,
        effectKind,
      });
    }

    await expect(Effect.runPromise(guardProgramRunDirectEffect("local_sandbox_file_edit"))).resolves.toMatchObject({
      directProgramRunExecutionAllowed: true,
    });
  });

  test("creates ref-first proposal records linked to Program Run evidence", async () => {
    const proposal = await Effect.runPromise(makeProbeActionSubmissionProposal(proposalInput()));

    expect(proposal.kind).toBe("probe_blueprint_action_submission_proposal");
    expect(proposal.effectKind).toBe("create_pull_request");
    expect(proposal.programRunRef).toBe("program_run.probe.apple_fm.test");
    expect(proposal.programRunAuthorityBoundary).toBe("evidence_only");
    expect(proposal.directExecution).toBe(false);
    expect(proposal.directProgramRunExecutionAllowed).toBe(false);
    expect(proposal.modelConfidenceBypassDisabled).toBe(true);
    expect(proposal.approvalRequired).toBe(true);
    expect(proposal.proposalOnly).toBe(true);
    expect(proposal.receiptRefs).toContain("receipt.action_submission");
    expect(proposal.evidenceRefs).toContain("evidence.program_run.summary_ref");
    expect(proposal.inputSnapshotHash).toStartWith("sha256:");
    expect(JSON.stringify(proposal)).not.toContain("raw email body");
  });

  test("rejects proposal records with private-data-shaped payloads", async () => {
    const proposal = await Effect.runPromise(makeProbeActionSubmissionProposal(proposalInput("send_email")));
    const unsafe: ProbeActionSubmissionProposal = {
      ...proposal,
      typedIntent: {
        rawEmail: "raw email body",
      },
    };

    await expect(Effect.runPromise(validateProbeActionSubmissionProposal(unsafe))).rejects.toMatchObject({
      _tag: "ProbeActionSubmissionProposalUnsafe",
    });
  });

  test("does not create Action Submission proposals for local Probe tool effects", async () => {
    await expect(
      Effect.runPromise(makeProbeActionSubmissionProposal(proposalInput("local_sandbox_file_edit"))),
    ).rejects.toMatchObject({
      _tag: "ProbeActionSubmissionProposalUnsafe",
    });
  });
});
