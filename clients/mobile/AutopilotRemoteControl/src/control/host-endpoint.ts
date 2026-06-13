export type DevClientPlatform = "ios" | "android"

export type HostControlServerInput = {
  platform: DevClientPlatform
  port: number
}

export type AuthedControlRequestInput = {
  baseUrl: string
  devToken: string
  path?: string
  method?: string
  headers?: HeadersInit
  body?: BodyInit | Record<string, unknown> | null
}

export function resolveHostControlServerBaseUrl(input: HostControlServerInput): string {
  assertValidPort(input.port)

  const host = input.platform === "android" ? "10.0.2.2" : "127.0.0.1"
  return `http://${host}:${input.port}`
}

export function buildAuthedControlRequest(input: AuthedControlRequestInput): Request {
  const url = joinBaseUrlAndPath(input.baseUrl, input.path ?? "")
  const headers = new Headers(input.headers)
  headers.set("Authorization", `Bearer ${input.devToken}`)

  let body = input.body
  if (isJsonBody(body)) {
    body = JSON.stringify(body)
    if (!headers.has("content-type")) headers.set("content-type", "application/json")
  }

  return new Request(url, {
    method: input.method ?? (body == null ? "GET" : "POST"),
    headers,
    body: body as BodyInit | null | undefined,
  })
}

function assertValidPort(port: number): void {
  if (!Number.isInteger(port) || port < 1 || port > 65_535) {
    throw new Error(`Invalid control server port: ${port}`)
  }
}

function joinBaseUrlAndPath(baseUrl: string, path: string): string {
  const trimmedBase = baseUrl.replace(/\/+$/, "")
  const trimmedPath = path.replace(/^\/+/, "")
  return trimmedPath.length > 0 ? `${trimmedBase}/${trimmedPath}` : trimmedBase
}

function isJsonBody(body: AuthedControlRequestInput["body"]): body is Record<string, unknown> {
  if (body == null || typeof body !== "object") return false
  if (body instanceof ArrayBuffer) return false
  if (typeof Blob !== "undefined" && body instanceof Blob) return false
  if (typeof FormData !== "undefined" && body instanceof FormData) return false
  if (typeof URLSearchParams !== "undefined" && body instanceof URLSearchParams) return false
  if (typeof ReadableStream !== "undefined" && body instanceof ReadableStream) return false

  return true
}
