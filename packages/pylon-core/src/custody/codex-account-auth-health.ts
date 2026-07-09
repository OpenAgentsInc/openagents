/**
 * Codex account auth health — moved from
 * `apps/pylon/src/codex-account-auth-health.ts` (issue #8578, PY-1). Its only
 * out-of-package dependency was `defaultCodexAuthValidityProbe` from
 * `account-connect.ts`, which stays in `apps/pylon` because it depends on the
 * vendored Codex CLI resolver in `codex-composer.ts` (not yet in-package) —
 * see the header comment in `custody/account-connect.ts`.
 *
 * `probeAndRecordCodexAccountAuthHealth`'s `probe` option was already
 * optional; production callers (`assignment.ts`, `codex-agent-executor.ts`)
 * never supply one, relying on the module's own internal default of
 * `defaultCodexAuthValidityProbe`. Since that default can't live here, when
 * `probe` is omitted this version treats the account as `{ valid: true }`
 * (no health issue detected) — the same fail-safe verdict
 * `defaultCodexAuthValidityProbe` itself already returns for every
 * inconclusive/unavailable case, so this is a faithful, honest default
 * rather than a new behavior. `apps/pylon/src/codex-account-auth-health.ts`
 * (the shim) wraps this function to default-inject the real
 * `defaultCodexAuthValidityProbe` when a caller doesn't supply one, so
 * production behavior for `assignment.ts` / `codex-agent-executor.ts` is
 * unchanged.
 */
import {
  pylonAccountEnvironment,
  type ResolvedPylonAccountSelection,
} from "./account-registry.js"
import type { BootstrapSummary } from "../shared/bootstrap.js"
import {
  classifyCodexAccountFailure,
  type PylonCodexAccountFailure,
} from "./codex-account-health.js"
import { recordCodexAccountHealthFailure } from "./codex-account-health-ledger.js"
import type { PylonCodexAuthValidity, PylonCodexAuthValidityProbe } from "./account-connect.js"

type Summary = Pick<BootstrapSummary, "paths">

type InvalidCodexAuthValidity = Extract<PylonCodexAuthValidity, { valid: false }>

export type PylonCodexAccountAuthHealthResult =
  | { state: "valid" }
  | { state: "invalid"; failure: PylonCodexAccountFailure }

export function codexFailureFromAuthValidity(
  validity: InvalidCodexAuthValidity,
): PylonCodexAccountFailure {
  if (validity.failure !== undefined) {
    return validity.reason === "auth_error" && validity.failure.reason === "other"
      ? { ...validity.failure, reason: "credentials_revoked" }
      : validity.failure
  }
  return classifyCodexAccountFailure(validity.reason)
}

export async function probeAndRecordCodexAccountAuthHealth(
  summary: Summary,
  input: {
    account: ResolvedPylonAccountSelection | null | undefined
    env: Record<string, string | undefined>
    now: Date
    probe?: PylonCodexAuthValidityProbe
  },
): Promise<PylonCodexAccountAuthHealthResult> {
  if (input.account === null || input.account === undefined) return { state: "valid" }
  const validity: PylonCodexAuthValidity = input.probe === undefined
    ? { valid: true }
    : await input.probe({
        env: pylonAccountEnvironment(input.env, input.account),
        home: input.account.home,
      })
  if (validity.valid) return { state: "valid" }
  const failure = codexFailureFromAuthValidity(validity)
  await recordCodexAccountHealthFailure(summary, {
    accountRefHash: input.account.accountRefHash,
    failure,
    now: input.now,
  }).catch(() => undefined)
  return { state: "invalid", failure }
}
