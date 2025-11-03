import type { Transport, TransportState } from './Transport'

export class NoopTransport implements Transport {
  private listeners = new Set<(evt: unknown) => void>()
  private _status: TransportState = 'closed'

  async connect(): Promise<void> {
    this._status = 'closed'
  }

  close(): void {
    this._status = 'closed'
  }

  send(_control: { name: string; args?: unknown }): void {
    // no-op
  }

  onMessage(cb: (evt: unknown) => void): () => void {
    this.listeners.add(cb)
    return () => { try { this.listeners.delete(cb) } catch {} }
  }

  status(): TransportState {
    return this._status
  }
}

