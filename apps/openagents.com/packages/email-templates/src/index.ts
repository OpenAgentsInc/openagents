import { Schema as S } from 'effect'

export const OrderSitesLifecycleEmailKind = S.Literals([
  'order_received',
  'scoping_started',
  'repository_source_needed',
  'autopilot_queued',
  'autopilot_running',
  'review_ready',
  'site_saved_version_ready',
  'site_deployed',
  'customer_input_needed',
  'unavailable_declined',
  'delivered',
  'adjustment_received',
  'adjustment_completed',
])
export type OrderSitesLifecycleEmailKind =
  typeof OrderSitesLifecycleEmailKind.Type

export const ORDER_SITES_LIFECYCLE_EMAIL_KINDS = [
  'order_received',
  'scoping_started',
  'repository_source_needed',
  'autopilot_queued',
  'autopilot_running',
  'review_ready',
  'site_saved_version_ready',
  'site_deployed',
  'customer_input_needed',
  'unavailable_declined',
  'delivered',
  'adjustment_received',
  'adjustment_completed',
] as const satisfies ReadonlyArray<OrderSitesLifecycleEmailKind>

export const AutopilotDecisionEmailKind = S.Literals([
  'decision_required',
  'work_delivered',
])
export type AutopilotDecisionEmailKind =
  typeof AutopilotDecisionEmailKind.Type

export const AUTOPILOT_DECISION_EMAIL_KINDS = [
  'decision_required',
  'work_delivered',
] as const satisfies ReadonlyArray<AutopilotDecisionEmailKind>

export const DripEmailKind = S.Literals([
  'signup_day_0',
  'signup_day_1',
  'signup_day_2',
])
export type DripEmailKind = typeof DripEmailKind.Type

export const DRIP_EMAIL_KINDS = [
  'signup_day_0',
  'signup_day_1',
  'signup_day_2',
] as const satisfies ReadonlyArray<DripEmailKind>

export class OrderSitesLifecycleTemplateProps extends S.Class<OrderSitesLifecycleTemplateProps>(
  'OrderSitesLifecycleTemplateProps',
)({
  appOrigin: S.String,
  artifactLabel: S.NullOr(S.String),
  artifactUrl: S.NullOr(S.String),
  customerSafeStatus: S.String,
  displayName: S.String,
  lifecycleKind: OrderSitesLifecycleEmailKind,
  nextAction: S.String,
  orderId: S.String,
  safeReason: S.NullOr(S.String),
  revisionUrl: S.NullOr(S.String),
  siteTitle: S.NullOr(S.String),
  siteUrl: S.NullOr(S.String),
  statusPageUrl: S.optionalKey(S.String),
}) {}

export class DripTemplateProps extends S.Class<DripTemplateProps>(
  'DripTemplateProps',
)({
  appOrigin: S.String,
  displayName: S.String,
  kind: DripEmailKind,
  managePreferencesUrl: S.String,
}) {}

export class AutopilotDecisionTemplateProps extends S.Class<AutopilotDecisionTemplateProps>(
  'AutopilotDecisionTemplateProps',
)({
  appOrigin: S.String,
  displayName: S.String,
  kind: AutopilotDecisionEmailKind,
  workOrderRef: S.String,
}) {}

export class PrivateWorkspaceInviteTemplateProps extends S.Class<PrivateWorkspaceInviteTemplateProps>(
  'PrivateWorkspaceInviteTemplateProps',
)({
  acceptUrl: S.String,
  displayName: S.String,
  expiresAt: S.String,
  workspaceLabel: S.String,
}) {}

export type RenderedEmailTemplate = Readonly<{
  html: string
  subject: string
  templateContext: Readonly<Record<string, unknown>>
  templateSlug: string
  text: string
}>

const escapeHtml = (value: string): string =>
  value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;')

const orderSitesSiteLabel = (
  input: OrderSitesLifecycleTemplateProps,
): string => input.siteTitle ?? 'your OpenAgents project'

