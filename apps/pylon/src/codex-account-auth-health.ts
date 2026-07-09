/**
 * Re-export shim — moved to
 * `@openagentsinc/pylon-core/custody/codex-account-auth-health` (issue
 * #8578, PY-1).
 *
 * `probeAndRecordCodexAccountAuthHealth` is wrapped (not a pure re-export):
 * pylon-core's version cannot depend on `defaultCodexAuthValidityProbe`
 * (`./account-connect.js`'s app-side default, coupled to
 * `codex-composer.ts`), so when a caller doesn't supply `probe` explicitly
 * (every current production call site — `assignment.ts`,
 * `codex-agent-executor.ts`) this wrapper injects the real default so
 * production behavior is unchanged from before the move. Callers that
 * supply their own probe pass straight through.
 */
import {
  probeAndRecordCodexAccountAuthHealth as probeAndRecordCodexAccountAuthHealthCore,
  type PylonCodexAccountAuthHealthResult,
} from "@openagentsinc/pylon-core/custody/codex-account-auth-health"
import type { ResolvedPylonAccountSelection } from "./account-registry.js"
import type { BootstrapSummary } from "./bootstrap.js"
import { defaultCodexAuthValidityProbe, type PylonCodexAuthValidityProbe } from "./account-connect.js"

export * from "@openagentsinc/pylon-core/custody/codex-account-auth-health"

type Summary = Pick<BootstrapSummary, "paths">

export async function probeAndRecordCodexAccountAuthHealth(
  summary: Summary,
  input: {
    account: ResolvedPylonAccountSelection | null | undefined
    env: Record<string, string | undefined>
    now: Date
    probe?: PylonCodexAuthValidityProbe
  },
): Promise<PylonCodexAccountAuthHealthResult> {
  return probeAndRecordCodexAccountAuthHealthCore(summary, {
    ...input,
    probe: input.probe ?? defaultCodexAuthValidityProbe,
  })
}
