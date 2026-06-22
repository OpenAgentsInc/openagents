#!/usr/bin/env node
/**
 * Local Gmail/gws CRM sender (epic #5980, sub-issue #5983).
 *
 * Ported off the old Laravel-coupled `scripts/crm-gmail.sh`: this drives the
 * NEW CRM in apps/openagents.com instead, and ADDS the write-back the old
 * script lacked so every send shows up in the CRM ledger.
 *
 * Flow per contact:
 *   1. GET  /api/operator/crm/contacts/:id/render?template=<slug>   (compose +
 *      eligibility) from the Worker.
 *   2. If not eligible (suppressed / unsubscribed), SKIP — never send.
 *   3. Send as the operator's own mailbox via the local `gws` CLI
 *      (draft-first by default; --send to send live). Gmail OAuth stays local.
 *   4. POST /api/operator/crm/contacts/:id/gmail-writeback to record the
 *      draft/sent message + a crm_activity.
 *
 * Env:
 *   CRM_BASE_URL        e.g. https://openagents.com  (default)
 *   CRM_ADMIN_TOKEN     Bearer OPENAGENTS_ADMIN_API_TOKEN  (required)
 *   GWS_BIN             gws binary (default: gws)
 *   CRM_FROM_EMAIL      optional From override recorded in the ledger
 *
 * Usage:
 *   node crm-gmail-send.mjs --contact <id> --template <slug> [--tenant <ref>] [--send]
 *
 * The exact `gws gmail` flags may need to match your installed gws version;
 * override the invocation via GWS_SEND_ARGS_JSON if needed (see buildGwsArgs).
 */
import { spawnSync } from 'node:child_process'

const arg = (name, fallback = undefined) => {
  const i = process.argv.indexOf(`--${name}`)
  if (i === -1) return fallback
  const next = process.argv[i + 1]
  return next === undefined || next.startsWith('--') ? true : next
}

const baseUrl = (process.env.CRM_BASE_URL ?? 'https://openagents.com').replace(/\/$/, '')
const token = process.env.CRM_ADMIN_TOKEN
const gwsBin = process.env.GWS_BIN ?? 'gws'
const contactId = arg('contact')
const templateSlug = arg('template')
const tenant = arg('tenant')
const live = arg('send', false) === true

if (!token) {
  console.error('CRM_ADMIN_TOKEN is required')
  process.exit(2)
}
if (!contactId || !templateSlug) {
  console.error('usage: crm-gmail-send.mjs --contact <id> --template <slug> [--tenant <ref>] [--send]')
  process.exit(2)
}

const authHeaders = { authorization: `Bearer ${token}` }
const tenantQs = tenant ? `&tenant=${encodeURIComponent(tenant)}` : ''

const buildGwsArgs = (to, subject, htmlPath) => {
  const override = process.env.GWS_SEND_ARGS_JSON
  if (override) {
    return JSON.parse(override).map((part) =>
      part
        .replace('{to}', to)
        .replace('{subject}', subject)
        .replace('{htmlPath}', htmlPath),
    )
  }
  // Default best-effort invocation; draft unless --send.
  const args = ['gmail', '+send', '--to', to, '--subject', subject, '--html-file', htmlPath]
  if (!live) args.push('--draft')
  return args
}

const main = async () => {
  // 1. Compose + eligibility
  const renderUrl = `${baseUrl}/api/operator/crm/contacts/${encodeURIComponent(contactId)}/render?template=${encodeURIComponent(templateSlug)}${tenantQs}`
  const renderResp = await fetch(renderUrl, { headers: authHeaders })
  if (!renderResp.ok) {
    console.error(`render failed: ${renderResp.status} ${await renderResp.text()}`)
    process.exit(1)
  }
  const { message, eligibility } = await renderResp.json()
  if (!eligibility?.allowed) {
    console.log(`SKIP ${message.toEmail}: not eligible (${eligibility?.reason ?? 'unknown'})`)
    return
  }

  // 2/3. Hand the composed HTML to gws (draft-first).
  const { writeFileSync, mkdtempSync } = await import('node:fs')
  const { tmpdir } = await import('node:os')
  const { join } = await import('node:path')
  const dir = mkdtempSync(join(tmpdir(), 'crm-gmail-'))
  const htmlPath = join(dir, 'body.html')
  writeFileSync(htmlPath, message.bodyHtml ?? message.bodyMarkdown)

  const gwsArgs = buildGwsArgs(message.toEmail, message.subject, htmlPath)
  console.log(`${live ? 'SEND' : 'DRAFT'} ${message.toEmail} via ${gwsBin} ${gwsArgs.join(' ')}`)
  const result = spawnSync(gwsBin, gwsArgs, { encoding: 'utf8' })
  if (result.status !== 0) {
    console.error(`gws failed: ${result.stderr ?? result.error}`)
    process.exit(1)
  }
  const stdout = (result.stdout ?? '').trim()
  // Best-effort id extraction (gws prints the created draft/message id).
  const idMatch = stdout.match(/\b([A-Za-z0-9_-]{12,})\b/)
  const providerId = idMatch ? idMatch[1] : null

  // 4. Write back to the CRM ledger.
  const writebackResp = await fetch(
    `${baseUrl}/api/operator/crm/contacts/${encodeURIComponent(contactId)}/gmail-writeback${tenant ? `?tenant=${encodeURIComponent(tenant)}` : ''}`,
    {
      body: JSON.stringify({
        bodyHtml: message.bodyHtml,
        bodyMarkdown: message.bodyMarkdown,
        ...(process.env.CRM_FROM_EMAIL ? { fromEmail: process.env.CRM_FROM_EMAIL } : {}),
        ...(live
          ? { providerMessageId: providerId, status: 'sent' }
          : { providerDraftId: providerId, status: 'draft' }),
        sendReason: 'crm_gmail_outreach',
        subject: message.subject,
        templateId: message.templateId,
        toEmail: message.toEmail,
      }),
      headers: { ...authHeaders, 'content-type': 'application/json' },
      method: 'POST',
    },
  )
  if (!writebackResp.ok) {
    console.error(`write-back failed: ${writebackResp.status} ${await writebackResp.text()}`)
    process.exit(1)
  }
  const { message: recorded } = await writebackResp.json()
  console.log(`OK recorded ${recorded.id} (${recorded.status})`)
}

main().catch((error) => {
  console.error(error)
  process.exit(1)
})
