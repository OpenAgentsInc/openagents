import type { Transport, TransportState } from './Transport';

export type WsConfig = { url: string; token?: string };

export class WsTransport implements Transport {
  private ws: WebSocket | null = null;
  private listeners = new Set<(evt: unknown) => void>();
  private _status: TransportState = 'closed';
  private url: string;

  constructor(cfg: WsConfig) {
    const u = new URL(cfg.url);
    if (cfg.token) u.searchParams.set('token', cfg.token);
    this.url = u.toString();
  }

  async connect(): Promise<void> {
    if (this.ws && this._status === 'open') return;
    this._status = 'connecting';
    await new Promise<void>((resolve, reject) => {
      const ws = new WebSocket(this.url);
      this.ws = ws;
      const to = setTimeout(() => {
        try { ws.close(); } catch {}
        this._status = 'error';
        reject(new Error('ws timeout'));
      }, 2000);
      ws.onopen = () => {
        clearTimeout(to);
        this._status = 'open';
        resolve();
      };
      ws.onmessage = (ev) => {
        try {
          const data = typeof ev.data === 'string' ? JSON.parse(ev.data) : ev.data;
          for (const cb of this.listeners) cb(data);
        } catch {
          // ignore parse errors
        }
      };
      ws.onerror = () => {
        clearTimeout(to);
        this._status = 'error';
        reject(new Error('ws error'));
      };
      ws.onclose = () => {
        clearTimeout(to);
        this._status = 'closed';
        reject(new Error('ws closed'));
      };
    });
  }

  close(): void {
    if (this.ws) {
      try { this.ws.close(); } catch {}
      this.ws = null;
    }
    this._status = 'closed';
  }

  send(control: { name: string; args?: unknown }): void {
    if (!this.ws || this._status !== 'open') return;
    const payload = { control: control.name, ...(control.args ? control.args : {}) };
    try { this.ws.send(JSON.stringify(payload)); } catch {}
  }

  onMessage(cb: (evt: unknown) => void): () => void {
    this.listeners.add(cb);
    return () => this.listeners.delete(cb);
  }

  status(): TransportState {
    return this._status;
  }
}
