import { MemoryWatermarkStore } from "./watermarkStore";
import type {
  CachedDocument,
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

type PhoenixFrame = [string | null, string | null, string, string, unknown];

type PendingReply = Readonly<{
  resolve: (value: unknown) => void;
  reject: (reason: unknown) => void;
}>;

const SOCKET_OPEN = 1;
const DEFAULT_RECONNECT_MIN_DELAY_MS = 1_000;
const DEFAULT_RECONNECT_MAX_DELAY_MS = 30_000;
const DEFAULT_TOPIC = "sync:v1";

const isObject = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);

const toNonNegativeInteger = (value: unknown): number | null =>
  Number.isInteger(value) && (value as number) >= 0 ? (value as number) : null;

const toTopicWatermarks = (value: unknown): ReadonlyArray<TopicWatermark> => {
  if (!Array.isArray(value)) return [];
  const watermarks: TopicWatermark[] = [];

  for (const item of value) {
    if (!isObject(item)) continue;
    const topic = typeof item.topic === "string" ? item.topic.trim() : "";
    const watermark = toNonNegativeInteger(item.watermark);
    if (topic !== "" && watermark !== null) {
      watermarks.push({ topic, watermark });
    }
  }

  return watermarks;
};

const normalizeUpdate = (value: unknown): SyncUpdate | null => {
  if (!isObject(value)) return null;

  const topic = typeof value.topic === "string" ? value.topic.trim() : "";
  const docKey = typeof value.doc_key === "string" ? value.doc_key.trim() : "";
  const docVersion = toNonNegativeInteger(value.doc_version);
  const watermark = toNonNegativeInteger(value.watermark);

  if (topic === "" || docKey === "" || docVersion === null || watermark === null) {
    return null;
  }

  return {
    topic,
    doc_key: docKey,
    doc_version: docVersion,
    payload: value.payload,
    payload_hash: typeof value.payload_hash === "string" ? value.payload_hash : null,
    watermark,
    hydration_required: value.hydration_required === true,
  };
};

const defaultSocketFactory: KhalaSocketFactory = (url: string): WebSocketLike => {
  if (typeof WebSocket === "undefined") {
    throw new Error("WebSocket is not available in this runtime");
  }

  return new WebSocket(url) as unknown as WebSocketLike;
};

const toBackoffDelayMs = (attempt: number, minMs: number, maxMs: number): number => {
  const power = Math.max(0, Math.floor(attempt));
  return Math.min(maxMs, minMs * Math.pow(2, power));
};

export class KhalaSyncClient {
  private readonly options: KhalaClientOptions;
  private readonly topic = DEFAULT_TOPIC;
  private readonly watermarkStore: WatermarkStore;
  private readonly socketFactory: KhalaSocketFactory;
  private readonly pendingReplies = new Map<string, PendingReply>();
  private readonly subscribedTopics = new Set<SyncTopic>();
  private readonly docsByKey = new Map<string, CachedDocument>();
  private readonly latestWatermarks = new Map<SyncTopic, number>();

  private socket: WebSocketLike | null = null;
  private joinRef: string | null = null;
  private status: KhalaClientStatus = "idle";
  private refCounter = 0;
  private reconnectAttempt = 0;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private manuallyClosed = false;
  private connectPromise: Promise<void> | null = null;

  constructor(options: KhalaClientOptions) {
    this.options = options;
    this.watermarkStore = options.watermarkStore ?? new MemoryWatermarkStore();
    this.socketFactory = options.socketFactory ?? defaultSocketFactory;
  }

  getStatus(): KhalaClientStatus {
    return this.status;
  }

  async connect(): Promise<void> {
    this.manuallyClosed = false;
    this.reconnectAttempt = 0;

    if (this.socket && this.socket.readyState === SOCKET_OPEN && this.joinRef) {
      this.setStatus("connected");
      return;
    }

    if (this.connectPromise) {
      return this.connectPromise;
    }

    this.connectPromise = this.openAndJoin().finally(() => {
      this.connectPromise = null;
    });

    return this.connectPromise;
  }

