import { Effect, Redacted, Schema as S } from 'effect'

import {
  type BusinessSignupRuntime,
  insertBusinessSignupRequest,
  systemBusinessSignupRuntime,
} from './business-signup-routes'
import { businessSourceRefForVertical } from './business-source-attribution'
import type { ResendEmailConfig } from './config'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'

type VerticalFunnelSlug = 'legal'

type VerticalFunnelTemplate = Readonly<{
  slug: VerticalFunnelSlug
  title: string
  audience: string
  sourceRoute: string
  applyRoute: string
  confirmedRoute: string
  followUpRoute: string
  applicationApiRoute: string
  bookingUrl: string
  worksheetFilename: string
}>

export type VerticalFunnelEmailAttachment = Readonly<{
  filename: string
  content: string
}>

export type VerticalFunnelEmailInput = Readonly<{
  from: string
  to: string
  subject: string
  html: string
  text: string
  idempotencyKey: string
  replyTo?: string | undefined
  attachments: ReadonlyArray<VerticalFunnelEmailAttachment>
}>

export type VerticalFunnelEmailResult =
  | Readonly<{ ok: true; providerMessageId: string | null }>
  | Readonly<{ ok: false; errorMessage: string }>

export type VerticalFunnelEmailSender = (
  input: VerticalFunnelEmailInput,
) => Promise<VerticalFunnelEmailResult>

type VerticalFunnelRouteInput = Readonly<{
  db: D1Database
  resend?: ResendEmailConfig | undefined
  sender?: VerticalFunnelEmailSender | undefined
  runtime?: BusinessSignupRuntime | undefined
}>

class VerticalFunnelApplicationFailure extends S.TaggedErrorClass<VerticalFunnelApplicationFailure>()(
  'VerticalFunnelApplicationFailure',
  {
    cause: S.Unknown,
  },
) {}

const legalTemplate: VerticalFunnelTemplate = {
  slug: 'legal',
  title: 'Legal workspace intake',
  audience: 'law firms and legal teams',
  sourceRoute: '/business/legal',
  applyRoute: '/business/legal/apply',
  confirmedRoute: '/business/legal/confirmed',
  followUpRoute: '/business/legal/follow-up',
  applicationApiRoute: '/api/business/vertical-funnels/legal/apply',
  bookingUrl: 'https://cal.com/openagents/legal-workspace-intake',
  worksheetFilename: 'openagents-legal-qualification-worksheet.txt',
}

const page = (template: VerticalFunnelTemplate, body: string): string => `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="robots" content="noindex">
<title>${template.title} - OpenAgents Business</title>
<style>
:root{color-scheme:dark;--bg:#000;--panel:#070707;--line:#252525;--ink:#f1efe8;--muted:rgba(241,239,232,.62);--blue:#8bbcff}
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--ink);font-family:ui-monospace,SFMono-Regular,Menlo,monospace}a{color:var(--blue)}
.shell{width:min(1120px,calc(100% - 32px));margin:0 auto;padding:32px 0 56px}.nav{display:flex;align-items:center;justify-content:space-between;border-bottom:1px solid var(--line);padding-bottom:14px}.brand{color:var(--ink);text-decoration:none}.grid{display:grid;grid-template-columns:1.1fr .9fr;gap:24px;align-items:start;margin-top:42px}.panel{border:1px solid var(--line);background:var(--panel);padding:22px}.eyebrow{margin:0 0 10px;color:var(--muted);font-size:12px;text-transform:uppercase;letter-spacing:.12em}h1{max-width:760px;margin:0;font-size:clamp(34px,7vw,76px);line-height:.95;letter-spacing:0}h2{margin:0 0 12px;font-size:20px}p{line-height:1.6}.muted{color:var(--muted)}.actions{display:flex;flex-wrap:wrap;gap:12px;margin-top:24px}.button{display:inline-flex;align-items:center;justify-content:center;border:1px solid var(--line);background:#101010;color:var(--ink);min-height:44px;padding:0 15px;text-decoration:none}.primary{border-color:#315f91;background:#102033}form{display:grid;gap:12px}label{display:grid;gap:6px;color:var(--muted);font-size:12px;text-transform:uppercase;letter-spacing:.08em}input,textarea,select{width:100%;border:1px solid var(--line);background:#000;color:var(--ink);padding:11px;font:inherit;text-transform:none;letter-spacing:0}textarea{min-height:112px;resize:vertical}button{border:1px solid #315f91;background:#102033;color:var(--ink);padding:12px 15px;font:inherit;cursor:pointer}.steps{display:grid;gap:10px;margin-top:18px}.step{border-left:2px solid #315f91;padding-left:12px;color:var(--muted)}pre{white-space:pre-wrap;line-height:1.55;color:var(--muted)}@media(max-width:760px){.grid{grid-template-columns:1fr}.shell{width:min(100% - 24px,1120px);padding-top:20px}h1{font-size:40px}}
</style>
</head>
<body><main class="shell"><nav class="nav"><a class="brand" href="/business-new">OpenAgents Business</a><a href="${template.applyRoute}">Apply</a></nav>${body}</main></body>
</html>`