export const orderSitesLifecycleSubject = (
  input: OrderSitesLifecycleTemplateProps,
): string => {
  const label = orderSitesSiteLabel(input)

  switch (input.lifecycleKind) {
    case 'order_received':
      return 'We received your OpenAgents order'
    case 'scoping_started':
      return 'OpenAgents is scoping your order'
    case 'repository_source_needed':
      return 'OpenAgents needs source access to continue'
    case 'autopilot_queued':
      return 'Autopilot is queued for your order'
    case 'autopilot_running':
      return 'Autopilot is working on your order'
    case 'review_ready':
      return `${label} is ready for review`
    case 'site_saved_version_ready':
      return `${label} has a saved version ready`
    case 'site_deployed':
      return `${label} is live`
    case 'customer_input_needed':
      return 'OpenAgents needs input for your order'
    case 'unavailable_declined':
      return 'OpenAgents cannot continue this order yet'
    case 'delivered':
      return 'Your OpenAgents order was delivered'
    case 'adjustment_received':
      return 'OpenAgents received your adjustment request'
    case 'adjustment_completed':
      return 'Your OpenAgents adjustment is complete'
  }
}

const orderSitesLifecycleLead = (
  input: OrderSitesLifecycleTemplateProps,
): string => {
  switch (input.lifecycleKind) {
    case 'order_received':
      return 'OpenAgents has your request and is preparing it for review.'
    case 'scoping_started':
      return 'OpenAgents is turning your request into a concrete first work slice.'
    case 'repository_source_needed':
      return 'OpenAgents needs the source repository, project files, or access details before work can continue.'
    case 'autopilot_queued':
      return 'Your order is queued for an Autopilot run after the current readiness checks.'
    case 'autopilot_running':
      return 'Autopilot is working on your order under OpenAgents supervision.'
    case 'review_ready':
      return 'A reviewable result is ready for you or the OpenAgents team to inspect.'
    case 'site_saved_version_ready':
      return 'A saved Site version is ready for review before any production deployment.'
    case 'site_deployed':
      return 'Your Site has been deployed and is ready to inspect.'
    case 'customer_input_needed':
      return 'OpenAgents needs your input before the next work step can move forward.'
    case 'unavailable_declined':
      return 'OpenAgents cannot continue this request yet based on the current scope and safety checks.'
    case 'delivered':
      return 'OpenAgents has delivered the requested work for review and acceptance.'
    case 'adjustment_received':
      return 'OpenAgents received your adjustment request and is preparing the next step.'
    case 'adjustment_completed':
      return 'OpenAgents completed the requested adjustment.'
  }
}

const orderStatusUrl = (appOrigin: string, orderId: string): string =>
  `${appOrigin.replace(/\/+$/, '')}/order?orderId=${encodeURIComponent(orderId)}`

const orderSitesStatusUrl = (
  input: OrderSitesLifecycleTemplateProps,
): string => input.statusPageUrl ?? orderStatusUrl(input.appOrigin, input.orderId)

const earlySoftwareReplyNote = (
  input: OrderSitesLifecycleTemplateProps,
): string | null =>
  input.lifecycleKind === 'review_ready' ||
  input.lifecycleKind === 'site_saved_version_ready' ||
  input.lifecycleKind === 'site_deployed' ||
  input.lifecycleKind === 'delivered' ||
  input.lifecycleKind === 'adjustment_completed'
    ? 'OpenAgents Sites is still very early software. If anything looks broken, confusing, or wrong, please reply to this email with the problem. Bug reports are genuinely appreciated.'
    : null

