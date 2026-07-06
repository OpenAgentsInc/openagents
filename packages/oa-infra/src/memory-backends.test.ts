/**
 * Conformance runs for the in-memory backends (CFG-2, issue #8517). These
 * always run — no external infrastructure required.
 */
import * as BlobStoreMemory from "./blob-store-memory.ts"
import * as DurableStreamMemory from "./durable-stream-memory.ts"
import * as JobQueueMemory from "./job-queue-memory.ts"
import * as KvStoreMemory from "./kv-store-memory.ts"
import * as MutexMemory from "./mutex-memory.ts"
import { runBlobStoreConformance } from "./conformance/blob-store.ts"
import { runDurableStreamConformance } from "./conformance/durable-stream.ts"
import { runJobQueueConformance } from "./conformance/job-queue.ts"
import { runKvStoreConformance } from "./conformance/kv-store.ts"
import { runMutexConformance } from "./conformance/mutex.ts"

runKvStoreConformance({ label: "memory", makeLayer: KvStoreMemory.layerMemory })
runBlobStoreConformance({ label: "memory", makeLayer: BlobStoreMemory.layerMemory })
runJobQueueConformance({ label: "memory", makeLayer: JobQueueMemory.layerMemory })
runDurableStreamConformance({ label: "memory", makeLayer: DurableStreamMemory.layerMemory })
runMutexConformance({ label: "memory", makeLayer: MutexMemory.layerMemory })
