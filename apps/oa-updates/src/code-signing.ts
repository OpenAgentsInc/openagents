import { createSign, createVerify } from "node:crypto"

export type ExpoSignatureHeader = {
  sig: string
  keyid: string
  alg: string
}

const EXPO_SIGNATURE_ALG = "rsa-v1_5-sha256"
const DEFAULT_KEY_ID = "main"

const toBytes = (value: Uint8Array | string): Uint8Array =>
  typeof value === "string" ? Buffer.from(value, "utf8") : value

const quoteSfvString = (value: string): string => {
  if (/[\x00-\x1f\x7f]/u.test(value)) {
    throw new Error("Structured field string contains a control character")
  }

  return `"${value.replaceAll("\\", "\\\\").replaceAll('"', '\\"')}"`
}

export const signManifest = (
  manifestBytes: Uint8Array | string,
  privateKeyPem: string,
  keyid = DEFAULT_KEY_ID,
): string => {
  const signer = createSign("RSA-SHA256")
  signer.update(toBytes(manifestBytes))
  signer.end()

  const sig = signer.sign(privateKeyPem).toString("base64")
  return `sig=${quoteSfvString(sig)}, keyid=${quoteSfvString(
    keyid,
  )}, alg=${quoteSfvString(EXPO_SIGNATURE_ALG)}`
}

export const parseSignatureHeader = (
  headerValue: string,
): ExpoSignatureHeader => {
  const parsed = new Map<string, string>()
  let index = 0

  const skipWhitespace = (): void => {
    while (headerValue[index] === " " || headerValue[index] === "\t") {
      index += 1
    }
  }

  const parseToken = (): string => {
    const start = index
    while (/[A-Za-z0-9_*.-]/u.test(headerValue[index] ?? "")) {
      index += 1
    }

    if (index === start) {
      throw new Error("Expected structured field dictionary key")
    }

    return headerValue.slice(start, index)
  }

  const parseString = (): string => {
    if (headerValue[index] !== '"') {
      throw new Error("Expected structured field string value")
    }

    index += 1
    let value = ""

    while (index < headerValue.length) {
      const char = headerValue[index]
      index += 1

      if (char === '"') {
        return value
      }

      if (char === "\\") {
        if (index >= headerValue.length) {
          throw new Error("Invalid structured field escape")
        }

        const escaped = headerValue[index]
        index += 1

        if (escaped !== '"' && escaped !== "\\") {
          throw new Error("Invalid structured field escape")
        }

        value += escaped
        continue
      }

      if (char < " " || char === "\x7f") {
        throw new Error("Invalid structured field string character")
      }

      value += char
    }

    throw new Error("Unterminated structured field string")
  }

  while (index < headerValue.length) {
    skipWhitespace()
    const key = parseToken()
    skipWhitespace()

    if (headerValue[index] !== "=") {
      throw new Error("Expected structured field dictionary assignment")
    }

    index += 1
    skipWhitespace()
    parsed.set(key, parseString())
    skipWhitespace()

    if (index >= headerValue.length) {
      break
    }

    if (headerValue[index] !== ",") {
      throw new Error("Expected structured field dictionary separator")
    }

    index += 1
  }

  const sig = parsed.get("sig")
  const keyid = parsed.get("keyid")
  const alg = parsed.get("alg")

  if (sig === undefined || keyid === undefined || alg === undefined) {
    throw new Error("Missing required expo-signature fields")
  }

  return { sig, keyid, alg }
}

export const verifyManifestSignature = (
  manifestBytes: Uint8Array | string,
  headerValue: string,
  publicKeyPem: string,
): boolean => {
  const { sig, alg } = parseSignatureHeader(headerValue)

  if (alg !== EXPO_SIGNATURE_ALG) {
    return false
  }

  const verifier = createVerify("RSA-SHA256")
  verifier.update(toBytes(manifestBytes))
  verifier.end()

  return verifier.verify(publicKeyPem, Buffer.from(sig, "base64"))
}
