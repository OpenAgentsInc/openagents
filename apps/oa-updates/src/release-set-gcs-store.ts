import type {
  ReleaseSetCandidate,
  ReleaseSetChannel,
  ReleaseSetFeedStore,
  ReleaseSetPointer,
} from "./release-set-feed.ts"

const STORED_CANDIDATE_LIMIT = 2 * 1024 * 1024
const STORED_POINTER_LIMIT = 16 * 1024

type StoredCandidateDocument = Readonly<{
  schema: "openagents.desktop.release_candidate.v2"
  channel: ReleaseSetChannel
  generation: string
  payloadBase64: string
  signatureBase64: string
}>

type AccessToken = Readonly<{ access_token: string; expires_in: number }>

const readBounded = async (response: Response, limit: number): Promise<Uint8Array> => {
  if (!response.ok || response.body === null) throw new Error(`storage_response_${response.status}`)
  const reader = response.body.getReader()
  const chunks: Uint8Array[] = []
  let length = 0
  while (true) {
    const part = await reader.read()
    if (part.done) break
    length += part.value.byteLength
    if (length > limit) {
      await reader.cancel("storage response limit exceeded")
      throw new Error("storage_response_oversized")
    }
    chunks.push(part.value)
  }
  const bytes = new Uint8Array(length)
  let offset = 0
  for (const chunk of chunks) {
    bytes.set(chunk, offset)
    offset += chunk.byteLength
  }
  return bytes
}

const parseJson = <T>(bytes: Uint8Array): T => {
  try {
    return JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes)) as T
  } catch {
    throw new Error("storage_document_invalid")
  }
}

const candidateName = (channel: ReleaseSetChannel, generation: string): string =>
  `desktop/release-set-v2/${channel}/candidates/${generation}.json`
const pointerName = (channel: ReleaseSetChannel): string =>
  `desktop/release-set-v2/${channel}/pointer.json`

