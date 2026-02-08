import { autopilotStories } from "./stories/autopilot"

import type { Story, StoryMeta } from "./types"

const byTitle = (a: Story, b: Story) => a.title.localeCompare(b.title)

export const allStories: ReadonlyArray<Story> = [...autopilotStories].sort(byTitle)

export const listStoryMeta = (): ReadonlyArray<StoryMeta> =>
  allStories.map((s) => ({ id: s.id, title: s.title, kind: s.kind }))

export const getStoryById = (id: string): Story | null => {
  for (const s of allStories) if (s.id === id) return s
  return null
}

