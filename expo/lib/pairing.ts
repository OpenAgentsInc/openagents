export type PairPayload = {
  v: number
  type: string
  provider?: string
  bridge: string
  convex?: string
  token?: string | null
}

// Tiny base64url decoder with no Node Buffer dependency
function b64urlDecode(s: string): string {
  try {
    const pad = s.length % 4 === 0 ? '' : '='.repeat(4 - (s.length % 4))
    const b64 = (s || '').replace(/-/g, '+').replace(/_/g, '/') + pad
    const atobFn: ((x: string) => string) | undefined = (globalThis as any).atob
    if (typeof atobFn === 'function') {
      const bin = atobFn(b64)
      let out = ''
      for (let i = 0; i < bin.length; i++) out += String.fromCharCode(bin.charCodeAt(i) & 0xff)
      try { return decodeURIComponent(escape(out)) } catch { return out }
    }
    // Manual decoder
    const table = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/'
    const clean = b64.replace(/[^A-Za-z0-9+/=]/g, '')
    let bytes: number[] = []
    let i = 0
    while (i < clean.length) {
      const enc1 = table.indexOf(clean.charAt(i++))
      const enc2 = table.indexOf(clean.charAt(i++))
      const enc3 = table.indexOf(clean.charAt(i++))
      const enc4 = table.indexOf(clean.charAt(i++))
      const chr1 = (enc1 << 2) | (enc2 >> 4)
      const chr2 = ((enc2 & 15) << 4) | (enc3 >> 2)
      const chr3 = ((enc3 & 3) << 6) | enc4
      bytes.push(chr1 & 0xff)
      if (enc3 !== 64 && clean.charAt(i - 2) !== '=') bytes.push(chr2 & 0xff)
      if (enc4 !== 64 && clean.charAt(i - 1) !== '=') bytes.push(chr3 & 0xff)
    }
    let out = ''
    for (let j = 0; j < bytes.length; j++) out += String.fromCharCode(bytes[j])
    try { return decodeURIComponent(escape(out)) } catch { return out }
  } catch { return '' }
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
