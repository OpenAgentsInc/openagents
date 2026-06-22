import { clsx } from 'clsx'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import {
  eyebrowClass,
  kitFamily,
  metaClass,
  statusDotClass,
  surfaceClass,
  toneTextClass,
} from '@openagentsinc/ui'
import type { Tone } from '@openagentsinc/ui'

// CRM CONTACTS PANE (epic #5980, sub-issue #5987).
//
// Presentational Foldkit/HTML builder for the Autopilot Desktop CRM read pane:
// the contact list, a selected contact's activity timeline + send ledger, and
// the local Gmail executor's queue depth. Mirrors `email-sequence-panel.ts` —
// pure builders over typed view inputs, NO data fetching and NO send authority.
// The desktop/web page supplies an already-projected `CrmContactsPaneModel`
// (shape it from the `/api/operator/crm/*` reads with `buildCrmContactsPaneModel`).
// Rendering here never sends or mutates anything.

export type CrmRelationshipStage =
  | 'new'
  | 'contacted'
  | 'qualified'
  | 'engaged'
  | 'won'
  | 'lost'
  | string

export type CrmContactRowModel = Readonly<{
  id: string
  displayName: string
  email: string
  relationshipStage: CrmRelationshipStage
  engagementScore: number
  lastContactedAt: string | null
}>

export type CrmActivityModel = Readonly<{
  activityType: string
  subject: string | null
  occurredAt: string
}>

export type CrmLedgerEntryModel = Readonly<{
  subject: string
  status: string
  channel: string
  sentAt: string | null
}>

export type CrmQueuedSendModel = Readonly<{
  id: string
  toEmail: string
  subject: string
  channel: string
}>

export type CrmContactDetailModel = Readonly<{
  contact: CrmContactRowModel
  activities: ReadonlyArray<CrmActivityModel>
  ledger: ReadonlyArray<CrmLedgerEntryModel>
}>

export type CrmContactsPaneModel = Readonly<{
  contacts: ReadonlyArray<CrmContactRowModel>
  selected: CrmContactDetailModel | null
  queue: ReadonlyArray<CrmQueuedSendModel>
}>

// --- pure derivations (unit tested directly) -------------------------------

/** Display label for a contact: name when present, else the email. */
export const crmContactDisplayName = (
  contact: Readonly<{ displayName?: string | null; email: string }>,
): string => {
  const name = (contact.displayName ?? '').trim()
  return name === '' ? contact.email : name
}

/** Map a relationship stage to a UI tone (only known-safe Tone literals). */
export const crmRelationshipTone = (stage: CrmRelationshipStage): Tone => {
  switch (stage) {
    case 'won':
    case 'engaged':
      return 'positive'
    case 'new':
    case 'qualified':
      return 'accent'
    case 'lost':
      return 'warning'
    default:
      return 'neutral'
  }
}

/** One-line summary of a contact's activity timeline. */
export const summarizeCrmActivities = (
  activities: ReadonlyArray<CrmActivityModel>,
): Readonly<{ count: number; lastType: string | null }> => ({
  count: activities.length,
  lastType: activities.length === 0 ? null : (activities[0]?.activityType ?? null),
})

/** Count of sent (vs queued/failed) ledger entries. */
export const countCrmSent = (ledger: ReadonlyArray<CrmLedgerEntryModel>): number =>
  ledger.filter(entry => entry.status === 'sent').length

const pluralize = (count: number, noun: string): string =>
  `${count} ${noun}${count === 1 ? '' : 's'}`

// --- read-model shaping (from /api/operator/crm/* responses) ---------------

type RawContact = Readonly<{
  id?: unknown
  fullName?: unknown
  primaryEmail?: unknown
  relationshipStage?: unknown
  engagementScore?: unknown
  lastContactedAt?: unknown
}>

const asString = (value: unknown): string => (typeof value === 'string' ? value : '')
const asNumber = (value: unknown): number =>
  typeof value === 'number' && Number.isFinite(value) ? value : 0
const asNullableString = (value: unknown): string | null =>
  typeof value === 'string' ? value : null

const shapeContact = (raw: RawContact): CrmContactRowModel => ({
  displayName: crmContactDisplayName({
    displayName: asNullableString(raw.fullName),
    email: asString(raw.primaryEmail),
  }),
  email: asString(raw.primaryEmail),
  engagementScore: asNumber(raw.engagementScore),
  id: asString(raw.id),
  lastContactedAt: asNullableString(raw.lastContactedAt),
  relationshipStage: asString(raw.relationshipStage) || 'new',
})

/**
 * Project the raw read API payloads into a pane model. Defensive: tolerates
 * missing fields so a partial fetch still renders.
 */
