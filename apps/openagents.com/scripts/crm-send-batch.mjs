#!/usr/bin/env node
/**
 * Dual-channel batch outreach driver — the ~150 "Sprint A" run (epic #5980,
 * sub-issue #5988). Sends one wave per call to `POST /api/operator/crm/send-batch`
 * with a pause between waves (Gmail/Resend daily limits).
 *
 * DRY-RUN BY DEFAULT: you must pass `--send` to actually send. Even with
 * `--send`, the Gmail channel only QUEUES (the local executor + your review
 * gate it); Resend sends only when the Worker is armed + the domain is verified.
 *
 * Contact ids come from --ids (comma list) or --ids-file (one id per line), or
 * are pulled from the contacts API with --from-contacts (optionally --search).
 *
 * Env: CRM_BASE_URL (default https://openagents.com), CRM_ADMIN_TOKEN (req).
 * Usage:
 *   CRM_ADMIN_TOKEN=... node scripts/crm-send-batch.mjs \
 *     --template <slug> --channel gmail_gws|resend \
 *     (--ids id1,id2 | --ids-file ids.txt | --from-contacts [--search term]) \
 *     [--tenant ref] [--wave 25] [--pause-ms 1500] [--send]
 */
import { readFileSync } from 'node:fs'
import { setTimeout as sleep } from 'node:timers/promises'

const arg = (name, fallback = undefined) => {
  const i = process.argv.indexOf(`--${name}`)
  if (i === -1) return fallback
  const next = process.argv[i + 1]
  return next === undefined || next.startsWith('--') ? true : next
}

const baseUrl = (process.env.CRM_BASE_URL ?? 'https://openagents.com').replace(/\/$/, '')
const token = process.env.CRM_ADMIN_TOKEN
const template = arg('template')
const channel = arg('channel', 'gmail_gws')
const tenant = arg('tenant')
const wave = Number(arg('wave', '25')) || 25
const pauseMs = Number(arg('pause-ms', '1500')) || 1500
const live = arg('send', false) === true

if (!token || !template) {
  console.error('usage: CRM_ADMIN_TOKEN=... crm-send-batch.mjs --template <slug> --channel gmail_gws|resend (--ids .. | --ids-file .. | --from-contacts) [--send]')
  process.exit(2)
}
const headers = { authorization: `Bearer ${token}`, 'content-type': 'application/json' }
const tenantQs = tenant ? `?tenant=${encodeURIComponent(tenant)}` : ''

const loadIds = async () => {
  const idsArg = arg('ids')
  if (typeof idsArg === 'string') return idsArg.split(',').map(s => s.trim()).filter(Boolean)
  const file = arg('ids-file')
  if (typeof file === 'string') {
    return readFileSync(file, 'utf8').split('\n').map(s => s.trim()).filter(Boolean)
  }
  if (arg('from-contacts', false) === true) {
    const search = arg('search')
    const url = `${baseUrl}/api/operator/crm/contacts${tenantQs}${tenant ? '&' : '?'}limit=500${typeof search === 'string' ? `&search=${encodeURIComponent(search)}` : ''}`
    const resp = await fetch(url, { headers })
    if (!resp.ok) throw new Error(`contacts fetch failed: ${resp.status}`)
    const { contacts } = await resp.json()
    return (contacts ?? []).map(c => c.id)
  }
  throw new Error('provide --ids, --ids-file, or --from-contacts')
}

const chunk = (arr, size) => {
  const out = []
  for (let i = 0; i < arr.length; i += size) out.push(arr.slice(i, i + size))
  return out
}

const main = async () => {
  const ids = await loadIds()
  console.log(`${ids.length} contact(s); channel=${channel}; mode=${live ? 'SEND' : 'DRY-RUN'}; wave=${wave}`)
  const waves = chunk(ids, wave)
  const totals = {}
  for (let w = 0; w < waves.length; w += 1) {
    const resp = await fetch(`${baseUrl}/api/operator/crm/send-batch`, {
      body: JSON.stringify({
        channel,
        contactIds: waves[w],
        dryRun: !live,
        sendReason: 'crm_sprint_a',
        templateSlug: template,
        ...(tenant ? { tenant } : {}),
      }),
      headers,
      method: 'POST',
    })
    const body = await resp.json().catch(() => ({}))
    if (!resp.ok) {
      console.error(`wave ${w + 1}/${waves.length} failed: ${resp.status} ${JSON.stringify(body)}`)
      process.exit(1)
    }
    const counts = body.summary?.counts ?? {}
    for (const [k, v] of Object.entries(counts)) totals[k] = (totals[k] ?? 0) + v
    console.log(`wave ${w + 1}/${waves.length}: ${JSON.stringify(counts)}`)
    if (w < waves.length - 1) await sleep(pauseMs)
  }
  console.log(`TOTAL: ${JSON.stringify(totals)}`)
  if (!live) console.log('DRY-RUN complete. Re-run with --send to send. (Gmail queues for the local executor; Resend needs the Worker armed + a verified domain.)')
}

main().catch(error => {
  console.error(error)
  process.exit(1)
})
