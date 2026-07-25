/**
 * `SARAH-CW-00-A1` — the verifier signs its own verification (omega#48).
 *
 * The fixtures under `packages/sarah/fixtures/community-verification/` are
 * byte-shared with `crates/workroom_receipts/fixtures/` in the omega repo. Both
 * sides decode the same bytes and must reach the same admission. The digests
 * asserted below are what makes that enforceable rather than conventional: edit
 * a fixture in one repo and that repo's test goes red until the other agrees.
 */
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, test } from "vite-plus/test";

import {
  admitIndependentVerification,
  buildIndependentVerificationTemplate,
  IndependentVerificationRuleError,
  INDEPENDENT_VERIFICATION_FEEDBACK_TYPE,
  SARAH_CW_00_A1_PACKET,
  SARAH_INDEPENDENT_VERIFICATION_SCHEMA,
  validateIndependentVerification,
  type CommunityBindingResolver,
  type IndependentVerificationEvent,
} from "./verification.ts";
import { COMMUNITY_ARBITRATION_FEEDBACK_KIND, COMMUNITY_FEEDBACK_TYPES } from "./types.ts";

const fixtureDir = join(
  dirname(fileURLToPath(import.meta.url)),
  "../../fixtures/community-verification",
);

/**
 * The exact bytes each side must hold. Not a convenience: a fixture is only
 * "byte-shared" if drift is detectable from inside one repo.
 */
const FIXTURE_DIGESTS: ReadonlyArray<readonly [string, string]> = [
  [
    "openagents.sarah.community_independent_verification.v1.canonical.json",
    "992085428cf36e74ee0f1b6bdb6b38c494e7b7cc1b68d668e82bef74fcfa928f",
  ],
  [
    "openagents.sarah.community_independent_verification.v1.negative-operators-not-independent.json",
    "35061fb7846a3fa961ac7335e96dd9cbd21b080ba5bf2fe0389402ce22d3333b",
  ],
  [
    "openagents.sarah.community_independent_verification.v1.negative-verifier-key-burned.json",
    "0a01c8665d52cf0542084897bc79eac52a5cca3b3ae0dfb725ead4f5a8dbcc36",
  ],
  [
    "openagents.sarah.community_independent_verification.v1.negative-verifier-not-author.json",
    "c2f91af69fe979acdcbe8550060d782c72d963c7346d8d6febb5705f87338271",
  ],
];

interface FixtureDoc {
  readonly fixture: string;
  readonly packet: string;
  readonly fixture_id: string;
  readonly expect: "admit" | "refuse";
  readonly binding: {
    readonly agents: ReadonlyArray<{
      readonly agentPubkey: string;
      readonly operatorPubkey: string;
    }>;
    readonly burnedAgentKeys: ReadonlyArray<string>;
  };
  readonly event: IndependentVerificationEvent;
  readonly expected: Record<string, unknown>;
}

const readFixture = (name: string): { doc: FixtureDoc; raw: string } => {
  const raw = readFileSync(join(fixtureDir, name), "utf8");
  return { doc: JSON.parse(raw) as FixtureDoc, raw };
};

const resolverFor = (doc: FixtureDoc): CommunityBindingResolver => {
  const bound = new Map(
    doc.binding.agents.map((agent) => [agent.agentPubkey, agent.operatorPubkey] as const),
  );
  const burned = new Set(doc.binding.burnedAgentKeys);
  return {
    operatorForAgent: (agentPubkey) => bound.get(agentPubkey) ?? null,
    isAgentKeyBurned: (agentPubkey) => burned.has(agentPubkey),
  };
};

const canonicalDoc = (): FixtureDoc =>
  readFixture(FIXTURE_DIGESTS[0]![0]).doc;

describe("SARAH-CW-00-A1 byte-shared fixtures", () => {
  for (const [name, digest] of FIXTURE_DIGESTS) {
    test(`${name} is the exact bytes both repos hold`, () => {
      const { raw } = readFixture(name);
      expect(createHash("sha256").update(raw).digest("hex")).toBe(digest);
    });
  }

  for (const [name] of FIXTURE_DIGESTS) {
  test(`${name} admits exactly what it declares`, () => {
    const { doc } = readFixture(name);
    expect(doc.fixture).toBe(SARAH_INDEPENDENT_VERIFICATION_SCHEMA);
    expect(doc.packet).toBe(SARAH_CW_00_A1_PACKET);

    const admission = admitIndependentVerification(doc.event, resolverFor(doc));
    expect(admission.admitted).toBe(doc.expect === "admit");
    const fields = admission as unknown as Record<string, unknown>;
    for (const [field, value] of Object.entries(doc.expected)) {
      expect({ [field]: fields[field] }).toEqual({ [field]: value });
    }
  });
  }
});

