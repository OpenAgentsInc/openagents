import type { SyncTopic, WatermarkStore } from "./types";

export class MemoryWatermarkStore implements WatermarkStore {
  private readonly watermarks = new Map<SyncTopic, number>();

  constructor(initial: Readonly<Record<SyncTopic, number>> = {}) {
    for (const [topic, watermark] of Object.entries(initial)) {
      if (Number.isInteger(watermark) && watermark >= 0) {
        this.watermarks.set(topic, watermark);
      }
    }
  }

  async load(topics: ReadonlyArray<SyncTopic>): Promise<Readonly<Record<SyncTopic, number>>> {
    const next: Record<SyncTopic, number> = {};

    for (const topic of topics) {
      const value = this.watermarks.get(topic);
      if (typeof value === "number" && Number.isInteger(value) && value >= 0) {
        next[topic] = value;
      }
    }

    return next;
  }

  async save(topic: SyncTopic, watermark: number): Promise<void> {
    if (!Number.isInteger(watermark) || watermark < 0) return;
    this.watermarks.set(topic, watermark);
  }

  async clear(topic: SyncTopic): Promise<void> {
    this.watermarks.delete(topic);
  }
}
