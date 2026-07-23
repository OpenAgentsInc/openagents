import { Effect, Option, Ref, Schema as S, Stream } from "effect";
import {
  type AgentDefinitionHarnessKind,
  type AgentRuntimeAdapterKind,
  decodeKhalaRuntimeEvent,
  KhalaRuntimeEventSchemaLiteral,
  type KhalaRuntimeFinishReason,
  type KhalaRuntimeSource,
} from "@openagentsinc/agent-runtime-schema";
import type { AgentHarness, HarnessStartOptions } from "./adapter.ts";
import { HarnessStartError } from "./adapter.ts";
import type { HarnessCapabilityUnsupported } from "./capability.ts";
import type { HarnessBuiltinTool } from "./common-tool.ts";
import { HarnessContinuationState, HarnessResumeState } from "./lifecycle-state.ts";
import type {
  HarnessContinueTurnOptions,
  HarnessPromptControl,
  HarnessPromptTurnOptions,
  HarnessSession,
  HarnessTurnResult,
} from "./session.ts";
import { HarnessTurnError } from "./session.ts";
import type { HarnessStreamEvent } from "./stream.ts";

/**
 * Meta-agent harness (ai#39): an {@link AgentHarness} that wraps a FLEET of
 * member harnesses behind the exact same contract every single-runtime adapter
 * satisfies. From the outside it is indistinguishable from any other adapter —
 * `start`/`promptTurn`/`continueTurn`/suspend/resume, one session-global
 * contiguous `HarnessStreamEvent` cursor, fail-closed capability refusal.
 * Inside, each prompt turn is routed to one member harness and the member's
 * events are re-framed onto the meta session's sequence space WITHOUT
 * laundering: every delegated event keeps the member's own `source`
 * attribution and gains a `cause.member.<id>.<memberEventId>` causality ref,
 * and the turn is bracketed with `agent.child.started` / `agent.child.finished`
 * events naming the member (`childAgentId`) and the meta harness
 * (`parentAgentId`).
 *
 * Capability honesty is intersection/delegation:
 * - `builtinTools` is the intersection of member built-in tools (a tool the
 *   meta cannot guarantee on every route is not advertised).
 * - `supportsBuiltinToolApprovals` / `supportsBuiltinToolFiltering` are true
 *   only when every member reports true.
 * - Session verbs (`suspendTurn`, `continueTurn`, `compact`, `detach`)
 *   delegate to the routed member and propagate its typed
 *   `HarnessCapabilityUnsupported` refusal unchanged — the meta never claims a
 *   capability a member on the active route lacks.
 *
 * Suspend/continue is cursor-exact across member boundaries when the member is
 * cursor-exact: the meta continuation state pins the meta cursor, the member's
 * own continuation state, and any meta-synthesized events that were generated
 * but not yet delivered, so the next slice attaches at `cursor + 1` with no
 * gap and no duplicate. When the member reports a lossy (re-driven)
 * continuation, the meta continuation honestly reports `lossy: true`.
 *
 * No credential custody: members arrive as already-configured harness
 * instances injected by the host.
 */

/** One fleet member: a stable id plus an already-configured harness. */
export interface MetaAgentMember {
  /**
   * Stable member id on the Khala safe-ref charset
   * (`/^[A-Za-z0-9][A-Za-z0-9._:-]*$/`). It becomes the `childAgentId` of the
   * delegation events and part of every attribution ref.
   */
  readonly id: string;
  /** The member harness, configured (credentials and all) by the host. */
  readonly harness: AgentHarness;
}

/** Input to the routing decision, one call per prompt turn. */
export interface MetaAgentRouteOptions {
  readonly sessionId: string;
  readonly turnId: string;
  readonly prompt: string;
  /** The configured member ids, in configuration order. */
  readonly memberIds: ReadonlyArray<string>;
}

export interface MetaAgentHarnessConfig {
  /** Stable kebab-case slug; defaults to `meta-agent`. */
  readonly harnessId?: string;
  /** Canonical harness kind; defaults to `openagents_native`. */
  readonly harnessKind?: AgentDefinitionHarnessKind;
  /** Dispatch adapter kind; defaults to `openagents_native`. */
  readonly adapterKind?: AgentRuntimeAdapterKind;
  /** The fleet. Must be non-empty with unique safe-ref ids. */
  readonly members: ReadonlyArray<MetaAgentMember>;
  /** Pick the member id that runs a prompt turn. A throw fails the turn typed. */
  readonly route: (options: MetaAgentRouteOptions) => string;
}

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._:-]*$/;

