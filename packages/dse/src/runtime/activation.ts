import { Schema as S } from "effect";

import {
  ACTIVATION_RECEIPT_SCHEMA_LITERAL,
  ActivationReceipt,
  RELEASE_CHANNEL_SCHEMA_LITERAL,
  ReleaseChannel,
  type ActivationTransition,
  type BaselinePointer,
  type CanaryPlan,
  type DseTimestamp,
  type ReleaseMode,
  type ReleasedArtifactPointer,
} from "../contract/index.js";

/**
 * Gated-activation resolution and transitions (AFS-09).
 *
 * `resolveActivation` is a pure, deterministic decision: given a channel and a
 * stable request key it returns whether the request should serve the
 * hand-written baseline or the released artifact. SHADOW and ROLLED_BACK always
 * serve the baseline, ACTIVE always serves the released artifact, and CANARY
 * serves the released artifact to a deterministic bounded population. No dispatch
 * or substitution happens here; a host enacts the decision.
 *
 * The transition helpers move a channel between modes and emit an append-only
 * `ActivationReceipt`. They are total pure functions that fail closed with a
 * typed reason; none of them rebuilds an application or serves a request.
 */

/** Which artifact a request should serve. */
export type ActivationDecision =
  | { readonly serve: "baseline" }
  | { readonly serve: "released"; readonly pointer: ReleasedArtifactPointer };

/** A 32-bit fraction in [0, 1) derived from the first 8 hex chars of a digest. */
const hashFraction = (digest: string): number =>
  Number.parseInt(digest.slice(0, 8), 16) / 0x1_0000_0000;

export interface ResolveActivationArgs {
  readonly channel: ReleaseChannel;
  /** A stable per-request key (e.g. the thread ref) so canary membership is sticky. */
  readonly requestKey: string;
  /** The injected pure hasher; the package never imports a Node crypto host. */
  readonly sha256: (text: string) => string;
}

/**
 * Resolve which artifact a request serves. SHADOW and ROLLED_BACK never serve
 * the released artifact, so shadow causes no user-visible substitution. CANARY
 * membership is a deterministic function of the signature and the request key,
 * so the same request is stable across turns and the served fraction is bounded.
 */
export const resolveActivation = (args: ResolveActivationArgs): ActivationDecision => {
  const { channel } = args;
  const baseline: ActivationDecision = { serve: "baseline" };
  switch (channel.mode) {
    case "shadow":
    case "rolled_back":
      return baseline;
    case "active":
      return channel.candidate === undefined
        ? baseline
        : { serve: "released", pointer: channel.candidate };
    case "canary": {
      if (channel.candidate === undefined || channel.canary === undefined) return baseline;
      const fraction = hashFraction(args.sha256(`${channel.signatureId}:${args.requestKey}`));
      return fraction < channel.canary.populationFraction
        ? { serve: "released", pointer: channel.candidate }
        : baseline;
    }
  }
};

const decodeChannel = S.decodeUnknownSync(ReleaseChannel);
const decodeReceipt = S.decodeUnknownSync(ActivationReceipt);

export type ActivationRefusalReason =
  | "signature_mismatch"
  | "missing_candidate"
  | "wrong_mode"
  | "no_rollback_target";

export type ActivationResult =
  | { readonly ok: true; readonly channel: ReleaseChannel; readonly receipt: ActivationReceipt }
  | { readonly ok: false; readonly reason: ActivationRefusalReason };

const receipt = (args: {
  readonly signatureId: ReleaseChannel["signatureId"];
  readonly transition: ActivationTransition;
  readonly fromMode: ReleaseMode;
  readonly toMode: ReleaseMode;
  readonly candidateId: string | null;
  readonly restoredCandidateId: string | null;
  readonly reason: string;
  readonly at: typeof DseTimestamp.Type;
}): ActivationReceipt =>
  decodeReceipt({
    schema: ACTIVATION_RECEIPT_SCHEMA_LITERAL,
    signatureId: args.signatureId,
    transition: args.transition,
    fromMode: args.fromMode,
    toMode: args.toMode,
    candidateId: args.candidateId,
    restoredCandidateId: args.restoredCandidateId,
    reason: args.reason,
    at: args.at,
  });

/**
 * Open a channel in SHADOW mode. The released candidate is held (so it can be
 * evaluated and later canaried) but the baseline is served. This is the safe
 * default: introducing a compiled artifact changes no live behavior.
 */
export const beginShadow = (args: {
  readonly signatureId: ReleaseChannel["signatureId"];
  readonly baseline: BaselinePointer;
  readonly candidate: ReleasedArtifactPointer;
  readonly now: () => typeof DseTimestamp.Type;
}): ActivationResult => {
  if (args.baseline.signatureId !== args.signatureId || args.candidate.signatureId !== args.signatureId) {
    return { ok: false, reason: "signature_mismatch" };
  }
  const now = args.now();
  const channel = decodeChannel({
    schema: RELEASE_CHANNEL_SCHEMA_LITERAL,
    signatureId: args.signatureId,
    mode: "shadow",
    baseline: args.baseline,
    candidate: args.candidate,
    updatedAt: now,
  });
  return {
    ok: true,
    channel,
    receipt: receipt({
      signatureId: args.signatureId,
      transition: "begin_shadow",
      fromMode: "shadow",
      toMode: "shadow",
      candidateId: args.candidate.candidateId,
      restoredCandidateId: null,
      reason: "shadow the compiled artifact against the hand-written baseline",
      at: now,
    }),
  };
};

