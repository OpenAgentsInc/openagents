export type OtaAssetRef = {
  key: string
  contentType: string
  url: string
  fileExtension: string | null
}

const normalizeRequiredString = (value: unknown): string | null => {
  if (typeof value !== "string") {
    return null
  }

  const normalized = value.trim()
  return normalized.length > 0 ? normalized : null
}

const normalizeOptionalString = (value: unknown): string | null => {
  if (typeof value !== "string") {
    return null
  }

  const normalized = value.trim()
  return normalized.length > 0 ? normalized : null
}

export function normalizeAsset(raw: unknown): OtaAssetRef | null {
  if (raw === null || typeof raw !== "object" || Array.isArray(raw)) {
    return null
  }

  const asset = raw as Record<string, unknown>
  const key = normalizeRequiredString(asset.key)
  const contentType = normalizeRequiredString(asset.contentType)
  const url = normalizeRequiredString(asset.url)

  if (key === null || contentType === null || url === null) {
    return null
  }

  return {
    key,
    contentType,
    url,
    fileExtension: normalizeOptionalString(asset.fileExtension),
  }
}
