import { ForgeCollaborationProjection } from "@openagentsinc/forge-protocol";
import { Schema } from "effect";
import { describe, expect, test } from "vitest";

import type { ForgeGitProjectedEvent } from "./admission.js";
import { projectForgeCollaboration } from "./collaboration.js";

const pubkey = "a".repeat(64);
const event = (
  eventId: string,
  kind: ForgeGitProjectedEvent["kind"],
  tags: ReadonlyArray<ReadonlyArray<string>>,
  content: string,
  createdAt: string,
): ForgeGitProjectedEvent => ({
  actorBindingRef: "binding.forge.member",
  authorPubkey: pubkey,
  createdAt,
  eventId,
  eventJson: JSON.stringify({
    content,
    created_at: Math.floor(new Date(createdAt).getTime() / 1000),
    id: eventId,
    kind,
    pubkey,
    sig: "b".repeat(128),
    tags,
  }),
  kind,
  objectIds: [],
  repositoryRef: "omega",
  tenantRef: "OpenAgentsInc",
});

const at = "2026-07-26T08:00:00.000Z";
const changeId = "1".repeat(64);
const head = "2".repeat(40);
const base = "3".repeat(40);
const workRef = "work.forge.9248";

describe("owned Forge collaboration projection", () => {
  test("projects only a receipt-backed signed merge from admitted records", () => {
    const projection = projectForgeCollaboration(
      {
        changeRef: changeId,
        owner: "OpenAgentsInc",
        repo: "omega",
        view: "change",
      },
      [
        event(
          changeId,
          1617,
          [
            ["a", `30617:${pubkey}:omega`],
            ["subject", "Ship the owned collaboration read"],
            ["commit", head],
            ["parent-commit", base],
          ],
          "Patch body",
          "2026-07-26T07:56:00.000Z",
        ),
        event(
          "4".repeat(64),
          1111,
          [
            ["a", `30617:${pubkey}:omega`],
            ["E", changeId],
          ],
          "This comment came from the admitted event ledger.",
          "2026-07-26T07:57:00.000Z",
        ),
        event(
          "5".repeat(64),
          30618,
          [
            ["d", "omega"],
            ["refs/heads/main", head],
            ["forge-merge-receipt", "refs/heads/main", "receipt.forge.merge.1"],
          ],
          "",
          "2026-07-26T07:58:00.000Z",
        ),
      ],
      new Date(at),
    );

    expect(Schema.decodeUnknownSync(ForgeCollaborationProjection)(projection)).toEqual(projection);
    expect(projection.change).toMatchObject({
      base: { value: base },
      changeRef: changeId,
      head: { value: head },
      merge: { outcome: "merged", signedReceipt: { receiptRef: "receipt.forge.merge.1" } },
      proposalDialect: "standard_1617",
      proposalResolution: "resolved",
      title: "Ship the owned collaboration read",
    });
    expect(projection.change?.comments).toHaveLength(1);
    expect(projection.change?.reviews).toEqual([]);
    expect(projection.change?.checks[0]).toMatchObject({ name: "Merge gates", state: "passed" });
    expect(projection.change?.receipts[0]).toMatchObject({ receiptRef: "receipt.forge.merge.1" });
  });

  test("projects work blockers and keeps unresolved proposals non-actionable", () => {
    const rows = [
      event(
        changeId,
        1618,
        [
          ["a", `30617:${pubkey}:omega`],
          ["subject", "Unresolved pointer proposal"],
        ],
        "",
        "2026-07-26T07:50:00.000Z",
      ),
      event(
        "6".repeat(64),
        1621,
        [
          ["a", `30617:${pubkey}:omega`],
          ["sol.work_item", workRef],
          ["subject", "Integrate the Forge viewer"],
        ],
        "Use the owned projection.",
        "2026-07-26T07:51:00.000Z",
      ),
      event(
        "7".repeat(64),
        1630,
        [
          ["a", `30617:${pubkey}:omega`],
          ["sol.work_item", workRef],
          ["sol.actor", "binding.forge.worker"],
          ["sol.evidence_kind", "blocker"],
          ["sol.evidence", "Signed merge receipt is not available."],
        ],
        "Journey remains blocked.",
        "2026-07-26T07:52:00.000Z",
      ),
    ];
    const change = projectForgeCollaboration(
      { changeRef: changeId, owner: "OpenAgentsInc", repo: "omega", view: "change" },
      rows,
      new Date(at),
    );
    const work = projectForgeCollaboration(
      { owner: "OpenAgentsInc", repo: "omega", view: "work", workRef },
      rows,
      new Date(at),
    );

    expect(change.change?.proposalResolution).toBe("unresolved");
    expect(change.change?.state.state).toBe("blocked");
    expect(change.attention[0]?.kind).toBe("disagreement");
    expect(work.work).toMatchObject({
      actor: { value: "binding.forge.worker" },
      state: { state: "blocked" },
      workRef,
    });
    expect(work.work?.blockers[0]?.value).toBe("Signed merge receipt is not available.");
  });

  test("does not present a bare signed state as a completed merge", () => {
    const projection = projectForgeCollaboration(
      { changeRef: changeId, owner: "OpenAgentsInc", repo: "omega", view: "change" },
      [
        event(
          changeId,
          1617,
          [
            ["a", `30617:${pubkey}:omega`],
            ["commit", head],
            ["parent-commit", base],
          ],
          "Receipt must be present.",
          "2026-07-26T07:56:00.000Z",
        ),
        event(
          "8".repeat(64),
          30618,
          [
            ["d", "omega"],
            ["refs/heads/main", head],
          ],
          "",
          "2026-07-26T07:58:00.000Z",
        ),
      ],
      new Date(at),
    );
    expect(projection.change?.merge).toBeNull();
    expect(projection.change?.receipts).toEqual([]);
  });
});
