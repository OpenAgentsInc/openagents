import { Context, Data, Effect, Schema as S, Stream } from "effect";
import type { Scope } from "effect";

import type {
  ArtifactRef,
  InferenceProviderDescriptor,
  OwnerBoundCandidateSet,
  ProviderTurnRef,
  ReleasedArtifact,
  RouteDecision,
  RouteRecommendation,
  TurnCandidate,
  TurnIntent,
  TurnProviderRef,
  TurnRefusalReason,
  TurnRequestRef,
  TurnThreadRef,
  WorkContextEnvelope,
} from "@openagentsinc/agent-runtime-schema";

import type { TurnStateRecord } from "./turn-state.js";

/**
 * AFS-01 turn kernel ports.
 *
 * These are the injected boundaries the UI-neutral turn kernel composes. Each is
 * a scoped `Context.Service`. The kernel owns turn policy application, state
 * transitions, and safe projection. It owns no provider credentials, no store
 * driver, no thread persistence engine, and no UI. Every side of the boundary is
 * a Layer the host composes: Electron main composes the Desktop transition
 * adapters, tests compose deterministic in-memory layers.
 */

/** Typed context-assembly failure. The renderer never fills these facts. */
export class ContextSourceError extends S.TaggedErrorClass<ContextSourceError>()(
  "agent-turn-runtime.ContextSourceError",
  { reason: S.Literals(["unavailable", "over_budget", "not_ready"]) },
) {}

/** Typed provider-start failure. It never silently changes the effective lane. */
export class ProviderStartError extends S.TaggedErrorClass<ProviderStartError>()(
  "agent-turn-runtime.ProviderStartError",
  { reason: S.Literals(["unavailable", "unauthorized", "unadmitted", "not_ready", "helper_missing"]) },
) {}

/** Typed journal-persistence failure. */
export class TurnJournalError extends S.TaggedErrorClass<TurnJournalError>()(
  "agent-turn-runtime.TurnJournalError",
  { reason: S.Literals(["storage_unavailable", "invalid_record", "conflicting_turn"]) },
) {}

/**
 * The bounded provider stream vocabulary. It is neither the provider SDK's raw
 * event nor the renderer projection: it is the minimal advisory terminal-or-
 * progress signal the kernel folds. `Completed` carries a typed advisory
 * candidate, never a host command.
 */
export type ProviderStreamEvent = Data.TaggedEnum<{
  Progress: Record<never, never>;
  Completed: { readonly candidate: TurnCandidate };
  Refused: { readonly reason: TurnRefusalReason };
  Failed: { readonly detail: string };
}>;
export const ProviderStreamEvent = Data.taggedEnum<ProviderStreamEvent>();

/** A started provider run: its bound turn ref plus its bounded advisory stream. */
export interface ProviderRun {
  readonly providerTurnRef: ProviderTurnRef;
  readonly events: Stream.Stream<ProviderStreamEvent>;
}

/** Input the host hands a provider adapter to start one admitted turn. */
export interface ProviderStartInput {
  readonly providerRef: TurnProviderRef;
  readonly requestRef: TurnRequestRef;
  readonly threadRef: TurnThreadRef;
  readonly intent: TurnIntent;
  readonly context: WorkContextEnvelope;
}

/**
 * `ContextSource` adapts the host context assembler. Only this service can
 * create the effective context manifest. The renderer can request context; it
 * cannot forge an authoritative manifest from note text.
 */
export interface ContextSourceInterface {
  readonly manifest: (
    input: { readonly threadRef: TurnThreadRef; readonly intent: TurnIntent },
  ) => Effect.Effect<WorkContextEnvelope, ContextSourceError>;
}
export class ContextSource extends Context.Service<ContextSource, ContextSourceInterface>()(
  "agent-turn-runtime.ContextSource",
) {}

/**
 * `TurnPolicy` derives the only route decision. A model recommendation can never
 * add a candidate to the owner-bound set. The service fails closed by returning a
 * `closed` decision, never by throwing an untyped error.
 */
