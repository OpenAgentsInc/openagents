type Pending = {
  readonly resolve: (value: any) => void
  readonly reject: (error: unknown) => void
}

export type CdpEvent = {
  readonly method: string
  readonly params?: any
}

export type CdpSession = {
  readonly send: <A = any>(method: string, params?: any) => Promise<A>
  readonly waitForEvent: (method: string, timeoutMs?: number) => Promise<CdpEvent>
  readonly close: () => Promise<void>
}

export async function connectCdp(wsUrl: string): Promise<CdpSession> {
  const ws = new WebSocket(wsUrl)
  await new Promise<void>((resolve, reject) => {
    ws.addEventListener("open", () => resolve(), { once: true })
    ws.addEventListener("error", (e) => reject(e), { once: true })
  })

  let nextId = 1
  const pending = new Map<number, Pending>()
  const waiters = new Map<string, Array<(event: CdpEvent) => void>>()

  ws.addEventListener("message", (e) => {
    const msg = JSON.parse(String(e.data))
    if (typeof msg.id === "number") {
      const p = pending.get(msg.id)
      if (!p) return
      pending.delete(msg.id)
      if (msg.error) p.reject(new Error(msg.error.message ?? "CDP error"))
      else p.resolve(msg.result)
      return
    }
    if (typeof msg.method === "string") {
      const event: CdpEvent = { method: msg.method, params: msg.params }
      const list = waiters.get(msg.method)
      if (list && list.length > 0) {
        const waiter = list.shift()
        waiter?.(event)
      }
      return
    }
  })

  const send = <A = any>(method: string, params?: any): Promise<A> =>
    new Promise<A>((resolve, reject) => {
      const id = nextId++
      pending.set(id, { resolve, reject })
      ws.send(JSON.stringify({ id, method, params }))
    })

  const waitForEvent = (method: string, timeoutMs = 30_000): Promise<CdpEvent> =>
    new Promise<CdpEvent>((resolve, reject) => {
      const onEvent = (event: CdpEvent) => {
        cleanup()
        resolve(event)
      }

      const list = waiters.get(method) ?? []
      list.push(onEvent)
      waiters.set(method, list)

      const t = setTimeout(() => {
        cleanup()
        reject(new Error(`Timeout waiting for CDP event: ${method}`))
      }, timeoutMs)

      const cleanup = () => {
        clearTimeout(t)
        const updated = waiters.get(method) ?? []
        waiters.set(
          method,
          updated.filter((w) => w !== onEvent),
        )
      }
    })

  const close = async () => {
    ws.close()
    await new Promise<void>((resolve) => ws.addEventListener("close", () => resolve(), { once: true }))
  }

  return { send, waitForEvent, close }
}
