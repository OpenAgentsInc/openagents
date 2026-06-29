import { describe, expect, it } from 'vitest'

import {
  buildCrmContactsPaneModel,
  countCrmSent,
  crmContactDisplayName,
  crmContactsPanel,
  crmRelationshipTone,
  summarizeCrmActivities,
} from './crm-contacts-panel'

describe('crmContactDisplayName', () => {
  it('uses the name when present, else the email', () => {
    expect(crmContactDisplayName({ displayName: 'Ada Lovelace', email: 'ada@x.com' })).toBe('Ada Lovelace')
    expect(crmContactDisplayName({ displayName: '  ', email: 'ada@x.com' })).toBe('ada@x.com')
    expect(crmContactDisplayName({ displayName: null, email: 'ada@x.com' })).toBe('ada@x.com')
  })
})

describe('crmRelationshipTone', () => {
  it('maps stages to known-safe tones', () => {
    expect(crmRelationshipTone('won')).toBe('positive')
    expect(crmRelationshipTone('engaged')).toBe('positive')
    expect(crmRelationshipTone('new')).toBe('accent')
    expect(crmRelationshipTone('lost')).toBe('warning')
    expect(crmRelationshipTone('something_else')).toBe('neutral')
  })
})

describe('summarizeCrmActivities + countCrmSent', () => {
  it('summarizes the timeline and counts sent ledger entries', () => {
    expect(summarizeCrmActivities([])).toEqual({ count: 0, lastType: null })
    expect(
      summarizeCrmActivities([
        { activityType: 'email_sent', occurredAt: '2026-06-22T00:00:00Z', subject: null },
        { activityType: 'email_drafted', occurredAt: '2026-06-21T00:00:00Z', subject: null },
      ]),
    ).toEqual({ count: 2, lastType: 'email_sent' })
    expect(
      countCrmSent([
        { channel: 'resend', sentAt: '2026-06-22T00:00:00Z', status: 'sent', subject: 'a' },
        { channel: 'gmail_gws', sentAt: null, status: 'queued', subject: 'b' },
      ]),
    ).toBe(1)
  })
})

describe('buildCrmContactsPaneModel', () => {
  it('shapes raw API payloads defensively', () => {
    const model = buildCrmContactsPaneModel({
      contacts: [
        { fullName: 'Ada Lovelace', id: 'c1', primaryEmail: 'ada@x.com', relationshipStage: 'engaged' },
        { id: 'c2', primaryEmail: 'bob@x.com' }, // missing name/stage
      ],
      queue: [{ channel: 'gmail_gws', id: 'q1', subject: 'Hi', toEmail: 'ada@x.com' }],
      selected: {
        activities: [{ activityType: 'email_sent', occurredAt: '2026-06-22T00:00:00Z', subject: 'Hi' }],
        contact: { fullName: 'Ada Lovelace', id: 'c1', primaryEmail: 'ada@x.com', relationshipStage: 'engaged' },
        ledger: [{ channel: 'resend', sentAt: '2026-06-22T00:00:00Z', status: 'sent', subject: 'Hi' }],
      },
    })
    expect(model.contacts).toHaveLength(2)
    expect(model.contacts[0]?.displayName).toBe('Ada Lovelace')
    expect(model.contacts[1]?.displayName).toBe('bob@x.com') // falls back to email
    expect(model.contacts[1]?.relationshipStage).toBe('new') // default
    expect(model.queue).toHaveLength(1)
    expect(model.selected?.activities[0]?.activityType).toBe('email_sent')
    expect(model.selected?.ledger[0]?.status).toBe('sent')
  })

  it('handles a null selection and empty payload', () => {
    const model = buildCrmContactsPaneModel({})
    expect(model.contacts).toEqual([])
    expect(model.queue).toEqual([])
    expect(model.selected).toBeNull()
  })
})

describe('crmContactsPanel', () => {
  it('builds an Html node for a populated model', () => {
    const node = crmContactsPanel(
      buildCrmContactsPaneModel({
        contacts: [{ fullName: 'Ada', id: 'c1', primaryEmail: 'ada@x.com', relationshipStage: 'new' }],
        queue: [],
        selected: null,
      }),
    )
    expect(node).toBeTruthy()
    expect(typeof node).toBe('object')
  })

  it('builds an Html node for an empty model', () => {
    const node = crmContactsPanel(buildCrmContactsPaneModel({}))
    expect(node).toBeTruthy()
  })
})
