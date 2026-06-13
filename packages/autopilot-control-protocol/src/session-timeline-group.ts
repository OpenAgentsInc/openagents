export type SessionTimelineEvent = {
  phase: string
  messageText: string
  observedAt: string
}

export type SessionTimelineGroup = {
  phase: string
  count: number
  items: { messageText: string; observedAt: string }[]
}

export type SessionTimelineGroups = {
  groups: SessionTimelineGroup[]
  total: number
}

export function groupTimeline(events: SessionTimelineEvent[]): SessionTimelineGroups {
  const groups: SessionTimelineGroup[] = []

  for (const event of events) {
    const item = {
      messageText: event.messageText,
      observedAt: event.observedAt,
    }
    const currentGroup = groups.at(-1)

    if (currentGroup?.phase === event.phase) {
      currentGroup.items.push(item)
      currentGroup.count += 1
      continue
    }

    groups.push({
      phase: event.phase,
      count: 1,
      items: [item],
    })
  }

  return {
    groups,
    total: events.length,
  }
}