export const buildCrmContactsPaneModel = (raw: {
  contacts?: ReadonlyArray<RawContact>
  selected?: {
    contact?: RawContact
    activities?: ReadonlyArray<{ activityType?: unknown; subject?: unknown; occurredAt?: unknown }>
    ledger?: ReadonlyArray<{ subject?: unknown; status?: unknown; channel?: unknown; sentAt?: unknown }>
  } | null
  queue?: ReadonlyArray<{ id?: unknown; toEmail?: unknown; subject?: unknown; channel?: unknown }>
}): CrmContactsPaneModel => ({
  contacts: (raw.contacts ?? []).map(shapeContact),
  queue: (raw.queue ?? []).map(entry => ({
    channel: asString(entry.channel),
    id: asString(entry.id),
    subject: asString(entry.subject),
    toEmail: asString(entry.toEmail),
  })),
  selected:
    raw.selected === null || raw.selected === undefined || raw.selected.contact === undefined
      ? null
      : {
          activities: (raw.selected.activities ?? []).map(a => ({
            activityType: asString(a.activityType),
            occurredAt: asString(a.occurredAt),
            subject: asNullableString(a.subject),
          })),
          contact: shapeContact(raw.selected.contact),
          ledger: (raw.selected.ledger ?? []).map(l => ({
            channel: asString(l.channel),
            sentAt: asNullableString(l.sentAt),
            status: asString(l.status),
            subject: asString(l.subject),
          })),
        },
})

// --- presentational builders ----------------------------------------------

const contactRow = <Message>(contact: CrmContactRowModel, selectedId: string | null): Html => {
  const h = html<Message>()
  const tone = crmRelationshipTone(contact.relationshipStage)
  const isSelected = selectedId !== null && selectedId === contact.id
  return h.li(
    [
      h.Class(
        clsx(
          'flex items-center justify-between gap-3 border-b border-white/5 px-3 py-2',
          isSelected ? 'bg-white/5' : '',
        ),
      ),
      h.DataAttribute('crm-contact-row', contact.id),
    ],
    [
      h.div(
        [h.Class('min-w-0')],
        [
          h.p([h.Class('m-0 truncate text-sm font-medium text-[#f1efe8]')], [contact.displayName]),
          h.p([h.Class(clsx(metaClass, 'truncate'))], [contact.email]),
        ],
      ),
      h.div(
        [h.Class(clsx('flex flex-none items-center gap-2', toneTextClass(tone)))],
        [
          h.span([h.Class(statusDotClass(tone))], []),
          h.span(
            [h.Class('text-xs font-medium uppercase tracking-[0.08em]')],
            [contact.relationshipStage],
          ),
        ],
      ),
    ],
  )
}

const detailSection = <Message>(detail: CrmContactDetailModel): Html => {
  const h = html<Message>()
  const activitySummary = summarizeCrmActivities(detail.activities)
  return h.div(
    [h.Class('grid gap-2 border-t border-white/10 pt-3'), h.DataAttribute('crm-contact-detail', detail.contact.id)],
    [
      h.p([h.Class(eyebrowClass)], ['Selected contact']),
      h.p([h.Class('m-0 text-base font-semibold text-[#f1efe8]')], [detail.contact.displayName]),
      h.p(
        [h.Class(metaClass)],
        [`${pluralize(activitySummary.count, 'activity')} · ${pluralize(countCrmSent(detail.ledger), 'sent email')}`],
      ),
      detail.activities.length === 0
        ? h.p([h.Class(metaClass)], ['No activity yet.'])
        : h.ul(
            [h.Class('m-0 grid list-none gap-1 p-0')],
            detail.activities
              .slice(0, 8)
              .map(activity =>
                h.li(
                  [h.Class('text-xs text-[#cfcabb]')],
                  [`${activity.activityType}${activity.subject === null ? '' : ` — ${activity.subject}`}`],
                ),
              ),
          ),
    ],
  )
}

export const crmContactsPanel = <Message>(model: CrmContactsPaneModel): Html => {
  const h = html<Message>()
  return h.section(
    [
      kitFamily<Message>('data-display/cards'),
      h.Class(clsx(surfaceClass, 'grid gap-4 p-4')),
      h.DataAttribute('crm-contacts-panel', ''),
    ],
    [
      h.div(
        [h.Class('flex items-start justify-between gap-3')],
        [
          h.div(
            [],
            [
              h.p([h.Class(eyebrowClass)], ['CRM']),
              h.p([h.Class('m-0 text-xl font-semibold text-[#f1efe8]')], ['Contacts']),
              h.p([h.Class(metaClass)], [pluralize(model.contacts.length, 'contact')]),
            ],
          ),
          h.div(
            [h.Class(clsx('flex flex-none items-center gap-2', toneTextClass('accent')))],
            [
              h.span([h.Class('text-xs font-medium uppercase tracking-[0.08em]')], [
                `${pluralize(model.queue.length, 'queued send')}`,
              ]),
            ],
          ),
        ],
      ),
      model.contacts.length === 0
        ? h.p([h.Class(metaClass), h.DataAttribute('crm-contacts-empty', '')], ['No contacts imported yet.'])
        : h.ul(
            [h.Class('m-0 grid list-none gap-0 p-0')],
            model.contacts.map(contact =>
              contactRow<Message>(contact, model.selected?.contact.id ?? null),
            ),
          ),
      model.selected === null ? null : detailSection<Message>(model.selected),
    ],
  )
}
