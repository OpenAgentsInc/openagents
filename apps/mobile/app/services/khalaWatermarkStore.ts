import type { SyncTopic, WatermarkStore } from "@openagentsinc/khala-sync"

import { loadString, remove, saveString } from "@/utils/storage"

const KEY_PREFIX = "khala.sync.watermark."

const keyForTopic = (topic: SyncTopic): string => `${KEY_PREFIX}${topic}`

export class MobileKhalaWatermarkStore implements WatermarkStore {
  async load(topics: ReadonlyArray<SyncTopic>): Promise<Readonly<Record<SyncTopic, number>>> {
    const values: Record<SyncTopic, number> = {}

    for (const topic of topics) {
      const raw = loadString(keyForTopic(topic))
      if (!raw) {
        continue
      }

      const parsed = Number.parseInt(raw, 10)
      if (Number.isInteger(parsed) && parsed >= 0) {
        values[topic] = parsed
      }
    }

    return values
  }

  async save(topic: SyncTopic, watermark: number): Promise<void> {
    if (!Number.isInteger(watermark) || watermark < 0) {
      return
    }

    saveString(keyForTopic(topic), String(watermark))
  }

  async clear(topic: SyncTopic): Promise<void> {
    remove(keyForTopic(topic))
  }
}
