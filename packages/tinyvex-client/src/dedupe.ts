import type { MessageRowTs } from '@openagents/bridge-types';

export class Dedupe {
  private byKey = new Map<string, MessageRowTs>();

  private key(row: MessageRowTs): string {
    const item = row.item_id ?? String(row.id);
    return `${row.thread_id}|${item}`;
  }

  merge(rows: MessageRowTs[]): MessageRowTs[] {
    for (const r of rows) {
      const k = this.key(r);
      const prev = this.byKey.get(k);
      if (!prev || (prev.updated_at ?? prev.ts) <= (r.updated_at ?? r.ts)) {
        this.byKey.set(k, r);
      }
    }
    return Array.from(this.byKey.values()).sort((a, b) => a.ts - b.ts);
  }

  stats() {
    return { total: this.byKey.size };
  }
}

