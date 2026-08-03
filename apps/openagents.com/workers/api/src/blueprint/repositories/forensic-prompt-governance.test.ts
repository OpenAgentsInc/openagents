import { forensicSha256Digest } from "@openagentsinc/forensic-contract";
import { Effect, Exit } from "effect";
import { describe, expect, test } from "vitest";

import type { ForensicPromptActiveTransition } from "../schemas/forensic-prompt-optimization";
import {
  ForensicPromptGovernanceError,
  makePostgresForensicPromptGovernanceStore,
  type ForensicPromptGovernanceSql,
} from "./forensic-prompt-governance";

const digest = (character: string) => `sha256:${character.repeat(64)}`;
const OWNER = "owner.forensic.operator";

type TransitionDraft = Omit<ForensicPromptActiveTransition, "transitionDigest">;

const seal = (draft: TransitionDraft): ForensicPromptActiveTransition => ({
  ...draft,
  transitionDigest: forensicSha256Digest(draft),
});

const activation = (
  overrides: Partial<TransitionDraft> = {},
): ForensicPromptActiveTransition =>
  seal({
    activePromptDigest: digest("1"),
    candidateDigest: digest("1"),
    candidateProducerRef: "identity.optimizer.generator",
    decidedAt: "2026-08-01T17:00:00.000Z",
    evaluationRef: "evaluation.forensic.candidate.1",
    operatorDecisionRef: "operator-decision.forensic.1",
    operatorIdentityRef: "identity.operator.release",
    priorActivePromptDigest: null,
    releaseGateRef: "release-gate.forensic.prompt.1",
    rollbackAnchorDigest: null,
    schema: "openagents.blueprint.forensic_prompt_transition.v1",
    sequence: 1,
    transitionRef: "transition.forensic.activate.1",
    transitionType: "activate",
    ...overrides,
  });

const rollback = (from: ForensicPromptActiveTransition): ForensicPromptActiveTransition =>
  seal({
    activePromptDigest: from.rollbackAnchorDigest,
    candidateDigest: from.candidateDigest,
    candidateProducerRef: from.candidateProducerRef,
    decidedAt: "2026-08-01T18:00:00.000Z",
    evaluationRef: from.evaluationRef,
    operatorDecisionRef: "operator-decision.forensic.rollback.1",
    operatorIdentityRef: "identity.operator.release",
    priorActivePromptDigest: from.activePromptDigest,
    releaseGateRef: from.releaseGateRef,
    rollbackAnchorDigest: from.rollbackAnchorDigest,
    schema: "openagents.blueprint.forensic_prompt_transition.v1",
    sequence: from.sequence + 1,
    transitionRef: "transition.forensic.rollback.1",
    transitionType: "rollback",
  });

type PointerRecord = { active: string | null; revision: number };
type TransitionRecord = {
  owner: string;
  sequence: number;
  ref: string;
  digest: string;
  json: string;
  decidedAt: string;
};

/**
 * A bounded stand-in for the migration's two tables. It reproduces exactly the
 * admission rule the SQL enforces: the append is admitted only when the stored
 * pointer still matches the revision and prior digest the caller observed, and
 * the transition's ref, sequence, and digest are all still free.
 */
const makeFakeSqlStore = () => {
  const pointers = new Map<string, PointerRecord>();
  const transitions: Array<TransitionRecord> = [];

  const sql = (async (strings: TemplateStringsArray, ...values: ReadonlyArray<unknown>) => {
    const statement = strings.join("?");

    if (statement.includes("WITH locked AS")) {
      const ownerRef = String(values[0]);
      const expectedRevision = Number(values[2]);
      const priorDigest = (values[3] ?? null) as string | null;
      const sequence = Number(values[7]);
      const transitionRef = String(values[8]);
      const transitionDigest = String(values[9]);
      const transitionJson = String(values[10]);
      const decidedAt = String(values[11]);
      const activeDigest = (values[13] ?? null) as string | null;

      const pointer = pointers.get(ownerRef);
      const admitted =
        pointer === undefined
          ? expectedRevision === 0 && priorDigest === null
          : pointer.revision === expectedRevision && pointer.active === priorDigest;
      if (!admitted) return [];
      const conflicts = transitions.some(
        (row) =>
          row.owner === ownerRef &&
          (row.sequence === sequence ||
            row.ref === transitionRef ||
            row.digest === transitionDigest),
      );
      if (conflicts) return [];
      transitions.push({
        decidedAt,
        digest: transitionDigest,
        json: transitionJson,
        owner: ownerRef,
        ref: transitionRef,
        sequence,
      });
      pointers.set(ownerRef, { active: activeDigest, revision: sequence });
      return [{ active_prompt_digest: activeDigest, revision: sequence }];
    }

    if (statement.includes("SELECT transition_digest")) {
      const ownerRef = String(values[0]);
      return [...transitions]
        .filter((row) => row.owner === ownerRef)
        .sort((left, right) => left.sequence - right.sequence)
        .map((row) => ({ transition_digest: row.digest, transition_json: row.json }));
    }

    const ownerRef = String(values[0]);
    const pointer = pointers.get(ownerRef);
    return pointer === undefined
      ? []
      : [{ active_prompt_digest: pointer.active, revision: pointer.revision }];
  }) as ForensicPromptGovernanceSql;

  return { pointers, sql, transitions };
};

