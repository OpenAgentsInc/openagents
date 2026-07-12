export const AUDIO_EDGE_STREAM_PATH = '/v1/stream'

type AudioEdgeEnv = Readonly<{
  OPENAGENTS_AUDIO_CLOUD_RUN_URL?: string
  OPENAGENTS_AUDIO_EDGE_GOOGLE_CLIENT_EMAIL?: string
  OPENAGENTS_AUDIO_EDGE_GOOGLE_PRIVATE_KEY?: string
}>

const base64Url = (input: Uint8Array | string): string => {
  const bytes = typeof input === 'string' ? new TextEncoder().encode(input) : input
  let binary = ''; for (const byte of bytes) binary += String.fromCharCode(byte)
  return btoa(binary).replaceAll('+', '-').replaceAll('/', '_').replace(/=+$/u, '')
}

const privateKeyBytes = (pem: string): Uint8Array => {
  const body = pem.replace(/-----BEGIN PRIVATE KEY-----|-----END PRIVATE KEY-----|\s/gu, '')
  const binary = atob(body); return Uint8Array.from(binary, char => char.charCodeAt(0))
}

let cachedIdentity: Readonly<{ audience: string; token: string; expiresAtSeconds: number }> | undefined
export const mintAudioEdgeIdentityToken = async (env: AudioEdgeEnv, nowSeconds = Math.floor(Date.now() / 1_000)): Promise<string | undefined> => {
  const audience = env.OPENAGENTS_AUDIO_CLOUD_RUN_URL?.trim().replace(/\/$/u, '')
  const email = env.OPENAGENTS_AUDIO_EDGE_GOOGLE_CLIENT_EMAIL?.trim()
  const pem = env.OPENAGENTS_AUDIO_EDGE_GOOGLE_PRIVATE_KEY?.trim()
  if (!audience?.startsWith('https://') || !email || !pem) return undefined
  if (cachedIdentity?.audience === audience && cachedIdentity.expiresAtSeconds > nowSeconds + 60) return cachedIdentity.token
  try {
    const header = base64Url(JSON.stringify({ alg: 'RS256', typ: 'JWT' }))
    const expiresAtSeconds = nowSeconds + 3_600
    const claims = base64Url(JSON.stringify({ iss: email, sub: email, aud: 'https://oauth2.googleapis.com/token', iat: nowSeconds, exp: expiresAtSeconds, target_audience: audience }))
    const unsigned = `${header}.${claims}`
    const key = await crypto.subtle.importKey('pkcs8', privateKeyBytes(pem), { hash: 'SHA-256', name: 'RSASSA-PKCS1-v1_5' }, false, ['sign'])
    const signature = await crypto.subtle.sign('RSASSA-PKCS1-v1_5', key, new TextEncoder().encode(unsigned))
    const response = await fetch('https://oauth2.googleapis.com/token', { method: 'POST', headers: { 'content-type': 'application/x-www-form-urlencoded' }, body: new URLSearchParams({ assertion: `${unsigned}.${base64Url(new Uint8Array(signature))}`, grant_type: 'urn:ietf:params:oauth:grant-type:jwt-bearer' }) })
    if (!response.ok) return undefined
    const value = await response.json() as { id_token?: unknown; expires_in?: unknown }
    if (typeof value.id_token !== 'string' || value.id_token.length < 100 || value.id_token.length > 8_192) return undefined
    cachedIdentity = { audience, token: value.id_token, expiresAtSeconds: nowSeconds + (typeof value.expires_in === 'number' ? Math.min(value.expires_in, 3_600) : 3_600) }
    return value.id_token
  } catch { return undefined }
}

export const makeAudioEdgeProxy = (dependencies: Readonly<{
  identityToken: (env: AudioEdgeEnv) => Promise<string | undefined>
  fetchUpstream: typeof fetch
}>) => async (request: Request, env: AudioEdgeEnv): Promise<Response> => {
  if (request.method !== 'GET' || request.headers.get('upgrade')?.toLowerCase() !== 'websocket') return Response.json({ error: 'upgrade_required' }, { status: 426 })
  const grant = request.headers.get('x-openagents-audio-grant') ?? ''
  if (grant.length < 16 || grant.length > 4_096 || request.url.includes('?')) return Response.json({ error: 'invalid_audio_edge_request' }, { status: 400 })
  const origin = env.OPENAGENTS_AUDIO_CLOUD_RUN_URL?.trim().replace(/\/$/u, '')
  const token = await dependencies.identityToken(env)
  if (!origin?.startsWith('https://') || token === undefined) return Response.json({ error: 'audio_edge_unavailable' }, { status: 503 })
  const upstream = await dependencies.fetchUpstream(`${origin}${AUDIO_EDGE_STREAM_PATH}`, { headers: { Authorization: `Bearer ${token}`, Upgrade: 'websocket', 'x-openagents-audio-grant': grant } })
  return upstream.status === 101 ? upstream : Response.json({ error: 'audio_gateway_unavailable' }, { status: 502 })
}

export const handleAudioEdgeProxy = makeAudioEdgeProxy({ identityToken: mintAudioEdgeIdentityToken, fetchUpstream: fetch })
