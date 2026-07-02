// Terminal/TUI scenarios — the journeys the terminal backend replays.
//
// A terminal scenario is the terminal-surface analogue of the browser
// `BrainStep` list in scenarios.ts: spawn a command (or TUI), then a list of
// deterministic steps that wait on a text snapshot, send input, or assert on
// what the terminal currently shows. NO sleeps — waits resolve on PTY output
// via the computer-use `TerminalView` (see probe-runtime). NO network: the
// shipped example uses a fake/echo command only.
//
// Like the browser scenarios, a deliberately-wrong variant proves the backend
// FAILS honestly: an assertion against text the terminal never shows is a real
// red, recorded in the snapshot timeline.

import type { TerminalCondition } from "@openagentsinc/probe-runtime/computer-use/terminal-snapshot";

/** One step in a terminal scenario. */
export type TerminalStep =
  | {
      readonly kind: "wait-for";
      readonly condition: TerminalCondition;
      readonly label?: string;
      readonly timeoutMs?: number;
    }
  | { readonly kind: "send"; readonly input: string; readonly label?: string }
  | {
      readonly kind: "snapshot";
      readonly label?: string;
    }
  | {
      readonly kind: "assert-contains";
      readonly value: string;
      readonly label?: string;
    }
  | {
      readonly kind: "assert-not-contains";
      readonly value: string;
      readonly label?: string;
    }
  | { readonly kind: "wait-exit"; readonly label?: string };

export interface TerminalScenario {
  /** Stable scenario name (lands in result.json). */
  readonly name: string;
  /** Command to spawn (run through the PTY's shell). */
  readonly command: string;
  /** Optional command args. */
  readonly args?: ReadonlyArray<string>;
  /** The ordered steps to replay. */
  readonly steps: ReadonlyArray<TerminalStep>;
}

/**
 * The shipped deterministic example. Uses a tiny portable shell script that
 * prints a banner, prompts for a name, echoes it back, and exits — exercising
 * spawn -> wait-for-text -> send-input -> assert-on-snapshot with NO network
 * and NO real TUI dependency. Pointing the assertion at text the script never
 * prints (see `echoPromptScenarioWrong`) FAILS honestly.
 */
export function echoPromptScenario(): TerminalScenario {
  return {
    name: "echo-prompt",
    // `printf` for the banner, `read` for the prompt, `echo` for the echo-back.
    // POSIX sh; no external tools, no network.
    command:
      "printf 'QA TERMINAL READY\\nname> '; read name; printf 'hello, %s!\\n' \"$name\"",
    steps: [
      { kind: "wait-for", condition: { kind: "text-visible", value: "QA TERMINAL READY" }, label: "banner renders" },
      { kind: "wait-for", condition: { kind: "text-visible", value: "name>" }, label: "prompt renders" },
      { kind: "send", input: "khala\n", label: "answer the prompt" },
      { kind: "wait-for", condition: { kind: "text-visible", value: "hello, khala!" }, label: "echo-back renders" },
      { kind: "wait-exit", label: "process exits cleanly" },
      { kind: "snapshot", label: "final-screen" },
      { kind: "assert-contains", value: "hello, khala!", label: 'snapshot contains "hello, khala!"' },
      { kind: "assert-not-contains", value: "Traceback", label: "no crash text" },
    ],
  };
}

/**
 * A deliberately-wrong variant: asserts the snapshot contains text the script
 * never prints. Used to prove a red is a real red (the failed assertion is
 * recorded with the snapshot at the moment of failure).
 */
export function echoPromptScenarioWrong(): TerminalScenario {
  const base = echoPromptScenario();
  return {
    ...base,
    name: "echo-prompt-wrong",
    steps: [
      { kind: "wait-for", condition: { kind: "text-visible", value: "QA TERMINAL READY" }, label: "banner renders" },
      { kind: "send", input: "khala\n", label: "answer the prompt" },
      { kind: "wait-exit", label: "process exits" },
      // WRONG on purpose: the script never prints "goodbye".
      { kind: "assert-contains", value: "goodbye, khala!", label: "snapshot contains goodbye (intentionally wrong)" },
    ],
  };
}
