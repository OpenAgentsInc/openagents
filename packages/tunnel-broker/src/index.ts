/**
 * Welcome to Cloudflare Workers! This is your first worker.
 *
 * - Run `npm run dev` in your terminal to start a development server
 * - Open a browser tab at http://localhost:8787/ to see your worker in action
 * - Run `npm run deploy` to publish your worker
 *
 * Bind resources to your worker in `wrangler.jsonc`. After adding bindings, a type definition for the
 * `Env` object can be regenerated with `npm run cf-typegen`.
 *
 * Learn more at https://developers.cloudflare.com/workers/
 */

interface Env {
  // Shared bearer for simple auth (set as a secret)
  BROKER_KEY: string
  // Cloudflare account + zone for API calls (set as secrets/vars)
  CF_ACCOUNT_ID: string
  CF_ZONE_ID: string
  CF_API_TOKEN: string
  // Base DNS suffix for issued hostnames (must be covered by Universal SSL wildcard), e.g. "openagents.com"
  TUNNEL_HOST_SUFFIX?: string
  // Optional prefix for hostnames to avoid collisions, e.g. "tunnel-"
  TUNNEL_HOST_PREFIX?: string
}

type Json = Record<string, unknown> | Array<unknown> | string | number | boolean | null

const json = (obj: Json, init: ResponseInit = {}) =>
  new Response(JSON.stringify(obj), { status: 200, headers: { 'content-type': 'application/json' }, ...init })

const notFound = () => new Response('not found', { status: 404 })
const bad = (msg: string, code = 400) => json({ error: msg }, { status: code })

export default {
  async fetch(request, env): Promise<Response> {
    const url = new URL(request.url)

    try {
      if (request.method === 'POST' && url.pathname === '/tunnels') {
        // One-command UX: allow public tunnel creation (no Authorization header required)
        const body = await safeBody(request)
        const deviceHint = typeof body?.deviceHint === 'string' ? String(body.deviceHint) : undefined
        const out = await createTunnelAndDns(env, deviceHint)
        // IMPORTANT: return token here for Tricoder; do not log it; clients must treat it as secret.
        return json(out)
      }
      // All other endpoints require Authorization unless BROKER_KEY is unset
      if (env.BROKER_KEY && !isAuthorized(request, env)) return new Response('unauthorized', { status: 401 })
      if (request.method === 'GET' && /^\/tunnels\//.test(url.pathname)) {
        const id = url.pathname.split('/')[2]
        if (!id) return bad('missing tunnelId')
        const out = await getTunnelStatus(env, id)
        return json(out)
      }
      if (request.method === 'DELETE' && /^\/tunnels\//.test(url.pathname)) {
        const id = url.pathname.split('/')[2]
        if (!id) return bad('missing tunnelId')
        await revokeTunnelAndDns(env, id)
        return json({ ok: true })
      }
      if (request.method === 'GET' && url.pathname === '/verify') {
        const v = await cfApi(env, ['user', 'tokens', 'verify'], { method: 'GET' })
        // Return limited fields for debugging scopes
        return json({ success: true, token: { status: v?.result?.status, expires_on: v?.result?.expires_on, policies: v?.result?.policies } })
      }
      return notFound()
    } catch (e: any) {
      return json({ error: String(e?.message || e) }, { status: 500 })
    }
  },
} satisfies ExportedHandler<Env>;

function isAuthorized(req: Request, env: Env): boolean {
  try {
    if (!env.BROKER_KEY) return true; // no key configured → open
    const h = req.headers.get('authorization') || ''
    if (!h.startsWith('Bearer ')) return false
    const token = h.slice(7).trim()
    return !!token && !!env.BROKER_KEY && token === env.BROKER_KEY
  } catch { return false }
}

async function safeBody(req: Request): Promise<any> {
  try { return await req.json() } catch { return {} }
}

function randLabel(n = 10): string {
  const alphabet = 'abcdefghijklmnopqrstuvwxyz0123456789'
  let out = ''
  for (let i = 0; i < n; i++) out += alphabet[Math.floor(Math.random() * alphabet.length)]
  return out
}

