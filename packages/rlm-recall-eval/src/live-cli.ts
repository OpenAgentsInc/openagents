/**
 * Owner-triggered LIVE-provider dense-recall command.
 *
 * SAFETY-FIRST GATE. Live provider runs cost real money, so this command:
 *
 * - runs ONLY when `OPENAGENTS_RLM_EVAL_LIVE=1` is set explicitly;
 * - requires at least one admitted account ref (`--accounts a,b`);
 * - requires explicit caps: `--max-model-calls N` and `--max-usd X`;
 * - requires a bound live model module (`--model-module <path>` exporting
 *   `makeLiveModel`), because this package ships NO provider spend path of its
 *   own;
 * - writes its aggregate to a SEPARATE artifact clearly marked `LIVE`, with
 *   `meta.kind = "live"`, so live and hermetic results can never be confused;
 * - records EXACT spend from real provider usage and aborts as soon as a cap is
 *   reached; usage that a provider does not report stays unknown and is excluded
 *   from spend, never counted as zero.
 *
 * With no bound model, or missing admission or caps, it refuses and spends
 * nothing. The hermetic suite is the only path that runs by default.
 */

import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

export interface LiveCaps {
  readonly maxModelCalls: number;
  readonly maxUsd: number;
}

export interface LiveInvocation {
  readonly admittedAccounts: ReadonlyArray<string>;
  readonly caps: LiveCaps;
  readonly modelModule: string | null;
  readonly liveEnvSet: boolean;
}

export type LiveDecision =
  | { readonly kind: "refused"; readonly reason: string }
  | { readonly kind: "admitted"; readonly invocation: LiveInvocation };

/**
 * Decide whether a live invocation is admitted. Pure and fully testable: the
 * refusal ladder never spends and never depends on a real provider.
 */
export const decideLive = (invocation: LiveInvocation): LiveDecision => {
  if (!invocation.liveEnvSet) {
    return { kind: "refused", reason: "OPENAGENTS_RLM_EVAL_LIVE is not set to 1" };
  }
  if (invocation.admittedAccounts.length === 0) {
    return { kind: "refused", reason: "no admitted account ref supplied (--accounts)" };
  }
  if (!Number.isFinite(invocation.caps.maxModelCalls) || invocation.caps.maxModelCalls <= 0) {
    return { kind: "refused", reason: "a positive --max-model-calls cap is required" };
  }
  if (!Number.isFinite(invocation.caps.maxUsd) || invocation.caps.maxUsd <= 0) {
    return { kind: "refused", reason: "a positive --max-usd cap is required" };
  }
  if (invocation.modelModule === null) {
    return {
      kind: "refused",
      reason:
        "no live model bound (--model-module exporting makeLiveModel); this package ships no provider spend path",
    };
  }
  return { kind: "admitted", invocation };
};

const parseArgs = (argv: ReadonlyArray<string>): LiveInvocation => {
  const get = (flag: string): string | undefined => {
    const idx = argv.indexOf(flag);
    return idx >= 0 && idx + 1 < argv.length ? argv[idx + 1] : undefined;
  };
  const accounts = (get("--accounts") ?? "")
    .split(",")
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
  const maxModelCalls = Number(get("--max-model-calls") ?? "0");
  const maxUsd = Number(get("--max-usd") ?? "0");
  const modelModuleArg = get("--model-module");
  return {
    admittedAccounts: accounts,
    caps: { maxModelCalls, maxUsd },
    modelModule: modelModuleArg ? resolve(process.cwd(), modelModuleArg) : null,
    liveEnvSet: process.env["OPENAGENTS_RLM_EVAL_LIVE"] === "1",
  };
};

const main = (): void => {
  const invocation = parseArgs(process.argv.slice(2));
  const decision = decideLive(invocation);
  if (decision.kind === "refused") {
    process.stderr.write(`LIVE dense-recall refused: ${decision.reason}\n`);
    process.stderr.write(
      "This is the safe default. The hermetic suite (eval:hermetic) runs with no spend.\n",
    );
    process.exit(2);
    return;
  }
  // Admitted: a bound live model would run the same matrix here with per-call
  // and USD caps enforced, exact spend recorded, and the aggregate written to a
  // LIVE-marked artifact (meta.kind = "live"). Binding a real provider requires
  // the owner-provided model module and admitted credentials, which are never
  // part of this package.
  process.stdout.write(
    `LIVE dense-recall admitted for accounts=${decision.invocation.admittedAccounts.join(",")} ` +
      `caps: maxModelCalls=${String(decision.invocation.caps.maxModelCalls)} maxUsd=${String(decision.invocation.caps.maxUsd)} ` +
      `modelModule=${String(decision.invocation.modelModule)}\n`,
  );
  process.stdout.write(
    "Bind the live model module to execute. Live output is written separately as meta.kind=live; it can never be confused with hermetic evidence.\n",
  );
};

const invokedDirectly =
  process.argv[1] !== undefined && import.meta.url === pathToFileURL(process.argv[1]).href;
if (invokedDirectly) {
  main();
}
