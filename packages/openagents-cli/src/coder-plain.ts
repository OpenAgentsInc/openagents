/**
 * The line-oriented mode for `openagents coder`.
 *
 * Used when standard input and output are not both TTYs, or when `--plain` is
 * given. It writes the same transcript the interface draws, without cursor
 * control, so the command stays usable in a pipe, in CI, and under an agent
 * that drives it programmatically.
 *
 * `docs/2026-08-23-openagents-coder-cli-spec.md` section 3.6 makes this the
 * contract: the interface is optional, the agent is not.
 *
 * It renders a subset on purpose. Assistant text is written as the Markdown
 * source the model produced, with no ANSI, because this is the path a script
 * reads. Tool calls are written as one line each, so a reader can still see
 * that a call happened between two sentences. Reasoning is not written: it is
 * a thought, and a pipe wants the answer.
 */

import { createInterface } from "node:readline";

import type { CoderEntry, CoderSession } from "./coder-session.js";

export interface CoderPlainOptions {
  readonly stdin: NodeJS.ReadableStream;
  readonly stdout: NodeJS.WritableStream;
  /** When set, answer this one prompt and exit rather than reading a loop. */
  readonly prompt?: string | undefined;
}

export async function runCoderPlain(
  session: CoderSession,
  options: CoderPlainOptions,
): Promise<number> {
  const { stdin, stdout, prompt } = options;

  let written = 0;
  // Notices and tool lines are tracked by their text rather than by position: a
  // failed turn removes the empty assistant entry, so an index into the
  // transcript can move backwards and skip the very notice that explains the
  // failure.
  const reported = new Set<string>();

  const announce = (line: string) => {
    if (reported.has(line)) return;
    reported.add(line);
    stdout.write(`${line}\n`);
  };

  const flush = () => {
    const entries = session.snapshot().entries;

    // Notices carry refusals and failures, and a tool call is the only sign
    // that the text either side of it came from two different places. Dropping
    // either here is how a turn becomes a silent or run-on reply.
    for (const entry of entries) {
      if (entry.role === "notice") announce(entry.text);
      else if (entry.role === "tool") announce(toolLine(entry));
    }

    const last = entries.at(-1);
    if (last === undefined || last.role !== "assistant") return;
    if (last.text.length > written) {
      stdout.write(last.text.slice(written));
      written = last.text.length;
    }
  };

  // Where the turns are recorded is a property of the session, not of the
  // interface, so the piped path says it too rather than leaving it to the
  // one reader who happens to be on a TTY.
  const scope = session.snapshot().scope;
  if (scope !== undefined) stdout.write(`${scope}\n`);

  const unsubscribe = session.onChange(flush);
  flush();

  const answer = async (line: string) => {
    written = 0;
    stdout.write(`\ncoder> `);
    await session.submit(line);
    stdout.write("\n");
  };

  try {
    if (prompt !== undefined) {
      await answer(prompt);
      return 0;
    }

    const reader = createInterface({ input: stdin, terminal: false });
    for await (const line of reader) {
      if (line.trim().length === 0) continue;
      await answer(line);
    }
    return 0;
  } finally {
    unsubscribe();
  }
}

/**
 * One line when the call starts and one when it ends.
 *
 * Both are keyed by their own text, so the start line is written once while
 * the call runs and the outcome line replaces nothing: a reader of a pipe sees
 * the call begin and sees how it ended.
 */
function toolLine(entry: CoderEntry): string {
  const tool = entry.tool;
  if (tool === undefined) return `[tool] ${entry.text}`;
  if (tool.status === "running") {
    return `\n[tool] ${tool.name} ${tool.arguments.replace(/\s+/g, " ").trim()}`;
  }
  return `[tool] ${tool.name} → ${tool.error === undefined ? "ok" : `failed: ${tool.error}`}`;
}