async function createTunnelAndDns(env: Env, deviceHint?: string): Promise<{ tunnelId: string, hostname: string, token: string, createdAt: string }> {
  const name = `tricoder-${deviceHint ? sanitize(deviceHint) + '-' : ''}${randLabel(6)}`.slice(0, 48)
  const suffix = env.TUNNEL_HOST_SUFFIX || 'openagents.com'
  const prefix = env.TUNNEL_HOST_PREFIX || 'tunnel-'
  // 1) Create tunnel
  const tRes = await cfApi(env, ['accounts', env.CF_ACCOUNT_ID, 'tunnels'], {
    method: 'POST',
    body: { name },
  }).catch(async () => {
    // Fallback to older endpoint path
    return cfApi(env, ['accounts', env.CF_ACCOUNT_ID, 'cfd_tunnel'], { method: 'POST', body: { name } })
  })
  const tunnelId = String(tRes?.result?.id || tRes?.result?.tunnel_id || tRes?.id)
  if (!tunnelId) throw new Error('failed to create tunnel')
  // 2) Create/assign connector token (API returns various shapes)
  let tokRes = await cfApi(env, ['accounts', env.CF_ACCOUNT_ID, 'tunnels', tunnelId, 'token'], { method: 'GET' }).catch(async () => {
    return cfApi(env, ['accounts', env.CF_ACCOUNT_ID, 'cfd_tunnel', tunnelId, 'token'], { method: 'GET' })
  })
  let token: string | undefined
  if (typeof tokRes?.result === 'string') token = tokRes.result
  else if (typeof tokRes?.result?.token === 'string') token = tokRes.result.token
  else if (typeof tokRes?.token === 'string') token = tokRes.token
  else if (typeof tokRes?.result?.connector_token === 'string') token = tokRes.result.connector_token
  // Some accounts require POST to mint/rotate a token
  if (!token) {
    tokRes = await cfApi(env, ['accounts', env.CF_ACCOUNT_ID, 'tunnels', tunnelId, 'token'], { method: 'POST' }).catch(async () => {
      return cfApi(env, ['accounts', env.CF_ACCOUNT_ID, 'cfd_tunnel', tunnelId, 'token'], { method: 'POST' })
    })
    if (typeof tokRes?.result === 'string') token = tokRes.result
    else if (typeof tokRes?.result?.token === 'string') token = tokRes.result.token
    else if (typeof tokRes?.token === 'string') token = tokRes.token
  }
  if (!token) throw new Error('failed to mint connector token')
  // 3) DNS CNAME <label>.<base> -> <id>.cfargotunnel.com
  const label = prefix + randLabel(10)
  const hostname = `${label}.${suffix}`
  const content = `${tunnelId}.cfargotunnel.com`
  await ensureDnsCname(env, hostname, content)
  // 4) Configure ingress so the hostname routes to the local bridge on 8787
  await setTunnelIngress(env, tunnelId, hostname).catch(() => {})
  const createdAt = new Date().toISOString()
  return { tunnelId, hostname, token, createdAt }
}

async function getTunnelStatus(env: Env, tunnelId: string): Promise<Record<string, unknown>> {
  const res = await cfApi(env, ['accounts', env.CF_ACCOUNT_ID, 'tunnels', tunnelId], { method: 'GET' }).catch(async () => {
    return cfApi(env, ['accounts', env.CF_ACCOUNT_ID, 'cfd_tunnel', tunnelId], { method: 'GET' })
  })
  const r = res?.result || res
  const conns: any[] = r?.connections || r?.conns || []
  const connected = Array.isArray(conns) && conns.length > 0
  return { tunnelId, connected, connections: conns }
}

async function revokeTunnelAndDns(env: Env, tunnelId: string): Promise<void> {
  // Delete DNS records pointing at <id>.cfargotunnel.com
  const content = `${tunnelId}.cfargotunnel.com`
  const dnsList = await cfApi(env, ['zones', env.CF_ZONE_ID, 'dns_records'], { method: 'GET', query: { type: 'CNAME', content } })
  const items: any[] = dnsList?.result || []
  for (const it of items) {
    try { await cfApi(env, ['zones', env.CF_ZONE_ID, 'dns_records', String(it.id)], { method: 'DELETE' }) } catch {}
  }
  await cfApi(env, ['accounts', env.CF_ACCOUNT_ID, 'tunnels', tunnelId], { method: 'DELETE' }).catch(async () => {
    await cfApi(env, ['accounts', env.CF_ACCOUNT_ID, 'cfd_tunnel', tunnelId], { method: 'DELETE' })
  })
}

