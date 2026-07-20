import { Context, Effect, Schema as S } from "effect";

import type { DseUsageTruth } from "../contract/index.js";

/**
 * The offline model port.
 *
 * `Predict`, evaluation, and compile consume Apple FM (or any local model) only
 * through this port. The package never imports a provider SDK, opens a socket,
 * or dispatches a real turn. An offline compile injects a deterministic model
 * layer, so a repeated compile is reproducible.
 */

export interface DseCompletion {
  readonly text: string;
  readonly usageTruth: DseUsageTruth;
}

export interface DseModelInterface {
  readonly complete: (input: {
    readonly rendered: string;
    readonly maxOutputChars: number;
  }) => Effect.Effect<DseCompletion, DseModelError>;
}

export class DseModel extends Context.Service<DseModel, DseModelInterface>()("dse/DseModel") {}

/** A typed model-boundary failure. It carries no raw prompt or credential. */
export class DseModelError extends S.TaggedErrorClass<DseModelError>()("dse/DseModelError", {
  reason: S.String,
}) {}
