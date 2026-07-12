import type { LiveProofJournalEntry, LiveProofStepName } from "./live-proof.ts"
import { requiredLiveProofSteps } from "./live-proof.ts"

export type LiveProofJournalVerdict = Readonly<{
  ok: boolean
  failed: ReadonlyArray<LiveProofStepName>
  missing: ReadonlyArray<LiveProofStepName>
}>

/**
 * The persisted journal is the acceptance authority, not the Electron child
 * exit code. A crashed/closed window can otherwise return zero after required
 * failures or before the summary is written.
 */
export const liveProofJournalVerdict = (
  journal: ReadonlyArray<LiveProofJournalEntry>,
): LiveProofJournalVerdict => {
  const required = requiredLiveProofSteps()
  const failed: Array<LiveProofStepName> = []
  const missing: Array<LiveProofStepName> = []
  for (const step of required) {
    const entries = journal.filter(entry => entry.step === step)
    if (entries.length === 0) missing.push(step)
    else if (!entries.some(entry => entry.ok)) failed.push(step)
  }
  return { failed, missing, ok: failed.length === 0 && missing.length === 0 }
}