/** Move a SHADOW channel to CANARY with an explicit rollout plan. */
export const beginCanary = (args: {
  readonly channel: ReleaseChannel;
  readonly plan: CanaryPlan;
  readonly reason: string;
  readonly now: () => typeof DseTimestamp.Type;
}): ActivationResult => {
  if (args.channel.mode !== "shadow") return { ok: false, reason: "wrong_mode" };
  if (args.channel.candidate === undefined) return { ok: false, reason: "missing_candidate" };
  const now = args.now();
  const channel = decodeChannel({
    ...args.channel,
    mode: "canary",
    canary: args.plan,
    updatedAt: now,
  });
  return {
    ok: true,
    channel,
    receipt: receipt({
      signatureId: args.channel.signatureId,
      transition: "begin_canary",
      fromMode: "shadow",
      toMode: "canary",
      candidateId: args.channel.candidate.candidateId,
      restoredCandidateId: null,
      reason: args.reason,
      at: now,
    }),
  };
};

/**
 * Promote a CANARY channel to ACTIVE. This is the rollout activation decision;
 * the artifact-level independent review already happened at promotion time. The
 * canary plan is cleared and the released artifact is served to every request.
 */
export const promoteActivation = (args: {
  readonly channel: ReleaseChannel;
  readonly reason: string;
  readonly now: () => typeof DseTimestamp.Type;
}): ActivationResult => {
  if (args.channel.mode !== "canary") return { ok: false, reason: "wrong_mode" };
  if (args.channel.candidate === undefined) return { ok: false, reason: "missing_candidate" };
  const now = args.now();
  const { canary: _canary, ...rest } = args.channel;
  const channel = decodeChannel({ ...rest, mode: "active", updatedAt: now });
  return {
    ok: true,
    channel,
    receipt: receipt({
      signatureId: args.channel.signatureId,
      transition: "promote",
      fromMode: "canary",
      toMode: "active",
      candidateId: args.channel.candidate.candidateId,
      restoredCandidateId: null,
      reason: args.reason,
      at: now,
    }),
  };
};

/** Abort a CANARY rollout back to the hand-written baseline. */
export const abortCanary = (args: {
  readonly channel: ReleaseChannel;
  readonly reason: string;
  readonly now: () => typeof DseTimestamp.Type;
}): ActivationResult => {
  if (args.channel.mode !== "canary") return { ok: false, reason: "wrong_mode" };
  const now = args.now();
  const { canary: _canary, ...rest } = args.channel;
  const channel = decodeChannel({ ...rest, mode: "rolled_back", updatedAt: now });
  return {
    ok: true,
    channel,
    receipt: receipt({
      signatureId: args.channel.signatureId,
      transition: "abort_canary",
      fromMode: "canary",
      toMode: "rolled_back",
      candidateId: args.channel.candidate?.candidateId ?? null,
      restoredCandidateId: null,
      reason: args.reason,
      at: now,
    }),
  };
};

/**
 * Roll back an ACTIVE channel. When a prior released artifact exists it is
 * restored as the served candidate WITHOUT an application rebuild (the channel
 * stays ACTIVE serving the prior artifact). When there is no prior release the
 * channel falls to ROLLED_BACK and serves the hand-written baseline.
 */
export const rollbackActivation = (args: {
  readonly channel: ReleaseChannel;
  readonly reason: string;
  readonly now: () => typeof DseTimestamp.Type;
}): ActivationResult => {
  if (args.channel.mode !== "active") return { ok: false, reason: "wrong_mode" };
  const now = args.now();
  const fromCandidateId = args.channel.candidate?.candidateId ?? null;
  if (args.channel.prior !== undefined) {
    const { prior: _prior, canary: _canary, ...rest } = args.channel;
    const channel = decodeChannel({
      ...rest,
      mode: "active",
      candidate: args.channel.prior,
      updatedAt: now,
    });
    return {
      ok: true,
      channel,
      receipt: receipt({
        signatureId: args.channel.signatureId,
        transition: "rollback",
        fromMode: "active",
        toMode: "active",
        candidateId: fromCandidateId,
        restoredCandidateId: args.channel.prior.candidateId,
        reason: args.reason,
        at: now,
      }),
    };
  }
  const { canary: _canary, ...rest } = args.channel;
  const channel = decodeChannel({ ...rest, mode: "rolled_back", updatedAt: now });
  return {
    ok: true,
    channel,
    receipt: receipt({
      signatureId: args.channel.signatureId,
      transition: "rollback",
      fromMode: "active",
      toMode: "rolled_back",
      candidateId: fromCandidateId,
      restoredCandidateId: null,
      reason: args.reason,
      at: now,
    }),
  };
};
