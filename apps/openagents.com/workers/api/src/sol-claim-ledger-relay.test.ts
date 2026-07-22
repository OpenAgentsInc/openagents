import {
  type SolClaim,
  type SolClaimRelease,
  type SolClaimStatus,
  type SolClaimWorkItem,
  SOL_CLAIM_ISSUE_KIND,
  SOL_CLAIM_STATUS_APPLIED_KIND,
  SOL_CLAIM_STATUS_OPEN_KIND,
} from "@openagentsinc/forge-protocol";
import { getPublicKey } from "nostr-effect/pure";
import { describe, expect, it } from "vitest";

import {
  SOL_CLAIM_LEDGER_KINDS,
  SolClaimLedgerNotAnEntryError,
  SolClaimLedgerRelayFrameError,
  SolClaimLedgerSignatureError,
  SolClaimLedgerSignerKeyError,
  parseRelayEventMessage,
  recoverSolClaimLedgerEntryFromRelayMessage,
  signSolClaimLedgerEvent,
  signedEventToLedgerEvent,
  solClaimLedgerRepositoryFilter,
  solClaimLedgerSigner,
  solClaimReleaseToRelayPublishMessage,
  solClaimStatusToRelayPublishMessage,
  solClaimToRelayPublishMessage,
  solWorkItemToRelayPublishMessage,
  toRelayRequestMessage,
  toRelaySubscriptionMessage,
  verifiedSolClaimLedgerEntry,
} from "./sol-claim-ledger-relay";

// Deterministic test key material. Never a production key.
const secretKeyHex = "ab".repeat(32);
const createdAtEpochSeconds = 1_760_000_000;

const repositoryCoordinate = `30617:${getPublicKey(
  Uint8Array.from({ length: 32 }, () => 0xab),
)}:openagents`;

const signOptions = { secretKeyHex, createdAtEpochSeconds };

const workItem: SolClaimWorkItem = {
  work_item_ref: "sol:issue:9185",
  subject: "Move the Sol claim ledger to the owned relay",
  body: "Full Auto claim coordination should not depend on GitHub issues.",
  labels: ["forge", "stage-2"],
  repository: repositoryCoordinate,
  priority_ref: "sol:priority:p0",
  github_mirror_ref: "gh:OpenAgentsInc/openagents#9185",
};

const claim: SolClaim = {
  work_item_ref: "sol:issue:9185",
  actor: "agent:fleet_lane_a",
  base: "origin/main@96b6ca4710",
  worktree: "wt-9185",
  scope: "sol claim ledger signing + relay bridge",
  paths: [
    "apps/openagents.com/workers/api/src/sol-claim-ledger-relay.ts",
    "apps/openagents.com/workers/api/src/sol-claim-ledger-relay.test.ts",
  ],
  hot_files: [],
  hot_contracts: ["packages/forge-protocol/src/sol-claim-ledger.ts"],
  verification: "vitest run src/sol-claim-ledger-relay.test.ts",
  claimed_at: "2026-07-22T00:00:00.000Z",
  legitimacy: "self_selected",
  citations: ["docs/sol/CLAIM_PROTOCOL.md", "issue #9185"],
  message: "Claiming the signed publish/read slice.",
  repository: repositoryCoordinate,
  github_mirror_ref: "gh:OpenAgentsInc/openagents#9185",
};

const claimStatus: SolClaimStatus = {
  work_item_ref: "sol:issue:9185",
  actor: "agent:fleet_lane_a",
  state: "open",
  evidence_kind: "test",
  evidence: "Test Files 1 passed (1)",
  observed_at: "2026-07-22T00:30:00.000Z",
  message: "Round-trip test green.",
  repository: repositoryCoordinate,
};

const claimRelease: SolClaimRelease = {
  work_item_ref: "sol:issue:9185",
  actor: "agent:fleet_lane_a",
  outcome: "landed",
  landed_sha: "c".repeat(40),
  verification: "vitest run src/sol-claim-ledger-relay.test.ts",
  residual: "EventStore durability + live subscription remain.",
  message: "Signed publish/read landed.",
  repository: repositoryCoordinate,
};