  async disconnect(): Promise<void> {
    this.manuallyClosed = true;
    this.clearReconnectTimer();
    this.rejectPendingReplies({ code: "socket_closed", message: "client disconnected" });

    if (this.socket) {
      this.socket.close(1000, "client disconnect");
      this.socket = null;
    }

    this.joinRef = null;
    this.setStatus("closed");
  }

  async subscribe(topics: ReadonlyArray<SyncTopic>): Promise<ReadonlyArray<TopicWatermark>> {
    const normalizedTopics = this.normalizeTopics(topics);
    if (normalizedTopics.length === 0) return [];

    await this.connect();

    const resumeAfter = await this.watermarkStore.load(normalizedTopics);

    const response = await this.channelPush("sync:subscribe", {
      topics: normalizedTopics,
      resume_after: resumeAfter,
      replay_batch_size: this.options.replayBatchSize,
    });

    const responseObject = isObject(response) ? response : {};
    const currentWatermarks = toTopicWatermarks(responseObject.current_watermarks);

    for (const topic of normalizedTopics) {
      this.subscribedTopics.add(topic);
    }

    for (const { topic, watermark } of currentWatermarks) {
      this.updateWatermark(topic, watermark);
    }

    return currentWatermarks;
  }

  async unsubscribe(topics: ReadonlyArray<SyncTopic>): Promise<void> {
    const normalizedTopics = this.normalizeTopics(topics);
    if (normalizedTopics.length === 0) return;

    if (!this.socket || this.socket.readyState !== SOCKET_OPEN || !this.joinRef) {
      for (const topic of normalizedTopics) {
        this.subscribedTopics.delete(topic);
      }
      return;
    }

    await this.channelPush("sync:unsubscribe", { topics: normalizedTopics });

    for (const topic of normalizedTopics) {
      this.subscribedTopics.delete(topic);
    }
  }

  getDocument(docKey: string): CachedDocument | null {
    return this.docsByKey.get(docKey) ?? null;
  }

  listDocumentsForTopic(topic: SyncTopic): ReadonlyArray<CachedDocument> {
    const normalizedTopic = topic.trim();
    if (normalizedTopic === "") return [];

    const docs: CachedDocument[] = [];
    for (const doc of this.docsByKey.values()) {
      if (doc.topic === normalizedTopic) docs.push(doc);
    }
    return docs;
  }

  getWatermark(topic: SyncTopic): number {
    return this.latestWatermarks.get(topic) ?? 0;
  }

  private async openAndJoin(): Promise<void> {
    this.setStatus(this.reconnectAttempt > 0 ? "reconnecting" : "connecting");

    const token = await this.options.tokenProvider();
    const socketUrl = this.buildSocketUrl(this.options.url, token);
    const socket = this.socketFactory(socketUrl);
    this.socket = socket;

    await new Promise<void>((resolve, reject) => {
      let settled = false;

      const onFailure = (error: KhalaClientError) => {
        if (settled) return;
        settled = true;
        reject(new Error(error.message));
      };

      socket.onopen = () => {
        if (settled) return;

        this.sendFrame(null, this.nextRef(), this.topic, "phx_join", {})
          .then((joinRef) => {
            this.joinRef = joinRef;

            if (settled) return;
            settled = true;
            this.setStatus("connected");
            resolve();
          })
          .catch((error) => {
            onFailure(this.toClientError("join_failed", "failed to join sync channel", error));
          });
      };

      socket.onmessage = (event: MessageEvent) => {
        void this.handleSocketMessage(event.data);
      };

      socket.onerror = () => {
        onFailure(this.toClientError("socket_error", "sync socket error"));
      };

      socket.onclose = () => {
        this.joinRef = null;
        this.rejectPendingReplies({ code: "socket_closed", message: "sync socket closed" });

        if (!this.manuallyClosed) {
          this.scheduleReconnect();
        } else {
          this.setStatus("closed");
        }
      };
    });

    if (this.subscribedTopics.size > 0) {
      const topics = Array.from(this.subscribedTopics);
      await this.subscribe(topics);
    }
  }

