import React from 'react'
import { View, Text, Pressable, ActivityIndicator, Platform } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'

type PeerItem = {
  id: string
  name: string
  ips: string[]
  online?: boolean
}

async function fetchWithTimeout(url: string, ms: number): Promise<Response> {
  try {
    const ctrl = new AbortController()
    const id = setTimeout(() => { try { ctrl.abort() } catch {} }, ms)
    try {
      const res = await fetch(url, { signal: (ctrl as any).signal })
      clearTimeout(id)
      return res
    } catch (e) {
      clearTimeout(id)
      throw e
    }
  } catch (e) {
    // RN older versions may not support AbortController; fallback without abort
    return fetch(url)
  }
}

function parsePeersFromStatus(status: any): PeerItem[] {
  try {
    const peers: PeerItem[] = []
    const peerMap = status?.Peer || status?.Peers || status?.peer || {}
    if (peerMap && typeof peerMap === 'object') {
      for (const [key, val] of Object.entries<any>(peerMap)) {
        const name = String(
          val?.DNSName || val?.dnsName || val?.HostName || val?.hostName || val?.Hostinfo?.HostName || ''
        ).replace(/\.$/, '')
        const ipsRaw: any[] = (val?.TailscaleIPs || val?.tailscaleIPs || val?.Addresses || val?.addresses || []) as any[]
        const ips: string[] = []
        for (const ip of ipsRaw) {
          const s = typeof ip === 'string' ? ip : (ip?.IP || ip?.ip || ip?.Addr || ip?.addr || '')
          if (!s) continue
          const cleaned = String(s).replace(/\/(?:32|128)$/, '')
          if (cleaned && !ips.includes(cleaned)) ips.push(cleaned)
        }
        const online = !!(val?.Online || val?.online || val?.Active || val?.active)
        peers.push({ id: key, name: name || key.slice(0, 8), ips, online })
      }
    }
    // Fallback: if no peers map, try Self info as a single device entry
    if (peers.length === 0 && status?.Self) {
      const s = status.Self
      const name = String(s?.DNSName || s?.HostName || '').replace(/\.$/, '') || 'This device'
      const ipsRaw: any[] = (s?.TailscaleIPs || s?.Addresses || []) as any[]
      const ips: string[] = []
      for (const ip of ipsRaw) {
        const t = typeof ip === 'string' ? ip : (ip?.IP || ip?.Addr || '')
        if (!t) continue
        const cleaned = String(t).replace(/\/(?:32|128)$/, '')
        if (cleaned && !ips.includes(cleaned)) ips.push(cleaned)
      }
      peers.push({ id: 'self', name, ips, online: true })
    }
    return peers
  } catch {
    return []
  }
}

export function TailscalePeers() {
  const [loading, setLoading] = React.useState(false)
  const [error, setError] = React.useState<string>('')
  const [peers, setPeers] = React.useState<PeerItem[]>([])

  const load = React.useCallback(async () => {
    setError('')
    setLoading(true)
    try {
      if (Platform.OS === 'ios') {
        // Tailscale LocalAPI is not accessible to third-party apps on iOS
        throw new Error('ios_localapi_unavailable')
      }
      // Best-effort LocalAPI probe per docs; may be unavailable on iOS or newer clients
      const res = await fetchWithTimeout('http://localhost:41112/localapi/v0/status', 1500)
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      const json = await res.json()
      const items = parsePeersFromStatus(json)
      setPeers(items)
    } catch (e: any) {
      const msg = String(e?.message || e || 'unavailable')
      setError(msg)
      setPeers([])
    } finally {
      setLoading(false)
    }
  }, [])

  React.useEffect(() => {
    load().catch(() => {})
  }, [load])

  const header = (
    <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between', marginTop: 8 }}>
      <Text style={{ color: Colors.secondary, fontFamily: Typography.bold, fontSize: 12 }}>Tailscale peers on this device</Text>
      <Pressable onPress={() => load()} accessibilityRole='button' style={{ paddingHorizontal: 8, paddingVertical: 4, borderWidth: 1, borderColor: Colors.border }}>
        <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12 }}>Refresh</Text>
      </Pressable>
    </View>
  )

  if (loading) {
    return (
      <View style={{ marginTop: 8, gap: 8 }}>
        {header}
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8, paddingVertical: 8 }}>
          <ActivityIndicator color={Colors.secondary} />
          <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12 }}>Checking LocalAPI…</Text>
        </View>
      </View>
    )
  }

  if (error) {
    return (
      <View style={{ marginTop: 8, gap: 6 }}>
        {header}
        <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12 }}>
          {error === 'ios_localapi_unavailable'
            ? 'LocalAPI unavailable on iOS for third‑party apps. Use Settings to connect by IP, or set up a registry/backend for discovery.'
            : `LocalAPI not accessible (${error}). On Android it may work; on iOS it is often restricted.`}
        </Text>
      </View>
    )
  }

  if (!peers || peers.length === 0) {
    return (
      <View style={{ marginTop: 8, gap: 6 }}>
        {header}
        <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12 }}>No peers found.</Text>
      </View>
    )
  }

  return (
    <View style={{ marginTop: 8 }}>
      {header}
      <View style={{ marginTop: 6, borderWidth: 1, borderColor: Colors.border, backgroundColor: Colors.card }}>
        {peers.map((p, idx) => (
          <View key={p.id} style={{ paddingHorizontal: 12, paddingVertical: 8, borderTopWidth: idx === 0 ? 0 : 1, borderTopColor: Colors.border }}>
            <Text style={{ color: Colors.foreground, fontFamily: Typography.bold, fontSize: 13 }}>{p.name}</Text>
            <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12 }}>{p.ips.join(', ') || 'No IPs'}</Text>
          </View>
        ))}
      </View>
    </View>
  )
}

export default TailscalePeers
