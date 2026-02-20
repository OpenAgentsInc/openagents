export type SyncTopic = string;

export type TopicWatermark = Readonly<{
  topic: SyncTopic;
  watermark: number;
}>;

export type SyncUpdate = Readonly<{
  topic: SyncTopic;
  doc_key: string;
  doc_version: number;
  payload?: unknown;
  payload_hash?: string | null;
  watermark: number;
  hydration_required?: boolean;
}>;

export type SyncUpdateBatch = Readonly<{
  updates: ReadonlyArray<SyncUpdate>;
  replay_complete?: boolean;
  head_watermarks?: ReadonlyArray<TopicWatermark>;
}>;

export type SyncErrorPayload = Readonly<{
  code?: string;
  message?: string;
  full_resync_required?: boolean;
  stale_topics?: ReadonlyArray<
    Readonly<{
      topic: SyncTopic;
      resume_after?: number;
      retention_floor?: number;
    }>
  >;
  [key: string]: unknown;
}>;

export type CachedDocument = Readonly<{
  topic: SyncTopic;
  docKey: string;
  docVersion: number;
  payload: unknown;
  payloadHash?: string | null;
  watermark: number;
  hydrationRequired: boolean;
}>;

export interface WatermarkStore {
  load(topics: ReadonlyArray<SyncTopic>): Promise<Readonly<Record<SyncTopic, number>>>;
  save(topic: SyncTopic, watermark: number): Promise<void>;
  clear(topic: SyncTopic): Promise<void>;
}

export interface WebSocketLike {
  readonly readyState: number;
  onopen: ((event: Event) => void) | null;
  onmessage: ((event: MessageEvent) => void) | null;
  onerror: ((event: Event) => void) | null;
  onclose: ((event: CloseEvent) => void) | null;
  send(data: string): void;
  close(code?: number, reason?: string): void;
}

export type KhalaSocketFactory = (url: string) => WebSocketLike;

export type KhalaClientStatus = "idle" | "connecting" | "connected" | "reconnecting" | "closed";

export type KhalaClientError = Readonly<{
  code: string;
  message: string;
  cause?: unknown;
  payload?: unknown;
}>;

export type KhalaClientOptions = Readonly<{
  url: string;
  tokenProvider: () => Promise<string>;
  watermarkStore?: WatermarkStore;
  socketFactory?: KhalaSocketFactory;
  replayBatchSize?: number;
  autoReconnect?: boolean;
  reconnectMinDelayMs?: number;
  reconnectMaxDelayMs?: number;
  onStatus?: (status: KhalaClientStatus) => void;
  onUpdateBatch?: (batch: SyncUpdateBatch) => void;
  onHeartbeat?: (watermarks: ReadonlyArray<TopicWatermark>) => void;
  onStaleCursor?: (payload: SyncErrorPayload) => void;
  onError?: (error: KhalaClientError) => void;
}>;
