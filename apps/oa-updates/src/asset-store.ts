import { createHash } from "node:crypto"

export interface AssetStore {
  put(bytes: Uint8Array): Promise<{ hash: string; url: string }>
  get(hash: string): Promise<Uint8Array | null>
}

export function assetKeyFromBytes(bytes: Uint8Array): string {
  return createHash("sha256")
    .update(bytes)
    .digest("base64")
    .replaceAll("+", "-")
    .replaceAll("/", "_")
    .replaceAll("=", "")
}
