export type FingerprintClassifyInput = {
  prev: string | null
  next: string
}

export type FingerprintClassifyResult = {
  mode: "ota" | "rebuild" | "initial"
  changed: boolean
  reason: string
}

export function classifyByFingerprint(input: FingerprintClassifyInput): FingerprintClassifyResult {
  if (input.prev === null) {
    return {
      mode: "initial",
      changed: true,
      reason: "no previous Expo Update fingerprint is available",
    }
  }

  if (input.prev === input.next) {
    return {
      mode: "ota",
      changed: false,
      reason: "Expo Update fingerprint is unchanged; JavaScript-only OTA is eligible",
    }
  }

  return {
    mode: "rebuild",
    changed: true,
    reason: "Expo Update fingerprint changed; native rebuild is required",
  }
}
