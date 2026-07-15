import { describe, expect, test } from "vite-plus/test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { evaluateNativeAc03Observation } from "./native-ac03-observation.ts";

type EvidenceAnchor = Readonly<{ path: string; tokens: ReadonlyArray<string> }>;
type NativeCriterionContract = Readonly<{
  criterion: `CW-AC-${string}`;
  requiredCapability: string;
  sharedAnchors: ReadonlyArray<EvidenceAnchor>;
  nativeIntegrationAnchors: ReadonlyArray<EvidenceAnchor>;
  blocker: string;
}>;

const root = resolve(import.meta.dirname, "../../..");

/**
 * Native-owned criterion catalog. Shared anchors establish reusable contracts,
 * but a candidate cannot confirm until at least one target-specific integration
 * anchor exists and every anchor is present. This prevents Electron tests or
 * the generic headed shell gate from being relabeled as Native MVP evidence.
 */
export const nativeMvpCriterionContracts: ReadonlyArray<NativeCriterionContract> = [
  {
    criterion: "CW-AC-01",
    requiredCapability: "signed Native app lifecycle and bundled compatible Codex",
    sharedAnchors: [],
    nativeIntegrationAnchors: [],
    blocker:
      "Native app/DMG signing, notarization, stapling, install, and bundled Codex resolution are absent.",
  },
  {
    criterion: "CW-AC-02",
    requiredCapability: "ordinary logged-in Codex session custody",
    sharedAnchors: [
      { path: "apps/openagents-desktop/src/codex-preflight.ts", tokens: ["CODEX_HOME"] },
    ],
    nativeIntegrationAnchors: [],
    blocker: "The fixture starts no real Codex runtime or current-session preflight.",
  },
  {
    criterion: "CW-AC-03",
    requiredCapability: "granted repository and durable work identity",
    sharedAnchors: [
      {
        path: "apps/openagents-desktop/src/desktop-coding-catalog.ts",
        tokens: ["DesktopWorkspaceAdmission", "workContextRef", "sessionRef"],
      },
      {
        path: "apps/openagents-desktop/src/desktop-workspace-admission.ts",
        tokens: ["openAdmittedDesktopWorkspace", "grantRef"],
      },
    ],
    nativeIntegrationAnchors: [
      {
        path: "apps/openagents-desktop/src/native-sidecar-contract.ts",
        tokens: ["coding.admit", "openDesktopNativeSidecarService", "request_conflict"],
      },
      {
        path: "apps/native-sdk-effect-native-spike/src/main.zig",
        tokens: ["request_repository_admit", "canvas_widget_file_drop", "coding.admit"],
      },
      {
        path: "apps/native-sdk-effect-native-spike/scripts/run-host-smoke.ts",
        tokens: ["repository-grant-admitted", "repository-identity-restored", "aliasCanonicalized"],
      },
    ],
    blocker:
      "The Native grant must retain one production work/session identity across a real host restart.",
  },
  {
    criterion: "CW-AC-04",
    requiredCapability: "real ProductSpec authoring and executable validation",
    sharedAnchors: [
      {
        path: "packages/product-spec/test/product-spec.test.ts",
        tokens: ["duplicate criterion IDs refuse executable admission"],
      },
    ],
    nativeIntegrationAnchors: [],
    blocker:
      "The Native renderer and host do not expose ProductSpec open/create/validate/dispatch behavior.",
  },
  {
    criterion: "CW-AC-05",
    requiredCapability: "confirmed ProductSpec revision and reconciliation",
    sharedAnchors: [
      {
        path: "apps/openagents-desktop/src/product-spec-workroom.ts",
        tokens: ["revision_not_incremented"],
      },
    ],
    nativeIntegrationAnchors: [],
    blocker: "Effect projection revisions are not ProductSpec revisions.",
  },
  {
    criterion: "CW-AC-06",
    requiredCapability: "durable plan, packet, lease, dependency, and child allocation",
    sharedAnchors: [
      {
        path: "apps/openagents-desktop/src/product-spec-workroom.ts",
        tokens: ["dependencyRefs", "leaseRef"],
      },
    ],
    nativeIntegrationAnchors: [],
    blocker: "The fixture has no plan graph, packet lease, or child allocation.",
  },
  {
    criterion: "CW-AC-07",
    requiredCapability: "pinned proposal-only ProductSpec skills",
    sharedAnchors: [
      {
        path: "apps/openagents-desktop/src/builtin-productspec-skill.ts",
        tokens: ["ProductSpecWorkSkillSha256"],
      },
    ],
    nativeIntegrationAnchors: [],
    blocker: "Native resources do not bundle or verify the ProductSpec skills.",
  },
  {
    criterion: "CW-AC-08",
    requiredCapability: "proposal-only agent tools with separate owner authority",
    sharedAnchors: [
      {
        path: "apps/openagents-desktop/src/product-spec-app-server-tools.ts",
        tokens: ["record_evidence", "propose_plan"],
      },
    ],
    nativeIntegrationAnchors: [],
    blocker: "The closed navigation bridge is not the ProductSpec app-server tool catalog.",
  },
  {
    criterion: "CW-AC-09",
    requiredCapability: "exact active-spec identity dispatch gate",
    sharedAnchors: [
      {
        path: "apps/openagents-desktop/src/product-spec-workroom.ts",
        tokens: ["revision_mismatch", "superseded"],
      },
    ],
    nativeIntegrationAnchors: [],
    blocker: "Stale renderer projection rejection is not active ProductSpec mismatch handling.",
  },
  {
    criterion: "CW-AC-10",
    requiredCapability: "real top-level history catalog and paging",
    sharedAnchors: [
      { path: "apps/openagents-desktop/src/renderer/history-workspace.ts", tokens: ["restore"] },
    ],
    nativeIntegrationAnchors: [],
    blocker: "The three-row fixture list has no metadata-first hydration or unbounded-age paging.",
  },
  {
    criterion: "CW-AC-11",
    requiredCapability: "real admitted root Codex turn with text, tool, and terminal evidence",
    sharedAnchors: [
      { path: "apps/openagents-desktop/src/codex-app-server-turn.ts", tokens: ["turn"] },
    ],
    nativeIntegrationAnchors: [],
    blocker:
      "The fixture only appends local deterministic messages and never executes a provider turn.",
  },
  {
    criterion: "CW-AC-12",
    requiredCapability: "shared typed command identities and durable idempotency",
    sharedAnchors: [
      {
        path: "apps/openagents-desktop/src/desktop-command-contract.ts",
        tokens: ["DesktopDeferredCommand"],
      },
    ],
    nativeIntegrationAnchors: [],
    blocker:
      "Bespoke Spike intents do not prove the production send/stop/steer/approval/review command set.",
  },
  {
    criterion: "CW-AC-13",
    requiredCapability: "nested child identity, graph, and independent transcript",
    sharedAnchors: [
      {
        path: "apps/openagents-desktop/src/local-runtime-event-persistence.ts",
        tokens: ["parent"],
      },
    ],
    nativeIntegrationAnchors: [],
    blocker: "The fixture creates no child agent or durable child event stream.",
  },
  {
    criterion: "CW-AC-14",
    requiredCapability: "grant-scoped workspace tree and typed read-only Git",
    sharedAnchors: [
      { path: "apps/openagents-desktop/src/workspace-service.ts", tokens: ["relative"] },
    ],
    nativeIntegrationAnchors: [],
    blocker: "The Native app has no folder grant, workspace service, status, or diff.",
  },
  {
    criterion: "CW-AC-15",
    requiredCapability: "host-owned exact-prefix recovery and handoff",
    sharedAnchors: [
      { path: "apps/openagents-desktop/src/codex-handoff-host.ts", tokens: ["handoff"] },
    ],
    nativeIntegrationAnchors: [],
    blocker: "localStorage fixture selection restore is not durable in-flight work recovery.",
  },
  {
    criterion: "CW-AC-16",
    requiredCapability: "typed delivery, replay, generation, and provider failures",
    sharedAnchors: [
      { path: "apps/openagents-desktop/src/provider-runtime-host.ts", tokens: ["incompatible"] },
    ],
    nativeIntegrationAnchors: [],
    blocker:
      "Monotonic fixture revisions do not exercise lost acknowledgements, cursor gaps, grants, auth, policy, or quota failures.",
  },
  {
    criterion: "CW-AC-17",
    requiredCapability: "bounded production bridge and privacy-safe diagnostics",
    sharedAnchors: [
      { path: "apps/openagents-desktop/src/diagnostics-host.ts", tokens: ["redact"] },
    ],
    nativeIntegrationAnchors: [],
    blocker:
      "The fixture bridge is bounded but does not cover production methods or diagnostics leakage falsifiers.",
  },
  {
    criterion: "CW-AC-18",
    requiredCapability:
      "fresh exact-candidate install, update, rollback, diagnostics, and uninstall journey",
    sharedAnchors: [
      { path: "apps/openagents-desktop/src/update-staging-host.ts", tokens: ["rollback"] },
    ],
    nativeIntegrationAnchors: [],
    blocker: "The headed binary restart is not an installed release lifecycle.",
  },
];