const failureOf = async <A>(effect: Effect.Effect<A, ForensicPromptGovernanceError>) => {
  const exit = await Effect.runPromiseExit(effect);
  expect(Exit.isFailure(exit)).toBe(true);
  return Effect.runPromise(Effect.flip(effect));
};

describe("durable forensic prompt governance", () => {
  test("appends an activation, then a rollback, without rewriting history", async () => {
    const fake = makeFakeSqlStore();
    const store = makePostgresForensicPromptGovernanceStore(fake.sql);

    const genesis = await Effect.runPromise(store.read(OWNER));
    expect(genesis).toMatchObject({ activePromptDigest: null, revision: 0 });
    expect(genesis.history).toHaveLength(0);

    const activated = activation();
    const afterActivation = await Effect.runPromise(store.append(OWNER, activated, 0));
    expect(afterActivation).toMatchObject({
      activePromptDigest: activated.activePromptDigest,
      revision: 1,
    });

    const reverted = rollback(activated);
    const afterRollback = await Effect.runPromise(store.append(OWNER, reverted, 1));
    expect(afterRollback.activePromptDigest).toBeNull();
    expect(afterRollback.revision).toBe(2);
    // The activation is still recorded verbatim: reversal appends, never edits.
    expect(afterRollback.history).toHaveLength(2);
    expect(afterRollback.history[0]).toStrictEqual(activated);
    expect(afterRollback.history[1]?.transitionType).toBe("rollback");
  });

  test("rejects a decision taken against a stale pointer read", async () => {
    const fake = makeFakeSqlStore();
    const store = makePostgresForensicPromptGovernanceStore(fake.sql);
    const activated = activation();
    await Effect.runPromise(store.append(OWNER, activated, 0));

    // A second promoter that still believes the pointer is at genesis.
    const competing = activation({
      activePromptDigest: digest("2"),
      candidateDigest: digest("2"),
      transitionRef: "transition.forensic.activate.competing",
    });
    const stale = await failureOf(store.append(OWNER, competing, 0));
    expect(stale.code).toBe("conflict");
    expect(stale.retryable).toBe(false);

    // A sequence that does not continue the observed revision is also refused
    // before any storage round trip.
    const skipped = await failureOf(store.append(OWNER, activation({ sequence: 5 }), 1));
    expect(skipped.code).toBe("conflict");
    expect((await Effect.runPromise(store.read(OWNER))).revision).toBe(1);
  });

  test("refuses a transition whose digest does not cover its own content", async () => {
    const fake = makeFakeSqlStore();
    const store = makePostgresForensicPromptGovernanceStore(fake.sql);
    const forged = { ...activation(), activePromptDigest: digest("3") };
    const failure = await failureOf(store.append(OWNER, forged, 0));
    expect(failure.code).toBe("invalid_transition");
    expect(fake.transitions).toHaveLength(0);
  });

  test("refuses to serve governance state that drifted in storage", async () => {
    const fake = makeFakeSqlStore();
    const store = makePostgresForensicPromptGovernanceStore(fake.sql);
    const activated = activation();
    await Effect.runPromise(store.append(OWNER, activated, 0));

    // A stored transition edited underneath the pointer.
    const edited = { ...activated, decidedAt: "2026-08-02T09:00:00.000Z" };
    fake.transitions[0]!.json = JSON.stringify(edited);
    expect((await failureOf(store.read(OWNER))).code).toBe("invalid_transition");

    // A pointer that no longer matches its own history.
    fake.transitions[0]!.json = JSON.stringify(activated);
    fake.pointers.set(OWNER, { active: digest("4"), revision: 1 });
    expect((await failureOf(store.read(OWNER))).code).toBe("invalid_transition");
  });

  test("reports storage failure as retryable without inventing a pointer", async () => {
    const failing = (async () => {
      throw new Error("connection reset");
    }) as ForensicPromptGovernanceSql;
    const store = makePostgresForensicPromptGovernanceStore(failing);
    const failure = await failureOf(store.read(OWNER));
    expect(failure.code).toBe("storage_unavailable");
    expect(failure.retryable).toBe(true);
  });
});