export const renderOrderSitesLifecycleEmail = (
  input: OrderSitesLifecycleTemplateProps,
): RenderedEmailTemplate => {
  const statusUrl = orderSitesStatusUrl(input)
  const escapedStatusUrl = escapeHtml(statusUrl)
  const escapedSiteUrl =
    input.siteUrl === null ? null : escapeHtml(input.siteUrl)
  const escapedRevisionUrl =
    input.revisionUrl === null ? null : escapeHtml(input.revisionUrl)
  const escapedArtifactUrl =
    input.artifactUrl === null ? null : escapeHtml(input.artifactUrl)
  const escapedArtifactLabel =
    input.artifactLabel === null
      ? 'Review artifact'
      : escapeHtml(input.artifactLabel)
  const replyNote = earlySoftwareReplyNote(input)

  const text = [
    `Hi ${input.displayName},`,
    '',
    orderSitesLifecycleLead(input),
    '',
    `Status: ${input.customerSafeStatus}`,
    `Next action: ${input.nextAction}`,
    input.safeReason === null ? null : `Reason: ${input.safeReason}`,
    input.siteUrl === null ? null : `Live Site: ${input.siteUrl}`,
    input.revisionUrl === null
      ? null
      : `Latest revision: ${input.revisionUrl}`,
    input.artifactUrl === null
      ? null
      : `${input.artifactLabel ?? 'Review artifact'}: ${input.artifactUrl}`,
    `Order status: ${statusUrl}`,
    replyNote === null ? null : '',
    replyNote,
    '',
    'OpenAgents',
  ]
    .filter((line): line is string => line !== null)
    .join('\n')

  const html = `<!doctype html>
<html>
  <head>
    <meta name="color-scheme" content="light" />
    <meta name="supported-color-schemes" content="light" />
  </head>
  <body bgcolor="#fbfaf6" style="margin:0;background:#fbfaf6 !important;color:#17211f !important;font-family:Inter,Arial,sans-serif;">
    <div style="max-width:560px;margin:0 auto;padding:40px 24px;background:#fbfaf6 !important;color:#17211f !important;">
      <p style="margin:0 0 28px;color:#5e6b68 !important;font-size:14px;">OpenAgents</p>
      <h1 style="margin:0;color:#17211f !important;font-size:28px;font-weight:600;line-height:1.2;">${escapeHtml(orderSitesLifecycleSubject(input))}</h1>
      <p style="margin:18px 0 0;color:#273330 !important;font-size:15px;line-height:1.6;">Hi ${escapeHtml(input.displayName)}, ${escapeHtml(orderSitesLifecycleLead(input))}</p>
      <p style="margin:18px 0 0;color:#273330 !important;font-size:15px;line-height:1.6;"><strong style="color:#17211f !important;">Project:</strong> ${escapeHtml(orderSitesSiteLabel(input))}</p>
      <p style="margin:18px 0 0;color:#273330 !important;font-size:15px;line-height:1.6;"><strong style="color:#17211f !important;">Status:</strong> ${escapeHtml(input.customerSafeStatus)}</p>
      <p style="margin:10px 0 0;color:#273330 !important;font-size:15px;line-height:1.6;"><strong style="color:#17211f !important;">Next action:</strong> ${escapeHtml(input.nextAction)}</p>
      ${
        input.safeReason === null
          ? ''
          : `<p style="margin:10px 0 0;color:#273330 !important;font-size:15px;line-height:1.6;"><strong style="color:#17211f !important;">Reason:</strong> ${escapeHtml(input.safeReason)}</p>`
      }
      ${
        escapedSiteUrl === null
          ? ''
          : `<p style="margin:18px 0 0;color:#273330 !important;font-size:15px;line-height:1.6;">Live Site: <a href="${escapedSiteUrl}" style="color:#11384c !important;">${escapedSiteUrl}</a></p>`
      }
      ${
        escapedRevisionUrl === null
          ? ''
          : `<p style="margin:10px 0 0;color:#273330 !important;font-size:15px;line-height:1.6;">Latest revision: <a href="${escapedRevisionUrl}" style="color:#11384c !important;">${escapedRevisionUrl}</a></p>`
      }
      ${
        escapedArtifactUrl === null
          ? ''
          : `<p style="margin:10px 0 0;color:#273330 !important;font-size:15px;line-height:1.6;">${escapedArtifactLabel}: <a href="${escapedArtifactUrl}" style="color:#11384c !important;">${escapedArtifactUrl}</a></p>`
      }
      <p style="margin:28px 0 0;">
        <a href="${escapedStatusUrl}" style="display:inline-block;border-radius:999px;background:#11384c !important;color:#ffffff !important;font-size:14px;font-weight:600;text-decoration:none;padding:11px 18px;">View status</a>
      </p>
      ${
        replyNote === null
          ? ''
          : `<p style="margin:24px 0 0;color:#5e6b68 !important;font-size:13px;line-height:1.6;">${escapeHtml(replyNote)}</p>`
      }
    </div>
  </body>
</html>`

  return {
    html,
    subject: orderSitesLifecycleSubject(input),
    templateContext: {
      artifactLabel: input.artifactLabel,
      artifactUrl: input.artifactUrl,
      customerSafeStatus: input.customerSafeStatus,
      displayName: input.displayName,
      lifecycleKind: input.lifecycleKind,
      nextAction: input.nextAction,
      orderId: input.orderId,
      safeReason: input.safeReason,
      revisionUrl: input.revisionUrl,
      siteTitle: input.siteTitle,
      siteUrl: input.siteUrl,
      statusPageUrl: statusUrl,
    },
    templateSlug: `order_sites.${input.lifecycleKind}.v1`,
    text,
  }
}

