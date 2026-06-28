import { describe, expect, test } from 'vitest'

import {
  artanisResponderTickRef,
  projectArtanisResponderTickReadiness,
  type ArtanisResponderTickActionRow,
  type ArtanisResponderTickRow,
} from './artanis-responder-ticks'

const tickAt = (hour: number): string =>
  `2026-06-28T${String(hour).padStart(2, '0')}:00:00.000Z`

const tickRow = (
  hour: number,
  overrides: Partial<ArtanisResponderTickRow> = {},
): ArtanisResponderTickRow => {
  const scheduledAt = tickAt(hour)
  return {
    compose_blocked: 0,
    compose_considered: 1,
    compose_responded: 1,
    compose_skipped_reason: null,
    compose_state: 'ran',
    compose_tipped: 0,
    scan_blocked: 0,
    scan_proposed: 1,
    scan_scanned: 1,
    scan_skipped: 0,
    scan_skipped_reason: null,
    scan_state: 'ran',
    scheduled_at: scheduledAt,
    tick_ref: artanisResponderTickRef(scheduledAt),
    ...overrides,
  }
}

const actionRow = (
  hour: number,
  overrides: Partial<ArtanisResponderTickActionRow> = {},
): ArtanisResponderTickActionRow => ({
  asker_provenance: 'external_contributor',
  id: `action-${hour}`,
  replied_at: `2026-06-28T${String(hour).padStart(2, '0')}:15:00.000Z`,
  reply_post_id: `post.public.artanis.${hour}`,
  state: 'responded',
  topic_id: `topic-${hour}`,
  ...overrides,
})

describe('projectArtanisResponderTickReadiness', () => {
  test('proves the ten unattended responder tick target from qualifying windows', () => {
    const ticks = Array.from({ length: 10 }, (_, index) => tickRow(index + 1))
    const actions = Array.from({ length: 10 }, (_, index) =>
      actionRow(index + 1),
    )

    const projection = projectArtanisResponderTickReadiness(ticks, actions)

    expect(projection.kind).toBe(
      'artanis_pylon_support_responder_tick_readiness',
    )
    expect(projection.publicSafe).toBe(true)
    expect(projection.tickTarget).toBe(10)
    expect(projection.qualifyingUnattendedResponderTickCount).toBe(10)
    expect(projection.unattendedResponderTicksProven).toBe(true)
    expect(projection.externalContributorAnsweredWithinTickWindow).toBe(true)
    expect(projection.tickWindows[0]?.replyPostRefs).toContain(
      'post.public.artanis.10',
    )
  })

  test('does not count skipped or replyless scheduled rows as qualifying ticks', () => {
    const projection = projectArtanisResponderTickReadiness(
      [
        tickRow(1, {
          scan_scanned: 0,
          scan_skipped_reason: 'mind_unconfigured',
          scan_state: 'skipped',
        }),
        tickRow(2, {
          compose_responded: 0,
        }),
      ],
      [actionRow(1)],
    )

    expect(projection.qualifyingUnattendedResponderTickCount).toBe(0)
    expect(projection.unattendedResponderTicksProven).toBe(false)
  })

  test('requires the answered external contributor to land inside a tick window', () => {
    const projection = projectArtanisResponderTickReadiness(
      [tickRow(1), tickRow(2)],
      [
        actionRow(1, {
          asker_provenance: 'owner_operator',
        }),
        actionRow(3, {
          replied_at: '2026-06-28T00:30:00.000Z',
        }),
      ],
    )

    expect(projection.externalContributorAnsweredWithinTickWindow).toBe(false)
    expect(projection.tickWindows[1]?.externalContributorAnsweredInWindow).toBe(
      false,
    )
  })
})