describe("sol claim ledger relay bridge", () => {
  it("signs a claim, serializes a publish frame, parses it back, verifies, and recovers the claim", () => {
    // build record -> project -> sign -> serialize (publish frame)
    const message = solClaimToRelayPublishMessage(claim, signOptions);

    // The wire frame is a NIP-01 client->relay EVENT message.
    const decodedFrame = JSON.parse(message);
    expect(decodedFrame[0]).toBe("EVENT");
    expect(decodedFrame[1].kind).toBe(SOL_CLAIM_STATUS_OPEN_KIND);
    expect(decodedFrame[1].pubkey).toBe(solClaimLedgerSigner(secretKeyHex));

    // parse frame -> verify signature -> project back -> recover record
    const entry = recoverSolClaimLedgerEntryFromRelayMessage(message);
    expect(entry.type).toBe("claim");
    if (entry.type !== "claim") throw new Error("unreachable");
    expect(entry.claim).toEqual(claim);
    expect(entry.signer).toBe(solClaimLedgerSigner(secretKeyHex));
    expect(entry.createdAtEpochSeconds).toBe(createdAtEpochSeconds);
    expect(entry.eventId).toMatch(/^[0-9a-f]{64}$/);
  });

  it("round-trips the work item, claim-status, and claim-release entries", () => {
    const workItemEntry = recoverSolClaimLedgerEntryFromRelayMessage(
      solWorkItemToRelayPublishMessage(workItem, signOptions),
    );
    expect(workItemEntry.type).toBe("work_item");
    if (workItemEntry.type !== "work_item") throw new Error("unreachable");
    expect(workItemEntry.workItem).toEqual(workItem);

    const statusEntry = recoverSolClaimLedgerEntryFromRelayMessage(
      solClaimStatusToRelayPublishMessage(claimStatus, signOptions),
    );
    expect(statusEntry.type).toBe("claim_status");
    if (statusEntry.type !== "claim_status") throw new Error("unreachable");
    expect(statusEntry.claimStatus).toEqual(claimStatus);

    const releaseEntry = recoverSolClaimLedgerEntryFromRelayMessage(
      solClaimReleaseToRelayPublishMessage(claimRelease, signOptions),
    );
    expect(releaseEntry.type).toBe("claim_release");
    if (releaseEntry.type !== "claim_release") throw new Error("unreachable");
    expect(releaseEntry.claimRelease).toEqual(claimRelease);
  });

  it("recovers an entry from a relay->client subscription delivery frame", () => {
    const signed = signSolClaimLedgerEvent(
      // Reuse the work-item projection through the signer directly.
      { kind: SOL_CLAIM_ISSUE_KIND, tags: [], content: "" },
      signOptions,
    );
    // Prove the subscription frame shape parses (kind-only stub, no entry tags).
    const subscriptionMessage = toRelaySubscriptionMessage("sub-1", signed);
    const parsed = parseRelayEventMessage(subscriptionMessage);
    expect(parsed.id).toBe(signed.id);
    expect(parsed.sig).toBe(signed.sig);

    // A real entry recovered through the subscription frame.
    const workItemSigned = signSolClaimLedgerEvent(
      signedEventToLedgerEvent(
        parseRelayEventMessage(solWorkItemToRelayPublishMessage(workItem, signOptions)),
      ),
      signOptions,
    );
    const entry = verifiedSolClaimLedgerEntry(
      parseRelayEventMessage(toRelaySubscriptionMessage("sub-2", workItemSigned)),
    );
    expect(entry.type).toBe("work_item");
  });

  it("verifies the recovered event signature cryptographically after JSON transport", () => {
    // finalizeEvent sets an in-memory verified symbol; JSON transport drops it,
    // so the recovered event forces a real schnorr check, not a cached bool.
    const message = solClaimToRelayPublishMessage(claim, signOptions);
    const parsed = parseRelayEventMessage(message);
    expect(Object.getOwnPropertySymbols(parsed)).toHaveLength(0);
    // Recovery succeeds only because the signature is valid.
    expect(() => verifiedSolClaimLedgerEntry(parsed)).not.toThrow();
  });

  it("rejects a tampered event with a signature error", () => {
    const message = solClaimToRelayPublishMessage(claim, signOptions);
    const parsed = parseRelayEventMessage(message);
    // Mutate content after signing: id/sig no longer match.
    parsed.content = "tampered coordination message";
    expect(() => verifiedSolClaimLedgerEntry(parsed)).toThrow(SolClaimLedgerSignatureError);
  });

  it("rejects an event whose id was left intact but content changed", () => {
    const message = solClaimToRelayPublishMessage(claim, signOptions);
    const parsed = parseRelayEventMessage(message);
    parsed.tags = [...parsed.tags, ["sol.injected", "not-signed"]];
    expect(() => verifiedSolClaimLedgerEntry(parsed)).toThrow(SolClaimLedgerSignatureError);
  });

  it("rejects a validly signed event that is not a ledger entry", () => {
    const signed = signSolClaimLedgerEvent(
      { kind: SOL_CLAIM_STATUS_OPEN_KIND, tags: [["t", "unrelated"]], content: "hi" },
      signOptions,
    );
    expect(() => verifiedSolClaimLedgerEntry(signed)).toThrow(SolClaimLedgerNotAnEntryError);
  });

  it("never leaks the signer secret key into the frame", () => {
    const message = solClaimToRelayPublishMessage(claim, signOptions);
    expect(message).not.toContain(secretKeyHex);
    // The derived public key differs from the secret and is present.
    expect(message).toContain(solClaimLedgerSigner(secretKeyHex));
  });

  it("rejects a malformed signer key without echoing it", () => {
    expect(() => solClaimLedgerSigner("nothex")).toThrow(SolClaimLedgerSignerKeyError);
    try {
      signSolClaimLedgerEvent(
        { kind: 1, tags: [], content: "" },
        {
          secretKeyHex: "zz".repeat(32),
          createdAtEpochSeconds,
        },
      );
      throw new Error("expected throw");
    } catch (error) {
      expect(error).toBeInstanceOf(SolClaimLedgerSignerKeyError);
      expect((error as Error).message).not.toContain("zz");
    }
  });

  it("rejects malformed relay frames", () => {
    expect(() => parseRelayEventMessage("{ not json")).toThrow(SolClaimLedgerRelayFrameError);
    expect(() => parseRelayEventMessage(JSON.stringify(["NOTICE", "x"]))).toThrow(
      SolClaimLedgerRelayFrameError,
    );
    expect(() => parseRelayEventMessage(JSON.stringify(["EVENT", { kind: 1 }]))).toThrow(
      SolClaimLedgerRelayFrameError,
    );
  });

  it("builds a repository-coordinate read filter over all five ledger kinds", () => {
    const filter = solClaimLedgerRepositoryFilter(repositoryCoordinate, {
      workItemRef: "sol:issue:9185",
      authors: [solClaimLedgerSigner(secretKeyHex)],
      sinceEpochSeconds: createdAtEpochSeconds,
      limit: 200,
    });
    expect(filter.kinds).toEqual([...SOL_CLAIM_LEDGER_KINDS]);
    expect(filter.kinds).toContain(SOL_CLAIM_ISSUE_KIND);
    expect(filter.kinds).toContain(SOL_CLAIM_STATUS_APPLIED_KIND);
    expect(filter["#a"]).toEqual([repositoryCoordinate]);
    expect(filter["#sol.work_item"]).toEqual(["sol:issue:9185"]);
    expect(filter.authors).toEqual([solClaimLedgerSigner(secretKeyHex)]);
    expect(filter.since).toBe(createdAtEpochSeconds);
    expect(filter.limit).toBe(200);

    const req = JSON.parse(toRelayRequestMessage("sol-ledger", filter));
    expect(req[0]).toBe("REQ");
    expect(req[1]).toBe("sol-ledger");
    expect(req[2].kinds).toEqual([...SOL_CLAIM_LEDGER_KINDS]);
  });

  it("allows narrowing the read filter to a subset of kinds", () => {
    const filter = solClaimLedgerRepositoryFilter(repositoryCoordinate, {
      kinds: [SOL_CLAIM_STATUS_OPEN_KIND],
    });
    expect(filter.kinds).toEqual([SOL_CLAIM_STATUS_OPEN_KIND]);
    expect(filter["#sol.work_item"]).toBeUndefined();
    expect(filter.authors).toBeUndefined();
  });
});
