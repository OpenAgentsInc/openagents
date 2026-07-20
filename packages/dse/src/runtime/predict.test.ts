import { Effect } from "effect";
import { describe, expect, test } from "vite-plus/test";

import { honestChatReplySignature, type HonestChatReplyOutput } from "../contract/index.js";
import {
  constantModelLayer,
  honestProgram,
  invalidThenValidModelLayer,
  testDeps,
} from "../test-support.js";
import { DseDecodeError, predict } from "./predict.js";

const run = <A, E>(effect: Effect.Effect<A, E, never>): Promise<A> => Effect.runPromise(effect);

const input = { conversation: "How do I read a file?" };

describe("Predict decode and bounded repair", () => {
  test("decodes a valid model output on the first attempt", async () => {
    const outcome = await run(
      predict({
        signature: honestChatReplySignature,
        candidateId: "cand:test",
        program: honestProgram("Answer honestly."),
        input,
        deps: testDeps,
      }).pipe(
        Effect.provide(constantModelLayer(JSON.stringify({ reply: "ok", claimedActions: [] }))),
      ),
    );
    const decoded: HonestChatReplyOutput = outcome.output;
    expect(decoded.reply).toBe("ok");
    expect(outcome.receipt.decodeOutcome).toBe("decoded");
    expect(outcome.receipt.repairCount).toBe(0);
    expect(outcome.receipt.usageTruth).toBe("estimated");
  });

  test("runs one bounded repair when the first output does not decode", async () => {
    const outcome = await run(
      predict({
        signature: honestChatReplySignature,
        candidateId: "cand:test",
        program: honestProgram("Answer honestly."),
        input,
        deps: testDeps,
      }).pipe(Effect.provide(invalidThenValidModelLayer(1))),
    );
    expect(outcome.receipt.decodeOutcome).toBe("repaired");
    expect(outcome.receipt.repairCount).toBe(1);
  });

  test("fails closed with a typed error when repair is exhausted", async () => {
    const result = await Effect.runPromise(
      predict({
        signature: honestChatReplySignature,
        candidateId: "cand:test",
        program: honestProgram("Answer honestly."),
        input,
        deps: testDeps,
      }).pipe(Effect.provide(constantModelLayer("never valid json")), Effect.flip),
    );
    expect(result).toBeInstanceOf(DseDecodeError);
    expect(result.repairCount).toBe(1);
  });
});
