/**
 * MM-H2 (#8488): pure onboarding logic — the suggested first-task list and
 * thread-title derivation. No native/RN imports so this stays unit-testable
 * under `bun test`; the orchestration (create thread, bind repo, push the
 * first message + start-turn intent, navigate) lives in
 * `onboarding-flow.tsx`.
 */

export type OnboardingSuggestedTask = Readonly<{
  id: string
  label: string
  prompt: string
}>

/** The audit's own examples ("explain this codebase", "fix a TODO"), plus
 * one more in the same spirit — deliberately generic enough to make sense
 * against almost any repo a new user picks. */
export const ONBOARDING_SUGGESTED_TASKS: ReadonlyArray<OnboardingSuggestedTask> = [
  {
    id: "explain",
    label: "Explain this codebase",
    prompt: "Explain this codebase: what does it do, how is it structured, and what are the main entry points?",
  },
  {
    id: "fix-todo",
    label: "Fix a TODO",
    prompt: "Find a TODO comment in this codebase and fix it.",
  },
  {
    id: "add-test",
    label: "Add a test",
    prompt: "Find a function or module that is missing test coverage and add a test for it.",
  },
]

const MAX_DERIVED_TITLE_LENGTH = 80

/** A thread's title when it's created straight from an onboarding task
 * prompt (never a raw suggestion id) — trims and caps length so a long
 * custom task doesn't produce an unreadable sidebar row. */
export const deriveThreadTitleFromTask = (taskText: string): string => {
  const trimmed = taskText.trim()
  if (trimmed.length === 0) return "New chat"
  if (trimmed.length <= MAX_DERIVED_TITLE_LENGTH) return trimmed
  return `${trimmed.slice(0, MAX_DERIVED_TITLE_LENGTH - 1).trimEnd()}…`
}

export type OnboardingRepoBinding = Readonly<{ defaultBranch: string; name: string; owner: string }>

/** Whether the "Start" action should be blocked on a confirmed (not merely
 * unavailable/unknown) zero balance — one of the "honest states at every
 * fork" the issue asks for. Deliberately permissive when the balance can't
 * be determined at all (matches the rest of this lane's posture: never
 * block on missing data, only on a confirmed fact). */
export const blocksOnZeroBalance = (
  balance: Readonly<{ ok: true; value: number } | { ok: false }>,
): boolean => balance.ok && balance.value <= 0
