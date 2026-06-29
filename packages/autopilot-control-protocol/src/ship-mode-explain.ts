import { classifyByFingerprint } from "./fingerprint-classify.js"

export type ShipModeExplainInput = {
  prev: string | null
  next: string
}

export type ShipModeExplanation = {
  mode: "ota" | "rebuild" | "initial"
  headline: string
  detail: string
}

export function explainShipMode(input: ShipModeExplainInput): ShipModeExplanation {
  const result = classifyByFingerprint(input)

  if (result.mode === "initial") {
    return {
      mode: result.mode,
      headline: "Initial build required",
      detail: "No previous Expo Update fingerprint exists, so ship the first native build.",
    }
  }

  if (result.mode === "ota") {
    return {
      mode: result.mode,
      headline: "JavaScript-only OTA eligible",
      detail: "The Expo Update fingerprint is unchanged, so the update can ship over the existing native build.",
    }
  }

  return {
    mode: result.mode,
    headline: "Native rebuild required",
    detail: "The Expo Update fingerprint changed, so a new native build is required before shipping.",
  }
}