const renderLanding = (template: VerticalFunnelTemplate): string =>
  page(
    template,
    `<section class="grid"><div><p class="eyebrow">${template.audience}</p><h1>Review-gated agent workspaces for legal teams.</h1><p class="muted">Start with a bounded intake. OpenAgents prepares selected-source workspace setup, qualification questions, and draft-ready workflow notes for human review. This is not legal advice and nothing external is sent without approval.</p><div class="actions"><a class="button primary" href="${template.applyRoute}">Apply for intake</a><a class="button" href="${template.followUpRoute}">See the worksheet</a></div></div><aside class="panel"><h2>What happens next</h2><div class="steps"><div class="step">Submit the application through the Worker-backed form.</div><div class="step">Get the qualification worksheet by email.</div><div class="step">Book a scoped intake slot from the confirmed page.</div></div></aside></section>`,
  )

const renderApply = (template: VerticalFunnelTemplate): string =>
  page(
    template,
    `<section class="grid"><div><p class="eyebrow">Application</p><h1>Tell us what legal work surface you want prepared.</h1><p class="muted">Use vertical descriptors only. Do not paste privileged, confidential, or client-identifying matter facts into this public intake.</p></div><form class="panel" method="post" action="${template.applicationApiRoute}"><label>Organization<input name="businessName" required maxlength="200" autocomplete="organization"></label><label>Contact email<input name="contactEmail" type="email" required maxlength="320" autocomplete="email"></label><label>Phone<input name="phone" required maxlength="80" autocomplete="tel"></label><label>Website<input name="website" type="url" maxlength="500" placeholder="https://"></label><label>Practice area<select name="practiceArea"><option>Business contracts</option><option>Employment</option><option>Real estate</option><option>General counsel</option><option>Other</option></select></label><label>Primary goal<textarea name="primaryGoal" required maxlength="1200" placeholder="Prepare a source-linked intake workspace and review checklist for recurring contract requests"></textarea></label><label>Current systems<textarea name="systems" maxlength="800" placeholder="Document library, CRM, practice management system, or none yet"></textarea></label><button type="submit">Submit application</button></form></section>`,
  )

const renderConfirmed = (
  template: VerticalFunnelTemplate,
  ref: string | null,
): string =>
  page(
    template,
    `<section class="grid"><div><p class="eyebrow">Confirmed</p><h1>Application received.</h1><p class="muted">The intake was recorded and the qualification worksheet follow-up was sent when Resend was configured. Keep the reference for support: ${ref ?? 'pending'}</p><div class="actions"><a class="button primary" href="${template.bookingUrl}">Book the intake slot</a><a class="button" href="${template.followUpRoute}">Open worksheet outline</a></div></div><aside class="panel"><h2>Authority boundary</h2><p class="muted">This receipt queues qualification only. It grants no legal advice, workspace access, spend, payout, or publishing authority.</p></aside></section>`,
  )

const renderFollowUp = (template: VerticalFunnelTemplate): string =>
  page(
    template,
    `<section class="grid"><div><p class="eyebrow">Follow-up template</p><h1>Qualification worksheet.</h1><p class="muted">The POST route attaches this worksheet to the Resend follow-up email so the next step is concrete.</p></div><aside class="panel"><pre>${worksheetText(template)}</pre></aside></section>`,
  )

const htmlResponse = (body: string) =>
  new Response(body, { headers: { 'content-type': 'text/html; charset=utf-8' } })

const escapeField = (value: string): string =>
  value.replace(/[&<>"']/g, character => {
    switch (character) {
      case '&':
        return '&amp;'
      case '<':
        return '&lt;'
      case '>':
        return '&gt;'
      case '"':
        return '&quot;'
      default:
        return '&#39;'
    }
  })

