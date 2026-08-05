import { describe, expect, test } from "vite-plus/test";

import { attributeEvidence, claimVerdict, provenRungView } from "./rungs.js";
import { evidence } from "./testkit.js";

describe("evidence rung attribution", () => {
  test("a status is a claim: its authority caps at pledged even when it says settled", () => {
    for (const authority of ["provider_status", "requester_status"] as const) {
      const fact = attributeEvidence(evidence({ authority, rung: "settled", final: true }));
      expect(fact.provenRung).toBe("pledged");
      expect(fact.overclaim).toBe(true);
    }
  });

  test("a relay observation caps at measured — observation is not authority", () => {
    const fact = attributeEvidence(
      evidence({ authority: "relay_observation", rung: "settled" }),
    );
    expect(fact.provenRung).toBe("measured");
    expect(fact.overclaim).toBe(true);
  });

  test("chain and Lightning facts keep their verifying source attached", () => {
    const view = provenRungView([
      evidence({ authority: "bitcoin_adapter", rung: "settled", final: true }),
      evidence({ authority: "provider_status", rung: "settled" }),
    ]);
    expect(view.proven).toBe("settled");
    expect(view.facts.map((fact) => fact.evidence.authority)).toEqual([
      "bitcoin_adapter",
      "provider_status",
    ]);
    expect(view.facts[1]!.provenRung).toBe("pledged");
  });

  test("no evidence proves no rung", () => {
    expect(provenRungView([]).proven).toBeNull();
  });

  test("completed requires settled final evidence; a bare claim is an overclaim", () => {
    const unproven = claimVerdict("completed", [
      evidence({ class: "bitcoin_spend", authority: "provider_status", rung: "settled" }),
    ]);
    expect(unproven.kind).toBe("unproven");
    if (unproven.kind === "unproven") {
      expect(unproven.error).toBe("swp_settlement_overclaim");
      expect(unproven.requiredRung).toBe("settled");
      expect(unproven.provenRung).toBe("pledged");
    }
    const proved = claimVerdict("completed", [
      evidence({ class: "bitcoin_spend", authority: "bitcoin_adapter", rung: "settled", final: true }),
    ]);
    expect(proved.kind).toBe("proved");
  });

  test("funding_final needs finality; measured evidence is not enough", () => {
    const verdict = claimVerdict("funding_final", [
      evidence({ authority: "bitcoin_adapter", rung: "measured" }),
    ]);
    expect(verdict.kind).toBe("unproven");
  });

  test("action claims carry no evidence requirement", () => {
    expect(claimVerdict("requester_funding_broadcast", []).kind).toBe("no_evidence_required");
    expect(claimVerdict("accepted", []).kind).toBe("no_evidence_required");
  });
});
