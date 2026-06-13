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

export function verifyAsset(bytes: Uint8Array, expectedHash: string): boolean {
  return assetKeyFromBytes(bytes) === expectedHash
}

export function createInMemoryAssetStore(baseUrl: string): AssetStore {
  const assets = new Map<string, Uint8Array>()

  return {
    async put(bytes) {
      const hash = assetKeyFromBytes(bytes)

      if (!assets.has(hash)) {
        assets.set(hash, Uint8Array.from(bytes))
      }

      return { hash, url: `${baseUrl}/assets/${hash}` }
    },

    async get(hash) {
      const bytes = assets.get(hash)

      return bytes ? Uint8Array.from(bytes) : null
    },
  }
}
