import React from 'react'
import { View, Text, Pressable, ActivityIndicator, Platform } from 'react-native'
import * as Network from 'expo-network'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'

type Props = {
  onPick?: (hostPort: string) => void
  port?: number
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
    return fetch(url)
  }
}

function ipToCidr24(ip: string): string | null {
  const m = /^(\d+)\.(\d+)\.(\d+)\.(\d+)$/.exec(ip)
  if (!m) return null
  return `${m[1]}.${m[2]}.${m[3]}.`
}

export function LocalBridgeDiscovery({ onPick, port = 8787 }: Props) {
  const [scanning, setScanning] = React.useState(false)
  const [error, setError] = React.useState<string>('')
  const [results, setResults] = React.useState<string[]>([])
  const [ipBase, setIpBase] = React.useState<string | null>(null)

  React.useEffect(() => {
    (async () => {
      try {
        const ip = await Network.getIpAddressAsync()
        const base = ipToCidr24(ip || '')
        setIpBase(base)
      } catch {}
    })()
  }, [])

  const scan = React.useCallback(async () => {
    setError('')
    setResults([])
    if (!ipBase) { setError('No local IP'); return }
    setScanning(true)
    try {
      const candidates: string[] = []
      for (let i = 1; i <= 254; i++) candidates.push(`${ipBase}${i}`)
      const found: string[] = []
      const limit = 16
      let idx = 0
      async function runOne(ip: string) {
        try {
          const url = `http://${ip}:${port}`
          const res = await fetchWithTimeout(url, 800)
          if (res && (res.ok || res.status >= 100)) {
            found.push(ip)
            setResults((prev) => Array.from(new Set([...prev, ip])))
          }
        } catch {}
      }
      const workers = new Array(limit).fill(0).map(async () => {
        while (idx < candidates.length) {
          const ip = candidates[idx++]
          await runOne(ip)
        }
      })
      await Promise.all(workers)
      if (found.length === 0) setError('No bridges found on local /24')
    } catch (e: any) {
      setError(String(e?.message || e || 'scan failed'))
    } finally {
      setScanning(false)
    }
  }, [ipBase, port])

  return (
    <View style={{ marginTop: 8 }}>
      <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between' }}>
        <Text style={{ color: Colors.secondary, fontFamily: Typography.bold, fontSize: 12 }}>Scan Local Network for Bridges</Text>
        <Pressable onPress={() => scan()} disabled={scanning} style={{ paddingHorizontal: 8, paddingVertical: 4, borderWidth: 1, borderColor: Colors.border, opacity: scanning ? 0.6 : 1 }}>
          <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12 }}>{scanning ? 'Scanning…' : 'Scan'}</Text>
        </Pressable>
      </View>
      <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12, marginTop: 4 }}>
        {ipBase ? `Subnet: ${ipBase}0/24` : 'Subnet: unknown'}
      </Text>
      {scanning ? (
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8, paddingVertical: 8 }}>
          <ActivityIndicator color={Colors.secondary} />
          <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12 }}>This may take ~10–15 seconds…</Text>
        </View>
      ) : null}
      {!!error && (
        <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12, marginTop: 6 }}>{error}</Text>
      )}
      {results.length > 0 && (
        <View style={{ marginTop: 6, borderWidth: 1, borderColor: Colors.border, backgroundColor: Colors.card }}>
          {results.map((ip) => (
            <View key={ip} style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between', paddingHorizontal: 12, paddingVertical: 8, borderTopWidth: 1, borderTopColor: Colors.border }}>
              <Text style={{ color: Colors.foreground, fontFamily: Typography.primary, fontSize: 13 }}>{ip}:{port}</Text>
              <Pressable onPress={() => onPick?.(`${ip}:${port}`)} style={{ paddingHorizontal: 8, paddingVertical: 6, borderWidth: 1, borderColor: Colors.border }}>
                <Text style={{ color: Colors.foreground, fontFamily: Typography.bold, fontSize: 12 }}>Use</Text>
              </Pressable>
            </View>
          ))}
        </View>
      )}
      {Platform.OS === 'ios' && (
        <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 11, marginTop: 6 }}>
          iOS may prompt for Local Network access to scan your LAN.
        </Text>
      )}
    </View>
  )
}

export default LocalBridgeDiscovery