describe("the verifier signs its own verification", () => {
  test("the canonical fixture is admitted with both operators read from the record", () => {
    const doc = canonicalDoc();
    const admission = admitIndependentVerification(doc.event, resolverFor(doc));
    if (!admission.admitted) throw new Error(`unexpected refusal: ${admission.code}`);
    expect(admission.verifierAgentPubkey).toBe(doc.event.pubkey);
    expect(admission.verifierOperatorPubkey).not.toBe(admission.producerOperatorPubkey);
    expect(admission.verdict).toBe("reproduced");
  });

  /**
   * The point of the amendment. A decision, or anyone else, signing "X verified
   * this" is the state this replaces — not a state it accepts.
   */
  test("an event that names a verifier other than its own author is refused", () => {
    const doc = canonicalDoc();
    const impostor: IndependentVerificationEvent = {
      ...doc.event,
      pubkey: "9".repeat(64),
    };
    const admission = admitIndependentVerification(impostor, resolverFor(doc));
    expect(admission.admitted).toBe(false);
    if (admission.admitted) return;
    expect(admission.code).toBe("verifier_not_author");
  });

  /**
   * The self-dealing hole, on the new carrier. The verifier asserts its own
   * operator in `cw_verifier_operator_ref`; if that tag were believed, an agent
   * could name any operator it liked and independence would render from a
   * self-authored claim.
   */
  test("a verifier claiming an operator the record does not bind it to is refused", () => {
    const doc = canonicalDoc();
    const lying: IndependentVerificationEvent = {
      ...doc.event,
      tags: doc.event.tags.map((tag) =>
        tag[0] === "cw_verifier_operator_ref" ? [tag[0], "5".repeat(64)] : tag,
      ),
    };
    const admission = admitIndependentVerification(lying, resolverFor(doc));
    expect(admission.admitted).toBe(false);
    if (admission.admitted) return;
    expect(admission.code).toBe("verifier_binding_unconfirmed");
  });

  test("distinct keys under one operator are not independent", () => {
    const { doc } = readFixture(
      "openagents.sarah.community_independent_verification.v1.negative-operators-not-independent.json",
    );
    const admission = admitIndependentVerification(doc.event, resolverFor(doc));
    expect(admission.admitted).toBe(false);
    if (admission.admitted) return;
    expect(admission.code).toBe("self_dealing_operators");
  });

  /** Revocation binds the subject whatever it signs and whenever it arrives. */
  test("a burned verifier key is refused even with a live binding row", () => {
    const { doc } = readFixture(
      "openagents.sarah.community_independent_verification.v1.negative-verifier-key-burned.json",
    );
    expect(resolverFor(doc).operatorForAgent(doc.event.pubkey)).not.toBeNull();
    const admission = admitIndependentVerification(doc.event, resolverFor(doc));
    expect(admission.admitted).toBe(false);
    if (admission.admitted) return;
    expect(admission.code).toBe("verifier_key_burned");
  });

  test("an unbound verifier key is refused rather than shown as independent", () => {
    const doc = canonicalDoc();
    const admission = admitIndependentVerification(doc.event, {
      operatorForAgent: (key) =>
        key === doc.event.pubkey ? null : "d4".repeat(32),
      isAgentKeyBurned: () => false,
    });
    expect(admission.admitted).toBe(false);
    if (admission.admitted) return;
    expect(admission.code).toBe("verifier_binding_unconfirmed");
  });
});

