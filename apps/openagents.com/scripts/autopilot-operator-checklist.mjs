#!/usr/bin/env node

const args = process.argv.slice(2)

const valueAfter = name => {
  const index = args.indexOf(name)

  return index === -1 ? undefined : args[index + 1]
}

const hasFlag = name => args.includes(name)

const token = process.env.OPENAGENTS_ADMIN_API_TOKEN

if (token === undefined || token.trim() === '') {
  console.error('Missing OPENAGENTS_ADMIN_API_TOKEN.')
  process.exit(2)
}

const baseUrl = process.env.OPENAGENTS_BASE_URL ?? 'https://openagents.com'
const endpoint = new URL('/api/omni/operator/autopilot/checklist', baseUrl)
const fields = [
  'userId',
  'email',
  'githubLogin',
  'login',
  'teamId',
  'projectId',
  'runId',
  'providerAccountRef',
]

for (const field of fields) {
  const value = valueAfter(`--${field}`)

  if (value !== undefined) {
    endpoint.searchParams.set(field, value)
  }
}

const response = await fetch(endpoint, {
  headers: {
    Authorization: `Bearer ${token}`,
    accept: 'application/json',
  },
  method: 'GET',
})
const payload = await response.json().catch(() => ({
  error: 'invalid_json_response',
  status: response.status,
}))

if (hasFlag('--json')) {
  console.log(JSON.stringify(payload, null, 2))
  process.exit(response.ok && payload.status !== 'blocked' ? 0 : 1)
}

console.log(`Autopilot operator checklist: ${payload.status ?? response.status}`)

if (Array.isArray(payload.checks)) {
  for (const check of payload.checks) {
    console.log(`- ${check.status} ${check.name}: ${check.message}`)
  }
}

if (payload.run !== null && payload.run !== undefined) {
  console.log(
    `Current run: ${payload.run.id} (${payload.run.status}, cursor ${payload.run.eventCursor})`,
  )
}

if (payload.nextSafeAction !== undefined) {
  console.log(`Next safe action: ${payload.nextSafeAction}`)
}

process.exit(response.ok && payload.status !== 'blocked' ? 0 : 1)