const dripCopy = (
  input: DripTemplateProps,
): Readonly<{ lead: string; nextAction: string; subject: string }> => {
  switch (input.kind) {
    case 'signup_day_0':
      return {
        lead:
          'Welcome to OpenAgents. You can submit a software request, track the public-safe status, and review revisions from your order page.',
        nextAction: 'Submit the first request you want Autopilot to scope.',
        subject: 'Start your first OpenAgents request',
      }
    case 'signup_day_1':
      return {
        lead:
          'OpenAgents works best when your request includes the desired audience, the first useful outcome, and any source access needed to begin.',
        nextAction:
          'Add repository or product context before the next Autopilot pass.',
        subject: 'Add the context your Autopilot run needs',
      }
    case 'signup_day_2':
      return {
        lead:
          'When a revision is ready, your order page will show the latest result and accept follow-up comments for the next queued pass.',
        nextAction: 'Review any ready revision and send one concrete adjustment.',
        subject: 'Review revisions from your OpenAgents order page',
      }
  }
}

export const renderDripEmail = (
  input: DripTemplateProps,
): RenderedEmailTemplate => {
  const copy = dripCopy(input)
  const escapedPreferencesUrl = escapeHtml(input.managePreferencesUrl)
  const text = [
    `Hi ${input.displayName},`,
    '',
    copy.lead,
    '',
    `Next action: ${copy.nextAction}`,
    `Manage email preferences: ${input.managePreferencesUrl}`,
    '',
    'OpenAgents',
  ].join('\n')
  const html = `<!doctype html>
<html>
  <body style="margin:0;background:#ffffff;color:#111318;font-family:Inter,Arial,sans-serif;">
    <div style="max-width:560px;margin:0 auto;padding:40px 24px;">
      <p style="margin:0 0 24px;color:#6a6f78;font-size:14px;">OpenAgents</p>
      <h1 style="margin:0;color:#111318;font-size:26px;font-weight:600;line-height:1.2;">${escapeHtml(copy.subject)}</h1>
      <p style="margin:18px 0 0;color:#252a31;font-size:15px;line-height:1.6;">Hi ${escapeHtml(input.displayName)}, ${escapeHtml(copy.lead)}</p>
      <p style="margin:18px 0 0;color:#252a31;font-size:15px;line-height:1.6;"><strong>Next action:</strong> ${escapeHtml(copy.nextAction)}</p>
      <p style="margin:28px 0 0;color:#6a6f78;font-size:13px;line-height:1.5;"><a href="${escapedPreferencesUrl}" style="color:#111318;">Manage email preferences</a></p>
    </div>
  </body>
</html>`

  return {
    html,
    subject: copy.subject,
    templateContext: {
      displayName: input.displayName,
      kind: input.kind,
      managePreferencesUrl: input.managePreferencesUrl,
      nextAction: copy.nextAction,
    },
    templateSlug: `drip.${input.kind}.v1`,
    text,
  }
}

