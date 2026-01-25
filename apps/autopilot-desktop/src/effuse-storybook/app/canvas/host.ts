/**
 * Canvas Host: Renders the selected story into the DOM
 */

import { Effect } from "effect"
import { StorybookService } from "../state"
import { getStory } from "../../story-index"
import { composeStory, resolveRenderOutput } from "./render"
import type { StoryContext } from "../../csf/csf"

export const CanvasHost = {
  // Renders the selected story into the provided container
  render: (container: HTMLElement) =>
    Effect.gen(function* () {
      const service = yield* StorybookService
      const state = yield* service.get

      if (!state.selectedStoryId) {
        container.innerHTML = `<div class="flex h-full items-center justify-center text-sm italic text-muted-foreground">Select a story to preview</div>`
        return
      }

      const story = getStory(state.selectedStoryId)
      if (!story) {
        container.innerHTML = `<div class="p-4 text-sm text-destructive">Story not found: ${state.selectedStoryId}</div>`
        return
      }

      // Prepare Context
      const context: StoryContext = {
        id: story.id,
        name: story.name,
        title: story.title,
        args: state.currentArgs, // Use live args from state
        parameters: story.parameters,
        viewMode: "story",
      }

      const result = yield* Effect.tryPromise(async () => {
        const renderFn = composeStory(story)
        const output = await Promise.resolve().then(() => renderFn(context))
        return resolveRenderOutput(output)
      }).pipe(
        Effect.catchAll((error) =>
          Effect.sync(() => {
            console.error("Story rendering failed:", error)
            container.innerHTML = `<div class="p-4 text-sm text-destructive">
              <h3>Render Error</h3>
              <pre>${String((error as Error)?.message ?? error)}</pre>
              <pre>${String((error as Error)?.stack ?? "")}</pre>
            </div>`
            return null
          })
        )
      )

      if (!result) {
        return
      }

      const { html, swapMode } = result

      // Render to DOM
      // TODO: Use Effuse's real DomService for swapping/morphing if available.
      if (swapMode === "inner") {
        container.innerHTML = html
      } else if (swapMode === "outer") {
        // Dangerous/Tricky for root canvas, usually inner is safer here
        container.innerHTML = html
      } else {
        container.innerHTML = html
      }

      // Run Play Function if exists
      // We only run play on initial mount of the story or explicit run,
      // but this effect runs on every render (arg update).
      // Storybook usually doesn't re-run play on arg updates.
      // We'll skip play here for now and move it to a dedicated effect controlled by state/buttons.
      // OR: checks if this is a "fresh" mount.
      // For v1: ignoring play in the main render loop.
    }),
}