const anchorSatisfied = (anchor: EvidenceAnchor): boolean => {
  const source = readFileSync(resolve(root, anchor.path), "utf8");
  return anchor.tokens.every((token) => source.includes(token));
};

const candidateSatisfied = (contract: NativeCriterionContract): boolean =>
  contract.nativeIntegrationAnchors.length > 0 &&
  [...contract.sharedAnchors, ...contract.nativeIntegrationAnchors].every(anchorSatisfied);

describe("OpenAgents Native SDK MVP criterion evidence", () => {
  const nativeAc03Candidate = {
    schema: "openagents.native-sdk.cw-ac-03.v1",
    criterionRef: "CW-AC-03",
    grantSource: "native_canvas_file_drop",
    initial: {
      generation: 1,
      sidecarPid: 4_201,
      catalogSessionCount: 1,
      admission: {
        grantRef: "grant.native.test",
        projectRef: "project.native.test",
        repositoryRef: "repository.native.test",
        worktreeRef: "worktree.native.test",
        workContextRef: "work-context.native.test",
        sessionRef: "session.native.test",
      },
    },
    restarted: {
      generation: 2,
      sidecarPid: 9_902,
      catalogSessionCount: 1,
      admission: {
        grantRef: "grant.native.test",
        projectRef: "project.native.test",
        repositoryRef: "repository.native.test",
        worktreeRef: "worktree.native.test",
        workContextRef: "work-context.native.test",
        sessionRef: "session.native.test",
      },
    },
    aliasCanonicalized: true,
    ambientInputsExcluded: true,
    privateBindingMode: "0600",
  } as const;

  for (const contract of nativeMvpCriterionContracts) {
    test(`${contract.criterion} candidate evidence remains bound`, () => {
      expect(candidateSatisfied(contract), contract.blocker).toBe(true);
      if (contract.criterion === "CW-AC-03") {
        expect(evaluateNativeAc03Observation(nativeAc03Candidate)).toMatchObject({
          verdict: "confirmed",
        });
      }
    });

    test(`${contract.criterion} missing-anchor falsifier is rejected`, () => {
      const mutated = { ...contract, nativeIntegrationAnchors: [] };
      expect(candidateSatisfied(mutated), contract.requiredCapability).toBe(false);
      if (contract.criterion === "CW-AC-03") {
        const falsifiers = [
          {
            ...nativeAc03Candidate,
            restarted: {
              ...nativeAc03Candidate.restarted,
              sidecarPid: nativeAc03Candidate.initial.sidecarPid,
            },
          },
          {
            ...nativeAc03Candidate,
            restarted: { ...nativeAc03Candidate.restarted, catalogSessionCount: 2 },
          },
          {
            ...nativeAc03Candidate,
            restarted: {
              ...nativeAc03Candidate.restarted,
              admission: {
                ...nativeAc03Candidate.restarted.admission,
                sessionRef: "session.drifted",
              },
            },
          },
          { ...nativeAc03Candidate, aliasCanonicalized: false },
          { ...nativeAc03Candidate, privateBindingMode: "0644" },
          { ...nativeAc03Candidate, ambientInputsExcluded: false },
          { ...nativeAc03Candidate, excess: true },
        ];
        expect(
          falsifiers.every(
            (value) =>
              evaluateNativeAc03Observation(value, ["/private/repository", "provider-thread.test"])
                .verdict === "refuted",
          ),
        ).toBe(true);
        expect(
          evaluateNativeAc03Observation(
            {
              ...nativeAc03Candidate,
              initial: {
                ...nativeAc03Candidate.initial,
                admission: {
                  ...nativeAc03Candidate.initial.admission,
                  sessionRef: "/private/repository",
                },
              },
            },
            ["/private/repository"],
          ).verdict,
        ).toBe("refuted");
      }
    });
  }
});