export const createGoogleCloudReleaseSetFeedStore = (input: Readonly<{
  bucket: string
  fetch?: typeof fetch
  token?: () => Promise<string>
}>): ReleaseSetFeedStore => {
  if (!/^[a-z0-9][a-z0-9._-]{1,221}[a-z0-9]$/.test(input.bucket)) {
    throw new Error("release feed bucket invalid")
  }
  const fetchFn = input.fetch ?? fetch
  let cachedToken: { value: string; expiresAt: number } | null = null
  const token = input.token ?? (async () => {
    if (cachedToken !== null && Date.now() < cachedToken.expiresAt) return cachedToken.value
    const response = await fetchFn(
      "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token",
      { headers: { "metadata-flavor": "Google" }, signal: AbortSignal.timeout(5_000) },
    )
    const value = parseJson<AccessToken>(await readBounded(response, 8 * 1024))
    if (typeof value.access_token !== "string" || !Number.isFinite(value.expires_in)) {
      throw new Error("storage_token_invalid")
    }
    cachedToken = {
      value: value.access_token,
      expiresAt: Date.now() + Math.max(30, value.expires_in - 60) * 1_000,
    }
    return value.access_token
  })
  const authHeaders = async (): Promise<Record<string, string>> => ({
    authorization: `Bearer ${await token()}`,
  })
  const mediaUrl = (name: string): string =>
    `https://storage.googleapis.com/storage/v1/b/${encodeURIComponent(input.bucket)}/o/${encodeURIComponent(name)}?alt=media`
  const uploadUrl = (name: string, generation: string): string => {
    const query = new URLSearchParams({
      uploadType: "media",
      name,
      ifGenerationMatch: generation,
    })
    return `https://storage.googleapis.com/upload/storage/v1/b/${encodeURIComponent(input.bucket)}/o?${query}`
  }
  const get = async (name: string, limit: number): Promise<Uint8Array | null> => {
    const response = await fetchFn(mediaUrl(name), { headers: await authHeaders() })
    if (response.status === 404) return null
    return readBounded(response, limit)
  }
  const put = async (
    name: string,
    bytes: Uint8Array,
    generation: string,
  ): Promise<"created" | "precondition_failed"> => {
    const response = await fetchFn(uploadUrl(name, generation), {
      method: "POST",
      headers: {
        ...await authHeaders(),
        "cache-control": "no-store",
        "content-type": "application/json",
      },
      body: Uint8Array.from(bytes),
    })
    if (response.status === 412) return "precondition_failed"
    if (!response.ok) throw new Error(`storage_write_${response.status}`)
    return "created"
  }
  const readPointerWithGeneration = async (
    channel: ReleaseSetChannel,
  ): Promise<{ pointer: ReleaseSetPointer; generation: string } | null> => {
    const response = await fetchFn(mediaUrl(pointerName(channel)), { headers: await authHeaders() })
    if (response.status === 404) return null
    const generation = response.headers.get("x-goog-generation")
    if (generation === null || !/^\d+$/.test(generation)) {
      throw new Error("storage_generation_missing")
    }
    const pointer = parseJson<ReleaseSetPointer>(await readBounded(response, STORED_POINTER_LIMIT))
    return { pointer, generation }
  }
  const decodeCandidate = (bytes: Uint8Array): ReleaseSetCandidate => {
    const document = parseJson<StoredCandidateDocument>(bytes)
    if (
      document.schema !== "openagents.desktop.release_candidate.v2" ||
      (document.channel !== "stable" && document.channel !== "rc") ||
      !/^[0-9a-f]{64}$/.test(document.generation)
    ) throw new Error("storage_candidate_invalid")
    const payloadBytes = Uint8Array.from(Buffer.from(document.payloadBase64, "base64"))
    const signatureBytes = Uint8Array.from(Buffer.from(document.signatureBase64, "base64"))
    const releaseSet = parseJson<ReleaseSetCandidate["releaseSet"]>(payloadBytes)
    const signature = parseJson<ReleaseSetCandidate["signature"]>(signatureBytes)
    return { ...document, releaseSet, signature, payloadBytes, signatureBytes }
  }
  const encodeCandidate = (candidate: ReleaseSetCandidate): Uint8Array =>
    new TextEncoder().encode(JSON.stringify({
      schema: "openagents.desktop.release_candidate.v2",
      channel: candidate.channel,
      generation: candidate.generation,
      payloadBase64: Buffer.from(candidate.payloadBytes).toString("base64"),
      signatureBase64: Buffer.from(candidate.signatureBytes).toString("base64"),
    } satisfies StoredCandidateDocument))

  return {
    async readCandidate(channel, generation) {
      const bytes = await get(candidateName(channel, generation), STORED_CANDIDATE_LIMIT)
      return bytes === null ? null : decodeCandidate(bytes)
    },
    async createCandidate(candidate) {
      const name = candidateName(candidate.channel, candidate.generation)
      const bytes = encodeCandidate(candidate)
      if (bytes.byteLength > STORED_CANDIDATE_LIMIT) throw new Error("storage_candidate_oversized")
      if (await put(name, bytes, "0") === "created") return "created"
      const existing = await get(name, STORED_CANDIDATE_LIMIT)
      return existing !== null && Buffer.from(existing).equals(Buffer.from(bytes))
        ? "exists"
        : "conflict"
    },
    async readPointer(channel) {
      return (await readPointerWithGeneration(channel))?.pointer ?? null
    },
    async compareAndSwapPointer(channel, expectedRevision, next) {
      const current = await readPointerWithGeneration(channel)
      if ((current?.pointer.revision ?? null) !== expectedRevision) return false
      const bytes = new TextEncoder().encode(JSON.stringify(next))
      return await put(pointerName(channel), bytes, current?.generation ?? "0") === "created"
    },
    async listCandidateGenerations(channel) {
      const prefix = `desktop/release-set-v2/${channel}/candidates/`
      const query = new URLSearchParams({ prefix, fields: "items(name),nextPageToken" })
      const response = await fetchFn(
        `https://storage.googleapis.com/storage/v1/b/${encodeURIComponent(input.bucket)}/o?${query}`,
        { headers: await authHeaders() },
      )
      const listing = parseJson<{ items?: { name?: string }[]; nextPageToken?: string }>(
        await readBounded(response, 1024 * 1024),
      )
      if (listing.nextPageToken !== undefined) throw new Error("storage_listing_truncated")
      return (listing.items ?? []).flatMap((item) => {
        const name = item.name ?? ""
        const match = name.match(/\/([0-9a-f]{64})\.json$/)
        return match === null ? [] : [match[1]]
      }).toSorted()
    },
  }
}
