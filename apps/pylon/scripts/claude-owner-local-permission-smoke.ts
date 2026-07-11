#!/usr/bin/env bun

import { createHash } from "node:crypto"
import { mkdtemp, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { resolvePylonAccountSelection } from "../src/account-registry.js"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap.js"
import {
  executeClaudeAgentAssignment,
  issueClaudeOwnerLocalPermissionAuthority,
} from "../src/claude-agent-executor.js"
import { claudeAgentSmokeLease } from "../src/claude-agent-task-smoke.js"
import { assertPublicProjectionSafe, ensurePylonLocalState } from "../src/state.js"

const args = process.argv.slice(2)
const option = (name: string): string | undefined => {
  const index = args.indexOf(name)
  return index < 0 ? undefined : args[index + 1]
}
const accountRef = option("--account-ref")
if (accountRef === undefined || accountRef.trim().length === 0) {
  throw new Error("usage: smoke:claude-owner-local-permission --account-ref <named-claude-account>")
}

const realSummary = createBootstrapSummary(parseBootstrapArgs(["--json"]), Bun.env)
const account = await resolvePylonAccountSelection(realSummary, {
  provider: "claude_agent",
  accountRef,
})
if (account === null || account.accountRef === null) {
  throw new Error("named Claude account is not registered")
}

const temporaryHome = await mkdtemp(join(tmpdir(), "pylon-claude-owner-local-smoke-"))
try {
  const summary = createBootstrapSummary(
    parseBootstrapArgs([
      "--json",
      "--pylon-ref",
      `pylon.owner_local_smoke.${createHash("sha256")
        .update(account.accountRefHash)
        .digest("hex")
        .slice(0, 16)}`,
    ]),
    { PYLON_HOME: temporaryHome },
  )
  const state = await ensurePylonLocalState(summary)
  const assignmentRef =
    `assignment.public.claude_owner_local_smoke.${createHash("sha256")
      .update(`${account.accountRefHash}:${new Date().toISOString()}`)
      .digest("hex")
      .slice(0, 16)}`
  const lease = claudeAgentSmokeLease({ assignmentRef })
  const runRef = `run.claude_owner_local_smoke.${createHash("sha256")
    .update(assignmentRef)
    .digest("hex")
    .slice(0, 16)}`
  const authority = issueClaudeOwnerLocalPermissionAuthority({
    authorizationRef:
      `authorization.pylon.claude_owner_local.${createHash("sha256")
        .update(`${state.identity.pylonRef}:${account.accountRefHash}`)
        .digest("hex")
        .slice(0, 24)}`,
    pylonRef: state.identity.pylonRef,
    runRef,
    operationRef: assignmentRef,
    accountRefHash: account.accountRefHash,
    now: new Date(),
  })
  const closeout = await executeClaudeAgentAssignment(state, lease, new Date(), {
    account,
    claudeOwnerLocalPermissionControl: { authority },
  })
  const receipt = {
    schema: "openagents.pylon.claude_owner_local_permission_smoke.v1",
    ok: closeout?.status === "accepted",
    accountRefHash: account.accountRefHash,
    authorityRef: authority.authorityRef,
    assignmentRef,
    runRef,
    closeoutStatus: closeout?.status ?? null,
    blockerRefs: closeout?.blockerRefs ?? [],
    proofRefs: closeout?.proofRefs ?? [],
    resultRefs: closeout?.resultRefs ?? [],
    testRefs: closeout?.testRefs ?? [],
    artifactRefs: closeout?.artifactRefs ?? [],
    cleanup: "temporary_state_removed",
  }
  assertPublicProjectionSafe(receipt, "claudeOwnerLocalPermissionSmokeReceipt")
  const serialized = JSON.stringify(receipt, null, 2)
  if (/bypassPermissions|acceptEdits|CLAUDE_CODE_OAUTH_TOKEN|\/Users\//u.test(serialized)) {
    throw new Error("owner-local Claude smoke receipt failed redaction scan")
  }
  process.stdout.write(`${serialized}\n`)
  if (!receipt.ok) process.exitCode = 1
} finally {
  await rm(temporaryHome, { recursive: true, force: true })
}
