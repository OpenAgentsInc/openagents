/**
 * Running a command on this machine.
 *
 * Without this, a session that wanted `pwd` had to start a child coding agent
 * on a hosted model to run it. That is minutes and real money for a line of
 * output, and the answer arrives second-hand.
 *
 * The session runs commands directly instead, and `delegate` goes back to what
 * it is for: work that is worth a whole agent.
 *
 * ## What the refusal list is, and is not
 *
 * It stops a small number of irreversible mistakes: erasing a home directory or
 * a disk, reformatting, halting the machine. It is not a security boundary and
 * cannot be one -- a command can be assembled from variables, decoded, or
 * written to a file and run, and no list of patterns sees that. It catches the
 * accident, not the intent.
 *
 * So it is kept short and aimed only at what cannot be undone. `rm -rf` on a
 * build directory is ordinary work and is allowed; `rm -rf` on `/` or `~` is
 * not, because no one means it.
 */

import { spawn } from "node:child_process";

/** How much of a command's output the model is shown. */
const OUTPUT_LIMIT = 30_000;

/** The default deadline, and the longest one a caller may ask for. */
const DEFAULT_TIMEOUT_MS = 120_000;
const MAXIMUM_TIMEOUT_MS = 600_000;

/**
 * Commands that cannot be undone, and are never what was meant.
 *
 * Each is paired with what to say, because a bare refusal reads as the tool
 * being broken rather than as the command being the problem.
 */
const REFUSED: ReadonlyArray<readonly [RegExp, string]> = [
  [
    // `rm -rf` aimed at a root, a home, or everything in one. Aimed at a build
    // directory it is ordinary work, so the target is what decides.
    /\brm\s+(-[a-zA-Z]*\s+)*-?[a-zA-Z]*[rR][a-zA-Z]*f?[a-zA-Z]*\s+(-[a-zA-Z]+\s+)*(\/|~|\$HOME|\/\*|~\/\*|\$HOME\/\*)(\s|$)/,
    "That would erase a root or a home directory.",
  ],
  [/\bmkfs(\.\w+)?\b/, "That would reformat a filesystem."],
  [/\bdd\b[^\n]*\bof=\/dev\/(disk|rdisk|sd|nvme|hd)/, "That would write over a raw device."],
  [/\bdiskutil\s+(erase|reformat|partition)/, "That would erase or repartition a disk."],
  [/\b(shutdown|reboot|halt|poweroff)\b/, "That would stop this machine."],
  [/:\(\)\s*\{\s*:\|:&\s*\}\s*;\s*:/, "That is a fork bomb."],
  [
    /\bchmod\s+(-[a-zA-Z]+\s+)*(-R|--recursive)\s+[0-7]{3,4}\s+(\/|~|\$HOME)(\s|$)/,
    "That would change the permissions of a whole root or home directory.",
  ],
  [/>\s*\/dev\/(disk|rdisk|sd|nvme|hd)/, "That would write over a raw device."],
];

/** Why this command will not be run, or undefined when it will. */
export const refusalFor = (command: string): string | undefined => {
  for (const [pattern, reason] of REFUSED) {
    if (pattern.test(command)) {
      return `${reason} This session refuses it. If you meant something narrower, name the directory.`;
    }
  }
  return undefined;
};

export interface ShellResult {
  readonly output: string;
  readonly code: number | undefined;
  readonly timedOut: boolean;
  /**
   * Characters the collector refused to hold, once the output passed the cap.
   *
   * Counted rather than discarded unrecorded: the reader of this result is a
   * model, and a `git log` it was handed half of reads exactly like a whole
   * one. The count is what lets the notice say how much is missing.
   */
  readonly dropped: number;
}

/**
 * Run one command and report what it said.
 *
 * Both streams are collected in the order they arrive, because a command's
 * error output is usually the part worth reading and separating them loses
 * which line came before which.
 *
 * No login shell: rc files are slow, and a command whose behaviour depends on
 * an interactive profile is one that will not reproduce.
 */
export async function runShell(
  command: string,
  options: { readonly cwd: string; readonly timeoutMs?: number; readonly signal: AbortSignal },
): Promise<ShellResult> {
  const timeoutMs = Math.min(options.timeoutMs ?? DEFAULT_TIMEOUT_MS, MAXIMUM_TIMEOUT_MS);

  return await new Promise<ShellResult>((resolve) => {
    const child = spawn("/bin/sh", ["-c", command], {
      cwd: options.cwd,
      // No terminal, so a command that would prompt reads end-of-file and
      // stops rather than waiting where nobody can see it.
      stdio: ["ignore", "pipe", "pipe"],
    });

    let output = "";
    let dropped = 0;
    let settled = false;
    const finish = (result: ShellResult) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      options.signal.removeEventListener("abort", onAbort);
      resolve(result);
    };

    const timer = setTimeout(() => {
      child.kill("SIGKILL");
      finish({ output, code: undefined, timedOut: true, dropped });
    }, timeoutMs);

    const onAbort = () => {
      child.kill("SIGKILL");
      finish({ output, code: undefined, timedOut: false, dropped });
    };
    options.signal.addEventListener("abort", onAbort, { once: true });

    const collect = (chunk: Buffer) => {
      const text = chunk.toString("utf8");
      // Past the cap the text is counted rather than kept. Counting it is the
      // whole difference between a result that says what is missing and one
      // that quietly ends mid-file.
      if (output.length < OUTPUT_LIMIT) output += text;
      else dropped += text.length;
    };
    child.stdout.on("data", collect);
    child.stderr.on("data", collect);

    child.on("error", (cause) => {
      finish({
        output: `The command could not be started: ${cause.message}`,
        code: undefined,
        timedOut: false,
        dropped: 0,
      });
    });
    child.on("close", (code) => {
      finish({ output, code: code ?? undefined, timedOut: false, dropped });
    });
  });
}

/** What the model is shown for one run. */
export const renderShell = (result: ShellResult, timeoutMs: number): string => {
  const kept = Math.min(result.output.length, OUTPUT_LIMIT);
  const cut = result.output.length - kept + result.dropped;
  const bounded =
    cut > 0
      ? `${result.output.slice(0, OUTPUT_LIMIT)}\n\n[The command printed ${String(kept + cut)} characters and this tool holds ${String(OUTPUT_LIMIT)}, so ${String(cut)} were cut from the end. What you have stops mid-output and must not be read as the whole of it: narrow the command, or write it to a file and read the part you need.]`
      : result.output;
  const body = bounded.trim();

  if (result.timedOut) {
    return `The command did not finish within ${String(Math.round(timeoutMs / 1000))}s and was stopped.\n\n${body}`;
  }
  if (result.code === undefined) return body.length === 0 ? "The command was interrupted." : body;
  // The exit code is reported on failure because an empty failure reads as an
  // empty success, and a command that fails silently is the one that misleads.
  if (result.code !== 0) {
    return `The command exited with code ${String(result.code)}.\n\n${body}`;
  }
  return body.length === 0 ? "Success" : body;
};

export { DEFAULT_TIMEOUT_MS, MAXIMUM_TIMEOUT_MS, OUTPUT_LIMIT };
