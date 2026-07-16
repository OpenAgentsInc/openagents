import { createHash } from "node:crypto"

import type { ReleaseSetArtifact } from "../../openagents-desktop/src/release-set-contract.ts"
import type { ReleaseSetArtifactVerifier } from "./release-set-feed.ts"

const MAX_REDIRECTS = 0

/**
 * Production public-style verifier. It hashes the response incrementally,
 * refuses redirects (the signed URL is the transport truth), never forwards
 * credentials, and aborts as soon as the signed size is exceeded.
 */
export const createPublicReleaseSetArtifactVerifier = (input: Readonly<{
  fetch?: typeof fetch
  timeoutMs?: number
}> = {}): ReleaseSetArtifactVerifier => {
  const fetchFn = input.fetch ?? fetch
  const timeoutMs = input.timeoutMs ?? 10 * 60_000
  return async (artifact) => {
    const url = new URL(artifact.url)
    if (
      url.protocol !== "https:" || url.username !== "" || url.password !== "" ||
      url.search !== "" || url.hash !== "" || MAX_REDIRECTS !== 0
    ) throw new Error("artifact_url_invalid")
    const response = await fetchFn(url, {
      method: "GET",
      redirect: "error",
      signal: AbortSignal.timeout(timeoutMs),
      headers: { accept: "application/octet-stream" },
    })
    if (!response.ok || response.body === null) throw new Error("artifact_fetch_failed")
    const declaredLength = response.headers.get("content-length")
    if (
      declaredLength !== null &&
      (!/^\d+$/.test(declaredLength) || Number(declaredLength) !== artifact.byteLength)
    ) throw new Error("artifact_length_header_mismatch")
    const digest = createHash("sha256")
    let byteLength = 0
    const reader = response.body.getReader()
    try {
      while (true) {
        const part = await reader.read()
        if (part.done) break
        byteLength += part.value.byteLength
        if (byteLength > artifact.byteLength) {
          await reader.cancel("signed artifact size exceeded")
          throw new Error("artifact_body_oversized")
        }
        digest.update(part.value)
      }
    } catch (error) {
      if (error instanceof Error && error.message.startsWith("artifact_")) throw error
      throw new Error("artifact_body_read_failed")
    }
    return { byteLength, sha256: digest.digest("hex") }
  }
}

export const verifyPublicReleaseSetArtifact = async (
  artifact: ReleaseSetArtifact,
): Promise<{ byteLength: number; sha256: string }> =>
  createPublicReleaseSetArtifactVerifier()(artifact)