const OBSERVED_AT = "2026-07-20T00:00:00.000Z";

/** Meta continuation payload: the member continuation plus the undelivered synthetic tail. */
const MetaContinuationData = S.Struct({
  memberId: S.NonEmptyString,
  turnId: S.NonEmptyString,
  /** Next meta sequence to allocate when the continuation resumes. */
  nextSequence: S.Number,
  /** Meta events generated (with final sequences) but not yet delivered. */
  pendingTail: S.Array(S.Unknown),
  /** The routed member's own continuation state, when the member still holds tail events. */
  memberContinuation: S.optionalKey(HarnessContinuationState),
});

const MetaResumeData = S.Struct({
  members: S.Array(S.Struct({ memberId: S.NonEmptyString, state: HarnessResumeState })),
});

/** Resume/continuation payload schema (see {@link AgentHarness.lifecycleStateSchema}). */
export const MetaAgentLifecycleData = S.Union([MetaContinuationData, MetaResumeData]);

/** Intersection of member built-in tools by native name, specs from the first member. */
const intersectBuiltinTools = (
  members: ReadonlyArray<MetaAgentMember>,
): ReadonlyArray<HarnessBuiltinTool> => {
  const [first, ...rest] = members;
  if (first === undefined) return [];
  return first.harness.builtinTools.filter((tool) =>
    rest.every((member) =>
      member.harness.builtinTools.some((other) => other.nativeName === tool.nativeName),
    ),
  );
};

interface ActiveTurn {
  readonly memberId: string;
  readonly session: HarnessSession;
  readonly turnId: string;
}