describe("a verification decides nothing", () => {
  test("an accepted/rejected verdict is not a known verdict", () => {
    const doc = canonicalDoc();
    const posing: IndependentVerificationEvent = {
      ...doc.event,
      tags: doc.event.tags.map((tag) =>
        tag[0] === "status" ? [tag[0], "accepted"] : tag,
      ),
    };
    const admission = admitIndependentVerification(posing, resolverFor(doc));
    expect(admission.admitted).toBe(false);
    if (admission.admitted) return;
    expect(admission.code).toBe("malformed");
  });

  test("a verification that does not disclaim payment is refused", () => {
    const doc = canonicalDoc();
    const settling: IndependentVerificationEvent = {
      ...doc.event,
      tags: doc.event.tags.map((tag) =>
        tag[0] === "cw_decides_payment" ? [tag[0], "true"] : tag,
      ),
    };
    const admission = admitIndependentVerification(settling, resolverFor(doc));
    expect(admission.admitted).toBe(false);
    if (admission.admitted) return;
    expect(admission.code).toBe("decides_payment_forbidden");
  });

  test("the built template carries no authority receipt of any kind", () => {
    const { template } = buildIndependentVerificationTemplate({
      verification: {
        schema: SARAH_INDEPENDENT_VERIFICATION_SCHEMA,
        packet: SARAH_CW_00_A1_PACKET,
        verificationRef: "verification.01",
        unitRef: "unit.01",
        requestEventId: "e5".repeat(32),
        resultEventId: "f6".repeat(32),
        producerAgentPubkey: "b2".repeat(32),
        verifierAgentPubkey: "a1".repeat(32),
        verifierOperatorRef: "c3".repeat(32),
        verdict: "reproduced",
        verificationReceiptRef: "receipt.01",
        evidenceRefs: [],
        decidesPayment: false,
        verifiedAt: "2026-07-25T12:00:00.000Z",
      },
    });
    expect(template.kind).toBe(COMMUNITY_ARBITRATION_FEEDBACK_KIND);
    expect(template.content).toBe("");
    const tagNames = template.tags.map((tag) => tag[0]);
    expect(tagNames).not.toContain("cw_authority_receipt_ref");
    expect(tagNames).not.toContain("cw_authority_receipt_schema");
    expect(tagNames).not.toContain("cw_decision_ref");
    expect(
      template.tags.find((tag) => tag[0] === "cw_decides_payment")?.[1],
    ).toBe("false");
  });
});

describe("the amendment's shape", () => {
  test("the discriminator gains a value and keeps the ones it had", () => {
    expect(COMMUNITY_FEEDBACK_TYPES).toContain(INDEPENDENT_VERIFICATION_FEEDBACK_TYPE);
    for (const existing of ["arbitration_decision", "dispute_appeal", "owner_ruling"]) {
      expect(COMMUNITY_FEEDBACK_TYPES).toContain(existing);
    }
  });

  test("an agent cannot sign a verification of its own result at all", () => {
    expect(() =>
      validateIndependentVerification({
        schema: SARAH_INDEPENDENT_VERIFICATION_SCHEMA,
        packet: SARAH_CW_00_A1_PACKET,
        verificationRef: "verification.01",
        unitRef: "unit.01",
        requestEventId: "e5".repeat(32),
        resultEventId: "f6".repeat(32),
        producerAgentPubkey: "a1".repeat(32),
        verifierAgentPubkey: "a1".repeat(32),
        verifierOperatorRef: "c3".repeat(32),
        verdict: "reproduced",
        verificationReceiptRef: "receipt.01",
        evidenceRefs: [],
        decidesPayment: false,
        verifiedAt: "2026-07-25T12:00:00.000Z",
      }),
    ).toThrow(IndependentVerificationRuleError);
  });

  test("a verdict that did not reproduce must name a typed reason class", () => {
    const base = {
      schema: SARAH_INDEPENDENT_VERIFICATION_SCHEMA,
      packet: SARAH_CW_00_A1_PACKET,
      verificationRef: "verification.01",
      unitRef: "unit.01",
      requestEventId: "e5".repeat(32),
      resultEventId: "f6".repeat(32),
      producerAgentPubkey: "b2".repeat(32),
      verifierAgentPubkey: "a1".repeat(32),
      verifierOperatorRef: "c3".repeat(32),
      verdict: "not_reproduced" as const,
      verificationReceiptRef: "receipt.01",
      evidenceRefs: [],
      decidesPayment: false as const,
      verifiedAt: "2026-07-25T12:00:00.000Z",
    };
    expect(() => validateIndependentVerification(base)).toThrow(
      IndependentVerificationRuleError,
    );
    expect(() =>
      validateIndependentVerification({ ...base, reasonClass: "verification_failed" }),
    ).not.toThrow();
  });
});
