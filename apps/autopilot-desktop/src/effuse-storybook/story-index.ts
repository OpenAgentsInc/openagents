/**
 * Story Index / Discovery
 * Uses Vite's import.meta.glob to find stories
 */

import { parseCsfModule } from "./csf/parse"
import type { CsfFile, CsfStory } from "./csf/csf"

// Registry of all parsed stories
export const storyRegistry = new Map<string, CsfStory>()
// Registry of all parsed files (meta)
export const fileRegistry = new Map<string, CsfFile>()

let isInitialized = false

/**
 * Discovers and loads all stories.
 * In a real app with many stories, we might want to load metadata lazily,
 * but for v1 with import.meta.glob eager=false, we still need to load modules
 * to parse their Meta/exports unless we write a custom Vite plugin.
 *
 * For this implementation, we will assume we load all story modules to build the index.
 * This is "eager" indexing even if import is dynamic.
 */
export async function loadStories() {
  if (isInitialized) return

  // Glob for story files
  // Note: Adjust pattern if needed. using absolute path from root usually works in Vite.
  // or relative to this file.
  // We use a broad pattern.
  const modules = import.meta.glob("/src/**/*.stories.{ts,tsx}", {
    eager: true, // Eager for v1 to simplify indexing (no async lazy loading of index)
  })

  for (const [path, mod] of Object.entries(modules)) {
    try {
      const csfFile = parseCsfModule(path, mod)
      fileRegistry.set(path, csfFile)

      for (const story of csfFile.stories) {
        storyRegistry.set(story.id, story)
      }
    } catch (e) {
      console.error(`Failed to parse story: ${path}`, e)
    }
  }

  isInitialized = true
  console.log(
    `[Storybook] Loaded ${storyRegistry.size} stories from ${fileRegistry.size} files`
  )
}

export function getAllStories(): CsfStory[] {
  return Array.from(storyRegistry.values())
}

export function getStory(id: string): CsfStory | undefined {
  return storyRegistry.get(id)
}
