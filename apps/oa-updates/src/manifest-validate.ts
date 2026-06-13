export type ManifestValidationResult = {
  readonly ok: boolean
  readonly missing: string[]
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function hasNonEmptyString(record: Record<string, unknown>, key: string): boolean {
  return typeof record[key] === "string" && record[key].trim().length > 0
}

export function validateExpoManifest(m: unknown): ManifestValidationResult {
  const missing: string[] = []

  if (!isRecord(m)) {
    return {
      ok: false,
      missing: [
        "id",
        "createdAt",
        "runtimeVersion",
        "launchAsset",
        "assets",
        "metadata",
        "extra",
      ],
    }
  }

  for (const key of ["id", "createdAt", "runtimeVersion"]) {
    if (!hasNonEmptyString(m, key)) {
      missing.push(key)
    }
  }

  if (!isRecord(m.launchAsset)) {
    missing.push("launchAsset")
  } else {
    for (const key of ["key", "contentType", "url"]) {
      if (!hasNonEmptyString(m.launchAsset, key)) {
        missing.push(`launchAsset.${key}`)
      }
    }
  }

  if (!Array.isArray(m.assets)) {
    missing.push("assets")
  }

  if (!isRecord(m.metadata)) {
    missing.push("metadata")
  }

  if (!isRecord(m.extra)) {
    missing.push("extra")
  }

  return {
    ok: missing.length === 0,
    missing,
  }
}
