function toHex(bytes: ArrayBuffer): string {
  const u8 = new Uint8Array(bytes)
  let out = ""
  for (let i = 0; i < u8.length; i++) out += u8[i]!.toString(16).padStart(2, "0")
  return out
}

export async function sha256HexString(input: string): Promise<string> {
  const data = new TextEncoder().encode(input)
  const digest = await crypto.subtle.digest("SHA-256", data)
  return toHex(digest)
}

export async function sha256IdFromString(input: string): Promise<string> {
  return `sha256:${await sha256HexString(input)}`
}

