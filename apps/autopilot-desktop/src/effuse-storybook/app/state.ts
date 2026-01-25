/**
 * Storybook Overlay State Management
 */

import { Effect, Context, Layer, Ref, Stream } from "effect"
import { getAllStories, getStory, loadStories } from "../story-index"
import type { CsfStory } from "../csf/csf"

// --- Domain Models ---

export interface StorybookState {
  isOpen: boolean
  selectedStoryId: string | null
  currentArgs: Record<string, any>
  stories: CsfStory[]
}

// --- Service Definition ---

export class StorybookService extends Context.Tag("StorybookService")<
  StorybookService,
  {
    readonly state$: Stream.Stream<StorybookState>
    readonly get: Effect.Effect<StorybookState>
    readonly toggle: Effect.Effect<void>
    readonly selectStory: (id: string) => Effect.Effect<void>
    readonly updateArgs: (args: Record<string, any>) => Effect.Effect<void>
    readonly resetArgs: Effect.Effect<void>
  }
>() {}

// --- Implementation ---

const make = Effect.gen(function* () {
  const stateRef = yield* Ref.make<StorybookState>({
    isOpen: false,
    selectedStoryId: null,
    currentArgs: {},
    stories: [],
  })

  // Initialize stories
  yield* Effect.promise(() => loadStories())
  const initialStories = getAllStories()
  yield* Ref.update(stateRef, (s) => ({ ...s, stories: initialStories }))

  const state$ = Stream.fromEffect(Ref.get(stateRef))

  // NOTE: In a real Effuse app, we'd use Effuse's StateService or Cell.
  // For this standalone module, let's expose a subscription mechanism or hook into the existing one.
  // Since we are integrating INTO Effuse, let's stick to simple Effect patterns first.

  const toggle = Ref.update(stateRef, (s) => ({ ...s, isOpen: !s.isOpen }))

  const selectStory = (id: string) =>
    Ref.update(stateRef, (s) => {
      const story = getStory(id)
      return {
        ...s,
        selectedStoryId: id,
        currentArgs: story ? { ...story.args } : {},
      }
    })

  const updateArgs = (args: Record<string, any>) =>
    Ref.update(stateRef, (s) => ({
      ...s,
      currentArgs: { ...s.currentArgs, ...args },
    }))

  const resetArgs = Ref.update(stateRef, (s) => {
    if (!s.selectedStoryId) return s
    const story = getStory(s.selectedStoryId)
    return {
      ...s,
      currentArgs: story ? { ...story.args } : {},
    }
  })

  // Helper to get current state (for UI rendering)
  const get = Ref.get(stateRef)

  return {
    state$,
    get,
    toggle,
    selectStory,
    updateArgs,
    resetArgs,
  }
})

export const StorybookServiceLive = Layer.effect(StorybookService, make)
