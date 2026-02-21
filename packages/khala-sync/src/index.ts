export type {
  CachedDocument,
  KhalaFrame,
  KhalaClientError,
  KhalaClientOptions,
  KhalaClientStatus,
  KhalaSocketFactory,
  SyncErrorPayload,
  SyncTopic,
  SyncUpdate,
  SyncUpdateBatch,
  TopicWatermark,
  WatermarkStore,
  WebSocketLike,
} from "./types";

export { KhalaSyncClient } from "./khalaSyncClient";
export { MemoryWatermarkStore } from "./watermarkStore";
