const encoder = new TextEncoder()
const bufferOf = (bytes: Uint8Array): ArrayBuffer => bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer

export const sha256Hex = async (bytes: Uint8Array): Promise<string> => {
  const digest = await crypto.subtle.digest("SHA-256", bufferOf(bytes))
  return [...new Uint8Array(digest)].map((value) => value.toString(16).padStart(2, "0")).join("")
}

export interface EncryptedEnvelope {
  readonly bytes: Uint8Array
  readonly keyEpoch: string
}

export const encryptEnvelope = async (input: {
  readonly plaintext: Uint8Array
  readonly key: CryptoKey
  readonly keyEpoch: string
  readonly objectRef: string
}): Promise<EncryptedEnvelope> => {
  const wrappingNonce = crypto.getRandomValues(new Uint8Array(12))
  const dataNonce = crypto.getRandomValues(new Uint8Array(12))
  const dataKey = await crypto.subtle.generateKey({ name: "AES-GCM", length: 256 }, true, ["encrypt", "decrypt"])
  const rawDataKey = await crypto.subtle.exportKey("raw", dataKey)
  const wrappedDataKey = new Uint8Array(await crypto.subtle.encrypt(
    { name: "AES-GCM", iv: bufferOf(wrappingNonce), additionalData: bufferOf(encoder.encode(`${input.objectRef}:key`)) },
    input.key,
    rawDataKey,
  ))
  const ciphertext = await crypto.subtle.encrypt(
    { name: "AES-GCM", iv: bufferOf(dataNonce), additionalData: bufferOf(encoder.encode(input.objectRef)) },
    dataKey,
    bufferOf(input.plaintext),
  )
  const bytes = new Uint8Array(1 + 12 + 2 + wrappedDataKey.byteLength + 12 + ciphertext.byteLength)
  bytes[0] = 1
  bytes.set(wrappingNonce, 1)
  new DataView(bytes.buffer).setUint16(13, wrappedDataKey.byteLength)
  bytes.set(wrappedDataKey, 15)
  bytes.set(dataNonce, 15 + wrappedDataKey.byteLength)
  bytes.set(new Uint8Array(ciphertext), 27 + wrappedDataKey.byteLength)
  return { bytes, keyEpoch: input.keyEpoch }
}

export const decryptEnvelope = async (input: {
  readonly encrypted: Uint8Array
  readonly key: CryptoKey
  readonly objectRef: string
}): Promise<Uint8Array> => {
  if (input.encrypted[0] !== 1) throw new Error("unsupported encrypted envelope version")
  const wrappedLength = new DataView(input.encrypted.buffer, input.encrypted.byteOffset, input.encrypted.byteLength).getUint16(13)
  const rawDataKey = await crypto.subtle.decrypt(
    { name: "AES-GCM", iv: bufferOf(input.encrypted.slice(1, 13)), additionalData: bufferOf(encoder.encode(`${input.objectRef}:key`)) },
    input.key,
    bufferOf(input.encrypted.slice(15, 15 + wrappedLength)),
  )
  const dataKey = await crypto.subtle.importKey("raw", rawDataKey, { name: "AES-GCM" }, false, ["decrypt"])
  return new Uint8Array(await crypto.subtle.decrypt(
    { name: "AES-GCM", iv: bufferOf(input.encrypted.slice(15 + wrappedLength, 27 + wrappedLength)), additionalData: bufferOf(encoder.encode(input.objectRef)) },
    dataKey,
    bufferOf(input.encrypted.slice(27 + wrappedLength)),
  ))
}