const textField = (fields: FormData, name: string, maxLength: number): string => {
  const value = fields.get(name)
  return typeof value === 'string'
    ? value.replace(/\s+/g, ' ').trim().slice(0, maxLength)
    : ''
}

const optionalUrl = (value: string): string | null => {
  if (value === '') {
    return null
  }
  try {
    const parsed = new URL(value)
    return parsed.protocol === 'http:' || parsed.protocol === 'https:'
      ? parsed.toString()
      : null
  } catch {
    return null
  }
}

const worksheetText = (template: VerticalFunnelTemplate): string =>
  [
    'OpenAgents legal qualification worksheet',
    '',
    '1. What repeatable legal workflow should the workspace prepare first?',
    '2. Which selected sources may be used for drafting or checklists?',
    '3. Which facts must a human confirm before anything leaves the workspace?',
    '4. What systems of record should stay authoritative?',
    '5. Who approves outbound drafts, sends, or published artifacts?',
    '',
    `Booking: ${template.bookingUrl}`,
    'Boundary: not legal advice; human review remains required.',
  ].join('\n')

const emailHtml = (
  template: VerticalFunnelTemplate,
  recordId: string,
): string =>
  `<p>Your OpenAgents ${escapeField(template.slug)} workspace application was received.</p><p>Reference: <code>${escapeField(recordId)}</code></p><p>Book the intake slot: <a href="${template.bookingUrl}">${template.bookingUrl}</a></p><p>The qualification worksheet is attached.</p><p>This is qualification only; no legal advice or workspace authority is granted by this email.</p>`

const emailText = (
  template: VerticalFunnelTemplate,
  recordId: string,
): string =>
  [
    `Your OpenAgents ${template.slug} workspace application was received.`,
    `Reference: ${recordId}`,
    `Book the intake slot: ${template.bookingUrl}`,
    'The qualification worksheet is attached.',
    'This is qualification only; no legal advice or workspace authority is granted by this email.',
  ].join('\n')

export const makeVerticalFunnelResendSender = (
  config: ResendEmailConfig,
  fetcher: typeof fetch = fetch,
): VerticalFunnelEmailSender => {
  return async input => {
    try {
      const response = await fetcher('https://api.resend.com/emails', {
        body: JSON.stringify({
          attachments: input.attachments,
          from: input.from,
          html: input.html,
          ...(input.replyTo === undefined ? {} : { reply_to: input.replyTo }),
          subject: input.subject,
          text: input.text,
          to: [input.to],
        }),
        headers: {
          Authorization: `Bearer ${Redacted.value(config.apiKey)}`,
          'content-type': 'application/json',
          'Idempotency-Key': input.idempotencyKey,
        },
        method: 'POST',
      })
      const payload = (await response.json().catch(() => ({}))) as Record<
        string,
        unknown
      >
      if (!response.ok) {
        return {
          ok: false,
          errorMessage:
            typeof payload.message === 'string'
              ? payload.message
              : `resend ${response.status}`,
        }
      }
      return {
        ok: true,
        providerMessageId: typeof payload.id === 'string' ? payload.id : null,
      }
    } catch (error) {
      return {
        ok: false,
        errorMessage: error instanceof Error ? error.message : String(error),
      }
    }
  }
}

const sendFollowUp = async (
  template: VerticalFunnelTemplate,
  resend: ResendEmailConfig | undefined,
  sender: VerticalFunnelEmailSender | undefined,
  input: Readonly<{ recordId: string; to: string }>,
): Promise<
  | Readonly<{ kind: 'sent'; providerMessageId: string | null }>
  | Readonly<{ kind: 'not_configured' }>
  | Readonly<{ kind: 'failed'; errorMessage: string }>
> => {
  if (resend === undefined) {
    return { kind: 'not_configured' }
  }

  const resolvedSender = sender ?? makeVerticalFunnelResendSender(resend)
  const result = await resolvedSender({
    attachments: [
      {
        filename: template.worksheetFilename,
        content: btoa(worksheetText(template)),
      },
    ],
    from: resend.fromEmail,
    html: emailHtml(template, input.recordId),
    idempotencyKey: `vertical_funnel:${template.slug}:${input.recordId}:follow_up_v1`,
    replyTo: resend.replyToEmail,
    subject: 'OpenAgents legal workspace qualification worksheet',
    text: emailText(template, input.recordId),
    to: input.to,
  })

  return result.ok
    ? { kind: 'sent', providerMessageId: result.providerMessageId }
    : { kind: 'failed', errorMessage: result.errorMessage }
}

