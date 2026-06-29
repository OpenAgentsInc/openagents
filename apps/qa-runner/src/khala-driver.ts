// The Khala driver: the real agent loop where `openagents/khala` decides each
// computer-use action.
//
// This is the live replacement for the inert `khalaBrain` seam. Each turn it:
//   1. presents the available computer-use tools + the goal + the running
//      observation log to the model as a ReAct/JSON-action prompt;
//   2. asks the OpenAI-compatible `/chat/completions` endpoint for ONE next
//      action (plain `fetch`, no native function-calling, no new dependency);
//   3. parses + validates the reply into exactly one typed `KhalaAction`;
//   4. yields the action to the caller (the khala session runner executes it
//      against the #6175 computer-use surface);
//   5. the caller feeds the outcome back via `recordObservation`, and the loop
//      repeats until the model emits `done`/`fail` or a step cap.
//
// Endpoint config is env-driven (no hardcoded secrets); see `khala-config.ts`.
// Robustness: a per-action HTTP timeout (in the chat client) and honest failure
// on unparseable / invalid replies (the driver throws `KhalaActionParseError`;
// the session runner records a real failure, never a fabricated success).

import { type KhalaAction, parseKhalaAction } from "./khala-action";

/** An executable action: the action vocabulary minus the loop-ending verbs. */
export type KhalaExecutableAction = Exclude<KhalaAction, { action: "done" } | { action: "fail" }>;

/** A single chat message in the OpenAI-compatible wire shape. */
export interface ChatMessage {
  readonly role: "system" | "user" | "assistant";
  readonly content: string;
}

/**
 * Minimal chat client. Injectable for tests (driver/session tests pass a fake
 * that returns canned action JSON; no network in CI). The real implementation is
 * `makeFetchChatClient` (plain `fetch`, no new dependency).
 */
export interface ChatClient {
  readonly complete: (messages: ReadonlyArray<ChatMessage>) => Promise<string>;
}

/** A public-safe record of one model turn, for the run transcript + trace. */
export interface KhalaTurnRecord {
  readonly turn: number;
  /** The action the model chose (verbatim typed action). */
  readonly action: KhalaAction;
  /** A neutral observation appended after the caller executed it. */
  readonly observation?: string;
}

export interface KhalaDriverOptions {
  readonly goal: string;
  readonly chat: ChatClient;
  /** Hard cap on model turns; defaults to 16. */
  readonly maxTurns?: number;
  /**
   * How many times to RE-prompt the model after an unparseable/invalid reply on
   * the SAME turn before failing honestly. Defaults to 1 (one corrective retry).
   * A retry does not consume a turn; it asks the model to fix its output.
   */
  readonly reparseAttempts?: number;
  /** Optional log sink for the run transcript (defaults to console.log). */
  readonly log?: (line: string) => void;
}

export interface KhalaDriver {
  /**
   * Ask the model for the next action. Returns the typed action, or null when
   * the model emitted `done`/`fail` or the step cap is reached. Throws
   * `KhalaActionParseError` on an unparseable/invalid reply (honest failure).
   */
  readonly nextAction: () => Promise<KhalaExecutableAction | null>;
  /** Feed the outcome of the last action back to the model. */
  readonly recordObservation: (observation: string) => void;
  /** The ordered turn records (action + observation). */
  readonly transcript: () => ReadonlyArray<KhalaTurnRecord>;
  /** The final verdict the model declared (or "incomplete" on cap). */
  readonly finalVerdict: () => "pass" | "fail" | "incomplete";
}

export const KHALA_SYSTEM_PROMPT = [
  "You are Khala, an autonomous QA agent driving a real web browser to verify a flow.",
  "Each turn you choose EXACTLY ONE next action and reply with a SINGLE JSON object and nothing else.",
  "Do not add prose, markdown fences, or multiple objects. Output JSON only.",
  "",
  "Available actions (the only valid shapes):",
  '  {"action":"navigate","url":"/path or https://..."}',
  '  {"action":"click","selector":"<css selector>"}',
  '  {"action":"type","selector":"<css selector>","text":"<text>"}',
  '  {"action":"readText","selector":"<optional css selector>"}',
  '  {"action":"waitFor","condition":{"kind":"url-includes","value":"..."},"timeoutMs":10000}',
  '  {"action":"waitFor","condition":{"kind":"text-visible","value":"..."}}',
  '  {"action":"waitFor","condition":{"kind":"selector-visible","selector":"..."}}',
  '  {"action":"screenshot","label":"<short-label>"}',
  '  {"action":"assert","label":"<what you assert>","check":{"kind":"url-includes","value":"..."}}',
  '  {"action":"assert","label":"<what you assert>","check":{"kind":"text-contains","value":"..."}}',
  '  {"action":"terminal_run","command":"<cmd>","args":["..."]}',
  '  {"action":"done","verdict":"pass","summary":"<one line>"}',
  '  {"action":"fail","reason":"<why you cannot proceed>"}',
  "",
  "Rules:",
  "- Never sleep; wait on a condition with waitFor.",
  "- Use readText to inspect the page before you assert.",
  "- Take a screenshot at the key moment so the recording shows the result.",
  "- Emit an assert action for each fact the goal requires. A false assert is an honest failure.",
  "- When the goal is verified, emit done with verdict pass. If you cannot proceed, emit done verdict fail or fail.",
].join("\n");

