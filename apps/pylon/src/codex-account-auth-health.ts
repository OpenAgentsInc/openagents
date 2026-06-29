import {
  defaultCodexAuthValidityProbe,
  type PylonCodexAuthValidity,
  type PylonCodexAuthValidityProbe,
} from "./account-connect.js"
import {
  pylonAccountEnvironment,
  type ResolvedPylonAccountSelection,
} from "./account-registry.js"
import type { BootstrapSummary } from "./bootstrap.js"
import {
  classifyCodexAccountFailure,
  type PylonCodexAccountFailure,
} from "./codex-account-health.js"
import { recordCodexAccountHealthFailure } from "./codex-account-health-ledger.js"

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
  const validity = await (input.probe ?? defaultCodexAuthValidityProbe)({
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
