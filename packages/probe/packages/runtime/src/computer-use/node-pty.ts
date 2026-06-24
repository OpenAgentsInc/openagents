// Real PTY-style adapter for the terminal seam.
//
// This module is the ONLY place that spawns a real process for the terminal
// surface; it is NOT imported by unit tests (which inject a fake `Pty`). It uses
// Node's `child_process.spawn` with a merged stdout/stderr stream — an honest
// streaming subprocess. For a true allocated pseudo-terminal (interactive TUIs,
// color, job control), swap this for `node-pty` behind the same `Pty` seam; the
// surface contract does not change.

import { type ChildProcessWithoutNullStreams, spawn } from "node:child_process";
import type { Pty, PtyExitResult, PtySession } from "./terminal";

export interface NodePtyOptions {
  /** Working directory the command runs in (scope this to a run workspace). */
  readonly cwd?: string;
  /** Environment for the child. Defaults to the current process env. */
  readonly env?: Readonly<Record<string, string | undefined>>;
  /** Shell to run commands through. Defaults to /bin/sh -c. */
  readonly shell?: string;
}

export function makeNodePty(options: NodePtyOptions = {}): Pty {
  return {
    spawn: (command, args): PtySession => {
      const shell = options.shell ?? "/bin/sh";
      const fullCommand = args && args.length > 0 ? `${command} ${args.join(" ")}` : command;
      const child: ChildProcessWithoutNullStreams = spawn(shell, ["-c", fullCommand], {
        cwd: options.cwd ?? process.cwd(),
        env: { ...process.env, ...options.env },
      }) as ChildProcessWithoutNullStreams;

      const listeners = new Set<(chunk: string) => void>();
      let output = "";
      const emit = (chunk: string) => {
        output += chunk;
        for (const l of listeners) l(chunk);
      };
      child.stdout.on("data", (d: Buffer) => emit(d.toString("utf8")));
      child.stderr.on("data", (d: Buffer) => emit(d.toString("utf8")));

      const exit = new Promise<PtyExitResult>((resolve) => {
        child.on("close", (code) => resolve({ code: code ?? 0, output }));
        child.on("error", (error) => emit(`spawn error: ${error.message}\n`));
      });

      return {
        write: (data) => child.stdin.write(data),
        onData: (cb) => {
          listeners.add(cb);
          return () => listeners.delete(cb);
        },
        wait: () => exit,
        kill: () => child.kill("SIGKILL"),
      };
    },
  };
}
