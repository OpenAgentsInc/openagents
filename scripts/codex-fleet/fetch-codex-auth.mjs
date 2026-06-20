#!/usr/bin/env node
// fetch-codex-auth.mjs — fetch a Codex/ChatGPT subscription auth blob from the
// CENTRAL device-flow provider-account store in openagents.com and materialize
// it as a codex-native `auth.json` under an isolated CODEX_HOME.
//
// This is the crux of the Codex fleet: cloud workers run `codex exec` on OUR
// ChatGPT/Codex SUBSCRIPTION without any per-machine interactive `codex login`.
// The OAuth material was collected once, centrally, via the device-code flow
// documented in:
//   apps/openagents.com/docs/2026-06-05-chatgpt-device-login-operator-runbook.md
//
// Two-token machine flow (no browser, no interactive login):
//   1. ADMIN  token -> POST /api/operator/provider-accounts/chatgpt-codex/leases
//      Selects a connected+healthy Codex account and returns { leaseRef, providerAccountRef }.
//   2. ADMIN  token -> POST /api/operator/provider-accounts/chatgpt-codex/leases/grant
//      Issues a short-lived, runner-scoped grant for the leased account -> { grant.grantRef }.
//   3. AGENT  token -> POST /api/provider-accounts/chatgpt-codex/grants/resolve
//      with { grantRef, providerAccountRef, includeAuthMaterial:true } -> returns
//      { authMaterial: { authContentEnv:'OPENCODE_AUTH_CONTENT', authContentJson } }.
//      authContentJson is the OpenCode/openauth auth.json: { openai:{ type:'oauth',
//      access, refresh, expires, accountId?, idToken? } }.
//   4. Translate that into the codex-CLI-native ~/.codex/auth.json shape
//      ({ auth_mode:'chatgpt', tokens:{ id_token, access_token, refresh_token,
//      account_id }, last_refresh }) and write it to CODEX_HOME/auth.json.
//
// The caller is responsible for RELEASING the lease (worker.sh does, via
// `release` subcommand) so accounts don't stay pinned.
//
// SECURITY: this script NEVER prints the token blob, access/refresh/id tokens,
// account ids, the admin token, or the agent token. It prints only public refs,
// lengths, booleans, statuses, and the CODEX_HOME path. stdout (for `lease`)
// emits ONLY public refs (leaseRef, providerAccountRef) as JSON.
//
// Subcommands:
//   lease   --action <a> [--assignmentId <id>] [--runId <id>] [--email <e>]
//             -> leases an account, issues a grant, resolves material, writes
//                CODEX_HOME/auth.json. Prints { leaseRef, providerAccountRef } JSON.
//   release --leaseRef <ref> [--status released|succeeded|failed] [--email <e>]
//             -> releases the lease.
//   sanity-all [--email <e>]
//             -> prints connected-account health summary (counts + classes; no secrets).
//
// Env (names only):
//   OPENAGENTS_ADMIN_API_TOKEN  required  (admin/operator bearer; central lease + grant)
//   OPENAGENTS_AGENT_TOKEN      required  (programmatic-agent bearer; grant resolve)
//   OPENAGENTS_BASE_URL         default https://openagents.com
//   OPENAGENTS_FLEET_EMAIL      default chris@openagents.com (target user for the account fleet)
//   CODEX_HOME                  required for `lease` (where auth.json is written)

import fs from 'node:fs'
import path from 'node:path'

const args = process.argv.slice(2)
const command = args[0]

const valueAfter = (flag) => {
  const i = args.indexOf(flag)
  return i >= 0 && i + 1 < args.length ? args[i + 1] : undefined
}

const adminToken = process.env.OPENAGENTS_ADMIN_API_TOKEN
const agentToken = process.env.OPENAGENTS_AGENT_TOKEN
const baseUrl = process.env.OPENAGENTS_BASE_URL ?? 'https://openagents.com'
const fleetEmail =
  valueAfter('--email') ??
  process.env.OPENAGENTS_FLEET_EMAIL ??
  'chris@openagents.com'

