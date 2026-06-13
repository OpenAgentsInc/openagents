export type OtaDirectiveDescription = {
  type: "noUpdateAvailable" | "rollBackToEmbedded" | "unknown"
  headline: string
}

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null

export function describeOtaDirective(raw: unknown): OtaDirectiveDescription {
  if (!isRecord(raw)) {
    return {
      type: "unknown",
      headline: "Unknown OTA directive",
    }
  }

  if (raw.type === "noUpdateAvailable") {
    return {
      type: "noUpdateAvailable",
      headline: "No OTA update available",
    }
  }

  if (raw.type === "rollBackToEmbedded") {
    return {
      type: "rollBackToEmbedded",
      headline: "Roll back to embedded app bundle",
    }
  }

  return {
    type: "unknown",
    headline: "Unknown OTA directive",
  }
}
