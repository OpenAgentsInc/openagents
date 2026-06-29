// Composer history + stash persistence (issue #4741), the staged subset of
// opencode's prompt/history.tsx and prompt/stash.tsx: submitted prompts and
// an unsent draft survive restarts via composer-history.json in the Pylon
// home directory.

import { mkdir, writeFile } from "node:fs/promises"
import { join } from "node:path"

export const composerStoreFileName = "composer-history.json"
export const maxComposerHistory = 50
// Drafts shorter than this are not worth stashing (opencode's threshold).
export const minStashLength = 20

export interface ComposerPersistedState {
  history: string[]
  stash: string
}

export function composerStorePath(homeDir: string): string {
  return join(homeDir, composerStoreFileName)
}

export async function loadComposerState(homeDir: string): Promise<ComposerPersistedState> {
  const file = Bun.file(composerStorePath(homeDir))
  if (!(await file.exists())) return { history: [], stash: "" }
  try {
    const parsed = JSON.parse(await file.text()) as Partial<ComposerPersistedState>
    return {
      history: Array.isArray(parsed.history)
        ? parsed.history.filter((entry): entry is string => typeof entry === "string").slice(-maxComposerHistory)
        : [],
      stash: typeof parsed.stash === "string" ? parsed.stash : "",
    }
  } catch {
    return { history: [], stash: "" }
  }
}

export async function saveComposerState(homeDir: string, state: ComposerPersistedState): Promise<void> {
  await mkdir(homeDir, { recursive: true })
  const bounded: ComposerPersistedState = {
    history: state.history.slice(-maxComposerHistory),
    stash: state.stash.length >= minStashLength ? state.stash : "",
  }
  await writeFile(composerStorePath(homeDir), `${JSON.stringify(bounded, null, 2)}\n`, "utf8")
}

export function pushHistory(history: string[], prompt: string): string[] {
  const trimmed = prompt.trim()
  if (!trimmed) return history
  const without = history.filter((entry) => entry !== trimmed)
  return [...without, trimmed].slice(-maxComposerHistory)
}