/** Build the meta-agent {@link AgentHarness} over an injected member fleet. */
export const metaAgentHarness = (config: MetaAgentHarnessConfig): AgentHarness => {
  const harnessId = config.harnessId ?? "meta-agent";
  const members = config.members;
  if (members.length === 0) {
    throw new Error("metaAgentHarness requires at least one member");
  }
  const membersById = new Map<string, MetaAgentMember>();
  for (const member of members) {
    if (!SAFE_REF_PATTERN.test(member.id)) {
      throw new Error(`metaAgentHarness member id is not a safe ref: ${JSON.stringify(member.id)}`);
    }
    if (membersById.has(member.id)) {
      throw new Error(`metaAgentHarness member id is duplicated: ${member.id}`);
    }
    membersById.set(member.id, member);
  }
  const memberIds = members.map((member) => member.id);

  const start = (options: HarnessStartOptions): Effect.Effect<HarnessSession, HarnessStartError> =>
    Effect.gen(function* () {
      const metaSource: KhalaRuntimeSource = options.source;
      const sessionId = options.sessionId;

      // Fresh-session continuation import (a different process resuming a slice).
      let importedContinuation:
        | {
            readonly memberId: string;
            readonly turnId: string;
            readonly pendingTail: ReadonlyArray<HarnessStreamEvent>;
            readonly memberContinuation: HarnessContinuationState | undefined;
          }
        | undefined;
      let seedSequence = 0;
      let seedCursor = -1;
      if (options.continueFrom !== undefined) {
        const data = S.decodeUnknownSync(MetaContinuationData)(options.continueFrom.data);
        importedContinuation = {
          memberId: data.memberId,
          turnId: data.turnId,
          pendingTail: data.pendingTail as ReadonlyArray<HarnessStreamEvent>,
          memberContinuation: data.memberContinuation,
        };
        seedSequence = data.nextSequence;
        seedCursor = options.continueFrom.cursor;
      }

      // Per-member resume state import (from a meta `detach`/`stop`).
      const memberResume = new Map<string, HarnessResumeState>();
      if (options.resumeFrom !== undefined) {
        const data = S.decodeUnknownSync(MetaResumeData)(options.resumeFrom.data);
        for (const entry of data.members) memberResume.set(entry.memberId, entry.state);
      }

      const sequenceRef = yield* Ref.make(seedSequence);
      const cursorRef = yield* Ref.make(seedCursor);
      /** Meta events generated (sequences allocated) but not yet delivered downstream. */
      const pendingRef = yield* Ref.make<ReadonlyArray<HarnessStreamEvent>>(
        importedContinuation?.pendingTail ?? [],
      );
      const activeRef = yield* Ref.make<Option.Option<ActiveTurn>>(Option.none());
      const memberSessionsRef = yield* Ref.make<ReadonlyMap<string, HarnessSession>>(new Map());

      const nextSequence = Ref.getAndUpdate(sequenceRef, (n) => n + 1);

      /** Member-attributed source: the member's adapter kind and refs, never the meta's. */
      const memberSourceFor = (member: MetaAgentMember): KhalaRuntimeSource => ({
        ...metaSource,
        adapterKind: member.harness.adapterKind,
        providerRef: `provider.member.${member.id}`,
        adapterSessionRef: `member.${member.id}.${sessionId}`,
      });

      const ensureMemberSession = (
        memberId: string,
        continueFrom?: HarnessContinuationState,
      ): Effect.Effect<HarnessSession, HarnessStartError> =>
        Effect.gen(function* () {
          const sessions = yield* Ref.get(memberSessionsRef);
          const existing = sessions.get(memberId);
          if (existing !== undefined) return existing;
          const member = membersById.get(memberId);
          if (member === undefined) {
            return yield* Effect.fail(
              new HarnessStartError({
                harnessId,
                sessionId,
                failureClass: "unknown_member",
                detail: `no configured member with id ${JSON.stringify(memberId)}`,
              }),
            );
          }
          const resume = memberResume.get(memberId);
          const session = yield* member.harness.start({
            sessionId,
            source: memberSourceFor(member),
            ...(options.skills === undefined ? {} : { skills: options.skills }),
            ...(options.permissionMode === undefined
              ? {}
              : { permissionMode: options.permissionMode }),
            ...(options.builtinToolFiltering === undefined
              ? {}
              : { builtinToolFiltering: options.builtinToolFiltering }),
            ...(resume === undefined ? {} : { resumeFrom: resume }),
            ...(continueFrom === undefined ? {} : { continueFrom }),
          });
          yield* Ref.update(memberSessionsRef, (map) => {
            const next = new Map(map);
            next.set(memberId, session);
            return next;
          });
          return session;
        });

      const metaBase = (turnId: string, sequence: number) => ({
        schema: KhalaRuntimeEventSchemaLiteral,
        eventId: `evt.meta.${turnId}.${sequence}`,
        turnId,
        threadId: sessionId,
        sequence,
        observedAt: OBSERVED_AT,
        source: metaSource,
        visibility: "private",
        redactionClass: "private_ref",
        causalityRefs: [] as ReadonlyArray<string>,
      });

      /**
       * Re-frame one member event onto the meta sequence space. The member's own
       * `source` is preserved verbatim (attribution, never laundering) and the
       * original member event id is kept as a causality ref.
       */
      const renumber = (
        memberEvent: HarnessStreamEvent,
        params: { readonly memberId: string; readonly turnId: string; readonly sequence: number },
      ): HarnessStreamEvent =>
        decodeKhalaRuntimeEvent({
          ...memberEvent,
          threadId: sessionId,
          sequence: params.sequence,
          eventId: `evt.meta.${params.turnId}.${params.sequence}`,
          causalityRefs: [
            ...memberEvent.causalityRefs,
            `cause.member.${params.memberId}.${memberEvent.eventId}`,
          ],
        });

      /**
       * Expand one member event into meta events, allocating meta sequences.
       * `turn.started` gains an `agent.child.started` frame right after it;
       * `turn.finished` gains an `agent.child.finished` frame right before it —
       * so the first and last events of a delegated turn stay `turn.started` /
       * `turn.finished` exactly like every other adapter.
       */
      const expand = (
        memberEvent: HarnessStreamEvent,
        params: { readonly memberId: string; readonly turnId: string },
      ): Effect.Effect<ReadonlyArray<HarnessStreamEvent>> =>
        Effect.gen(function* () {
          const { memberId, turnId } = params;
          if (memberEvent.kind === "turn.started") {
            const s0 = yield* nextSequence;
            const s1 = yield* nextSequence;
            return [
              renumber(memberEvent, { memberId, turnId, sequence: s0 }),
              decodeKhalaRuntimeEvent({
                ...metaBase(turnId, s1),
                kind: "agent.child.started",
                childAgentId: memberId,
                childRunId: `run.${memberId}.${turnId}`,
                parentAgentId: harnessId,
              }),
            ];
          }
          if (memberEvent.kind === "turn.finished") {
            const s0 = yield* nextSequence;
            const s1 = yield* nextSequence;
            return [
              decodeKhalaRuntimeEvent({
                ...metaBase(turnId, s0),
                kind: "agent.child.finished",
                childAgentId: memberId,
                childRunId: `run.${memberId}.${turnId}`,
                parentAgentId: harnessId,
                finishReason: memberEvent.finishReason,
                ...(memberEvent.usage === undefined ? {} : { usage: memberEvent.usage }),
              }),
              renumber(memberEvent, { memberId, turnId, sequence: s1 }),
            ];
          }
          const sequence = yield* nextSequence;
          return [renumber(memberEvent, { memberId, turnId, sequence })];
        });

      const finishReasonOf = (
        events: ReadonlyArray<HarnessStreamEvent>,
      ): KhalaRuntimeFinishReason => {
        for (let i = events.length - 1; i >= 0; i -= 1) {
          const event = events[i];
          if (event !== undefined && event.kind === "turn.finished") return event.finishReason;
        }
        return "unknown";
      };

      /** Wrap a member prompt control into the meta control (attribution + re-sequencing). */
      const makeDelegatedControl = (params: {
        readonly memberId: string;
        readonly turnId: string;
        readonly memberControl: HarnessPromptControl;
        /** Already-sequenced meta events replayed before the member tail. */
        readonly prefix: ReadonlyArray<HarnessStreamEvent>;
      }): HarnessPromptControl => {
        const { memberId, turnId, memberControl, prefix } = params;

        const transformed = memberControl.events.pipe(
          Stream.mapEffect((memberEvent) =>
            Effect.gen(function* () {
              const out = yield* expand(memberEvent, { memberId, turnId });
              yield* Ref.update(pendingRef, (pending) => [...pending, ...out]);
              return out;
            }),
          ),
          Stream.flatMap((events) => Stream.fromIterable(events)),
        );

        const stream: Stream.Stream<HarnessStreamEvent, HarnessTurnError> = Stream.fromIterable(
          prefix,
        ).pipe(
          Stream.concat(transformed),
          Stream.tap((event) =>
            Effect.gen(function* () {
              yield* Ref.set(cursorRef, event.sequence);
              yield* Ref.update(pendingRef, (pending) =>
                pending.filter((e) => e.sequence > event.sequence),
              );
            }),
          ),
        );

        const done: Effect.Effect<HarnessTurnResult, HarnessTurnError> = Effect.gen(function* () {
          const memberResult = yield* memberControl.done;
          const cursor = yield* Ref.get(cursorRef);
          const pending = yield* Ref.get(pendingRef);
          return {
            turnId,
            finishReason: pending.length === 0 ? memberResult.finishReason : "interrupted",
            ...(memberResult.usage === undefined ? {} : { usage: memberResult.usage }),
            lastCursor: cursor,
          } satisfies HarnessTurnResult;
        });

        return {
          turnId,
          events: stream,
          done,
          submitToolResult: (result) => memberControl.submitToolResult(result),
          submitToolApproval: (toolCallId, decision) =>
            memberControl.submitToolApproval(toolCallId, decision),
          submitUserMessage: (text) => memberControl.submitUserMessage(text),
          interrupt: () => memberControl.interrupt(),
        };
      };

      /** Control over an already-fully-generated synthetic tail (member side complete). */
      const makeSyntheticControl = (params: {
        readonly turnId: string;
        readonly prefix: ReadonlyArray<HarnessStreamEvent>;
      }): HarnessPromptControl => {
        const { turnId, prefix } = params;
        const stream: Stream.Stream<HarnessStreamEvent, HarnessTurnError> = Stream.fromIterable(
          prefix,
        ).pipe(
          Stream.tap((event) =>
            Effect.gen(function* () {
              yield* Ref.set(cursorRef, event.sequence);
              yield* Ref.update(pendingRef, (pending) =>
                pending.filter((e) => e.sequence > event.sequence),
              );
            }),
          ),
        );
        const done: Effect.Effect<HarnessTurnResult, HarnessTurnError> = Effect.gen(function* () {
          const cursor = yield* Ref.get(cursorRef);
          const pending = yield* Ref.get(pendingRef);
          return {
            turnId,
            finishReason: pending.length === 0 ? finishReasonOf(prefix) : "interrupted",
            lastCursor: cursor,
          } satisfies HarnessTurnResult;
        });
        const notActive = () =>
          new HarnessTurnError({
            harnessId,
            sessionId,
            turnId,
            failureClass: "no_active_tool_call",
            detail: "the delegated member turn already completed; only the synthetic tail remains",
          });
        return {
          turnId,
          events: stream,
          done,
          submitToolResult: () => Effect.fail(notActive()),
          submitToolApproval: () => Effect.fail(notActive()),
          submitUserMessage: () => Effect.void,
          interrupt: () => Effect.void,
        };
      };

      const promptTurn = (opts: HarnessPromptTurnOptions) =>
        Effect.gen(function* () {
          const memberId = yield* Effect.try({
            try: () =>
              config.route({
                sessionId,
                turnId: opts.turnId,
                prompt: opts.prompt,
                memberIds,
              }),
            catch: (cause) =>
              new HarnessTurnError({
                harnessId,
                sessionId,
                turnId: opts.turnId,
                failureClass: "route_failed",
                detail: String(cause),
              }),
          });
          if (!membersById.has(memberId)) {
            return yield* Effect.fail(
              new HarnessTurnError({
                harnessId,
                sessionId,
                turnId: opts.turnId,
                failureClass: "unknown_member",
                detail: `route selected unknown member ${JSON.stringify(memberId)}`,
              }),
            );
          }
          const memberSession = yield* ensureMemberSession(memberId).pipe(
            Effect.mapError(
              (error) =>
                new HarnessTurnError({
                  harnessId,
                  sessionId,
                  turnId: opts.turnId,
                  failureClass: "member_start_failed",
                  detail: `${error.failureClass}${error.detail === undefined ? "" : `: ${error.detail}`}`,
                }),
            ),
          );
          yield* Ref.set(pendingRef, []);
          yield* Ref.set(
            activeRef,
            Option.some({ memberId, session: memberSession, turnId: opts.turnId }),
          );
          const memberControl = yield* memberSession.promptTurn(opts);
          return makeDelegatedControl({
            memberId,
            turnId: opts.turnId,
            memberControl,
            prefix: [],
          });
        });

      const continueTurn = (
        opts: HarnessContinueTurnOptions,
      ): Effect.Effect<HarnessPromptControl, HarnessTurnError | HarnessCapabilityUnsupported> =>
        Effect.gen(function* () {
          const active = yield* Ref.get(activeRef);
          if (Option.isSome(active)) {
            const turn = active.value;
            const pending = yield* Ref.get(pendingRef);
            const memberControl = yield* turn.session.continueTurn(opts);
            return makeDelegatedControl({
              memberId: turn.memberId,
              turnId: turn.turnId,
              memberControl,
              prefix: pending,
            });
          }
          if (importedContinuation === undefined) {
            return yield* Effect.fail(
              new HarnessTurnError({
                harnessId,
                sessionId,
                turnId: "unknown",
                failureClass: "no_turn_to_continue",
              }),
            );
          }
          const imported = importedContinuation;
          const pending = yield* Ref.get(pendingRef);
          if (imported.memberContinuation === undefined) {
            // The member side already finished; only the synthetic tail remains.
            return makeSyntheticControl({ turnId: imported.turnId, prefix: pending });
          }
          const memberSession = yield* ensureMemberSession(
            imported.memberId,
            imported.memberContinuation,
          ).pipe(
            Effect.mapError(
              (error) =>
                new HarnessTurnError({
                  harnessId,
                  sessionId,
                  turnId: imported.turnId,
                  failureClass: "member_start_failed",
                  detail: `${error.failureClass}${error.detail === undefined ? "" : `: ${error.detail}`}`,
                }),
            ),
          );
          yield* Ref.set(
            activeRef,
            Option.some({
              memberId: imported.memberId,
              session: memberSession,
              turnId: imported.turnId,
            }),
          );
          const memberControl = yield* memberSession.continueTurn(opts);
          return makeDelegatedControl({
            memberId: imported.memberId,
            turnId: imported.turnId,
            memberControl,
            prefix: pending,
          });
        });

      const suspendTurn = (): Effect.Effect<
        HarnessContinuationState,
        HarnessCapabilityUnsupported
      > =>
        Effect.gen(function* () {
          const active = yield* Ref.get(activeRef);
          const cursor = yield* Ref.get(cursorRef);
          const pending = yield* Ref.get(pendingRef);
          const currentSequence = yield* Ref.get(sequenceRef);
          if (Option.isNone(active)) {
            // No delegated turn: continuation covers only the imported synthetic tail.
            const turnId = importedContinuation?.turnId ?? "unknown";
            return {
              harnessId,
              sessionId,
              turnId,
              cursor,
              lossy: false,
              data: {
                memberId: importedContinuation?.memberId ?? "unknown",
                turnId,
                nextSequence: currentSequence,
                pendingTail: pending,
              },
            };
          }
          const turn = active.value;
          // Delegate to the member; its typed capability refusal propagates honestly.
          const memberContinuation = yield* turn.session.suspendTurn();
          return {
            harnessId,
            sessionId,
            turnId: turn.turnId,
            cursor,
            lossy: memberContinuation.lossy,
            data: {
              memberId: turn.memberId,
              turnId: turn.turnId,
              nextSequence: currentSequence,
              pendingTail: pending,
              memberContinuation,
            },
          };
        });

      const startedSessions = Effect.gen(function* () {
        const sessions = yield* Ref.get(memberSessionsRef);
        return [...sessions.entries()];
      });

      const compact = (
        customInstructions?: string,
      ): Effect.Effect<void, HarnessCapabilityUnsupported> =>
        Effect.gen(function* () {
          const sessions = yield* startedSessions;
          for (const [, session] of sessions) {
            yield* session.compact(customInstructions);
          }
        });

      const detach = (): Effect.Effect<HarnessResumeState, HarnessCapabilityUnsupported> =>
        Effect.gen(function* () {
          const sessions = yield* startedSessions;
          const states: Array<{ memberId: string; state: HarnessResumeState }> = [];
          for (const [memberId, session] of sessions) {
            states.push({ memberId, state: yield* session.detach() });
          }
          return { harnessId, sessionId, data: { members: states } };
        });

      const stop = (): Effect.Effect<HarnessResumeState> =>
        Effect.gen(function* () {
          const sessions = yield* startedSessions;
          const states: Array<{ memberId: string; state: HarnessResumeState }> = [];
          for (const [memberId, session] of sessions) {
            states.push({ memberId, state: yield* session.stop() });
          }
          return { harnessId, sessionId, data: { members: states } };
        });

      const destroy = (): Effect.Effect<void> =>
        Effect.gen(function* () {
          const sessions = yield* startedSessions;
          for (const [, session] of sessions) {
            yield* session.destroy();
          }
          yield* Ref.set(memberSessionsRef, new Map());
          yield* Ref.set(activeRef, Option.none());
        });

      const session: HarnessSession = {
        sessionId,
        isResume: options.resumeFrom !== undefined || options.continueFrom !== undefined,
        modelId: `${harnessId}/fleet`,
        promptTurn,
        continueTurn,
        suspendTurn,
        compact,
        detach,
        stop,
        destroy,
      };
      return session;
    });

  return {
    specificationVersion: "agent-harness-v1",
    harnessId,
    harnessKind: config.harnessKind ?? "openagents_native",
    adapterKind: config.adapterKind ?? "openagents_native",
    builtinTools: intersectBuiltinTools(members),
    // Honest intersection: the meta only advertises what EVERY member supports.
    supportsBuiltinToolApprovals: members.every(
      (member) => member.harness.supportsBuiltinToolApprovals === true,
    ),
    supportsBuiltinToolFiltering: members.every(
      (member) => member.harness.supportsBuiltinToolFiltering === true,
    ),
    lifecycleStateSchema: MetaAgentLifecycleData,
    start,
  };
};