export interface TurnPolicyInterface {
  readonly decide: (
    input: {
      readonly requestRef: TurnRequestRef;
      readonly intent: TurnIntent;
      readonly context: WorkContextEnvelope;
      readonly candidateSet: OwnerBoundCandidateSet;
      readonly recommendation: RouteRecommendation | null;
    },
  ) => Effect.Effect<RouteDecision>;
}
export class TurnPolicy extends Context.Service<TurnPolicy, TurnPolicyInterface>()(
  "agent-turn-runtime.TurnPolicy",
) {}

/**
 * `ProviderRegistry` (the inference provider registry) exposes one typed
 * interface for local and remote inference. `start` acquires a scoped run;
 * closing the scope cancels the provider and releases its resources.
 */
export interface ProviderRegistryInterface {
  readonly describe: Effect.Effect<ReadonlyArray<InferenceProviderDescriptor>>;
  readonly start: (
    input: ProviderStartInput,
  ) => Effect.Effect<ProviderRun, ProviderStartError, Scope.Scope>;
}
export class ProviderRegistry extends Context.Service<ProviderRegistry, ProviderRegistryInterface>()(
  "agent-turn-runtime.ProviderRegistry",
) {}

/**
 * `TurnJournal` persists the driver-neutral turn state record. A concrete store
 * driver (Desktop JSON, in-memory) implements it; the kernel never imports one.
 */
export interface TurnJournalInterface {
  readonly record: (state: TurnStateRecord) => Effect.Effect<void, TurnJournalError>;
  readonly load: (requestRef: TurnRequestRef) => Effect.Effect<TurnStateRecord | null, TurnJournalError>;
  readonly list: Effect.Effect<ReadonlyArray<TurnStateRecord>, TurnJournalError>;
}
export class TurnJournal extends Context.Service<TurnJournal, TurnJournalInterface>()(
  "agent-turn-runtime.TurnJournal",
) {}

/** A bounded thread message the host persists as canonical turn state. */
export interface ThreadTurnMessage {
  readonly role: "user" | "assistant";
  readonly text: string;
}

/**
 * `ThreadRepository` adapts the canonical thread/journal authority. The kernel
 * persists canonical user and assistant turn state through it; it never owns the
 * bounded composer cache or the JSON file layout.
 */
export interface ThreadRepositoryInterface {
  readonly exists: (threadRef: TurnThreadRef) => Effect.Effect<boolean>;
  readonly appendUser: (
    threadRef: TurnThreadRef,
    message: ThreadTurnMessage,
  ) => Effect.Effect<void>;
  readonly appendAssistant: (
    threadRef: TurnThreadRef,
    message: ThreadTurnMessage,
  ) => Effect.Effect<void>;
}
export class ThreadRepository extends Context.Service<ThreadRepository, ThreadRepositoryInterface>()(
  "agent-turn-runtime.ThreadRepository",
) {}

/**
 * `ArtifactResolver` resolves a released immutable artifact read-only. The kernel
 * can never publish, promote, or replace an artifact through it.
 */
export interface ArtifactResolverInterface {
  readonly resolve: (ref: ArtifactRef) => Effect.Effect<ReleasedArtifact | null>;
}
export class ArtifactResolver extends Context.Service<ArtifactResolver, ArtifactResolverInterface>()(
  "agent-turn-runtime.ArtifactResolver",
) {}

/**
 * `ActionBroker` receives an advisory terminal candidate. Inference output is
 * advisory: a real file mutation, task run, debug step, source-control action, or
 * provider delegation must go through the existing IDE-08/10/11/12 and provider
 * dispatch services. The default broker performs no action; later packets wire
 * the real brokers.
 */
export interface ActionBrokerInterface {
  readonly deliver: (candidate: TurnCandidate) => Effect.Effect<void>;
}
export class ActionBroker extends Context.Service<ActionBroker, ActionBrokerInterface>()(
  "agent-turn-runtime.ActionBroker",
) {}