const die = (msg) => {
  console.error(`fetch-codex-auth: ${msg}`)
  process.exit(1)
}

if (command === undefined) {
  die('usage: fetch-codex-auth.mjs <lease|release|sanity-all> [flags]')
}

const post = async (pathname, token, body) => {
  const response = await fetch(new URL(pathname, baseUrl), {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${token}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(body),
  })
  const text = await response.text()
  let payload = {}
  try {
    payload = text === '' ? {} : JSON.parse(text)
  } catch {
    payload = { error: 'bad_json', status: response.status }
  }
  return { ok: response.ok, status: response.status, payload }
}

const optionalString = (v) =>
  typeof v === 'string' && v.trim() !== '' ? v : undefined

// Translate the central OpenCode/openauth auth blob into the codex-CLI-native
// ~/.codex/auth.json shape. Mirrors the server's codexOAuthAuthFromAuthMaterial
// extraction in operator-provider-account-routes.ts (openai.{access,refresh,
// expires,accountId,idToken}) and the codex CLI's tokens.{id_token,access_token,
// refresh_token,account_id} layout.
const codexAuthJsonFromAuthMaterial = (authMaterial) => {
  const authContentJson = optionalString(authMaterial?.authContentJson)
  if (authContentJson === undefined) {
    return undefined
  }
  let parsed
  try {
    parsed = JSON.parse(authContentJson)
  } catch {
    return undefined
  }
  const openai =
    parsed && typeof parsed.openai === 'object' && parsed.openai !== null
      ? parsed.openai
      : undefined
  if (openai === undefined) {
    return undefined
  }
  const access = optionalString(openai.access)
  const refresh = optionalString(openai.refresh)
  if (openai.type !== 'oauth' || access === undefined || refresh === undefined) {
    return undefined
  }
  return {
    OPENAI_API_KEY: null,
    auth_mode: 'chatgpt',
    tokens: {
      id_token: optionalString(openai.idToken) ?? '',
      access_token: access,
      refresh_token: refresh,
      account_id: optionalString(openai.accountId) ?? '',
    },
    last_refresh: new Date().toISOString(),
  }
}

