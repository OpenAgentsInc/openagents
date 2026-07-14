import { describe, expect, test } from "vite-plus/test";

import { decodeFleetRunClientProjection } from "./fleet-run-client-projection.js";

const timestamp = "2026-07-13T10:55:20.179Z";

describe("FleetRun client projection", () => {
  test("decodes one bounded refs-only hybrid run", () => {
    const projection = decodeFleetRunClientProjection({
      schema: "openagents.fleet_run_client_projection.v1",
      privateMaterialExcluded: true,
      generatedAt: timestamp,
      runs: [
        {
          runRef: "fleet_run.sarah.f566771758bbe0ab5fc5",
          authorityStatus: "claimed_by_pylon",
          executionState: "completed",
          lastSequence: 96,
          attempts: [
            {
              workUnitRef: "unit.fc4.owner_local.acceptance.202607131047",
              workClaimRef:
                "fleet_run.sarah.f566771758bbe0ab5fc5.claim.owner_local",
              intakeClaimRef: "claim.sarah_fleet_run.0123456789abcdef01234567",
              assignmentRef: "assignment.public.khala_coding.chatcmpl_c9db1507",
              accountRefHash: "account.pylon.codex.f88a4773edd26cae162ceb2f",
              requestedTarget: "owner_local",
              selectedTarget: "owner_local",
              fallback: { truth: "not_applicable" },
              outcome: "accepted",
              closeoutRef:
                "assignment.closeout.summary.e2d06ebe9e9eaf48dd9e8d74",
              artifactRefs: ["artifact.pylon.codex_agent_task.patch.7c1f592e"],
              proofRefs: ["proof.pylon.codex_agent_task.test.fcf8dc5d"],
              authorityReceiptRefs: [],
              usageTruth: "exact",
              usageEvidenceRef:
                "event.inference.served-tokens.pylon-codex.21e4110b85f1e525f26ea55231050aa0",
              tokenUsageRefs: [
                "event.inference.served-tokens.pylon-codex.21e4110b85f1e525f26ea55231050aa0",
              ],
              usageCaveatRefs: [],
              blockerRefs: [],
              terminalAt: timestamp,
              updatedAt: timestamp,
            },
          ],
          createdAt: timestamp,
          updatedAt: timestamp,
        },
      ],
    });

    expect(projection.runs[0]?.attempts[0]).toMatchObject({
      requestedTarget: "owner_local",
      selectedTarget: "owner_local",
      outcome: "accepted",
    });
  });

  test("rejects private-shaped and overlong projections", () => {
    expect(() =>
      decodeFleetRunClientProjection({
        schema: "openagents.fleet_run_client_projection.v1",
        privateMaterialExcluded: true,
        generatedAt: timestamp,
        runs: [
          {
            runRef: "fleet_run.sarah.f566771758bbe0ab5fc5",
            authorityStatus: "claimed_by_pylon",
            executionState: "completed",
            lastSequence: 96,
            attempts: [
              {
                workUnitRef: "/Users/private/repo",
                workClaimRef: "claim.valid",
                intakeClaimRef:
                  "claim.sarah_fleet_run.0123456789abcdef01234567",
                assignmentRef: null,
                accountRefHash: null,
                requestedTarget: "owner_local",
                selectedTarget: "owner_local",
                fallback: { truth: "not_applicable" },
                outcome: "accepted",
                closeoutRef: null,
                artifactRefs: [],
                proofRefs: [],
                authorityReceiptRefs: [],
                usageTruth: "pending",
                usageEvidenceRef: null,
                tokenUsageRefs: [],
                usageCaveatRefs: [],
                blockerRefs: [],
                terminalAt: timestamp,
                updatedAt: timestamp,
              },
            ],
            createdAt: timestamp,
            updatedAt: timestamp,
          },
        ],
      }),
    ).toThrow();
  });
});
