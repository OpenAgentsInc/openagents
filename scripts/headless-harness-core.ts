/**
 * Headless harness smoke core (#9161 slice 1).
 *
 * Pure parsing, classification, and summary logic for driving one coding
 * agent harness programmatically and grading the resulting conversation.
 * Slice 1 implements the Codex lane over `codex exec --json` against the
 * owner's live default Codex home (exec only — login flows stay forbidden
 * there). Shapes below are taken from the observed 0.145.0-alpha.27 wire,
 * matching the `CodexEvent` vocabulary in
 * `@openagentsinc/agent-harness-contract`.
 */

export type HeadlessHarnessId = "codex";

/** Typed failure classes, aligned with the harness-contract vocabulary. */
export type HeadlessFailureClass =
  | "account_auth_failed"
  | "account_exhausted"
  | "account_rate_limited"
  | "spawn_failed"
  | "execution_failed";

export interface CodexExecUsage {
  readonly inputTokens: number;
  readonly cachedInputTokens: number;
  readonly outputTokens: number;
  readonly reasoningOutputTokens: number;
}

export type CodexExecEvent =
  | { readonly type: "thread.started"; readonly threadId: string }
  | { readonly type: "turn.started" }
  | {
      readonly type: "item.completed";
      readonly itemType: string;
      readonly itemId: string;
      readonly text: string | null;
    }
  | { readonly type: "turn.completed"; readonly usage: CodexExecUsage | null }
  | { readonly type: "turn.failed"; readonly message: string }
  | { readonly type: "error"; readonly message: string };

interface JsonRecord {
  readonly [key: string]: unknown;
}

const asRecord = (value: unknown): JsonRecord | null =>
  typeof value === "object" && value !== null ? (value as JsonRecord) : null;

const asString = (value: unknown): string | null =>
  typeof value === "string" ? value : null;

const asNumber = (value: unknown): number => (typeof value === "number" ? value : 0);

/** Parse one `codex exec --json` stdout line. Unknown lines return null. */
export const parseCodexExecLine = (line: string): CodexExecEvent | null => {
  const trimmed = line.trim();
  if (trimmed === "") return null;
  let value: unknown;
  try {
    value = JSON.parse(trimmed);
  } catch {
    return null;
  }
  const record = asRecord(value);
  if (record === null) return null;
  switch (record.type) {
    case "thread.started": {
      const threadId = asString(record.thread_id);
      return threadId === null ? null : { type: "thread.started", threadId };
    }
    case "turn.started":
      return { type: "turn.started" };
    case "item.completed": {
      const item = asRecord(record.item);
      if (item === null) return null;
      const itemType = asString(item.type);
      const itemId = asString(item.id);
      if (itemType === null || itemId === null) return null;
      return {
        type: "item.completed",
        itemType,
        itemId,
        text: asString(item.text),
      };
    }
    case "turn.completed": {
      const usage = asRecord(record.usage);
      return {
        type: "turn.completed",
        usage:
          usage === null
            ? null
            : {
                inputTokens: asNumber(usage.input_tokens),
                cachedInputTokens: asNumber(usage.cached_input_tokens),
                outputTokens: asNumber(usage.output_tokens),
                reasoningOutputTokens: asNumber(usage.reasoning_output_tokens),
              },
      };
    }
    case "turn.failed": {
      const error = asRecord(record.error);
      const message = asString(error?.message) ?? "turn failed";
      return { type: "turn.failed", message };
    }
    case "error": {
      const message = asString(record.message) ?? "error";
      return { type: "error", message };
    }
    default:
      return null;
  }
};

export const parseCodexExecOutput = (stdout: string): CodexExecEvent[] => {
  const events: CodexExecEvent[] = [];
  for (const line of stdout.split("\n")) {
    const event = parseCodexExecLine(line);
    if (event !== null) events.push(event);
  }
  return events;
};

/** Classify a failure message into the typed operator-facing class. */
export const classifyCodexFailure = (message: string): HeadlessFailureClass => {
  const lowered = message.toLowerCase();
  if (
    lowered.includes("token could not be refreshed") ||
    lowered.includes("refresh token was revoked") ||
    lowered.includes("sign in again") ||
    lowered.includes("log out and sign in")
  ) {
    return "account_auth_failed";
  }
  if (
    lowered.includes("usage limit") ||
    lowered.includes("quota") ||
    lowered.includes("purchase more credits")
  ) {
    return "account_exhausted";
  }
  if (
    lowered.includes("rate limit") ||
    lowered.includes("429") ||
    lowered.includes("too many requests")
  ) {
    return "account_rate_limited";
  }
  return "execution_failed";
};

export interface HeadlessRunSummary {
  readonly harness: HeadlessHarnessId;
  readonly status: "completed" | "failed";
  readonly threadId: string | null;
  readonly finalAnswer: string | null;
  readonly usage: CodexExecUsage | null;
  readonly failureClass: HeadlessFailureClass | null;
  readonly failureMessage: string | null;
  readonly itemCounts: Readonly<Record<string, number>>;
}

/** Derive the run summary from the ordered exec events. */
export const summarizeCodexRun = (
  events: readonly CodexExecEvent[],
): HeadlessRunSummary => {
  let threadId: string | null = null;
  let finalAnswer: string | null = null;
  let usage: CodexExecUsage | null = null;
  let failureMessage: string | null = null;
  let completed = false;
  const itemCounts: Record<string, number> = {};
  for (const event of events) {
    switch (event.type) {
      case "thread.started":
        threadId = event.threadId;
        break;
      case "item.completed":
        itemCounts[event.itemType] = (itemCounts[event.itemType] ?? 0) + 1;
        if (event.itemType === "agent_message" && event.text !== null) {
          finalAnswer = event.text;
        }
        break;
      case "turn.completed":
        completed = true;
        usage = event.usage;
        break;
      case "turn.failed":
      case "error":
        failureMessage = event.message;
        break;
    }
  }
  const failed = !completed || finalAnswer === null;
  return {
    harness: "codex",
    status: failed ? "failed" : "completed",
    threadId,
    finalAnswer,
    usage,
    failureClass:
      failed && failureMessage !== null ? classifyCodexFailure(failureMessage) : failed ? "execution_failed" : null,
    failureMessage: failed ? failureMessage : null,
    itemCounts,
  };
};

/**
 * Codex binary candidates, mirroring the Desktop's discovery order
 * (`provider-runtime-host.ts`): standalone install, ChatGPT.app bundle, then
 * common PATH locations.
 */
export const codexBinaryCandidates = (home: string): readonly string[] => [
  `${home}/.local/bin/codex`,
  "/Applications/ChatGPT.app/Contents/Resources/codex",
  `${home}/Applications/ChatGPT.app/Contents/Resources/codex`,
  "/opt/homebrew/bin/codex",
  "/usr/local/bin/codex",
  "/usr/bin/codex",
];

/** Build the `codex exec` argv for one bounded read-only smoke turn. */
export const codexExecArgs = (params: {
  readonly model: string;
  readonly effort: string;
  readonly workdir: string;
  readonly prompt: string;
}): readonly string[] => [
  "exec",
  "--json",
  "--skip-git-repo-check",
  "-m",
  params.model,
  "-c",
  `model_reasoning_effort="${params.effort}"`,
  "-s",
  "read-only",
  "--cd",
  params.workdir,
  params.prompt,
];
