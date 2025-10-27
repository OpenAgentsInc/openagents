export type PairPayload = {
  v: number
  type: string
  provider?: string
  bridge: string
  convex?: string
  token?: string | null
}

function b64urlDecode(s: string): string {
  const pad = s.length % 4 === 0 ? '' : '='.repeat(4 - (s.length % 4))
  const b64 = s.replace(/-/g, '+').replace(/_/g, '/') + pad
  try { return Buffer.from(b64, 'base64').toString('utf8') } catch { return '' }
}

export function parseBridgeCode(code: string): { bridgeHost?: string; convexUrl?: string; token?: string | null } | null {
  try {
    const raw = String(code || '').trim()
    if (!raw) return null
    let json = ''
    if (raw.startsWith('openagents://') || raw.startsWith('oa://')) {
      const u = new URL(raw)
      const j = u.searchParams.get('j') || u.searchParams.get('data') || ''
      json = b64urlDecode(j)
    } else if (/^[A-Za-z0-9_-]+$/.test(raw)) {
      json = b64urlDecode(raw)
    } else if (raw.startsWith('{')) {
      json = raw
    }
    if (!json) return null
    const obj = JSON.parse(json) as PairPayload
    if (!obj || typeof obj !== 'object') return null
    const out: { bridgeHost?: string; convexUrl?: string; token?: string | null } = {}
    if (obj.bridge && typeof obj.bridge === 'string') {
      try {
        const u = new URL(obj.bridge)
        const host = u.hostname
        const port = u.port || (u.protocol === 'wss:' ? '443' : '80')
        out.bridgeHost = `${host}:${port}`
      } catch {}
    }
    if (obj.convex && typeof obj.convex === 'string') {
      try {
        const u = new URL(obj.convex)
        if (u.protocol === 'http:' || u.protocol === 'https:') out.convexUrl = u.toString()
      } catch {}
    }
    if ('token' in obj) out.token = obj.token ?? null
    return (out.bridgeHost || out.convexUrl) ? out : null
  } catch {
    return null
  }
}

