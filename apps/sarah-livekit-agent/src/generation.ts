import {
  SARAH_LIVEKIT_MODEL,
  SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
  type SarahLiveKitJobEvent,
} from "@openagentsinc/audio-contract";

type UsageNumbers = Readonly<{
  inputTokens: number;
  outputTokens: number;
  cachedInputTokens: number;
  audioInputTokens: number;
  audioOutputTokens: number;
}>;

const safeCount = (value: unknown): number | undefined =>
  typeof value === "number" && Number.isSafeInteger(value) && value >= 0 ? value : undefined;

const record = (value: unknown): Record<string, unknown> | undefined =>
  typeof value === "object" && value !== null ? (value as Record<string, unknown>) : undefined;

const parseUsage = (value: unknown): UsageNumbers | undefined => {
  const usage = record(value);
  const inputDetails = record(usage?.input_token_details);
  const outputDetails = record(usage?.output_token_details);
  const cachedDetails = record(inputDetails?.cached_tokens_details);
  const inputTokens = safeCount(usage?.input_tokens);
  const outputTokens = safeCount(usage?.output_tokens);
  const cachedInputTokens = safeCount(inputDetails?.cached_tokens) ?? 0;
  const audioInputTokens =
    safeCount(inputDetails?.audio_tokens) ?? safeCount(cachedDetails?.audio_tokens) ?? 0;
  const audioOutputTokens = safeCount(outputDetails?.audio_tokens) ?? 0;
  if (inputTokens === undefined || outputTokens === undefined) return undefined;
  return {
    inputTokens,
    outputTokens,
    cachedInputTokens,
    audioInputTokens,
    audioOutputTokens,
  };
};

const boundedProviderRef = (value: unknown): string | undefined =>
  typeof value === "string" && value.trim() === value && value.length > 0 && value.length <= 256
    ? value
    : undefined;

export const isAdmittedRealtimeSessionCreated = (event: unknown): boolean | undefined => {
  const envelope = record(event);
  if (envelope?.type !== "session.created") return undefined;
  return record(envelope.session)?.model === SARAH_LIVEKIT_MODEL;
};

export const responseUsageEvent = (
  event: unknown,
  identity: Readonly<{
    sessionRef: string;
    generation: number;
    jobRef: string;
  }>,
): SarahLiveKitJobEvent | undefined => {
  const envelope = record(event);
  if (envelope?.type !== "response.done") return undefined;
  const response = record(envelope.response);
  const providerResponseRef = boundedProviderRef(response?.id);
  const status = response?.status;
  const usage = parseUsage(response?.usage);
  if (
    providerResponseRef === undefined ||
    usage === undefined ||
    !["completed", "cancelled", "failed", "incomplete"].includes(String(status))
  ) {
    return undefined;
  }
  return {
    schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
    _tag: "response_usage",
    ...identity,
    eventRef: `response:${providerResponseRef}`,
    providerResponseRef,
    status: status as "completed" | "cancelled" | "failed" | "incomplete",
    ...usage,
  };
};

export const transcriptionUsageEvent = (
  event: unknown,
  identity: Readonly<{
    sessionRef: string;
    generation: number;
    jobRef: string;
  }>,
): SarahLiveKitJobEvent | undefined => {
  const envelope = record(event);
  if (envelope?.type !== "conversation.item.input_audio_transcription.completed") {
    return undefined;
  }
  const providerTranscriptionRef = boundedProviderRef(envelope.item_id);
  const usage = parseUsage(envelope.usage);
  if (providerTranscriptionRef === undefined || usage === undefined) {
    return undefined;
  }
  return {
    schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
    _tag: "transcription_usage",
    ...identity,
    eventRef: `transcription:${providerTranscriptionRef}`,
    providerTranscriptionRef,
    ...usage,
  };
};

export class SarahGenerationFence {
  readonly pending = new Set<Promise<void>>();
  #settled = false;
  #sealed = false;
  #closeReason: Extract<SarahLiveKitJobEvent, { _tag: "close" }>["reason"] = "worker_error";

  get settled(): boolean {
    return this.#settled;
  }

  settle(reason: Extract<SarahLiveKitJobEvent, { _tag: "close" }>["reason"]): boolean {
    if (this.#settled) return false;
    this.#settled = true;
    this.#closeReason = reason;
    return true;
  }

  get closeReason() {
    return this.#closeReason;
  }

  accepts(event: SarahLiveKitJobEvent): boolean {
    if (this.#sealed || event._tag === "close") return false;
    if (!this.#settled) return true;
    return event._tag === "response_usage" || event._tag === "transcription_usage";
  }

  seal(): void {
    this.#sealed = true;
  }

  track(operation: Promise<void>): void {
    if (this.#sealed) return;
    this.pending.add(operation);
    operation.finally(() => this.pending.delete(operation)).catch(() => {});
  }

  async drain(): Promise<void> {
    while (this.pending.size > 0) {
      // A provider can append final accounting behind the operation currently
      // being delivered, so each newly visible batch must settle in order.
      // eslint-disable-next-line no-await-in-loop
      await Promise.allSettled(this.pending);
    }
  }
}
