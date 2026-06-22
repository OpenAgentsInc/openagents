#!/usr/bin/env node
/**
 * Local Gmail/gws CRM executor (epic #5980, sub-issue #5987).
 *
 * Drains the unified-dispatch Gmail queue (#5985): for each `queued` gmail_gws
 * message it sends as the operator's own mailbox via `gws` (draft-first), then
 * writes the outcome BACK to the same ledger row (closing queue->sent without a
 * duplicate). Gmail OAuth stays local — this is the machine-side half of the
 * desktop CRM pane.
 *
 * Flow:
 *   GET  /api/operator/crm/gmail-queue                 -> queued messages
 *   (send each via gws; draft unless --send)
 *   POST /api/operator/crm/contacts/:id/gmail-writeback
 *        { messageId, status, providerDraftId|providerMessageId, ... }
 *
 * Env:  CRM_BASE_URL (default https://openagents.com), CRM_ADMIN_TOKEN (req),
 *       GWS_BIN (default gws), GWS_SEND_ARGS_JSON (optional override).
 * Usage: CRM_ADMIN_TOKEN=... node scripts/crm-gmail-executor.mjs [--tenant <ref>] [--send] [--limit N]
 */
import { spawnSync } from 'node:child_process'
import { mkdtempSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

const arg = (name, fallback = undefined) => {
  const i = process.argv.indexOf(`--${name}`)
  if (i === -1) return fallback
  const next = process.argv[i + 1]
  return next === undefined || next.startsWith('--') ? true : next
}

const baseUrl = (process.env.CRM_BASE_URL ?? 'https://openagents.com').replace(/\/$/, '')
const token = process.env.CRM_ADMIN_TOKEN
const gwsBin = process.env.GWS_BIN ?? 'gws'
const tenant = arg('tenant')
const live = arg('send', false) === true
const limit = Number(arg('limit', '50')) || 50

if (!token) {
  console.error('CRM_ADMIN_TOKEN is required')
  process.exit(2)
}
const headers = { authorization: `Bearer ${token}`, 'content-type': 'application/json' }
const tenantQs = tenant ? `&tenant=${encodeURIComponent(tenant)}` : ''
const tenantOnly = tenant ? `?tenant=${encodeURIComponent(tenant)}` : ''

const buildGwsArgs = (to, subject, htmlPath) => {
  const override = process.env.GWS_SEND_ARGS_JSON
  if (override) {
    return JSON.parse(override).map(part =>
      part.replace('{to}', to).replace('{subject}', subject).replace('{htmlPath}', htmlPath),
    )
  }
  const args = ['gmail', '+send', '--to', to, '--subject', subject, '--html-file', htmlPath]
  if (!live) args.push('--draft')
  return args
}

const sendOne = (message) => {
  const dir = mkdtempSync(join(tmpdir(), 'crm-gmail-exec-'))
  const htmlPath = join(dir, 'body.html')
  writeFileSync(htmlPath, message.bodyHtml ?? message.bodyMarkdown ?? '')
  const result = spawnSync(gwsBin, buildGwsArgs(message.toEmail, message.subject, htmlPath), {
    encoding: 'utf8',
  })
  if (result.status !== 0) {
    return { error: result.stderr ?? String(result.error) }
  }
  const idMatch = (result.stdout ?? '').match(/\b([A-Za-z0-9_-]{12,})\b/)
  return { providerId: idMatch ? idMatch[1] : null }
}

const main = async () => {
  const queueResp = await fetch(`${baseUrl}/api/operator/crm/gmail-queue${tenantOnly}`, { headers })
  if (!queueResp.ok) {
    console.error(`queue fetch failed: ${queueResp.status} ${await queueResp.text()}`)
    process.exit(1)
  }
  const { messages } = await queueResp.json()
  const batch = (messages ?? []).slice(0, limit)
  console.log(`${batch.length} queued gmail message(s); mode=${live ? 'SEND' : 'DRAFT'}`)

  let ok = 0
  let failed = 0
  for (const message of batch) {
    const sent = sendOne(message)
    if (sent.error) {
      failed += 1
      console.error(`FAIL ${message.toEmail}: ${sent.error}`)
      continue
    }
    const writeback = await fetch(
      `${baseUrl}/api/operator/crm/contacts/${encodeURIComponent(message.contactId)}/gmail-writeback${tenantOnly}`,
      {
        body: JSON.stringify({
          bodyMarkdown: message.bodyMarkdown,
          messageId: message.id,
          subject: message.subject,
          toEmail: message.toEmail,
          ...(live
            ? { providerMessageId: sent.providerId, status: 'sent' }
            : { providerDraftId: sent.providerId, status: 'draft' }),
        }),
        headers,
        method: 'POST',
      },
    )
    if (!writeback.ok) {
      failed += 1
      console.error(`WRITEBACK FAIL ${message.id}: ${writeback.status}`)
      continue
    }
    ok += 1
    console.log(`OK ${message.toEmail} (${message.id}) -> ${live ? 'sent' : 'draft'}`)
  }
  console.log(`done: ${ok} ok, ${failed} failed`)
  if (failed > 0 && ok === 0) process.exit(1)
}

main().catch(error => {
  console.error(error)
  process.exit(1)
})
