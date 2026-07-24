import { describe, expect, it } from "vite-plus/test";

import {
  FORBIDDEN_SARAH_AUTHORITY_MARKERS,
  SARAH_COMMUNITY_TICK_DECOMPOSITION_SCHEMA,
  SARAH_COMMUNITY_UNIT_AUTHORITY_CLASS,
  SARAH_COMMUNITY_WORK_UNIT_GRANT_SCHEMA,
  SARAH_COMMUNITY_WORK_UNIT_MAX_PER_TICK,
  SARAH_COMMUNITY_WORK_UNIT_SCHEMA,
  SARAH_CW_03_ISSUE,
  SARAH_CW_03_PACKET,
  SarahCommunityWorkUnitError,
  assertGrantActive,
  assertNoSarahGrant,
  buildSarahCommunityWorkUnit,
  buildSarahCommunityWorkUnitGrant,
  decodeSarahCommunityTickDecomposition,
  decodeSarahCommunityWorkUnit,
  decomposeSarahTickToWorkUnits,
  isGrantActive,
  nip40ExpirationTag,
  wireTagsForActiveGrant,
  type SarahCommunityWorkUnitCandidate,
} from "./work-units.ts";

const NOW = 1_721_836_800; // fixed fixture unix second

const baseCandidate = (
  overrides: Partial<SarahCommunityWorkUnitCandidate> = {},
): SarahCommunityWorkUnitCandidate => ({
  objective: "Implement public-safe fixture task and return evidence.",
  targetRef: "repo:OpenAgentsInc/openagents",
  allowedActions: ["execute_public_objective", "return_evidence"],
  budget: { kind: "experience_tier", amount: 1 },
  expiresAtUnix: NOW + 3_600,
  idempotencyId: "idem.cw03.fixture.1",
  experienceTier: 1,
  ...overrides,
});

describe("SARAH-CW-03 community work unit grant", () => {
  it("builds a narrow grant with community authority class only", () => {
    const grant = buildSarahCommunityWorkUnitGrant({
      targetRef: "repo:OpenAgentsInc/openagents",
      allowedActions: ["quote_work_unit", "execute_public_objective"],
      budget: { kind: "experience_tier", amount: 2 },
      expiresAtUnix: NOW + 600,
      idempotencyId: "idem.cw03.grant.a",
    });
    expect(grant.schema).toBe(SARAH_COMMUNITY_WORK_UNIT_GRANT_SCHEMA);
    expect(grant.authorityClass).toBe(SARAH_COMMUNITY_UNIT_AUTHORITY_CLASS);
    expect(grant.authorityClass).not.toContain("sarah");
    expect(grant.targetRef).toBe("repo:OpenAgentsInc/openagents");
    expect(grant.allowedActions).toEqual([
      "quote_work_unit",
      "execute_public_objective",
    ]);
  });

  it("refuses Sarah grant material on a unit payload", () => {
    expect(() =>
      assertNoSarahGrant({
        grantRef: "grant.sarah.delegated_operations",
      }),
    ).toThrow(SarahCommunityWorkUnitError);
    expect(() =>
      assertNoSarahGrant({
        role: "sarah_orchestrator",
      }),
    ).toThrowError(/Sarah authority material/);
    for (const marker of FORBIDDEN_SARAH_AUTHORITY_MARKERS) {
      expect(() => assertNoSarahGrant({ marker })).toThrow(
        SarahCommunityWorkUnitError,
      );
    }
  });

  it("refuses an expired grant and never extends it", () => {
    const grant = buildSarahCommunityWorkUnitGrant({
      targetRef: "repo:OpenAgentsInc/openagents",
      allowedActions: ["return_evidence"],
      budget: { kind: "experience_tier", amount: 1 },
      expiresAtUnix: NOW,
      idempotencyId: "idem.cw03.expired",
    });
    expect(isGrantActive(grant, NOW)).toBe(false);
    expect(isGrantActive(grant, NOW + 1)).toBe(false);
    expect(() => assertGrantActive(grant, NOW)).toThrowError(/refuse, do not extend/);
    const error = (() => {
      try {
        assertGrantActive(grant, NOW + 10);
        return null;
      } catch (e) {
        return e as SarahCommunityWorkUnitError;
      }
    })();
    expect(error).toBeInstanceOf(SarahCommunityWorkUnitError);
    expect(error?.code).toBe("expired_grant");
    // Expiration is immutable — still the original timestamp.
    expect(grant.expiresAtUnix).toBe(NOW);
  });

  it("accepts an active grant and emits NIP-40 expiration tags", () => {
    const grant = buildSarahCommunityWorkUnitGrant({
      targetRef: "repo:OpenAgentsInc/openagents",
      allowedActions: ["execute_public_objective"],
      budget: { kind: "experience_tier", amount: 1 },
      expiresAtUnix: NOW + 120,
      idempotencyId: "idem.cw03.wire",
    });
    expect(isGrantActive(grant, NOW)).toBe(true);
    expect(nip40ExpirationTag(grant)).toEqual(["expiration", String(NOW + 120)]);
    const tags = wireTagsForActiveGrant(grant, NOW);
    expect(tags).toContainEqual(["expiration", String(NOW + 120)]);
    expect(tags).toContainEqual(["authority_class", "community_unit_narrow"]);
    expect(() => wireTagsForActiveGrant(grant, NOW + 120)).toThrowError(
      /expired/,
    );
  });

  it("rejects invalid experience_tier budgets", () => {
    expect(() =>
      buildSarahCommunityWorkUnitGrant({
        targetRef: "repo:OpenAgentsInc/openagents",
        allowedActions: ["return_evidence"],
        budget: { kind: "experience_tier", amount: 0 },
        expiresAtUnix: NOW + 60,
        idempotencyId: "idem.cw03.bad-tier",
      }),
    ).toThrowError(/experience_tier/);
  });
});