  private async sendFrame(
    joinRef: string | null,
    ref: string,
    topic: string,
    event: string,
    payload: unknown,
  ): Promise<string> {
    const socket = this.socket;
    if (!socket || socket.readyState !== SOCKET_OPEN) {
      throw new Error("sync socket is not open");
    }

    const frame: PhoenixFrame = [joinRef, ref, topic, event, payload];
    socket.send(JSON.stringify(frame));
    return ref;
  }

  private async channelPush(event: string, payload: unknown): Promise<unknown> {
    if (!this.joinRef) {
      throw new Error("sync socket is not joined");
    }

    const ref = this.nextRef();
    await this.sendFrame(this.joinRef, ref, this.topic, event, payload);

    return await new Promise<unknown>((resolve, reject) => {
      this.pendingReplies.set(ref, { resolve, reject });
    });
  }

  private async handleSocketMessage(rawData: unknown): Promise<void> {
    const frame = this.parseFrame(rawData);
    if (!frame) return;

    const [, ref, topic, event, payload] = frame;
    if (topic !== this.topic) return;

    if (event === "phx_reply" && ref) {
      this.handleReply(ref, payload);
      return;
    }

    if (event === "sync:update_batch") {
      this.handleUpdateBatch(payload);
      return;
    }

    if (event === "sync:error") {
      this.handleSyncError(payload);
      return;
    }

    if (event === "sync:heartbeat") {
      const heartbeatPayload = isObject(payload) ? payload : {};
      const watermarks = toTopicWatermarks(heartbeatPayload.watermarks);
      this.options.onHeartbeat?.(watermarks);
      return;
    }
  }

  private handleReply(ref: string, payload: unknown): void {
    const pending = this.pendingReplies.get(ref);
    if (!pending) return;

    this.pendingReplies.delete(ref);

    const reply = isObject(payload) ? payload : {};
    const status = reply.status;
    const response = reply.response;

    if (status === "ok") {
      pending.resolve(response);
      return;
    }

    const responseError = isObject(response) ? (response as SyncErrorPayload) : {};
    if (responseError.code === "stale_cursor") {
      this.options.onStaleCursor?.(responseError);
      void this.resetStaleTopics(responseError);
    }

    pending.reject(
      this.toClientError(
        typeof responseError.code === "string" ? responseError.code : "request_failed",
        typeof responseError.message === "string" ? responseError.message : "sync request failed",
        undefined,
        responseError,
      ),
    );
  }

  private handleUpdateBatch(payload: unknown): void {
    const raw = isObject(payload) ? payload : {};
    const rawUpdates = Array.isArray(raw.updates) ? raw.updates : [];
    const updates: SyncUpdate[] = [];

    for (const candidate of rawUpdates) {
      const update = normalizeUpdate(candidate);
      if (!update) continue;

      this.applyUpdate(update);
      updates.push(update);
    }

    const batch: SyncUpdateBatch = {
      updates,
      replay_complete: raw.replay_complete === true,
      head_watermarks: toTopicWatermarks(raw.head_watermarks),
    };

    this.options.onUpdateBatch?.(batch);
  }

  private handleSyncError(payload: unknown): void {
    const errorPayload: SyncErrorPayload = isObject(payload) ? payload : {};
    const code = typeof errorPayload.code === "string" ? errorPayload.code : "sync_error";
    const message = typeof errorPayload.message === "string" ? errorPayload.message : "sync error";

    if (code === "stale_cursor") {
      this.options.onStaleCursor?.(errorPayload);
      void this.resetStaleTopics(errorPayload);
      return;
    }

    this.options.onError?.(this.toClientError(code, message, undefined, errorPayload));
  }

  private async resetStaleTopics(payload: SyncErrorPayload): Promise<void> {
    const staleTopics = Array.isArray(payload.stale_topics) ? payload.stale_topics : [];

    for (const item of staleTopics) {
      if (!isObject(item)) continue;
      const topic = typeof item.topic === "string" ? item.topic.trim() : "";
      if (topic === "") continue;

      this.latestWatermarks.delete(topic);
      await this.watermarkStore.clear(topic);
    }
  }

