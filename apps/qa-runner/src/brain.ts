// The Brain: the decision-maker that drives a computer-use session.
//
// Modeled on executor's brains (scripted vs live-inference). Two implementations:
//   - `scriptedBrain`: a deterministic list of steps. Runs NOW; used by the demo
//     and by tests. No inference.
//   - `khalaBrain`: the BrainStep-shaped seam for `runQaSession`. It is INERT by
//     default (throws "not armed" unless an injected driver is supplied) so the
//     fixed-step pump never makes a live inference call without one.
//
// The REAL autonomous Khala driver does NOT use the BrainStep pump: the live
// ReAct/JSON-action loop needs to feed observations back to the model each turn,
// which the `next(context) => BrainStep` shape cannot express. That live loop is
// `runKhalaSession` (see `khala-session.ts` + `khala-driver.ts`): Khala chooses
// one JSON action per turn, the runner executes it against the #6175
// computer-use surface, appends the observation, and repeats. `khalaBrain` stays
// as the simpler injectable seam; `scriptedBrain` stays for deterministic CI.
//
// A Brain is just `next(context) => BrainStep | null`. The runner pumps it until
// it returns null (done), executing each step against the browser surface and
// recording assertions into the result.

import type { BrowserSurface, WaitForCondition } from "@openagentsinc/probe-runtime";

export type BrainStep =
  | { readonly kind: "navigate"; readonly url: string; readonly label?: string }
  | { readonly kind: "click"; readonly selector: string; readonly label?: string }
  | { readonly kind: "type"; readonly selector: string; readonly text: string; readonly label?: string }
  | { readonly kind: "wait-for"; readonly condition: WaitForCondition; readonly timeoutMs?: number; readonly label?: string }
  | { readonly kind: "screenshot"; readonly label: string }
  | {
      // Assert a fact about the current page; failure is honest (recorded as a
      // failed assertion -> non-passing result).
      readonly kind: "assert";
      readonly label: string;
      readonly check:
        | { readonly kind: "url-includes"; readonly value: string }
        | { readonly kind: "url-not-includes"; readonly value: string }
        | { readonly kind: "text-contains"; readonly value: string; readonly selector?: string }
        | { readonly kind: "text-not-contains"; readonly value: string; readonly selector?: string };
    };

export interface BrainContext {
  /** Index of the next step (0-based). */
  readonly stepIndex: number;
  /** The browser surface, for brains that read state to decide. */
  readonly browser: BrowserSurface;
}

export interface Brain {
  readonly name: string;
  /** Decide the next step, or null when the session is complete. */
  readonly next: (context: BrainContext) => Promise<BrainStep | null>;
}

/**
 * A deterministic brain: replays a fixed list of steps in order. This is the
 * runnable-now decision-maker for the demo and tests.
 */
export function scriptedBrain(steps: ReadonlyArray<BrainStep>): Brain {
  return {
    name: "scripted",
    next: async ({ stepIndex }) => steps[stepIndex] ?? null,
  };
}

export class KhalaBrainNotArmedError extends Error {
  constructor() {
    super(
      "khalaBrain is not armed: live openagents/khala inference is owner/flag-gated. " +
        "Inject a driver to enable it; CI and the default demo use scriptedBrain.",
    );
    this.name = "KhalaBrainNotArmedError";
  }
}

export interface KhalaBrainDriver {
  /** Decide the next step from the page state via Khala inference. */
  readonly next: (context: BrainContext) => Promise<BrainStep | null>;
}

export interface KhalaBrainOptions {
  /**
   * The live driver (Probe runtime + `openagents/khala`). When absent, the
   * brain is INERT and throws `KhalaBrainNotArmedError` on first use.
   */
  readonly driver?: KhalaBrainDriver;
}

/**
 * The Khala-driven brain seam. Owner/flag-gated: without an injected `driver`
 * it is inert and throws on use, so no live inference happens in CI or the
 * default demo. Wiring the real driver (Probe runtime issuing `openagents/khala`
 * tool-calls) is a gated follow-up.
 */
export function khalaBrain(options: KhalaBrainOptions = {}): Brain {
  return {
    name: "khala",
    next: async (context) => {
      if (!options.driver) throw new KhalaBrainNotArmedError();
      return options.driver.next(context);
    },
  };
}
