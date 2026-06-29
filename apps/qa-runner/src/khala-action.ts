// The Khala action protocol: a strict JSON next-action schema + parser/validator.
//
// We DO NOT rely on native function-calling (the served model — gpt-oss-20b
// behind `openagents/khala` — may not support it reliably). Instead the model
// emits ONE structured next action as JSON in its message content; we parse +
// validate it here. The action vocabulary is exactly the computer-use surface
// the runner can execute (navigate/click/type/readText/waitFor/screenshot/
// terminal_run) plus `done` (with a verdict) and `fail` (honest give-up).
//
// Robustness contract: parsing is total — an unparseable or schema-invalid model
// reply yields a typed `KhalaActionParseError` (never a fabricated success). The
// driver decides what to do with a parse error (record an honest failure).

import { Schema as S } from "effect";

/** A wait condition mirroring the computer-use `WaitForCondition`. */
export const KhalaWaitCondition = S.Union([
  S.Struct({ kind: S.Literal("url-includes"), value: S.String }),
  S.Struct({ kind: S.Literal("url-not-includes"), value: S.String }),
  S.Struct({ kind: S.Literal("text-visible"), value: S.String }),
  S.Struct({ kind: S.Literal("selector-visible"), selector: S.String }),
]);
export type KhalaWaitCondition = typeof KhalaWaitCondition.Type;

/** An assertion the model can make about the current page. */
export const KhalaAssertCheck = S.Union([
  S.Struct({ kind: S.Literal("url-includes"), value: S.String }),
  S.Struct({ kind: S.Literal("url-not-includes"), value: S.String }),
  S.Struct({ kind: S.Literal("text-contains"), value: S.String, selector: S.optional(S.String) }),
  S.Struct({ kind: S.Literal("text-not-contains"), value: S.String, selector: S.optional(S.String) }),
]);
export type KhalaAssertCheck = typeof KhalaAssertCheck.Type;

/**
 * The strict action vocabulary the model chooses from each turn. Exactly one
 * action per turn. `done` carries a verdict (pass/fail) so the model states the
 * outcome honestly; `fail` is an explicit give-up with a reason.
 */
export const KhalaAction = S.Union([
  S.Struct({ action: S.Literal("navigate"), url: S.String, reason: S.optional(S.String) }),
  S.Struct({ action: S.Literal("click"), selector: S.String, reason: S.optional(S.String) }),
  S.Struct({ action: S.Literal("type"), selector: S.String, text: S.String, reason: S.optional(S.String) }),
  S.Struct({ action: S.Literal("readText"), selector: S.optional(S.String), reason: S.optional(S.String) }),
  S.Struct({
    action: S.Literal("waitFor"),
    condition: KhalaWaitCondition,
    timeoutMs: S.optional(S.Number),
    reason: S.optional(S.String),
  }),
  S.Struct({ action: S.Literal("screenshot"), label: S.String, reason: S.optional(S.String) }),
  S.Struct({
    action: S.Literal("assert"),
    label: S.String,
    check: KhalaAssertCheck,
    reason: S.optional(S.String),
  }),
  S.Struct({
    action: S.Literal("terminal_run"),
    command: S.String,
    args: S.optional(S.Array(S.String)),
    reason: S.optional(S.String),
  }),
  S.Struct({
    action: S.Literal("done"),
    verdict: S.Literals(["pass", "fail"]),
    summary: S.optional(S.String),
  }),
  S.Struct({ action: S.Literal("fail"), reason: S.String }),
]);
export type KhalaAction = typeof KhalaAction.Type;

export class KhalaActionParseError extends Error {
  constructor(
    reason: string,
    /** The raw model text we failed to parse (truncated, public-safe-ish). */
    readonly raw: string,
  ) {
    super(`khala_action_parse_error: ${reason}`);
    this.name = "KhalaActionParseError";
  }
}

const decodeSync = S.decodeUnknownSync(KhalaAction);

/**
 * Extract the FIRST balanced top-level JSON object from a text blob. The model
 * may wrap its JSON in prose or a ```json fence; we find the first `{ ... }`
 * that balances. Returns the substring or undefined.
 */
export function extractFirstJsonObject(text: string): string | undefined {
  const start = text.indexOf("{");
  if (start === -1) return undefined;
  let depth = 0;
  let inString = false;
  let escaped = false;
  for (let i = start; i < text.length; i++) {
    const ch = text[i]!;
    if (inString) {
      if (escaped) escaped = false;
      else if (ch === "\\") escaped = true;
      else if (ch === '"') inString = false;
      continue;
    }
    if (ch === '"') inString = true;
    else if (ch === "{") depth++;
    else if (ch === "}") {
      depth--;
      if (depth === 0) return text.slice(start, i + 1);
    }
  }
  return undefined;
}

/**
 * Parse + validate the model's reply into exactly one `KhalaAction`. Total:
 * returns a typed action on success, throws `KhalaActionParseError` otherwise.
 * Never returns a fabricated default.
 */
export function parseKhalaAction(modelText: string): KhalaAction {
  const trimmed = modelText.trim();
  const jsonText = extractFirstJsonObject(trimmed) ?? trimmed;
  let value: unknown;
  try {
    value = JSON.parse(jsonText);
  } catch (error) {
    throw new KhalaActionParseError(
      `not valid JSON (${error instanceof Error ? error.message : String(error)})`,
      truncate(modelText),
    );
  }
  try {
    return decodeSync(value);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new KhalaActionParseError(
      `JSON did not match the action schema: ${message.length > 300 ? `${message.slice(0, 300)}…` : message}`,
      truncate(modelText),
    );
  }
}

function truncate(text: string, max = 400): string {
  return text.length > max ? `${text.slice(0, max)}…` : text;
}