const autopilotDecisionCopy = (
  input: AutopilotDecisionTemplateProps,
): Readonly<{ lead: string; nextAction: string; subject: string }> => {
  switch (input.kind) {
    case 'decision_required':
      return {
        lead:
          'Autopilot delivered work that is now waiting on your decision. Nothing proceeds until you review it, and your decision is recorded as a gated submission with a receipt trail.',
        nextAction:
          'Open your decision queue and approve, request changes, or reject the delivered work.',
        subject: 'Autopilot work delivered - your decision is required',
      }
    case 'work_delivered':
      return {
        lead:
          'Autopilot delivered work on your order. You can inspect the delivered refs and receipts from your decision queue.',
        nextAction: 'Open your decision queue to review the delivered work.',
        subject: 'Your Autopilot work order was delivered',
      }
  }
}

const decisionsUrl = (appOrigin: string): string =>
  `${appOrigin.replace(/\/+$/, '')}/decisions`

export const renderAutopilotDecisionEmail = (
  input: AutopilotDecisionTemplateProps,
): RenderedEmailTemplate => {
  const copy = autopilotDecisionCopy(input)
  const queueUrl = decisionsUrl(input.appOrigin)
  const escapedQueueUrl = escapeHtml(queueUrl)
  const text = [
    `Hi ${input.displayName},`,
    '',
    copy.lead,
    '',
    `Work order: ${input.workOrderRef}`,
    `Next action: ${copy.nextAction}`,
    `Decision queue: ${queueUrl}`,
    '',
    'OpenAgents',
  ].join('\n')
  const html = `<!doctype html>
<html>
  <body style="margin:0;background:#ffffff;color:#111318;font-family:Inter,Arial,sans-serif;">
    <div style="max-width:560px;margin:0 auto;padding:40px 24px;">
      <p style="margin:0 0 24px;color:#6a6f78;font-size:14px;">OpenAgents</p>
      <h1 style="margin:0;color:#111318;font-size:26px;font-weight:600;line-height:1.2;">${escapeHtml(copy.subject)}</h1>
      <p style="margin:18px 0 0;color:#252a31;font-size:15px;line-height:1.6;">Hi ${escapeHtml(input.displayName)}, ${escapeHtml(copy.lead)}</p>
      <p style="margin:18px 0 0;color:#252a31;font-size:15px;line-height:1.6;"><strong>Work order:</strong> ${escapeHtml(input.workOrderRef)}</p>
      <p style="margin:10px 0 0;color:#252a31;font-size:15px;line-height:1.6;"><strong>Next action:</strong> ${escapeHtml(copy.nextAction)}</p>
      <p style="margin:28px 0 0;">
        <a href="${escapedQueueUrl}" style="display:inline-block;border-radius:999px;background:#11384c;color:#ffffff;font-size:14px;font-weight:600;text-decoration:none;padding:11px 18px;">Open decision queue</a>
      </p>
    </div>
  </body>
</html>`

  return {
    html,
    subject: copy.subject,
    templateContext: {
      decisionsUrl: queueUrl,
      displayName: input.displayName,
      kind: input.kind,
      nextAction: copy.nextAction,
      workOrderRef: input.workOrderRef,
    },
    templateSlug: `autopilot_decisions.${input.kind}.v1`,
    text,
  }
}