/**
 * Construct a live Khala driver over a chat client. The session runner pumps
 * `nextAction`, executes each action against the computer-use surface, and feeds
 * the result back via `recordObservation`.
 */
export function makeKhalaDriver(options: KhalaDriverOptions): KhalaDriver {
  const maxTurns = options.maxTurns ?? 16;
  const reparseAttempts = options.reparseAttempts ?? 1;
  const log = options.log ?? ((line: string) => console.log(line));
  const messages: ChatMessage[] = [
    { role: "system", content: KHALA_SYSTEM_PROMPT },
    { role: "user", content: `Goal: ${options.goal}\n\nChoose your first action (JSON only).` },
  ];
  const records: KhalaTurnRecord[] = [];
  let pending: { turn: number; action: KhalaAction } | undefined;
  let verdict: "pass" | "fail" | "incomplete" = "incomplete";
  let turn = 0;

  const recordObservation = (observation: string): void => {
    if (pending) {
      records.push({ turn: pending.turn, action: pending.action, observation });
      pending = undefined;
    }
    messages.push({
      role: "user",
      content: `Observation: ${observation}\n\nChoose the next action (JSON only).`,
    });
  };

  const nextAction = async (): Promise<KhalaExecutableAction | null> => {
    // Flush a still-pending record (action executed but no observation reported).
    if (pending) {
      records.push({ turn: pending.turn, action: pending.action });
      pending = undefined;
    }
    if (turn >= maxTurns) {
      log(`[khala] step cap (${maxTurns} turns) reached without a verdict`);
      verdict = "incomplete";
      return null;
    }
    turn += 1;
    // Ask for one action; on an unparseable/invalid reply, re-prompt up to
    // `reparseAttempts` times (a bounded corrective loop, not an infinite one),
    // then fail honestly by re-throwing the parse error.
    let action: KhalaAction | undefined;
    let lastError: unknown;
    for (let attempt = 0; attempt <= reparseAttempts; attempt++) {
      const raw = await options.chat.complete(messages);
      messages.push({ role: "assistant", content: raw });
      try {
        action = parseKhalaAction(raw);
        break;
      } catch (error) {
        lastError = error;
        if (attempt < reparseAttempts) {
          log(`[khala] turn ${turn}: invalid action, re-prompting (attempt ${attempt + 1}/${reparseAttempts})`);
          messages.push({
            role: "user",
            content:
              "Your last reply was not a single valid JSON action object. Reply with EXACTLY ONE JSON object from the allowed action shapes, and nothing else.",
          });
        }
      }
    }
    if (!action) {
      throw lastError instanceof Error ? lastError : new Error(String(lastError));
    }
    log(`[khala] turn ${turn}: ${describeAction(action)}`);
    if (action.action === "done") {
      verdict = action.verdict;
      records.push({ turn, action });
      return null;
    }
    if (action.action === "fail") {
      verdict = "fail";
      records.push({ turn, action });
      return null;
    }
    pending = { turn, action };
    return action;
  };

  return {
    nextAction,
    recordObservation,
    transcript: () => [...records],
    finalVerdict: () => verdict,
  };
}

export function describeAction(action: KhalaAction): string {
  switch (action.action) {
    case "navigate":
      return `navigate ${action.url}`;
    case "click":
      return `click ${action.selector}`;
    case "type":
      return `type into ${action.selector} (len ${action.text.length})`;
    case "readText":
      return `readText ${action.selector ?? "(body)"}`;
    case "waitFor":
      return `waitFor ${JSON.stringify(action.condition)}`;
    case "screenshot":
      return `screenshot ${action.label}`;
    case "assert":
      return `assert "${action.label}" ${JSON.stringify(action.check)}`;
    case "terminal_run":
      return `terminal_run ${action.command}`;
    case "done":
      return `done verdict=${action.verdict}`;
    case "fail":
      return `fail: ${action.reason}`;
  }
}
