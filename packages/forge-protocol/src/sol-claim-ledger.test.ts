import { describe, expect, test } from "vite-plus/test";

import {
  SOL_CLAIM_ISSUE_KIND,
  SOL_CLAIM_LEDGER_SCHEMA_VERSION,
  SOL_CLAIM_STALE_AFTER_MS,
  SOL_CLAIM_STATUS_APPLIED_KIND,
  SOL_CLAIM_STATUS_CLOSED_KIND,
  SOL_CLAIM_STATUS_DRAFT_KIND,
  SOL_CLAIM_STATUS_OPEN_KIND,
  type SolClaim,
  type SolClaimRelease,
  type SolClaimStatus,
  type SolClaimWorkItem,
  decodeSolClaim,
  decodeSolClaimRelease,
  decodeSolClaimStatus,
  decodeSolClaimWorkItem,
  isSolClaimStale,
  parseSolClaimEvent,
  parseSolClaimReleaseEvent,
  parseSolClaimStatusEvent,
  parseSolWorkItemEvent,
  solClaimCollision,
  solClaimLedgerEntryType,
  solClaimReleaseCoordinationState,
  solClaimReleaseToLedgerEvent,
  solClaimStatusToLedgerEvent,
  solClaimToLedgerEvent,
  solClaimsCollide,
  solWorkItemToLedgerEvent,
} from "./sol-claim-ledger.js";

const repo = "30617:abcabcabcabcabcabcabcabcabcabcabcabcabcabcabcabcabcabcabcabcabca:openagents";

const workItem: SolClaimWorkItem = {
  work_item_ref: "issue:9185",
  subject: "Move the Sol claim ledger to the owned relay",
  body: "Forge Stage 2: move the claim/status ledger to NIP-34 events.",
  labels: ["roadmap:sol", "forge"],
  repository: repo,
  priority_ref: "prio:0-pr-burndown",
  github_mirror_ref: "github:OpenAgentsInc/openagents#9185",
};

const claim: SolClaim = {
  work_item_ref: "issue:9185",
  actor: "fleet-lane:forge-stage-2",
  base: "001cbab690a3ef900696f7325df1e34ea8cf7da8",
  worktree: "wt-9185",
  scope: "Sol claim ledger NIP-34 projection in forge-protocol",
  paths: ["packages/forge-protocol/src/sol-claim-ledger.ts"],
  hot_files: ["packages/forge-protocol/src/index.ts"],
  hot_contracts: ["forge-protocol schema version"],
  verification: "vitest run sol-claim-ledger.test.ts",
  claimed_at: "2026-07-22T18:00:00.000Z",
  legitimacy: "issue",
  citations: ["issue:9185", "docs/forge/2026-07-22-nostr-git-forge-github-replacement-audit.md"],
  message: "CLAIM: forge Stage 2 ledger profile.",
  repository: repo,
  root_event_id: "eventid".padEnd(64, "0"),
  github_mirror_ref: "github:OpenAgentsInc/openagents#9185",
};

const selfClaim: SolClaim = {
  work_item_ref: "work-packet:sol-ledger-profile",
  actor: "full-auto:run-1",
  base: "001cbab690a3ef900696f7325df1e34ea8cf7da8",
  worktree: "wt-selfclaim",
  scope: "self-selected leaf",
  paths: ["packages/forge-protocol/src/sol-claim-ledger.ts"],
  hot_files: [],
  hot_contracts: [],
  verification: "vitest run",
  claimed_at: "2026-07-22T18:05:00.000Z",
  legitimacy: "self_selected",
  citations: ["docs/sol/CLAIM_PROTOCOL.md"],
  message: "CLAIM (self_selected).",
};

