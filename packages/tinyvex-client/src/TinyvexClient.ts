import type { Transport } from './Transport';
import type { Logger } from './logger';
import { silentLogger } from './logger';
import { Dedupe } from './dedupe';
import { LiveAggregator, type LiveState } from './aggregator';
import { resolveAlias } from './identity';
import type { BridgeEvent, MessageRowTs, TinyvexQueryResult, TinyvexSnapshot } from '@openagents/bridge-types';

export type EventsHandlers = {
  history: (rows: MessageRowTs[]) => void;
  live: (ev: LiveState) => void;
  debug?: (info: unknown) => void;
};

export class TinyvexClient {
  private dedupe = new Dedupe();
  private live = new LiveAggregator();
  private unsub: (() => void) | null = null;
  private canonicalThreadId: string | null = null;

  constructor(private t: Transport, private log: Logger = silentLogger) {}

  async init(idOrAlias: string) {
    await this.t.connect();
    const { thread_id } = await resolveAlias(idOrAlias);
    this.canonicalThreadId = thread_id;
    this.subscribeThread(thread_id);
    this.queryHistory(thread_id);
    return thread_id;
  }

  onEvents(handlers: EventsHandlers) {
    if (this.unsub) this.unsub();
    this.unsub = this.t.onMessage((evt: unknown) => this.handleEvent(evt as BridgeEvent, handlers));
    return () => {
      if (this.unsub) this.unsub();
      this.unsub = null;
    };
  }

  send(text: string, opts?: { resumeId?: 'last'; provider?: string }) {
    const thread_id = this.canonicalThreadId ?? undefined;
    this.t.send({ name: 'run.submit', args: { text, thread_id, ...opts } });
  }

  private subscribeThread(threadId: string) {
    this.t.send({ name: 'tvx.subscribe', args: { stream: 'messages', thread_id: threadId } });
  }
  private queryHistory(threadId: string) {
    this.t.send({ name: 'tvx.query', args: { name: 'messages.list', args: { thread_id: threadId, limit: 500 } } });
  }

  private handleEvent(evt: BridgeEvent, handlers: EventsHandlers) {
    switch ((evt as any).type) {
      case 'tinyvex.snapshot': {
        const snap = evt as TinyvexSnapshot<MessageRowTs>;
        if (snap.stream !== 'messages') return;
        const rows = snap.rows.filter((r) => r.kind === 'message' && !!r.role);
        handlers.history(this.dedupe.merge(rows));
        handlers.debug?.({ tag: 'snapshot', totals: this.dedupe.stats() });
        break;
      }
      case 'tinyvex.query_result': {
        const res = evt as TinyvexQueryResult<MessageRowTs>;
        if (!res.name.includes('messages')) return;
        const rows = res.rows.filter((r) => r.kind === 'message' && !!r.role);
        handlers.history(this.dedupe.merge(rows));
        handlers.debug?.({ tag: 'query_result', totals: this.dedupe.stats() });
        break;
      }
      case 'tinyvex.update': {
        // Persisted row changed; rely on client dedupe after requery or direct row payloads
        // Future: optionally trigger a targeted query.
        handlers.debug?.({ tag: 'update' });
        break;
      }
      default: {
        // Ephemeral ACP live updates may be surfaced via bridge-specific events (not standardized here)
        break;
      }
    }
    // Emit a snapshot of the live aggregator for UI live rendering
    handlers.live(this.live.snapshot());
  }
}