if (command === 'lease') {
  if (adminToken === undefined || adminToken.trim() === '') {
    die('Missing OPENAGENTS_ADMIN_API_TOKEN.')
  }
  if (agentToken === undefined || agentToken.trim() === '') {
    die('Missing OPENAGENTS_AGENT_TOKEN.')
  }
  const codexHome = process.env.CODEX_HOME
  if (codexHome === undefined || codexHome.trim() === '') {
    die('Missing CODEX_HOME (where auth.json is written).')
  }
  const requestedAction = valueAfter('--action') ?? 'codex_fleet_promise_work'
  const assignmentId = valueAfter('--assignmentId')
  const runId = valueAfter('--runId') ?? assignmentId ?? 'codex-fleet'

  // 1. Lease a connected+healthy Codex account (central selector, admin token).
  const lease = await post(
    '/api/operator/provider-accounts/chatgpt-codex/leases',
    adminToken,
    {
      requestedAction,
      email: fleetEmail,
      ...(assignmentId === undefined ? {} : { assignmentId }),
      runId,
    },
  )
  if (!lease.ok) {
    die(
      `lease failed (HTTP ${lease.status}): ${lease.payload.error ?? 'unknown'} ${
        lease.payload.reason ?? lease.payload.message ?? ''
      }`,
    )
  }
  const leaseRef = optionalString(lease.payload.leaseRef)
  const providerAccountRef = optionalString(lease.payload.providerAccountRef)
  if (leaseRef === undefined || providerAccountRef === undefined) {
    die('lease response missing leaseRef/providerAccountRef')
  }

  // 2. Issue a runner-scoped grant for the leased account (admin token).
  const grantResp = await post(
    '/api/operator/provider-accounts/chatgpt-codex/leases/grant',
    adminToken,
    { leaseRef, email: fleetEmail, runId },
  )
  if (!grantResp.ok) {
    die(
      `grant issue failed (HTTP ${grantResp.status}): ${
        grantResp.payload.error ?? 'unknown'
      }`,
    )
  }
  const grantRef = optionalString(grantResp.payload?.grant?.grantRef)
  if (grantRef === undefined) {
    die('grant issue response missing grant.grantRef')
  }

  // 3. Resolve the grant WITH auth material (programmatic-agent token).
  const resolved = await post(
    '/api/provider-accounts/chatgpt-codex/grants/resolve',
    agentToken,
    { grantRef, providerAccountRef, includeAuthMaterial: true, runId },
  )
  if (!resolved.ok) {
    const err = resolved.payload.error ?? 'unknown'
    // Surface the most common owner-gated case clearly without leaking material.
    const hint =
      err === 'provider_account_auth_material_unavailable'
        ? ' (account needs owner reconnect: Settings -> Connections -> reconnect ChatGPT/Codex)'
        : ''
    die(`grant resolve failed (HTTP ${resolved.status}): ${err}${hint}`)
  }
  const authMaterial = resolved.payload.authMaterial
  const codexAuth = codexAuthJsonFromAuthMaterial(authMaterial)
  if (codexAuth === undefined) {
    die('resolved auth material is not a usable Codex OAuth blob')
  }

  // 4. Write the codex-native auth.json into the isolated CODEX_HOME (0600).
  fs.mkdirSync(codexHome, { recursive: true })
  const authPath = path.join(codexHome, 'auth.json')
  fs.writeFileSync(authPath, JSON.stringify(codexAuth), { mode: 0o600 })
  try {
    fs.chmodSync(authPath, 0o600)
  } catch {
    /* best-effort */
  }

  // Public-only stderr breadcrumbs; never the material.
  console.error(
    `fetch-codex-auth: leased+resolved Codex auth -> ${authPath} (id_token:${
      codexAuth.tokens.id_token !== '' ? 'present' : 'absent'
    } account_id:${codexAuth.tokens.account_id !== '' ? 'present' : 'absent'})`,
  )
  // stdout: ONLY public refs for the caller to release later.
  process.stdout.write(JSON.stringify({ leaseRef, providerAccountRef }) + '\n')
  process.exit(0)
}

if (command === 'release') {
  if (adminToken === undefined || adminToken.trim() === '') {
    die('Missing OPENAGENTS_ADMIN_API_TOKEN.')
  }
  const leaseRef = valueAfter('--leaseRef')
  if (leaseRef === undefined || leaseRef.trim() === '') {
    die('release requires --leaseRef')
  }
  const status = valueAfter('--status') ?? 'released'
  const resp = await post(
    '/api/operator/provider-accounts/chatgpt-codex/leases/release',
    adminToken,
    { leaseRef, status },
  )
  console.error(
    `fetch-codex-auth: release lease -> HTTP ${resp.status} status=${
      resp.payload.status ?? status
    }`,
  )
  // Releasing a lease is best-effort cleanup; never fail the worker on it.
  process.exit(0)
}

if (command === 'sanity-all') {
  if (adminToken === undefined || adminToken.trim() === '') {
    die('Missing OPENAGENTS_ADMIN_API_TOKEN.')
  }
  const resp = await post(
    '/api/operator/provider-accounts/chatgpt-codex/sanity',
    adminToken,
    { email: fleetEmail, all: true },
  )
  if (!resp.ok) {
    die(`sanity failed (HTTP ${resp.status}): ${resp.payload.error ?? 'unknown'}`)
  }
  const checks = resp.payload.checks ?? resp.payload.results ?? []
  let healthy = 0
  for (const c of checks) {
    const cls = c.classification ?? c.health ?? 'unknown'
    if (cls === 'healthy') {
      healthy += 1
    }
    console.error(
      `- ${cls} "${c.accountLabel ?? 'Unlabeled'}" ref=...${String(
        c.providerAccountRef ?? '',
      ).slice(-8)}`,
    )
  }
  console.error(`fetch-codex-auth: ${healthy}/${checks.length} healthy`)
  process.exit(healthy > 0 ? 0 : 1)
}

die(`unknown command: ${command}`)
