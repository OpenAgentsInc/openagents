#!/usr/bin/env node
/**
 * CRM Resend deliverability smoke (epic #5980, sub-issue #5984).
 *
 * Drives ONE live CRM Resend send to a seed address and verifies it lands in
 * the CRM ledger as `sent` with a provider message id. This is the operator
 * half of the deliverability proof; the HUMAN confirms the email actually
 * arrived (and that bounce/complaint webhooks flow into email_provider_events).
 *
 * PREREQUISITES (owner-gated — this is why the green is not auto-claimable):
 *   - The Worker must have RESEND_API_KEY + a VERIFIED sending domain
 *     (SPF/DKIM/DMARC) and RESEND_FROM_EMAIL set.
 *   - CRM_RESEND_SEND_ENABLED must be truthy on the Worker (arms the sender).
 *   - A seed crm_contact + template must exist (import + template upsert).
 *
 * Env:
 *   CRM_BASE_URL      default https://openagents.com
 *   CRM_ADMIN_TOKEN   required (Bearer OPENAGENTS_ADMIN_API_TOKEN)
 *
 * Usage:
 *   CRM_ADMIN_TOKEN=... node scripts/crm-resend-smoke.mjs \
 *     --contact <crm_contact_id> --template <slug> [--tenant <ref>]
 */
const arg = (name) => {
  const i = process.argv.indexOf(`--${name}`)
  return i === -1 ? undefined : process.argv[i + 1]
}

const baseUrl = (process.env.CRM_BASE_URL ?? 'https://openagents.com').replace(/\/$/, '')
const token = process.env.CRM_ADMIN_TOKEN
const contactId = arg('contact')
const templateSlug = arg('template')
const tenant = arg('tenant')

if (!token || !contactId || !templateSlug) {
  console.error('usage: CRM_ADMIN_TOKEN=... crm-resend-smoke.mjs --contact <id> --template <slug> [--tenant <ref>]')
  process.exit(2)
}

const headers = { authorization: `Bearer ${token}`, 'content-type': 'application/json' }

const main = async () => {
  const sendResp = await fetch(
    `${baseUrl}/api/operator/crm/contacts/${encodeURIComponent(contactId)}/resend-send`,
    {
      body: JSON.stringify({ sendReason: 'crm_resend_smoke', templateSlug, ...(tenant ? { tenant } : {}) }),
      headers,
      method: 'POST',
    },
  )
  const body = await sendResp.json().catch(() => ({}))
  const kind = body?.result?.kind
  console.log(`resend-send -> HTTP ${sendResp.status}, kind=${kind}`)

  if (kind === 'dry_run') {
    console.error('NOT ARMED: CRM_RESEND_SEND_ENABLED is off on the Worker. Arm it + verify the domain, then retry.')
    process.exit(1)
  }
  if (kind === 'not_configured') {
    console.error('NOT CONFIGURED: RESEND_API_KEY / RESEND_FROM_EMAIL missing on the Worker.')
    process.exit(1)
  }
  if (kind === 'suppressed') {
    console.error(`SUPPRESSED: ${body.result.reason}. Pick a non-suppressed seed address.`)
    process.exit(1)
  }
  if (kind === 'failed') {
    console.error(`FAILED at provider: ${body.result.errorMessage}`)
    process.exit(1)
  }
  if (kind !== 'sent') {
    console.error(`UNEXPECTED result: ${JSON.stringify(body)}`)
    process.exit(1)
  }

  const providerMessageId = body.result.message?.providerMessageId
  console.log(`SENT. providerMessageId=${providerMessageId}`)
  console.log('Now CONFIRM the human receipt: check the seed inbox, and that a delivered event arrives in email_provider_events via the Resend webhook.')

  // Read-back the ledger.
  const ledgerResp = await fetch(
    `${baseUrl}/api/operator/crm/contacts/${encodeURIComponent(contactId)}/emails${tenant ? `?tenant=${encodeURIComponent(tenant)}` : ''}`,
    { headers },
  )
  const ledger = await ledgerResp.json().catch(() => ({}))
  const last = ledger?.messages?.[0]
  console.log(`ledger latest: status=${last?.status} provider_message_id=${last?.providerMessageId}`)
}

main().catch((error) => {
  console.error(error)
  process.exit(1)
})
