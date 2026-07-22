import { Effect, Schema } from "effect";

import type { PylonPortableControlSessionLifecycle } from "./node/control-sessions.js";
import type {
  PylonPortableControlBinding,
  PylonPortableSessionFence,
  PylonPortableSessionOperationLedger,
} from "./portable-session-operation-ledger.js";
import {
  createPylonOwnerLocalExecutionTarget,
  type PylonOwnerLocalExecutionTarget,
} from "./portable-session-target.js";

const SAFE_REF = /^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$/u;

export class PylonOwnerLocalTargetStartupError extends Schema.TaggedErrorClass<PylonOwnerLocalTargetStartupError>()(
  "PylonOwnerLocalTargetStartupError",
  {
    reason: Schema.Literals([
      "invalid_authority",
      "binding_absent",
      "binding_ambiguous",
      "binding_stale",
      "target_registration_failed",
    ]),
  },
) {}

export type PylonOwnerLocalTargetRegistry = Readonly<{
  registerTarget: (target: PylonOwnerLocalExecutionTarget) => void;
  target: (targetRef: string) => PylonOwnerLocalExecutionTarget | undefined;
}>;

export type PylonOwnerLocalTargetStartupInput = Readonly<{
  pylonRef: string;
  targetRef: string;
  sessionRef: string;
  ledger: PylonPortableSessionOperationLedger;
  lifecycle: PylonPortableControlSessionLifecycle;
  registry: PylonOwnerLocalTargetRegistry;
  onBinding?: (binding: PylonPortableControlBinding) => void;
}>;

const fail = (
  reason: PylonOwnerLocalTargetStartupError["reason"],
): PylonOwnerLocalTargetStartupError => new PylonOwnerLocalTargetStartupError({ reason });

const exactBinding = async (
  input: PylonOwnerLocalTargetStartupInput,
): Promise<PylonPortableControlBinding> => {
  let bindings: ReadonlyArray<PylonPortableControlBinding>;
  try {
    bindings = await Effect.runPromise(input.ledger.listControlBindings());
  } catch {
    throw fail("binding_absent");
  }
  const matching = bindings.filter((binding) => binding.sessionRef === input.sessionRef);
  if (matching.length === 0) throw fail("binding_absent");
  if (matching.length !== 1 || bindings.length !== 1) throw fail("binding_ambiguous");
  const binding = matching[0];
  if (binding === undefined || binding.state === "cleaned" || binding.agents.length === 0) {
    throw fail("binding_stale");
  }
  let fence: PylonPortableSessionFence;
  try {
    fence = await Effect.runPromise(input.ledger.readSession(input.sessionRef));
  } catch {
    throw fail("binding_stale");
  }
  if (
    fence.attachmentRef !== binding.attachmentRef ||
    fence.generation !== binding.generation ||
    (binding.state === "accepting" && !fence.acceptingWork)
  ) {
    throw fail("binding_stale");
  }
  return binding;
};

/**
 * Constructs one owner-local target from an exact durable local binding.
 * The caller supplies the same Pylon and target refs used by the authenticated
 * phase worker. This function does not create destination or artifact custody
 * authority.
 */
export const registerPylonOwnerLocalExecutionTarget = async (
  input: PylonOwnerLocalTargetStartupInput,
): Promise<PylonOwnerLocalExecutionTarget> => {
  if (![input.pylonRef, input.targetRef, input.sessionRef].every((ref) => SAFE_REF.test(ref))) {
    throw fail("invalid_authority");
  }
  const binding = await exactBinding(input);
  let target: PylonOwnerLocalExecutionTarget;
  try {
    target = await createPylonOwnerLocalExecutionTarget({
      targetRef: input.targetRef,
      ledger: input.ledger,
      lifecycle: input.lifecycle,
      binding: {
        sessionRef: binding.sessionRef,
        attachmentRef: binding.attachmentRef,
        generation: binding.generation,
        agents: binding.agents.map((agent) => ({
          agentRef: agent.agentRef,
          controlSessionRef: agent.controlSessionRef,
        })),
      },
    });
    input.registry.registerTarget(target);
  } catch (error) {
    if (error instanceof PylonOwnerLocalTargetStartupError) throw error;
    throw fail("target_registration_failed");
  }
  if (input.registry.target(input.targetRef) !== target) {
    throw fail("target_registration_failed");
  }
  input.onBinding?.(binding);
  return target;
};