const handleApplyPost = (
  request: Request,
  template: VerticalFunnelTemplate,
  input: VerticalFunnelRouteInput,
) =>
  Effect.tryPromise({
    try: async () => {
      if (request.method !== 'POST') {
        return methodNotAllowed(['POST'])
      }

      const fields = await request.formData()
      const businessName = textField(fields, 'businessName', 200)
      const contactEmail = textField(fields, 'contactEmail', 320).toLowerCase()
      const phone = textField(fields, 'phone', 80)
      const website = optionalUrl(textField(fields, 'website', 500))
      const primaryGoal = textField(fields, 'primaryGoal', 1200)
      const practiceArea = textField(fields, 'practiceArea', 120)
      const systems = textField(fields, 'systems', 800)

      if (
        businessName === '' ||
        !/^[^\s@<>]+@[^\s@<>]+\.[^\s@<>]+$/.test(contactEmail) ||
        phone === '' ||
        primaryGoal === ''
      ) {
        return noStoreJsonResponse(
          { error: 'vertical_funnel_application_validation_error' },
          { status: 400 },
        )
      }

      const runtime = input.runtime ?? systemBusinessSignupRuntime
      const record = await insertBusinessSignupRequest(
        input.db,
        {
          businessName,
          contactEmail,
          helpWith: [
            `vertical=${template.slug}`,
            practiceArea === '' ? undefined : `practice_area=${practiceArea}`,
            `primary_goal=${primaryGoal}`,
            systems === '' ? undefined : `systems=${systems}`,
          ]
            .filter(Boolean)
            .join('\n'),
          phone,
          referralCode: null,
          requestSlackChannel: false,
          sourceRef: businessSourceRefForVertical(template.slug),
          sourceRoute: template.applyRoute,
          website,
        },
        runtime,
      )

      const followUp = await sendFollowUp(template, input.resend, input.sender, {
        recordId: record.id,
        to: contactEmail,
      })

      if (request.headers.get('accept')?.includes('application/json')) {
        return noStoreJsonResponse(
          {
            application: {
              id: record.id,
              sourceRoute: record.sourceRoute,
              bookingUrl: template.bookingUrl,
              followUpEmail: followUp,
            },
          },
          { status: followUp.kind === 'failed' ? 502 : 201 },
        )
      }

      if (followUp.kind === 'failed') {
        return noStoreJsonResponse(
          { error: 'vertical_funnel_follow_up_failed' },
          { status: 502 },
        )
      }

      return Response.redirect(
        new URL(
          `${template.confirmedRoute}?ref=${encodeURIComponent(record.id)}`,
          request.url,
        ).toString(),
        303,
      )
    },
    catch: cause => new VerticalFunnelApplicationFailure({ cause }),
  }).pipe(
    Effect.catch(() =>
      Effect.succeed(
        noStoreJsonResponse(
          { error: 'vertical_funnel_application_error' },
          { status: 500 },
        ),
      ),
    ),
  )

export const handleVerticalFunnelRequest = (
  request: Request,
  input: VerticalFunnelRouteInput,
) => {
  const url = new URL(request.url)
  const template = legalTemplate

  if (url.pathname === template.sourceRoute) {
    return request.method === 'GET'
      ? Effect.succeed(htmlResponse(renderLanding(template)))
      : Effect.succeed(methodNotAllowed(['GET']))
  }

  if (url.pathname === template.applyRoute) {
    return request.method === 'GET'
      ? Effect.succeed(htmlResponse(renderApply(template)))
      : Effect.succeed(methodNotAllowed(['GET']))
  }

  if (url.pathname === template.confirmedRoute) {
    return request.method === 'GET'
      ? Effect.succeed(
          htmlResponse(renderConfirmed(template, url.searchParams.get('ref'))),
        )
      : Effect.succeed(methodNotAllowed(['GET']))
  }

  if (url.pathname === template.followUpRoute) {
    return request.method === 'GET'
      ? Effect.succeed(htmlResponse(renderFollowUp(template)))
      : Effect.succeed(methodNotAllowed(['GET']))
  }

  if (url.pathname === template.applicationApiRoute) {
    return handleApplyPost(request, template, input)
  }

  return undefined
}
