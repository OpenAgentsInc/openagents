import type { WorkerEnv } from "./env"

const demoPayload = (path: string, requestId: string) => ({
  source: "openagents.ep212",
  route: path,
  generatedAt: "2026-02-12T00:00:00.000Z",
  requestId,
  signals: [
    {
      symbol: "BTC",
      horizon: "4h",
      confidence: 0.8123,
      direction: "up",
    },
    {
      symbol: "BTC",
      horizon: "24h",
      confidence: 0.6674,
      direction: "neutral",
    },
  ],
})

const json = (status: number, body: unknown): Response =>
  new Response(JSON.stringify(body), {
    status,
    headers: {
      "content-type": "application/json; charset=utf-8",
      "cache-control": "no-store",
    },
  })

const methodNotAllowed = (): Response =>
  json(405, {
    ok: false,
    error: "method_not_allowed",
  })

const requestIdFrom = (request: Request): string => {
  const value = request.headers.get("x-oa-request-id")?.trim()
  return value && value.length > 0 ? value : "unknown"
}

export const handleEp212DemoRoutes = async (
  request: Request,
  _env: WorkerEnv,
): Promise<Response | null> => {
  const url = new URL(request.url)
  const requestId = requestIdFrom(request)

  if (url.pathname === "/ep212/premium-signal") {
    if (request.method !== "GET") return methodNotAllowed()
    return json(200, {
      ok: true,
      ...demoPayload(url.pathname, requestId),
      tier: "under-cap",
      note: "Designed for paid-success route behind Aperture L402.",
    })
  }

  if (url.pathname === "/ep212/expensive-signal") {
    if (request.method !== "GET") return methodNotAllowed()
    return json(200, {
      ok: true,
      ...demoPayload(url.pathname, requestId),
      tier: "over-cap",
      note: "Designed for quoted-over-cap policy block rehearsals.",
    })
  }

  return null
}
