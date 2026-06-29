#!/usr/bin/env node

import { spawnSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import { pathToFileURL } from 'node:url'

const DEFAULT_D1_DATABASE = 'openagents-autopilot'
const WORKER_CWD = new URL('../workers/api/', import.meta.url)

export const usage = () => `Usage:
  node scripts/private-workspace-setup-check.mjs [options]

Options:
  --team-id <id>                  Private team id to verify.
  --project-id <id>               Optional private project id to verify.
  --invite-id <id>                Optional invite id returned by the create route.
  --email-message-id <id>         Optional email ledger message id.
  --session-ready <yes|no|unknown>
                                  Browser session readiness for the teammate walkthrough.
  --live-d1                       Run read-only D1 checks through Wrangler.
  --live-config                   Check Wrangler secrets and Worker vars for Resend config.
  --remote                        Use remote D1 with --live-d1. Local is the default.
  --d1-database <name>            D1 binding/database name. Defaults to openagents-autopilot.
  --json                          Print JSON. Human-safe text is the default.
  --help                          Show this help.

Environment:
  OPENAGENTS_ADMIN_API_TOKEN      Required for the live invite-create call in the runbook.

This script is read-only. It never prints raw invite tokens, email addresses,
provider request/response bodies, or caller-supplied team/project/invite ids.`

const valueFlags = new Set([
  'd1-database',
  'd1Database',
  'email-message-id',
  'emailMessageId',
  'invite-id',
  'inviteId',
  'project-id',
  'projectId',
  'session-ready',
  'sessionReady',
  'team-id',
  'teamId',
])

const booleanFlags = new Set([
  'help',
  'h',
  'json',
  'live-config',
  'liveConfig',
  'live-d1',
  'liveD1',
  'remote',
])

const canonicalFlagName = (name) =>
  ({
    d1Database: 'd1-database',
    emailMessageId: 'email-message-id',
    h: 'help',
    inviteId: 'invite-id',
    liveConfig: 'live-config',
    liveD1: 'live-d1',
    projectId: 'project-id',
    sessionReady: 'session-ready',
    teamId: 'team-id',
  })[name] || name

export const parseSetupArgs = (argv) => {
  const flags = new Map()

  for (let index = 0; index < argv.length; index += 1) {
    const raw = argv[index]

    if (!raw.startsWith('--')) {
      throw new Error(`Unexpected positional argument: ${raw}`)
    }

    const name = canonicalFlagName(raw.slice(2))

    if (booleanFlags.has(name)) {
      flags.set(name, true)
      continue
    }

    if (!valueFlags.has(name)) {
      throw new Error(`Unknown option: ${raw}`)
    }

    const value = argv[index + 1]

    if (value === undefined || value.startsWith('--')) {
      throw new Error(`Missing value for ${raw}`)
    }

    flags.set(name, value)
    index += 1
  }

  return { flags }
}

const flagText = (parsed, name) => {
  const value = parsed.flags.get(name)

  return typeof value === 'string' && value.trim() !== ''
    ? value.trim()
    : undefined
}

const hasFlag = (parsed, name) => parsed.flags.get(name) === true

const escapeRegExp = (value) => value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')

export const redactTranscriptUnsafeText = (text, extraValues = []) => {
  const base = String(text)
    .replace(/Bearer\s+[A-Za-z0-9._-]+/g, 'Bearer <redacted>')
    .replace(
      /OPENAGENTS_ADMIN_API_TOKEN=[^\s]+/g,
      'OPENAGENTS_ADMIN_API_TOKEN=<redacted>',
    )
    .replace(/oa_team_invite_[A-Za-z0-9_-]+/g, 'oa_team_invite_<redacted>')
    .replace(/token=[A-Za-z0-9._~:/?#[\]@!$&'()*+,;=%-]+/g, 'token=<redacted>')
    .replace(/[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}/gi, '<email:redacted>')

  return extraValues
    .filter((value) => typeof value === 'string' && value.trim().length > 2)
    .reduce(
      (redacted, value) =>
        redacted.replace(
          new RegExp(escapeRegExp(value.trim()), 'g'),
          '<redacted:value>',
        ),
      base,
    )
}

const defaultRunCommand = (command, commandArgs, options = {}) => {
  const result = spawnSync(command, commandArgs, {
    cwd: options.cwd ?? WORKER_CWD,
    encoding: 'utf8',
  })

  if (result.status !== 0) {
    const message = `${command} ${commandArgs.join(' ')} failed: ${result.stderr.trim()}`
    throw new Error(
      redactTranscriptUnsafeText(message, options.redactValues ?? []),
    )
  }

  return result.stdout
}

const sqlLiteral = (value) => `'${String(value).replaceAll("'", "''")}'`

const parseWranglerRows = (stdout) => {
  const parsed = JSON.parse(stdout)

  if (!Array.isArray(parsed)) {
    return []
  }

  return Array.isArray(parsed[0]?.results) ? parsed[0].results : []
}

const numberValue = (value) => {
  const number = Number(value)

  return Number.isFinite(number) ? number : 0
}

const firstCount = (rows) => numberValue(rows[0]?.count)

const rowPresence = (rows) => (rows.length > 0 ? 'found' : 'missing')

const configuredLocation = (secretNames, workerVarNames, name) =>
  secretNames.has(name)
    ? 'secret'
    : workerVarNames.has(name)
      ? 'worker-var'
      : 'missing'

const collectEmailConfig = (runCommand, readTextFile, redactValues) => {
  const secretsJson = runCommand(
    'bunx',
    ['wrangler', 'secret', 'list', '--format=json'],
    { cwd: WORKER_CWD, redactValues },
  )
  const secrets = JSON.parse(secretsJson)
  const secretNames = new Set(
    Array.isArray(secrets)
      ? secrets
          .map((secret) => secret?.name)
          .filter((name) => typeof name === 'string')
      : [],
  )
  const wranglerConfig = readTextFile(
    new URL('../workers/api/wrangler.jsonc', import.meta.url),
    'utf8',
  )
  const workerVarNames = new Set(
    [...wranglerConfig.matchAll(/"([A-Z0-9_]+)"\s*:/g)].map(
      (match) => match[1],
    ),
  )
  const apiKey = secretNames.has('RESEND_API_KEY') ? 'present' : 'missing'
  const fromEmail = configuredLocation(
    secretNames,
    workerVarNames,
    'RESEND_FROM_EMAIL',
  )
  const replyToEmail = configuredLocation(
    secretNames,
    workerVarNames,
    'RESEND_REPLY_TO_EMAIL',
  )

  return {
    apiKey,
    fromEmail,
    replyToEmail,
    status:
      apiKey === 'present' && fromEmail !== 'missing' ? 'present' : 'missing',
  }
}

const d1Rows = (parsed, runCommand, sql, redactValues) => {
  const database = flagText(parsed, 'd1-database') ?? DEFAULT_D1_DATABASE
  const mode = hasFlag(parsed, 'remote') ? '--remote' : '--local'
  const stdout = runCommand(
    'bunx',
    ['wrangler', 'd1', 'execute', database, mode, '--json', '--command', sql],
    { cwd: WORKER_CWD, redactValues },
  )

  return parseWranglerRows(stdout)
}

const sessionStateFromFlag = (value) => {
  if (value === undefined || value === 'unknown') {
    return 'unknown'
  }

  if (value === 'yes') {
    return 'ready'
  }

  if (value === 'no') {
    return 'missing'
  }

  throw new Error('--session-ready must be yes, no, or unknown.')
}

export const collectWorkspaceSetupObservation = async (
  parsed,
  options = {},
) => {
  const env = options.env ?? process.env
  const runCommand = options.runCommand ?? defaultRunCommand
  const readTextFile = options.readTextFile ?? readFileSync
  const teamId = flagText(parsed, 'team-id')
  const projectId = flagText(parsed, 'project-id')
  const inviteId = flagText(parsed, 'invite-id')
  const providedEmailMessageId = flagText(parsed, 'email-message-id')
  const redactValues = [teamId, projectId, inviteId, providedEmailMessageId]
  const liveD1 = hasFlag(parsed, 'live-d1')
  const liveConfig = hasFlag(parsed, 'live-config')

  const observation = {
    adminTokenPresent:
      typeof env.OPENAGENTS_ADMIN_API_TOKEN === 'string' &&
      env.OPENAGENTS_ADMIN_API_TOKEN.trim() !== '',
    emailConfig: liveConfig
      ? collectEmailConfig(runCommand, readTextFile, redactValues)
      : {
          apiKey: 'unknown',
          fromEmail: 'unknown',
          replyToEmail: 'unknown',
          status: 'unknown',
        },
    emailLedger: { checked: false, rows: [] },
    invite: { checked: false, rows: [] },
    project: { checked: false, rows: [] },
    requested: {
      hasEmailMessageId: providedEmailMessageId !== undefined,
      hasInviteId: inviteId !== undefined,
      hasProjectId: projectId !== undefined,
      hasTeamId: teamId !== undefined,
      liveConfig,
      liveD1,
    },
    sessionState: sessionStateFromFlag(flagText(parsed, 'session-ready')),
    team: { checked: false, rows: [] },
  }

  if (liveD1 && teamId !== undefined) {
    observation.team = {
      checked: true,
      rows: d1Rows(
        parsed,
        runCommand,
        `SELECT COUNT(*) AS count
           FROM teams
          WHERE id = ${sqlLiteral(teamId)}
            AND status = 'active'
            AND archived_at IS NULL;`,
        redactValues,
      ),
    }
  }

  if (liveD1 && projectId !== undefined && teamId !== undefined) {
    observation.project = {
      checked: true,
      rows: d1Rows(
        parsed,
        runCommand,
        `SELECT COUNT(*) AS count
           FROM team_projects
          WHERE id = ${sqlLiteral(projectId)}
            AND team_id = ${sqlLiteral(teamId)}
            AND status = 'active'
            AND archived_at IS NULL;`,
        redactValues,
      ),
    }
  }

  let emailMessageId = providedEmailMessageId

  if (liveD1 && inviteId !== undefined) {
    const rows = d1Rows(
      parsed,
      runCommand,
      `SELECT status,
              send_count,
              CASE WHEN email_message_id IS NULL THEN 0 ELSE 1 END AS has_email_message_id,
              email_message_id
         FROM team_workspace_invites
        WHERE id = ${sqlLiteral(inviteId)}
        LIMIT 1;`,
      redactValues,
    )
    observation.invite = { checked: true, rows }

    if (
      emailMessageId === undefined &&
      typeof rows[0]?.email_message_id === 'string'
    ) {
      emailMessageId = rows[0].email_message_id
      redactValues.push(emailMessageId)
    }
  }

  if (liveD1 && emailMessageId !== undefined) {
    observation.emailLedger = {
      checked: true,
      rows: d1Rows(
        parsed,
        runCommand,
        `SELECT email_messages.status AS message_status,
                email_messages.provider AS provider,
                CASE WHEN email_messages.provider_message_id IS NULL THEN 0 ELSE 1 END AS has_provider_message_id,
                email_messages.error_name AS error_name,
                COUNT(email_deliveries.id) AS delivery_count,
                SUM(CASE WHEN email_deliveries.status = 'accepted' THEN 1 ELSE 0 END) AS accepted_delivery_count,
                SUM(CASE WHEN email_deliveries.status = 'failed' THEN 1 ELSE 0 END) AS failed_delivery_count
           FROM email_messages
           LEFT JOIN email_deliveries
             ON email_deliveries.message_id = email_messages.id
          WHERE email_messages.id = ${sqlLiteral(emailMessageId)}
          GROUP BY email_messages.id
          LIMIT 1;`,
        redactValues,
      ),
    }
  }

  return observation
}

const check = (id, status, summary) => ({ id, status, summary })

export const summarizeWorkspaceSetupPreflight = (observation) => {
  const checks = []

  checks.push(
    check(
      'operator_admin_token',
      observation.adminTokenPresent ? 'ready' : 'blocked',
      observation.adminTokenPresent
        ? 'Admin API token is present in the operator environment.'
        : 'Set OPENAGENTS_ADMIN_API_TOKEN in a private shell before creating the invite.',
    ),
  )

  checks.push(
    check(
      'browser_session',
      observation.sessionState === 'ready'
        ? 'ready'
        : observation.sessionState === 'missing'
          ? 'blocked'
          : 'manual',
      observation.sessionState === 'ready'
        ? 'Teammate browser session was confirmed ready.'
        : observation.sessionState === 'missing'
          ? 'Teammate must sign in with the invited address before accepting.'
          : 'Confirm the teammate can sign in with the invited address before the call.',
    ),
  )

  checks.push(
    check(
      'email_config',
      observation.emailConfig.status === 'present'
        ? 'ready'
        : observation.emailConfig.status === 'missing'
          ? 'blocked'
          : 'manual',
      observation.emailConfig.status === 'present'
        ? `Resend config is present; sender=${observation.emailConfig.fromEmail}, replyTo=${observation.emailConfig.replyToEmail}.`
        : observation.emailConfig.status === 'missing'
          ? 'Resend config is missing RESEND_API_KEY or RESEND_FROM_EMAIL.'
          : 'Run with --live-config to verify Resend secret/Worker-var readiness.',
    ),
  )

  checks.push(
    check(
      'team_exists',
      !observation.requested.hasTeamId
        ? 'blocked'
        : !observation.requested.liveD1
          ? 'manual'
          : firstCount(observation.team.rows) > 0
            ? 'ready'
            : 'blocked',
      !observation.requested.hasTeamId
        ? 'Provide --team-id for the private workspace.'
        : !observation.requested.liveD1
          ? 'Run with --live-d1 to verify the active team row.'
          : firstCount(observation.team.rows) > 0
            ? 'Active team row exists.'
            : 'Active team row was not found.',
    ),
  )

  checks.push(
    check(
      'project_exists',
      !observation.requested.hasProjectId
        ? 'skipped'
        : !observation.requested.liveD1
          ? 'manual'
          : firstCount(observation.project.rows) > 0
            ? 'ready'
            : 'blocked',
      !observation.requested.hasProjectId
        ? 'No project id supplied; team-level invite only.'
        : !observation.requested.liveD1
          ? 'Run with --live-d1 to verify the active project row.'
          : firstCount(observation.project.rows) > 0
            ? 'Active project row exists for the team.'
            : 'Active project row was not found for the team.',
    ),
  )

  const inviteRow = observation.invite.rows[0]
  const inviteStatus = inviteRow?.status
  checks.push(
    check(
      'invite_status',
      !observation.requested.hasInviteId
        ? 'manual'
        : !observation.requested.liveD1
          ? 'manual'
          : rowPresence(observation.invite.rows) === 'missing'
            ? 'blocked'
            : inviteStatus === 'pending' || inviteStatus === 'accepted'
              ? 'ready'
              : 'blocked',
      !observation.requested.hasInviteId
        ? 'After creating the invite, rerun with --invite-id.'
        : !observation.requested.liveD1
          ? 'Run with --live-d1 to inspect invite status.'
          : rowPresence(observation.invite.rows) === 'missing'
            ? 'Invite row was not found.'
            : `Invite is ${inviteStatus}; sendCount=${numberValue(inviteRow.send_count)}, emailLinked=${numberValue(inviteRow.has_email_message_id) > 0 ? 'yes' : 'no'}.`,
    ),
  )

  const emailRow = observation.emailLedger.rows[0]
  const failedDeliveryCount = numberValue(emailRow?.failed_delivery_count)
  const acceptedDeliveryCount = numberValue(emailRow?.accepted_delivery_count)
  checks.push(
    check(
      'email_ledger',
      !observation.requested.hasEmailMessageId &&
        !(numberValue(inviteRow?.has_email_message_id) > 0)
        ? 'manual'
        : !observation.requested.liveD1
          ? 'manual'
          : rowPresence(observation.emailLedger.rows) === 'missing'
            ? 'blocked'
            : failedDeliveryCount > 0 && acceptedDeliveryCount === 0
              ? 'blocked'
              : 'ready',
      !observation.requested.hasEmailMessageId &&
        !(numberValue(inviteRow?.has_email_message_id) > 0)
        ? 'Email ledger can be checked after the invite has an email message id.'
        : !observation.requested.liveD1
          ? 'Run with --live-d1 to inspect email message and delivery rows.'
          : rowPresence(observation.emailLedger.rows) === 'missing'
            ? 'Email message row was not found.'
            : `Email message=${emailRow.message_status ?? 'unknown'}, provider=${emailRow.provider ?? 'unknown'}, deliveries=${numberValue(emailRow.delivery_count)}, acceptedDeliveries=${acceptedDeliveryCount}, failedDeliveries=${failedDeliveryCount}.`,
    ),
  )

  const blocked = checks.filter((item) => item.status === 'blocked')
  const manual = checks.filter((item) => item.status === 'manual')
  const state =
    blocked.length > 0
      ? 'blocked'
      : manual.length > 0
        ? 'needs_operator_action'
        : 'ready'

  return {
    authority: {
      d1MutationAllowed: false,
      emailProviderRequestBodyPrinted: false,
      inviteTokenPrinted: false,
      privateWorkspaceMaterialPrinted: false,
      rawEmailAddressPrinted: false,
    },
    blockers: blocked.map((item) => item.id),
    checks,
    nextActions:
      state === 'ready'
        ? [
            'Open the private workspace with the teammate and confirm access on the call.',
          ]
        : checks
            .filter(
              (item) => item.status === 'blocked' || item.status === 'manual',
            )
            .map((item) => item.summary),
    state,
  }
}

const printHumanSummary = (summary) => {
  console.log(`Private workspace setup preflight: ${summary.state}`)
  for (const item of summary.checks) {
    console.log(`- ${item.id}: ${item.status} — ${item.summary}`)
  }
  console.log(
    'Transcript safety: no invite tokens, raw emails, provider bodies, or private workspace material printed.',
  )
}

const main = async () => {
  const parsed = parseSetupArgs(process.argv.slice(2))

  if (hasFlag(parsed, 'help')) {
    console.log(usage())
    return 0
  }

  const observation = await collectWorkspaceSetupObservation(parsed)
  const summary = summarizeWorkspaceSetupPreflight(observation)

  if (hasFlag(parsed, 'json')) {
    console.log(JSON.stringify(summary, null, 2))
  } else {
    printHumanSummary(summary)
  }

  return summary.state === 'blocked' ? 1 : 0
}

if (
  process.argv[1] !== undefined &&
  import.meta.url === pathToFileURL(process.argv[1]).href
) {
  main()
    .then((code) => {
      process.exitCode = code
    })
    .catch((error) => {
      console.error(
        redactTranscriptUnsafeText(
          error instanceof Error ? error.message : String(error),
        ),
      )
      process.exitCode = 2
    })
}
