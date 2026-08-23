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
 */

import { createInterface } from "node:readline";

import type { CoderSession } from "./coder-session.js";

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
  const flush = () => {
    const snapshot = session.snapshot();
    const last = snapshot.entries.at(-1);
    if (last === undefined || last.role !== "assistant") return;
    if (last.text.length > written) {
      stdout.write(last.text.slice(written));
      written = last.text.length;
    }
  };

  const unsubscribe = session.onChange(flush);

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
