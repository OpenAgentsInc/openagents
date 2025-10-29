import net from 'node:net'

export async function isPortAvailable(port: number, host = '0.0.0.0'): Promise<boolean> {
  return new Promise((resolve) => {
    const srv = net.createServer()
    const onError = (err: any) => {
      try { srv.close(); } catch {}
      if (err && (err as any).code === 'EADDRINUSE') return resolve(false)
      return resolve(false)
    }
    srv.once('error', onError)
    srv.listen({ port, host, exclusive: true }, () => {
      srv.close(() => resolve(true))
    })
  })
}

export async function findAvailablePort(start: number, maxTries = 20, host = '0.0.0.0'): Promise<number> {
  for (let i = 0; i < maxTries; i++) {
    const p = start + i
    // eslint-disable-next-line no-await-in-loop
    const ok = await isPortAvailable(p, host)
    if (ok) return p
  }
  throw new Error(`No free port in range ${start}-${start + maxTries - 1}`)
}

