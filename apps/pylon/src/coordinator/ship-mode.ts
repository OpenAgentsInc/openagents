export type ShipMode = "ota" | "rebuild" | "none"

export type ShipModeInput = {
  deployedFingerprint: string | null
  currentFingerprint: string
  hasMobileChange: boolean
}

export function classifyShipMode(input: ShipModeInput): ShipMode {
  if (!input.hasMobileChange) {
    return "none"
  }

  if (input.deployedFingerprint === input.currentFingerprint) {
    return "ota"
  }

  return "rebuild"
}

export function explain(input: ShipModeInput): string {
  const mode = classifyShipMode(input)

  switch (mode) {
    case "none":
      return "No mobile change detected; no app shipment is required."
    case "ota":
      return `Mobile change detected and runtime fingerprint matches deployed fingerprint (${input.currentFingerprint}); ship via EAS Update OTA.`
    case "rebuild":
      if (input.deployedFingerprint === null) {
        return `Mobile change detected but no deployed runtime fingerprint is available; a new native build is required for current fingerprint ${input.currentFingerprint}.`
      }

      return `Mobile change detected and runtime fingerprint changed from ${input.deployedFingerprint} to ${input.currentFingerprint}; a new native build is required.`
  }
}