describe("SARAH-CW-03 tick decomposition", () => {
  it("decomposes one tick into many bounded units under the cap", () => {
    const candidates = Array.from({ length: 3 }, (_, i) =>
      baseCandidate({
        idempotencyId: `idem.cw03.multi.${i + 1}`,
        objective: `Public objective ${i + 1} for community compute.`,
        experienceTier: ((i % 3) + 1) as 1 | 2 | 3,
      }),
    );
    const decomposition = decomposeSarahTickToWorkUnits({
      tickRef: "tick.sarah.fixture.b42",
      candidates,
      nowUnix: NOW,
    });
    expect(decomposition.schema).toBe(SARAH_COMMUNITY_TICK_DECOMPOSITION_SCHEMA);
    expect(decomposition.packet).toBe(SARAH_CW_03_PACKET);
    expect(decomposition.units).toHaveLength(3);
    expect(decomposition.unitCap).toBe(SARAH_COMMUNITY_WORK_UNIT_MAX_PER_TICK);
    expect(decomposition.truncatedCandidateCount).toBe(0);
    for (const unit of decomposition.units) {
      expect(unit.schema).toBe(SARAH_COMMUNITY_WORK_UNIT_SCHEMA);
      expect(unit.tickRef).toBe("tick.sarah.fixture.b42");
      expect(unit.grant.authorityClass).toBe(
        SARAH_COMMUNITY_UNIT_AUTHORITY_CLASS,
      );
      expect(unit.grant.authorityClass).not.toMatch(/sarah/i);
      assertNoSarahGrant(unit);
      assertGrantActive(unit.grant, NOW);
    }
    // Round-trip through schema decode.
    const again = decodeSarahCommunityTickDecomposition(decomposition);
    expect(again.units).toHaveLength(3);
  });

  it("caps units per tick and reports truncated candidates", () => {
    const candidates = Array.from(
      { length: SARAH_COMMUNITY_WORK_UNIT_MAX_PER_TICK + 5 },
      (_, i) =>
        baseCandidate({
          idempotencyId: `idem.cw03.cap.${i + 1}`,
          objective: `Capped objective number ${i + 1} for community.`,
        }),
    );
    const decomposition = decomposeSarahTickToWorkUnits({
      tickRef: "tick.sarah.fixture.b99",
      candidates,
      nowUnix: NOW,
    });
    expect(decomposition.units).toHaveLength(
      SARAH_COMMUNITY_WORK_UNIT_MAX_PER_TICK,
    );
    expect(decomposition.truncatedCandidateCount).toBe(5);
  });

  it("honors a tighter unitCap still within the hard max", () => {
    const candidates = Array.from({ length: 6 }, (_, i) =>
      baseCandidate({
        idempotencyId: `idem.cw03.tight.${i + 1}`,
        objective: `Tight cap objective ${i + 1} for community.`,
      }),
    );
    const decomposition = decomposeSarahTickToWorkUnits({
      tickRef: "tick.sarah.fixture.b7",
      candidates,
      nowUnix: NOW,
      unitCap: 2,
    });
    expect(decomposition.units).toHaveLength(2);
    expect(decomposition.truncatedCandidateCount).toBe(4);
  });

  it("refuses unitCap above the hard max", () => {
    expect(() =>
      decomposeSarahTickToWorkUnits({
        tickRef: "tick.sarah.fixture.b8",
        candidates: [baseCandidate()],
        nowUnix: NOW,
        unitCap: SARAH_COMMUNITY_WORK_UNIT_MAX_PER_TICK + 1,
      }),
    ).toThrowError(/unitCap/);
  });

  it("refuses duplicate idempotency identities inside one tick", () => {
    expect(() =>
      decomposeSarahTickToWorkUnits({
        tickRef: "tick.sarah.fixture.b1",
        candidates: [
          baseCandidate({ idempotencyId: "idem.same" }),
          baseCandidate({
            idempotencyId: "idem.same",
            objective: "Second candidate with the same idempotency id.",
          }),
        ],
        nowUnix: NOW,
      }),
    ).toThrowError(/duplicate idempotency/);
  });

  it("refuses already-expired candidates without extending them", () => {
    expect(() =>
      decomposeSarahTickToWorkUnits({
        tickRef: "tick.sarah.fixture.b2",
        candidates: [
          baseCandidate({
            expiresAtUnix: NOW - 1,
            idempotencyId: "idem.cw03.past",
          }),
        ],
        nowUnix: NOW,
      }),
    ).toThrowError(/already expired/);
  });

  it("refuses candidates that smuggle Sarah grant refs into the objective", () => {
    expect(() =>
      decomposeSarahTickToWorkUnits({
        tickRef: "tick.sarah.fixture.b3",
        candidates: [
          baseCandidate({
            objective:
              "Run with grant.sarah.delegated_operations against the fleet.",
            idempotencyId: "idem.cw03.smuggle",
          }),
        ],
        nowUnix: NOW,
      }),
    ).toThrowError(/Sarah authority material/);
  });

  it("builds a single unit that decodes and never names Sarah authority", () => {
    const unit = buildSarahCommunityWorkUnit({
      unitRef: "unit.cw.fixture.1",
      tickRef: "tick.sarah.fixture.b0",
      objective: "Return public-safe evidence for the pinned verification.",
      experienceTier: 3,
      createdAtUnix: NOW,
      grant: {
        targetRef: "repo:OpenAgentsInc/openagents",
        allowedActions: [
          "execute_public_objective",
          "return_evidence",
          "verify_peer_result",
        ],
        budget: { kind: "msats", amount: 0 },
        expiresAtUnix: NOW + 1_800,
        idempotencyId: "idem.cw03.single",
      },
    });
    expect(decodeSarahCommunityWorkUnit(unit).unitRef).toBe("unit.cw.fixture.1");
    expect(JSON.stringify(unit)).not.toMatch(/grant\.sarah\./);
    expect(JSON.stringify(unit)).not.toMatch(/principal\.sarah/);
    expect(JSON.stringify(unit)).not.toMatch(
      /openagents\.sarah-owner-orchestrator/,
    );
  });

  it("records packet and issue constants for the CW-03 surface", () => {
    expect(SARAH_CW_03_PACKET).toBe("SARAH-CW-03");
    expect(SARAH_CW_03_ISSUE).toBe("OpenAgentsInc/openagents#9225");
  });
});
