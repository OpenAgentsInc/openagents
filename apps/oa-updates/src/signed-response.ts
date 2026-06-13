import { signManifest } from "./code-signing"

export type SignedManifestResponseInput = {
  manifest: unknown
  privateKeyPem: string
  keyid?: string
}

export type SignedManifestResponse = {
  body: string
  headers: Record<string, string>
}

const stableJsonValue = (value: unknown): unknown => {
  if (value === null || typeof value !== "object") {
    return value
  }

  if (Array.isArray(value)) {
    return value.map(stableJsonValue)
  }

  const jsonValue =
    "toJSON" in value && typeof value.toJSON === "function"
      ? value.toJSON()
      : value

  if (jsonValue !== value) {
    return stableJsonValue(jsonValue)
  }

  return Object.fromEntries(
    Object.entries(value)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, entryValue]) => [key, stableJsonValue(entryValue)]),
  )
}

export const buildSignedManifestResponse = ({
  manifest,
  privateKeyPem,
  keyid,
}: SignedManifestResponseInput): SignedManifestResponse => {
  const body = JSON.stringify(stableJsonValue(manifest))

  if (body === undefined) {
    throw new Error("Manifest must be JSON serializable")
  }

  return {
    body,
    headers: {
      "expo-signature": signManifest(body, privateKeyPem, keyid),
      "content-type": "application/json",
      "expo-protocol-version": "1",
      "expo-sfv-version": "0",
    },
  }
}
