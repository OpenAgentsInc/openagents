// HARN-05: derive the desktop's coding-agent readiness through the ONE unified
// harness readiness projection (`projectHarnessReadiness`) instead of ad hoc
// per-consumer derivation. The Apple FM router candidate set, the Pylon-style
// capacity refs, and the admitted (ready) subset all read out of the same
// projection here — the single source the harvest analysis (H3/H5) calls for.
//
// This is behavior-preserving for the Apple FM candidate set: the same lanes,
// order, and `ready` flags come out; they are now routed through the shared
// projection so a second consumer never re-derives readiness a different way.

import {
  admittedHarnessIds,
  projectHarnessReadiness,
  type HarnessReadinessInput,
  type HarnessReadinessProjection,
} from "@openagentsinc/agent-harness-contract";

/** The Apple FM router candidate identity for each coding lane. */
export type HarnessCandidateId = "codex" | "claude" | "grok_acp";

interface LaneReadinessInput {
  readonly candidate: HarnessCandidateId;
  readonly ready: boolean;
}

const HARNESS_META: Readonly<
  Record<
    HarnessCandidateId,
    {
      readonly label: string;
      readonly harnessKind: HarnessReadinessInput["harnessKind"];
      readonly adapterKind: HarnessReadinessInput["adapterKind"];
    }
  >
> = {
  codex: { label: "Codex", harnessKind: "codex", adapterKind: "codex" },
  claude: { label: "Claude Code", harnessKind: "claude_code", adapterKind: "claude_code" },
  grok_acp: { label: "Grok", harnessKind: "grok_cli", adapterKind: "agent_client_protocol" },
};

/**
 * Project the live lane readiness into the unified harness readiness view. The
 * input order is preserved so downstream candidate ordering is stable.
 */
export const buildDesktopHarnessReadiness = (
  lanes: ReadonlyArray<LaneReadinessInput>,
): HarnessReadinessProjection => {
  const inputs: ReadonlyArray<HarnessReadinessInput> = lanes.map((lane) => {
    const meta = HARNESS_META[lane.candidate];
    return {
      harnessId: lane.candidate,
      harnessKind: meta.harnessKind,
      adapterKind: meta.adapterKind,
      ready: lane.ready,
    };
  });
  return projectHarnessReadiness(inputs);
};

/** Apple FM available-agent shape (mirrors the router's expected candidate). */
export interface HarnessAvailableAgent {
  readonly candidate: HarnessCandidateId;
  readonly label: string;
  readonly ready: boolean;
  readonly canDelegate: boolean;
}

/**
 * Derive the Apple FM candidate set from the unified projection. Every candidate
 * is delegatable; the `ready` flag is the projection's readiness — so the router
 * and the capacity refs come from one source.
 */
export const appleFmAgentsFromReadiness = (
  projection: HarnessReadinessProjection,
): ReadonlyArray<HarnessAvailableAgent> =>
  projection.candidates.map((candidate) => ({
    candidate: candidate.harnessId as HarnessCandidateId,
    label: HARNESS_META[candidate.harnessId as HarnessCandidateId].label,
    ready: candidate.ready,
    canDelegate: true,
  }));

/** The admitted (ready) candidate ids — the router's constrained vocabulary. */
export { admittedHarnessIds };