async function ensureDnsCname(env: Env, name: string, content: string): Promise<void> {
  // If record already exists, do nothing; else create
  const existing = await cfApi(env, ['zones', env.CF_ZONE_ID, 'dns_records'], { method: 'GET', query: { name, type: 'CNAME' } })
  const found = (existing?.result || []).find((r: any) => r?.name === name)
  if (found) return
  await cfApi(env, ['zones', env.CF_ZONE_ID, 'dns_records'], {
    method: 'POST',
    body: { type: 'CNAME', name, content, ttl: 1, proxied: true },
  })
}

async function setTunnelIngress(env: Env, tunnelId: string, hostname: string): Promise<void> {
  const ingress = [
    { hostname, service: 'http://localhost:8787' },
    { service: 'http_status:404' },
  ]
  // Try new endpoint (tunnels) then legacy (cfd_tunnel), trying PUT then PATCH then POST
  const tryPaths: Array<[string[], string]> = [
    [['accounts', env.CF_ACCOUNT_ID, 'tunnels', tunnelId, 'configurations'], 'PUT'],
    [['accounts', env.CF_ACCOUNT_ID, 'tunnels', tunnelId, 'configurations'], 'PATCH'],
    [['accounts', env.CF_ACCOUNT_ID, 'tunnels', tunnelId, 'configurations'], 'POST'],
    [['accounts', env.CF_ACCOUNT_ID, 'cfd_tunnel', tunnelId, 'configurations'], 'PUT'],
    [['accounts', env.CF_ACCOUNT_ID, 'cfd_tunnel', tunnelId, 'configurations'], 'PATCH'],
    [['accounts', env.CF_ACCOUNT_ID, 'cfd_tunnel', tunnelId, 'configurations'], 'POST'],
  ]
  for (const [p, m] of tryPaths) {
    try { await cfApi(env, p, { method: m, body: { ingress } }); return } catch {}
  }
}

async function cfApi(env: Env, path: string[], init: { method?: string, body?: any, query?: Record<string, string> }) {
  const url = new URL('https://api.cloudflare.com/client/v4/')
  url.pathname += path.map(encodeURIComponent).join('/')
  if (init?.query) Object.entries(init.query).forEach(([k, v]) => url.searchParams.set(k, v))
  const headers: Record<string, string> = { 'content-type': 'application/json' }
  // Prefer API Token; fallback to Global API Key if provided
  if (env.CF_API_TOKEN) headers['authorization'] = `Bearer ${env.CF_API_TOKEN}`
  // @ts-ignore: allow optional binding
  const anyEnv: any = env as any
  if (anyEnv.CF_GLOBAL_API_KEY && anyEnv.CF_API_EMAIL) {
    headers['x-auth-email'] = String(anyEnv.CF_API_EMAIL)
    headers['x-auth-key'] = String(anyEnv.CF_GLOBAL_API_KEY)
  }
  const res = await fetch(url, { method: init?.method || 'GET', headers, body: init?.body ? JSON.stringify(init.body) : undefined })
  const data = await res.json<any>().catch(() => ({}))
  if (!res.ok || data?.success === false) {
    const msg = data?.errors?.[0]?.message || res.statusText || 'cf api error'
    const code = data?.errors?.[0]?.code
    const detail = data?.errors?.[0]?.error || undefined
    throw new Error(`cloudflare api: ${msg}${code ? ` (code ${code})` : ''}${detail ? `: ${detail}` : ''}`)
  }
  return data
}

function sanitize(s: string): string {
  return s.toLowerCase().replace(/[^a-z0-9-]/g, '').replace(/--+/g, '-').replace(/^-+|-+$/g, '')
}
