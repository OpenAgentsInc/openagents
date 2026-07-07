import type {
  ContainerFetchInit,
  ContainerPathFetch,
} from './http/container-fetch'
import { optionalMdkContainerSecret } from './mdk-container-env'

// CFG-15 (EPIC #8515): config-driven HTTP endpoints for the MDK money-path
// services (treasury, tips buffer, checkout sidecar).
//
// Today the Worker reaches these daemons through Cloudflare Containers via
// Durable Object `containerFetch`. The Workers Paid plan was cancelled, so the
// daemons are being evacuated to Google Cloud Run. When one of the
// `MDK_*_SERVICE_URL` config vars is set, the Worker calls the daemon over
// plain HTTPS with the existing per-service token header instead of the DO
// path. The same seam is what the CFG-9 Cloud Run monolith uses — it has no DO
// bindings at all, so the HTTP mode is its only route to the money daemons.
//
// SAFETY: setting a service URL re-points ALL Worker traffic for that daemon.
// The production treasury cutover is owner-gated; see
// docs/cloud/2026-07-06-mdk-treasury-cloudrun-cutover-runbook.md. Never point
// the URL at a daemon running the production mnemonic while the Cloudflare
// container for the same mnemonic can still be woken — two live daemons on one
// mnemonic is a fund-loss scenario.

export type MdkServiceHttpFetch = (
  input: Request | string,
  init?: RequestInit,
) => Promise<Response>

/**
 * Validate and normalize a configured MDK service base URL.
 *
 * Returns the base URL without a trailing slash, or `undefined` when the
 * value is missing, unparsable, or uses a non-HTTPS scheme (plain HTTP is
 * allowed only for loopback hosts so local smoke stacks keep working).
 */
export const mdkServiceHttpBaseUrl = (
  rawUrl: string | undefined,
): string | undefined => {
  const value = optionalMdkContainerSecret(rawUrl)

  if (value === undefined) {
    return undefined
  }

  let parsed: URL

  try {
    parsed = new URL(value)
  } catch {
    return undefined
  }

  const loopback =
    parsed.hostname === 'localhost' || parsed.hostname === '127.0.0.1'

  if (parsed.protocol !== 'https:' && !(parsed.protocol === 'http:' && loopback)) {
    return undefined
  }

  return value.replace(/\/+$/u, '')
}

/**
 * Path-based fetch against an HTTP MDK daemon (treasury / tips buffer),
 * mirroring the Durable Object `ContainerPathFetch` contract: JSON content
 * type plus the per-service token header on every request.
 */
export const makeMdkServiceHttpPathFetch = (options: {
  readonly baseUrl: string
  readonly serviceToken?: string | undefined
  readonly serviceTokenHeader: string
  readonly fetchImpl?: MdkServiceHttpFetch | undefined
}): ContainerPathFetch => {
  const fetchImpl = options.fetchImpl ?? fetch

  return (path: string, init?: ContainerFetchInit) => {
    // Cloud Run's Google Frontend reserves `/healthz` on `run.app` domains
    // and answers 404 before the request reaches the container; the MDK
    // daemons serve `/health` as the alias (CFG-15 runbook). Rewrite the
    // health-probe path so the HTTP seam's health checks reach the daemon.
    const daemonPath = path === '/healthz' ? '/health' : path

    return fetchImpl(
      new Request(`${options.baseUrl}${daemonPath}`, {
        ...(init?.body === undefined ? {} : { body: init.body }),
        headers: {
          'content-type': 'application/json',
          ...(options.serviceToken === undefined
            ? {}
            : { [options.serviceTokenHeader]: options.serviceToken }),
        },
        method: init?.method ?? 'GET',
        ...(init?.signal === undefined ? {} : { signal: init.signal }),
      }),
    )
  }
}

export const MDK_SIDECAR_SERVICE_TOKEN_HEADER = 'x-mdk-sidecar-service-token'

/**
 * Forward a full inbound Worker request (path, query, method, body, MDK
 * checkout headers) to an HTTP MDK sidecar. Used for the `/api/mdk` checkout
 * route where the daemon parses the request itself. Adds the sidecar service
 * token so a publicly-routable Cloud Run daemon can reject non-Worker
 * callers.
 */
export const makeMdkSidecarHttpRequestForward = (options: {
  readonly baseUrl: string
  readonly serviceToken?: string | undefined
  readonly fetchImpl?: MdkServiceHttpFetch | undefined
}): ((request: Request) => Promise<Response>) => {
  const fetchImpl = options.fetchImpl ?? fetch

  return (request: Request) => {
    const url = new URL(request.url)
    const headers = new Headers(request.headers)

    headers.delete('host')

    if (options.serviceToken !== undefined) {
      headers.set(MDK_SIDECAR_SERVICE_TOKEN_HEADER, options.serviceToken)
    }

    return fetchImpl(
      new Request(`${options.baseUrl}${url.pathname}${url.search}`, {
        body:
          request.method === 'GET' || request.method === 'HEAD'
            ? undefined
            : request.body,
        headers,
        method: request.method,
        // Streaming request bodies require an explicit duplex mode on fetch.
        ...(request.method === 'GET' || request.method === 'HEAD'
          ? {}
          : { duplex: 'half' as const }),
      } as RequestInit),
    )
  }
}
