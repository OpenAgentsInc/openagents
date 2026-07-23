/**
 * Programmatic multi-turn conversation driver (openagents#9161).
 *
 * Runs a scripted list of user turns against ONE harness session through the
 * production adapter path — no renderer, no Playwright — and produces a
 * combined gradable transcript (user_message / khala / assistant_message
 * JSONL lines with lane + model attribution, the format the openagents
 * coherence grader parses). Session continuity is the adapter's own resume
 * machinery: Codex exec re-drives `codex exec resume <threadId>`, Claude
 * passes `resume: sessionId`, opencode reuses its server session.
 */

import { Effect, Stream } from "effect";
import type { KhalaRuntimeSource } from "@openagentsinc/agent-runtime-schema";
import type { AgentHarness } from "./adapter.ts";
import type { HarnessPermissionMode } from "./permission.ts";

export interface ConversationTranscriptLine {
  readonly type: string;
  readonly [key: string]: unknown;
}

export interface ConversationTurnResult {
  readonly prompt: string;
  readonly answer: string;
  readonly finishReason: string;
  readonly eventKindCounts: Readonly<Record<string, number>>;
}

export interface HarnessConversationResult {
  readonly lane: string;
  readonly model: string;
  readonly turns: readonly ConversationTurnResult[];
  readonly transcriptLines: readonly ConversationTranscriptLine[];
}

export interface HarnessConversationParams {
  readonly adapter: AgentHarness;
  readonly lane: string;
  readonly model: string;
  readonly source: KhalaRuntimeSource;
  readonly sessionId: string;
  /** Ordered user turns, sent sequentially on one session. */
  readonly userTurns: readonly string[];
  readonly permissionMode?: HarnessPermissionMode;
}

/** Drive one multi-turn conversation through a harness adapter. */
export const runHarnessConversation = (
  params: HarnessConversationParams,
): Effect.Effect<HarnessConversationResult, unknown> =>
  Effect.gen(function* () {
    const lines: ConversationTranscriptLine[] = [];
    const turns: ConversationTurnResult[] = [];
    const session = yield* params.adapter.start({
      sessionId: params.sessionId,
      source: params.source,
      ...(params.permissionMode === undefined ? {} : { permissionMode: params.permissionMode }),
    });
    for (let index = 0; index < params.userTurns.length; index += 1) {
      const prompt = params.userTurns[index];
      lines.push({ type: "user_message", text: prompt });
      const control = yield* session.promptTurn({ turnId: `turn-${index + 1}`, prompt });
      const kindCounts: Record<string, number> = {};
      let answer = "";
      yield* control.events.pipe(
        Stream.runForEach((event) =>
          Effect.sync(() => {
            kindCounts[event.kind] = (kindCounts[event.kind] ?? 0) + 1;
            lines.push({ type: "khala", lane: params.lane, model: params.model, event });
            const payload = event as { kind: string; text?: string };
            if (payload.kind === "text.delta" && typeof payload.text === "string") {
              answer += payload.text;
            }
            if (payload.kind === "text.completed" && typeof payload.text === "string") {
              answer = payload.text;
            }
          }),
        ),
      );
      const result = yield* control.done;
      lines.push({ type: "assistant_message", text: answer, lane: params.lane });
      turns.push({
        prompt,
        answer,
        finishReason: result.finishReason,
        eventKindCounts: kindCounts,
      });
    }
    yield* session.stop();
    return {
      lane: params.lane,
      model: params.model,
      turns,
      transcriptLines: lines,
    };
  });
