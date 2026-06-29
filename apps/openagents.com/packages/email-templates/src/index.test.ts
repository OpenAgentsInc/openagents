import { describe, expect, test } from 'vitest'

import {
  AUTOPILOT_DECISION_EMAIL_KINDS,
  AutopilotDecisionTemplateProps,
  DRIP_EMAIL_KINDS,
  ORDER_SITES_LIFECYCLE_EMAIL_KINDS,
  DripTemplateProps,
  OrderSitesLifecycleTemplateProps,
  renderAutopilotDecisionEmail,
  renderDripEmail,
  renderEmailTemplatePreviewCatalog,
  renderOrderSitesLifecycleEmail,
} from './index'

const lifecycleProps = (lifecycleKind: string) =>
  new OrderSitesLifecycleTemplateProps({
    appOrigin: 'https://openagents.com',
    artifactLabel: null,
    artifactUrl: null,
    customerSafeStatus: `safe status for ${lifecycleKind}`,
    displayName: 'Alex <Customer>',
    lifecycleKind: lifecycleKind as never,
    nextAction: `safe next action for ${lifecycleKind}`,
    orderId: 'software_order_otec',
    revisionUrl:
      'https://sites.openagents.com/otec/versions/site_version_otec_20260605_revision_3',
    safeReason: null,
    siteTitle: 'OTEC <Launch>',
    siteUrl: 'https://sites.openagents.com/otec',
  })

describe('email template package', () => {
  test('renders every order/Sites lifecycle template with safe status and action', () => {
    for (const lifecycleKind of ORDER_SITES_LIFECYCLE_EMAIL_KINDS) {
      const input = lifecycleProps(lifecycleKind)
      const rendered = renderOrderSitesLifecycleEmail(input)

      expect(rendered.templateSlug).toBe(`order_sites.${lifecycleKind}.v1`)
      expect(rendered.subject.length).toBeGreaterThan(0)
      expect(rendered.text).toContain(input.customerSafeStatus)
      expect(rendered.text).toContain(input.nextAction)
      expect(rendered.text).toContain('Live Site:')
      expect(rendered.text).toContain('Latest revision:')
      expect(rendered.html).toContain('Alex &lt;Customer&gt;')
      expect(rendered.html).toContain('OTEC &lt;Launch&gt;')
      expect(rendered.html).toContain('Live Site:')
      expect(rendered.html).toContain('Latest revision:')
      expect(rendered.html).not.toContain('OpenAgents email ledger')
      expect(rendered.html).not.toContain('Alex <Customer>')
      expect(rendered.templateContext).toMatchObject({
        customerSafeStatus: input.customerSafeStatus,
        lifecycleKind,
        nextAction: input.nextAction,
      })
    }
  })

  test('renders a non-Sites review artifact link', () => {
    const rendered = renderOrderSitesLifecycleEmail(
      new OrderSitesLifecycleTemplateProps({
        ...lifecycleProps('review_ready'),
        artifactLabel: 'Review pull request',
        artifactUrl: 'https://github.com/customer/app/pull/7',
        revisionUrl: null,
        siteUrl: null,
      }),
    )

    expect(rendered.text).toContain(
      'Review pull request: https://github.com/customer/app/pull/7',
    )
    expect(rendered.html).toContain('Review pull request:')
    expect(rendered.html).toContain('https://github.com/customer/app/pull/7')
    expect(rendered.text).not.toContain('Live Site:')
    expect(rendered.text).not.toContain('Latest revision:')
  })

  test('asks review-ready recipients to reply with early software bug reports', () => {
    const rendered = renderOrderSitesLifecycleEmail(lifecycleProps('review_ready'))

    expect(rendered.text).toContain(
      'OpenAgents Sites is still very early software.',
    )
    expect(rendered.text).toContain('please reply to this email')
    expect(rendered.text).toContain('Bug reports are genuinely appreciated.')
    expect(rendered.html).toContain('OpenAgents Sites is still very early software.')
    expect(rendered.html).toContain('please reply to this email')
  })

  test('does not add early software reply copy to order receipt emails', () => {
    const rendered = renderOrderSitesLifecycleEmail(lifecycleProps('order_received'))

    expect(rendered.text).not.toContain(
      'OpenAgents Sites is still very early software.',
    )
    expect(rendered.html).not.toContain('please reply to this email')
  })

  test('renders day 0/day 1/day 2 drip templates with preference links', () => {
    for (const kind of DRIP_EMAIL_KINDS) {
      const rendered = renderDripEmail(
        new DripTemplateProps({
          appOrigin: 'https://openagents.com',
          displayName: 'Alex <Customer>',
          kind,
          managePreferencesUrl: 'https://openagents.com/email/preferences',
        }),
      )

      expect(rendered.templateSlug).toBe(`drip.${kind}.v1`)
      expect(rendered.subject.length).toBeGreaterThan(0)
      expect(rendered.text).toContain('Manage email preferences')
      expect(rendered.html).toContain('Alex &lt;Customer&gt;')
      expect(rendered.html).not.toContain('Alex <Customer>')
    }
  })

  test('renders Autopilot decision-queue templates with the decisions link', () => {
    for (const kind of AUTOPILOT_DECISION_EMAIL_KINDS) {
      const rendered = renderAutopilotDecisionEmail(
        new AutopilotDecisionTemplateProps({
          appOrigin: 'https://openagents.com',
          displayName: 'Alex <Customer>',
          kind,
          workOrderRef: 'autopilot_work_order.decision_test_1',
        }),
      )

      expect(rendered.templateSlug).toBe(`autopilot_decisions.${kind}.v1`)
      expect(rendered.subject.length).toBeGreaterThan(0)
      expect(rendered.text).toContain('https://openagents.com/decisions')
      expect(rendered.text).toContain(
        'Work order: autopilot_work_order.decision_test_1',
      )
      expect(rendered.html).toContain('https://openagents.com/decisions')
      expect(rendered.html).toContain('Alex &lt;Customer&gt;')
      expect(rendered.html).not.toContain('Alex <Customer>')
      expect(rendered.templateContext).toMatchObject({
        decisionsUrl: 'https://openagents.com/decisions',
        kind,
        workOrderRef: 'autopilot_work_order.decision_test_1',
      })
    }
  })

  test('asks decision-required recipients to act from the decision queue', () => {
    const rendered = renderAutopilotDecisionEmail(
      new AutopilotDecisionTemplateProps({
        appOrigin: 'https://openagents.com',
        displayName: 'Alex Customer',
        kind: 'decision_required',
        workOrderRef: 'autopilot_work_order.decision_test_1',
      }),
    )

    expect(rendered.subject).toBe(
      'Autopilot work delivered - your decision is required',
    )
    expect(rendered.text).toContain('approve, request changes, or reject')
    expect(rendered.text).toContain('gated submission')
  })

  test('builds a local preview catalog', () => {
    const catalog = renderEmailTemplatePreviewCatalog('https://openagents.com')

    expect(catalog.map(template => template.templateSlug)).toEqual([
      'order_sites.review_ready.v1',
      'drip.signup_day_0.v1',
      'drip.signup_day_1.v1',
      'drip.signup_day_2.v1',
      'autopilot_decisions.decision_required.v1',
      'autopilot_decisions.work_delivered.v1',
      'team_workspace_invite.v1',
    ])
  })
})
