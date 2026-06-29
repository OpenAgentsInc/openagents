#!/usr/bin/env node

import { spawnSync } from 'node:child_process'
import { readFileSync } from 'node:fs'

const usage = `
Usage:
  node scripts/operator-email-smoke.mjs [--idempotencyKey key]

Checks:
  - Wrangler production secret names include RESEND_API_KEY.
  - Sender/reply-to are configured as production secrets or Worker vars.
  - Remote D1 can inspect email_messages / email_deliveries for the idempotency key.

This script never prints secret values or provider request bodies.
`

const args = process.argv.slice(2)
const valueAfter = flag => {
  const index = args.indexOf(flag)

  return index === -1 ? undefined : args[index + 1]
}

if (args.includes('--help') || args.includes('-h')) {
  console.log(usage.trim())
  process.exit(0)
}

const idempotencyKey =
  valueAfter('--idempotencyKey') ??
  `operator-email-smoke:${new Date().toISOString().slice(0, 10)}`

const run = (command, commandArgs) => {
  const result = spawnSync(command, commandArgs, {
    cwd: new URL('../workers/api', import.meta.url),
    encoding: 'utf8',
  })

  if (result.status !== 0) {
    throw new Error(
      `${command} ${commandArgs.join(' ')} failed: ${result.stderr.trim()}`,
    )
  }

  return result.stdout
}

const secretsJson = run('bunx', ['wrangler', 'secret', 'list', '--format=json'])
const secrets = JSON.parse(secretsJson)
const secretNames = new Set(
  Array.isArray(secrets)
    ? secrets
        .map(secret => secret?.name)
        .filter(name => typeof name === 'string')
    : [],
)
const wranglerConfig = readFileSync(
  new URL('../workers/api/wrangler.jsonc', import.meta.url),
  'utf8',
)
const workerVarNames = new Set(
  [...wranglerConfig.matchAll(/"([A-Z0-9_]+)"\s*:/g)].map(match => match[1]),
)
const configuredName = name => secretNames.has(name) || workerVarNames.has(name)
const configuredLocation = name =>
  secretNames.has(name)
    ? 'secret'
    : workerVarNames.has(name)
      ? 'worker-var'
      : 'missing'
const hasApiKey = secretNames.has('RESEND_API_KEY')
const hasFromEmail = configuredName('RESEND_FROM_EMAIL')
const configStatus = hasApiKey && hasFromEmail ? 'present' : 'missing'
const safeSqlKey = idempotencyKey.replaceAll("'", "''")
const ledgerJson = run('bunx', [
  'wrangler',
  'd1',
  'execute',
  'openagents-autopilot',
  '--remote',
  '--json',
  '--command',
  `SELECT email_messages.id AS email_message_id,
          email_messages.status AS message_status,
          email_messages.provider AS provider,
          email_messages.provider_message_id AS provider_message_id,
          email_messages.error_name AS error_name,
          email_messages.error_message AS error_message,
          email_deliveries.id AS delivery_id,
          email_deliveries.status AS delivery_status
     FROM email_messages
     LEFT JOIN email_deliveries
       ON email_deliveries.message_id = email_messages.id
    WHERE email_messages.idempotency_key = '${safeSqlKey}'
    ORDER BY email_deliveries.attempted_at DESC
    LIMIT 5;`,
])
const ledger = JSON.parse(ledgerJson)
const rows = ledger?.[0]?.results ?? []

console.log(`Email config: ${configStatus}`)
console.log(`RESEND_API_KEY: ${hasApiKey ? 'present' : 'missing'} (secret)`)
console.log(`RESEND_FROM_EMAIL: ${configuredLocation('RESEND_FROM_EMAIL')}`)
console.log(`RESEND_REPLY_TO_EMAIL: ${configuredLocation('RESEND_REPLY_TO_EMAIL')}`)
console.log(`Idempotency: ${idempotencyKey}`)

if (rows.length === 0) {
  console.log('Ledger rows: none')
} else {
  console.log(`Ledger rows: ${rows.length}`)
  for (const row of rows) {
    console.log(
      [
        `message=${row.email_message_id ?? 'none'}`,
        `status=${row.message_status ?? 'none'}`,
        `provider=${row.provider ?? 'none'}`,
        `providerMessage=${row.provider_message_id ?? 'none'}`,
        `delivery=${row.delivery_id ?? 'none'}`,
        `deliveryStatus=${row.delivery_status ?? 'none'}`,
        `error=${row.error_name ?? 'none'}`,
      ].join(' '),
    )
  }
}