describe("@openagentsinc/forge-protocol sol claim ledger", () => {
  test("exports the ledger schema version and NIP-34 kinds", () => {
    expect(SOL_CLAIM_LEDGER_SCHEMA_VERSION).toBe("openagents.sol.claim-ledger.v0.1");
    expect(SOL_CLAIM_ISSUE_KIND).toBe(1621);
    expect(SOL_CLAIM_STATUS_OPEN_KIND).toBe(1630);
    expect(SOL_CLAIM_STATUS_APPLIED_KIND).toBe(1631);
    expect(SOL_CLAIM_STATUS_CLOSED_KIND).toBe(1632);
    expect(SOL_CLAIM_STATUS_DRAFT_KIND).toBe(1633);
  });

  test("work item round-trips through a kind 1621 issue event", () => {
    const event = solWorkItemToLedgerEvent(workItem);
    expect(event.kind).toBe(SOL_CLAIM_ISSUE_KIND);
    expect(solClaimLedgerEntryType(event)).toBe("work_item");
    expect(parseSolWorkItemEvent(event)).toEqual(workItem);
    expect(decodeSolClaimWorkItem(parseSolWorkItemEvent(event))).toEqual(workItem);
  });

  test("work item round-trips with only required fields", () => {
    const minimal: SolClaimWorkItem = {
      work_item_ref: "issue:1",
      subject: "s",
      body: "b",
      labels: [],
    };
    const event = solWorkItemToLedgerEvent(minimal);
    expect(parseSolWorkItemEvent(event)).toEqual(minimal);
  });

  test("claim round-trips through a kind 1630 status event", () => {
    const event = solClaimToLedgerEvent(claim);
    expect(event.kind).toBe(SOL_CLAIM_STATUS_OPEN_KIND);
    expect(solClaimLedgerEntryType(event)).toBe("claim");
    const parsed = parseSolClaimEvent(event);
    expect(parsed).toEqual(claim);
    expect(decodeSolClaim(parsed)).toEqual(claim);
  });

  test("claim preserves every claim-protocol field verbatim", () => {
    const parsed = parseSolClaimEvent(solClaimToLedgerEvent(claim));
    expect(parsed.actor).toBe(claim.actor);
    expect(parsed.base).toBe(claim.base);
    expect(parsed.worktree).toBe(claim.worktree);
    expect(parsed.scope).toBe(claim.scope);
    expect(parsed.paths).toEqual(claim.paths);
    expect(parsed.hot_files).toEqual(claim.hot_files);
    expect(parsed.hot_contracts).toEqual(claim.hot_contracts);
    expect(parsed.verification).toBe(claim.verification);
    expect(parsed.claimed_at).toBe(claim.claimed_at);
    expect(parsed.legitimacy).toBe(claim.legitimacy);
    expect(parsed.citations).toEqual(claim.citations);
  });

  test("self-selected claim round-trips and keeps its legitimacy basis", () => {
    const parsed = parseSolClaimEvent(solClaimToLedgerEvent(selfClaim));
    expect(parsed).toEqual(selfClaim);
    expect(parsed.legitimacy).toBe("self_selected");
    expect(parsed.citations.length).toBeGreaterThan(0);
  });

  test("claim status round-trips as open (1630)", () => {
    const status: SolClaimStatus = {
      work_item_ref: "issue:9185",
      actor: "fleet-lane:forge-stage-2",
      state: "open",
      evidence_kind: "commit",
      evidence: "deadbeef",
      observed_at: "2026-07-22T18:30:00.000Z",
      message: "CLAIM-STATUS: committed.",
      repository: repo,
      root_event_id: "eventid".padEnd(64, "0"),
    };
    const event = solClaimStatusToLedgerEvent(status);
    expect(event.kind).toBe(SOL_CLAIM_STATUS_OPEN_KIND);
    const parsed = parseSolClaimStatusEvent(event);
    expect(parsed).toEqual(status);
    expect(decodeSolClaimStatus(parsed)).toEqual(status);
  });

  test("blocked claim status round-trips as draft (1633)", () => {
    const status: SolClaimStatus = {
      work_item_ref: "issue:9185",
      actor: "fleet-lane:forge-stage-2",
      state: "draft",
      evidence_kind: "blocker",
      evidence: "waiting on owner relay",
      observed_at: "2026-07-22T18:40:00.000Z",
      message: "CLAIM-STATUS: blocked.",
    };
    const event = solClaimStatusToLedgerEvent(status);
    expect(event.kind).toBe(SOL_CLAIM_STATUS_DRAFT_KIND);
    expect(parseSolClaimStatusEvent(event)).toEqual(status);
  });

  test("landed release round-trips as applied (1631)", () => {
    const release: SolClaimRelease = {
      work_item_ref: "issue:9185",
      actor: "fleet-lane:forge-stage-2",
      outcome: "landed",
      landed_sha: "cafebabe",
      verification: "vitest run",
      residual: "relay stand-up + load test remain",
      message: "CLAIM-RELEASE: landed.",
      repository: repo,
    };
    const event = solClaimReleaseToLedgerEvent(release);
    expect(event.kind).toBe(SOL_CLAIM_STATUS_APPLIED_KIND);
    const parsed = parseSolClaimReleaseEvent(event);
    expect(parsed).toEqual(release);
    expect(decodeSolClaimRelease(parsed)).toEqual(release);
    expect(solClaimReleaseCoordinationState(parsed)).toBe("applied");
  });

  test("not-landed release round-trips as closed (1632)", () => {
    const release: SolClaimRelease = {
      work_item_ref: "issue:9185",
      actor: "fleet-lane:forge-stage-2",
      outcome: "not_landed",
      disposition: "superseded",
      verification: "n/a",
      residual: "none",
      message: "CLAIM-RELEASE: superseded.",
    };
    const event = solClaimReleaseToLedgerEvent(release);
    expect(event.kind).toBe(SOL_CLAIM_STATUS_CLOSED_KIND);
    expect(parseSolClaimReleaseEvent(event)).toEqual(release);
    expect(solClaimReleaseCoordinationState(release)).toBe("closed");
  });

  test("wrong-kind and mistagged events are rejected", () => {
    const claimEvent = solClaimToLedgerEvent(claim);
    expect(() => parseSolWorkItemEvent(claimEvent)).toThrow();
    expect(() => parseSolClaimEvent(solWorkItemToLedgerEvent(workItem))).toThrow();
    const mistagged = {
      ...claimEvent,
      tags: claimEvent.tags.map((t) => (t[0] === "sol" ? ["sol", "work_item"] : t)),
    };
    expect(() => parseSolClaimEvent(mistagged)).toThrow();
  });

  describe("staleness (90-minute window plus audit)", () => {
    test("window value is 90 minutes", () => {
      expect(SOL_CLAIM_STALE_AFTER_MS).toBe(90 * 60 * 1000);
    });

    test("elapsed time alone never authorizes taking a claim", () => {
      expect(
        isSolClaimStale({
          last_evidence_at: "2026-07-22T18:00:00.000Z",
          now: "2026-07-22T21:00:00.000Z", // 3h later
          audit_found_active_work: true,
        }),
      ).toBe(false);
    });

    test("stale requires both the window passed and the audit finding no work", () => {
      expect(
        isSolClaimStale({
          last_evidence_at: "2026-07-22T18:00:00.000Z",
          now: "2026-07-22T19:31:00.000Z", // 91m later
          audit_found_active_work: false,
        }),
      ).toBe(true);
    });

    test("fresh evidence is never stale even with no active work found", () => {
      expect(
        isSolClaimStale({
          last_evidence_at: "2026-07-22T18:00:00.000Z",
          now: "2026-07-22T18:45:00.000Z", // 45m later
          audit_found_active_work: false,
        }),
      ).toBe(false);
    });
  });

  describe("collision semantics", () => {
    test("same work item collides", () => {
      const a = { ...claim, paths: ["a"], hot_files: [], hot_contracts: [] };
      const b = { ...claim, paths: ["b"], hot_files: [], hot_contracts: [] };
      const result = solClaimCollision(a, b);
      expect(result.collides).toBe(true);
      expect(result.reasons).toContain("same_work_item");
    });

    test("shared path collides across different work items", () => {
      const a = { ...claim, work_item_ref: "issue:1", paths: ["x", "y"] };
      const b = {
        ...selfClaim,
        work_item_ref: "issue:2",
        paths: ["y", "z"],
      };
      const result = solClaimCollision(a, b);
      expect(result.collides).toBe(true);
      expect(result.reasons).toContain("shared_path");
      expect(result.shared_paths).toEqual(["y"]);
    });

    test("shared hot contract collides", () => {
      const a = {
        ...claim,
        work_item_ref: "issue:1",
        paths: ["a"],
        hot_contracts: ["migrations:0251"],
      };
      const b = {
        ...selfClaim,
        work_item_ref: "issue:2",
        paths: ["b"],
        hot_contracts: ["migrations:0251"],
      };
      expect(solClaimsCollide(a, b)).toBe(true);
      expect(solClaimCollision(a, b).reasons).toContain("shared_hot_contract");
    });

    test("disjoint claims on different work items do not collide", () => {
      const a = {
        ...claim,
        work_item_ref: "issue:1",
        paths: ["a"],
        hot_files: ["f1"],
        hot_contracts: ["c1"],
      };
      const b = {
        ...selfClaim,
        work_item_ref: "issue:2",
        paths: ["b"],
        hot_files: ["f2"],
        hot_contracts: ["c2"],
      };
      expect(solClaimsCollide(a, b)).toBe(false);
      expect(solClaimCollision(a, b).reasons).toEqual([]);
    });
  });
});
