import { classifyByFingerprint } from "./fingerprint-classify.js"

export type ShipFingerprintHistoryEntry = {
  fingerprint: string
  at: string
}

export type ShipFingerprintDrift = {
  currentFingerprint: string | null
  lastNativeChangeAt: string | null
  otaSafeSince: string | null
  changes: number
}

export function detectFingerprintDrift(history: ShipFingerprintHistoryEntry[]): ShipFingerprintDrift {
  let currentFingerprint: string | null = null
  let lastNativeChangeAt: string | null = null
  let otaSafeSince: string | null = null
  let changes = 0

  for (const entry of history) {
    const result = classifyByFingerprint({
      prev: currentFingerprint,
      next: entry.fingerprint,
    })

    if (currentFingerprint === null) {
      currentFingerprint = entry.fingerprint
      otaSafeSince = entry.at
      continue
    }

    if (result.changed) {
      changes += 1
      currentFingerprint = entry.fingerprint
      lastNativeChangeAt = entry.at
      otaSafeSince = entry.at
    }
  }

  return {
    currentFingerprint,
    lastNativeChangeAt,
    otaSafeSince,
    changes,
  }
}