export const renderPrivateWorkspaceInviteEmail = (
  input: PrivateWorkspaceInviteTemplateProps,
): RenderedEmailTemplate => {
  const subject = 'Your private OpenAgents workspace invite'
  const escapedAcceptUrl = escapeHtml(input.acceptUrl)
  const text = [
    `Hi ${input.displayName},`,
    '',
    `You have been invited to ${input.workspaceLabel} in OpenAgents.`,
    '',
    `Accept invite: ${input.acceptUrl}`,
    `Expires: ${input.expiresAt}`,
    '',
    'Next action: sign in with the invited email address, accept the invite, and open the workspace before the call.',
    '',
    'OpenAgents',
  ].join('\n')
  const html = `<!doctype html>
<html>
  <body style="margin:0;background:#ffffff;color:#111318;font-family:Inter,Arial,sans-serif;">
    <div style="max-width:560px;margin:0 auto;padding:40px 24px;">
      <p style="margin:0 0 24px;color:#6a6f78;font-size:14px;">OpenAgents</p>
      <h1 style="margin:0;color:#111318;font-size:26px;font-weight:600;line-height:1.2;">${escapeHtml(subject)}</h1>
      <p style="margin:18px 0 0;color:#252a31;font-size:15px;line-height:1.6;">Hi ${escapeHtml(input.displayName)}, you have been invited to ${escapeHtml(input.workspaceLabel)} in OpenAgents.</p>
      <p style="margin:18px 0 0;color:#252a31;font-size:15px;line-height:1.6;"><strong>Expires:</strong> ${escapeHtml(input.expiresAt)}</p>
      <p style="margin:28px 0 0;">
        <a href="${escapedAcceptUrl}" style="display:inline-block;border-radius:999px;background:#11384c;color:#ffffff;font-size:14px;font-weight:600;text-decoration:none;padding:11px 18px;">Accept invite</a>
      </p>
      <p style="margin:24px 0 0;color:#6a6f78;font-size:13px;line-height:1.5;">Sign in with the invited email address before accepting the workspace invite.</p>
    </div>
  </body>
</html>`

  return {
    html,
    subject,
    templateContext: {
      acceptUrl: input.acceptUrl,
      displayName: input.displayName,
      expiresAt: input.expiresAt,
      workspaceLabel: input.workspaceLabel,
    },
    templateSlug: 'team_workspace_invite.v1',
    text,
  }
}

export const renderEmailTemplatePreviewCatalog = (
  appOrigin: string,
): ReadonlyArray<RenderedEmailTemplate> => [
  renderOrderSitesLifecycleEmail(
    new OrderSitesLifecycleTemplateProps({
      appOrigin,
      artifactLabel: null,
      artifactUrl: null,
      customerSafeStatus: 'Ready for review',
      displayName: 'Alex Customer',
      lifecycleKind: 'review_ready',
      nextAction:
        'Open your order status page, review the latest Site revision, and send any follow-up comment.',
      orderId: 'software_order_preview',
      safeReason: null,
      revisionUrl:
        'https://sites.openagents.com/otec/versions/site_version_otec_20260605_revision_3',
      siteTitle: 'OTEC Floating Datacenter',
      siteUrl: 'https://sites.openagents.com/otec',
    }),
  ),
  ...DRIP_EMAIL_KINDS.map(kind =>
    renderDripEmail(
      new DripTemplateProps({
        appOrigin,
        displayName: 'Alex Customer',
        kind,
        managePreferencesUrl: `${appOrigin.replace(/\/+$/, '')}/email/preferences`,
      }),
    ),
  ),
  ...AUTOPILOT_DECISION_EMAIL_KINDS.map(kind =>
    renderAutopilotDecisionEmail(
      new AutopilotDecisionTemplateProps({
        appOrigin,
        displayName: 'Alex Customer',
        kind,
        workOrderRef: 'autopilot_work_order.preview',
      }),
    ),
  ),
  renderPrivateWorkspaceInviteEmail(
    new PrivateWorkspaceInviteTemplateProps({
      acceptUrl: `${appOrigin.replace(/\/+$/, '')}/api/team-workspace-invites/accept?token=preview`,
      displayName: 'Alex Customer',
      expiresAt: '2026-06-19T12:00:00.000Z',
      workspaceLabel: 'a private OpenAgents workspace',
    }),
  ),
]