  private applyUpdate(update: SyncUpdate): void {
    this.updateWatermark(update.topic, update.watermark);

    const existing = this.docsByKey.get(update.doc_key);
    if (existing && existing.docVersion > update.doc_version) {
      return;
    }

    this.docsByKey.set(update.doc_key, {
      topic: update.topic,
      docKey: update.doc_key,
      docVersion: update.doc_version,
      payload: update.payload,
      payloadHash: update.payload_hash ?? null,
      watermark: update.watermark,
      hydrationRequired: update.hydration_required === true,
    });
  }

  private updateWatermark(topic: SyncTopic, watermark: number): void {
    const current = this.latestWatermarks.get(topic) ?? 0;
    if (watermark <= current) return;

    this.latestWatermarks.set(topic, watermark);
    void this.watermarkStore.save(topic, watermark);
  }

  private normalizeTopics(topics: ReadonlyArray<SyncTopic>): ReadonlyArray<SyncTopic> {
    const normalized = topics
      .filter((topic): topic is string => typeof topic === "string")
      .map((topic) => topic.trim())
      .filter((topic) => topic !== "");

    return Array.from(new Set(normalized));
  }

  private buildSocketUrl(baseUrl: string, token: string): string {
    const socketUrl = new URL(baseUrl);
    socketUrl.searchParams.set("token", token);
    socketUrl.searchParams.set("vsn", "2.0.0");
    return socketUrl.toString();
  }

  private parseFrame(rawData: unknown): PhoenixFrame | null {
    if (typeof rawData !== "string") return null;

    let parsed: unknown;
    try {
      parsed = JSON.parse(rawData) as unknown;
    } catch {
      return null;
    }

    if (!Array.isArray(parsed) || parsed.length !== 5) return null;

    const [joinRef, ref, topic, event, payload] = parsed;
    if (typeof topic !== "string" || typeof event !== "string") return null;
    if (joinRef !== null && typeof joinRef !== "string") return null;
    if (ref !== null && typeof ref !== "string") return null;

    return [joinRef, ref, topic, event, payload];
  }

  private scheduleReconnect(): void {
    if (this.manuallyClosed || this.options.autoReconnect === false) {
      this.setStatus("closed");
      return;
    }

    if (this.reconnectTimer) return;

    this.setStatus("reconnecting");

    const minDelayMs =
      this.options.reconnectMinDelayMs ?? DEFAULT_RECONNECT_MIN_DELAY_MS;
    const maxDelayMs =
      this.options.reconnectMaxDelayMs ?? DEFAULT_RECONNECT_MAX_DELAY_MS;

    const delayMs = toBackoffDelayMs(this.reconnectAttempt, minDelayMs, maxDelayMs);

    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      this.reconnectAttempt += 1;

      this.openAndJoin()
        .then(() => {
          this.reconnectAttempt = 0;
        })
        .catch((error) => {
          this.options.onError?.(
            this.toClientError("reconnect_failed", "failed to reconnect sync socket", error),
          );
          this.scheduleReconnect();
        });
    }, delayMs);
  }

  private clearReconnectTimer(): void {
    if (!this.reconnectTimer) return;
    clearTimeout(this.reconnectTimer);
    this.reconnectTimer = null;
  }

  private rejectPendingReplies(error: KhalaClientError): void {
    for (const pending of this.pendingReplies.values()) {
      pending.reject(error);
    }
    this.pendingReplies.clear();
  }

  private nextRef(): string {
    this.refCounter += 1;
    return String(this.refCounter);
  }

  private setStatus(status: KhalaClientStatus): void {
    if (this.status === status) return;
    this.status = status;
    this.options.onStatus?.(status);
  }

  private toClientError(
    code: string,
    message: string,
    cause?: unknown,
    payload?: unknown,
  ): KhalaClientError {
    return { code, message, cause, payload };
  }
}
