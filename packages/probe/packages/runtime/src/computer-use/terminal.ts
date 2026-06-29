// Terminal tool surface (computer-use) over a real PTY.
//
// `Pty` is the seam: the REAL implementation (see `node-pty.ts`) spawns a shell
// in a pseudo-terminal; unit tests inject a deterministic fake. The surface
// streams output, can send input, and records named beats on the timeline.
//
// Outbound/destructive commands ride Probe's permission model at the tool layer
// (see `tools.ts`) — the surface itself is the mechanism, the gating is above.

import { type Timeline } from "./timeline";

export interface PtyExitResult {
  readonly code: number;
  readonly output: string;
}

export interface PtySession {
  /** Write input to the PTY (e.g. answering a prompt). */
  readonly write: (data: string) => void;
  /** Subscribe to streamed output chunks. Returns an unsubscribe fn. */
  readonly onData: (cb: (chunk: string) => void) => () => void;
  /** Resolve when the process exits, with code + full captured output. */
  readonly wait: () => Promise<PtyExitResult>;
  /** Kill the process (used on teardown/interrupt). */
  readonly kill: () => void;
}

export interface Pty {
  /** Spawn `command` (with optional args) in a fresh PTY session. */
  readonly spawn: (command: string, args?: ReadonlyArray<string>) => PtySession;
}

export interface TerminalSurface {
  /**
   * Run a command to completion, streaming output to `onChunk` if provided.
   * Records a beat with the exit code (never the raw output, which may contain
   * secrets). Returns the exit code + captured output.
   */
  readonly run: (
    command: string,
    args?: ReadonlyArray<string>,
    options?: { readonly onChunk?: (chunk: string) => void; readonly label?: string },
  ) => Promise<PtyExitResult>;
}

export interface MakeTerminalSurfaceOptions {
  readonly pty: Pty;
  readonly timeline: Timeline;
}

export function makeTerminalSurface(options: MakeTerminalSurfaceOptions): TerminalSurface {
  const { pty, timeline } = options;
  return {
    run: async (command, args, opts) => {
      const session = pty.spawn(command, args);
      let captured = "";
      const unsubscribe = session.onData((chunk) => {
        captured += chunk;
        opts?.onChunk?.(chunk);
      });
      try {
        const result = await session.wait();
        timeline.beat({
          surface: "terminal",
          label: opts?.label ?? `run ${command}`,
          status: result.code === 0 ? "ok" : "error",
          // Only the command + exit code are public-safe; output is withheld.
          detail: { command, exitCode: result.code },
        });
        return result;
      } finally {
        unsubscribe();
      }
    },
  };
}
