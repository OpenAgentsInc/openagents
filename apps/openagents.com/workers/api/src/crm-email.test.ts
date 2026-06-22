import { describe, expect, test } from 'vitest'

import {
  crmMarkdownToHtml,
  type CrmEmailTemplate,
  renderCrmEmail,
  renderCrmTemplateString,
} from './crm-email'
import { type CrmContact } from './crm-store'

const contact = (over: Partial<CrmContact> = {}): CrmContact => ({
  accountId: null,
  contactType: 'investor',
  createdAt: '2026-06-22T00:00:00.000Z',
  engagementScore: 0,
  externalSourceId: null,
  externalSourceLabel: null,
  firstName: 'Ada',
  fullName: 'Ada Lovelace',
  id: 'crm_contact_1',
  jobTitle: 'Founder',
  lastContactedAt: null,
  lastEngagedAt: null,
  lastName: 'Lovelace',
  lastRepliedAt: null,
  lifecycleStage: 'lead',
  notes: null,
  portalAccessStatus: 'none',
  primaryEmail: 'ada@example.com',
  relationshipStage: 'new',
  secondaryEmail: null,
  tenantRef: 'tenant.openagents',
  updatedAt: '2026-06-22T00:00:00.000Z',
  ...over,
})

const template = (over: Partial<CrmEmailTemplate> = {}): CrmEmailTemplate => ({
  bodyMarkdownTemplate: 'Hi {{ contact.first_name_or_there }}, welcome to {{ app.name }}.',
  createdAt: '2026-06-22T00:00:00.000Z',
  id: 'crm_template_1',
  name: 'Welcome',
  slug: 'welcome',
  status: 'active',
  subjectTemplate: '{{ app.name }} for {{ contact.first_name }}',
  tenantRef: 'tenant.openagents',
  updatedAt: '2026-06-22T00:00:00.000Z',
  ...over,
})

describe('renderCrmTemplateString', () => {
  test('replaces known tokens and blanks unknown ones', () => {
    const out = renderCrmTemplateString('A {{ contact.first_name }} B {{ nope }} C', {
      'contact.first_name': 'Ada',
    })
    expect(out).toBe('A Ada B  C')
  })

  test('tolerates loose whitespace inside the braces', () => {
    expect(renderCrmTemplateString('{{contact.first_name}}', { 'contact.first_name': 'Ada' })).toBe(
      'Ada',
    )
  })
})

describe('crmMarkdownToHtml', () => {
  test('escapes html, then applies bold and links, splitting paragraphs', () => {
    const html = crmMarkdownToHtml('Hello **world** <script>\n\nSee [site](https://openagents.com)')
    expect(html).toContain('<strong>world</strong>')
    expect(html).toContain('&lt;script&gt;')
    expect(html).toContain('<a href="https://openagents.com">site</a>')
    expect(html.split('<p>').length).toBe(3) // two paragraphs => 2 opening tags + leading split
  })
})

describe('renderCrmEmail', () => {
  test('personalizes subject + body from the contact', () => {
    const rendered = renderCrmEmail(template(), contact())
    expect(rendered.subject).toBe('OpenAgents for Ada')
    expect(rendered.bodyMarkdown).toBe('Hi Ada, welcome to OpenAgents.')
    expect(rendered.bodyHtml).toContain('<p>Hi Ada, welcome to OpenAgents.</p>')
  })

  test('first_name_or_there falls back to "there" when no name', () => {
    const rendered = renderCrmEmail(
      template({ bodyMarkdownTemplate: 'Hi {{ contact.first_name_or_there }}' }),
      contact({ firstName: null, fullName: null }),
    )
    expect(rendered.bodyMarkdown).toBe('Hi there')
  })
})
