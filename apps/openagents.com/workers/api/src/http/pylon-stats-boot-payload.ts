import { Effect } from 'effect'

import { parseJsonUnknown } from '../json-boundary'
import { handlePublicPylonStatsApi } from '../public-pylon-stats-routes'

export const PYLON_STATS_BOOT_SCRIPT_ID = 'openagents-pylon-stats-snapshot'

type PublicPylonStatsRouteEnv = Parameters<typeof handlePublicPylonStatsApi>[1]
type AssetEnv = Readonly<{
  ASSETS: { fetch: (request: Request) => Response | Promise<Response> }
}>

type PylonStatsBootEnv = AssetEnv & PublicPylonStatsRouteEnv

const shouldInjectPylonStatsBootPayload = (request: Request): boolean => {
  if (request.method !== 'GET') return false

  // The pylon-stats snapshot is only needed by the Pylon scene, which now lives
  // at /pylons. The root `/` is the landing 3D scene and must NOT carry this
  // ~90KB no-store payload — injecting it at `/` made the homepage large AND
  // uncacheable (cache-control: no-store below), which was the slow first load.
  const pathname = new URL(request.url).pathname
  return pathname === '/pylons'
}

const responseIsHtml = (response: Response): boolean =>
  response.headers
    .get('content-type')
    ?.toLowerCase()
    .includes('text/html') === true

const escapeJsonForHtml = (json: string): string =>
  json.replaceAll('<', '\\u003c')

export const pylonStatsBootScript = (snapshotJson: string): string =>
  `<script id="${PYLON_STATS_BOOT_SCRIPT_ID}" type="application/json">${escapeJsonForHtml(snapshotJson)}</script>`

export const injectPylonStatsBootPayload = (
  html: string,
  snapshotJson: string,
): string => {
  if (
    html.includes(`id="${PYLON_STATS_BOOT_SCRIPT_ID}"`) ||
    html.includes(`id='${PYLON_STATS_BOOT_SCRIPT_ID}'`)
  ) {
    return html
  }

  const script = pylonStatsBootScript(snapshotJson)
  const lower = html.toLowerCase()
  const marker = '</body>'
  const index = lower.lastIndexOf(marker)

  if (index < 0) return `${html}\n${script}`

  return `${html.slice(0, index)}${script}\n${html.slice(index)}`
}

const envCanReadPublicPylonStats = (env: PublicPylonStatsRouteEnv): boolean =>
  (env as { OPENAGENTS_DB?: D1Database }).OPENAGENTS_DB !== undefined

const loadPublicPylonStatsSnapshotJson = async (
  request: Request,
  env: PublicPylonStatsRouteEnv,
): Promise<string | null> => {
  if (!envCanReadPublicPylonStats(env)) return null

  const url = new URL('/api/public/pylon-stats', request.url)
  const response = await Effect.runPromise(
    handlePublicPylonStatsApi(
      new Request(url, {
        headers: { accept: 'application/json' },
        method: 'GET',
      }),
      env,
    ),
  )

  if (!response.ok) return null

  const text = await response.text()

  try {
    const parsed = parseJsonUnknown(text)
    if (typeof parsed !== 'object' || parsed === null || Array.isArray(parsed)) {
      return null
    }
    return JSON.stringify(parsed)
  } catch {
    return null
  }
}

export const injectPylonStatsBootPayloadIntoAssetResponse = async (
  request: Request,
  env: PublicPylonStatsRouteEnv,
  assetResponse: Response,
  loadSnapshotJson: () => Promise<string | null> = () =>
    loadPublicPylonStatsSnapshotJson(request, env),
): Promise<Response> => {
  if (
    !shouldInjectPylonStatsBootPayload(request) ||
    !assetResponse.ok ||
    !responseIsHtml(assetResponse)
  ) {
    return assetResponse
  }

  const snapshotJson = await loadSnapshotJson()
  if (snapshotJson === null) return assetResponse

  const headers = new Headers(assetResponse.headers)
  headers.delete('content-length')
  headers.set('cache-control', 'no-store')

  return new Response(
    injectPylonStatsBootPayload(await assetResponse.text(), snapshotJson),
    {
      headers,
      status: assetResponse.status,
      statusText: assetResponse.statusText,
    },
  )
}

export const fetchAppShellWithPylonStatsBootPayload = async (
  request: Request,
  env: PylonStatsBootEnv,
): Promise<Response> => {
  const assetResponse = await env.ASSETS.fetch(request)
  return injectPylonStatsBootPayloadIntoAssetResponse(
    request,
    env,
    assetResponse,
  )
}
