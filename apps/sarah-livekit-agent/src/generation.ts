import {
  SARAH_LIVEKIT_MODEL,
  SARAH_LIVEKIT_TRANSCRIPTION_MODEL,
  SARAH_LIVEKIT_VOICE,
  SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
  type SarahLiveKitJobEvent,
} from "@openagentsinc/audio-contract";
import { createHash } from "node:crypto";

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

const exactKeys = (value: Record<string, unknown>, keys: ReadonlyArray<string>): boolean =>
  Object.keys(value).every((key) => keys.includes(key));

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

const boundedProviderRef = (value: unknown, eventPrefix: string): string | undefined =>
  typeof value === "string" &&
  value.trim() === value &&
  value.length > 0 &&
  value.length <= 256 - eventPrefix.length
    ? value
    : undefined;

const providerSessionRef = (event: unknown): string | undefined => {
  const envelope = record(event);
  const session = record(envelope?.session);
  return boundedProviderRef(session?.id, "");
};

const digest = (value: string): string => createHash("sha256").update(value).digest("hex");

const canonicalJson = (value: unknown): string => {
  if (value === null || typeof value !== "object") {
    return JSON.stringify(value);
  }
  if (Array.isArray(value)) {
    return `[${value.map(canonicalJson).join(",")}]`;
  }
  const object = value as Readonly<Record<string, unknown>>;
  return `{${Object.keys(object)
    .sort()
    .map((key) => `${JSON.stringify(key)}:${canonicalJson(object[key])}`)
    .join(",")}}`;
};

const canonicalProviderTransportConfiguration = {
  model: SARAH_LIVEKIT_MODEL,
  outputModalities: ["audio"],
  inputAudio: {
    format: { type: "audio/pcm", rate: 24_000 },
    transcription: { model: SARAH_LIVEKIT_TRANSCRIPTION_MODEL },
    turnDetection: {
      type: "semantic_vad",
      eagerness: "high",
      create_response: true,
      interrupt_response: true,
    },
  },
  outputAudio: {
    format: { type: "audio/pcm", rate: 24_000 },
    voice: SARAH_LIVEKIT_VOICE,
  },
} as const;

export type SarahRealtimeProviderTool = Readonly<{
  type: "function";
  name: string;
  description: string;
  parameters: unknown;
}>;

export type SarahRealtimeProviderProfile = Readonly<{
  instructions: string;
  tools: ReadonlyArray<SarahRealtimeProviderTool>;
  toolChoice: "auto";
}>;

const sortedProviderTools = (
  tools: ReadonlyArray<SarahRealtimeProviderTool>,
): ReadonlyArray<SarahRealtimeProviderTool> =>
  [...tools].sort((left, right) => left.name.localeCompare(right.name));

export const sarahProviderConfigurationDigest = (profile: SarahRealtimeProviderProfile): string =>
  digest(
    canonicalJson({
      ...canonicalProviderTransportConfiguration,
      profile: {
        instructions: profile.instructions,
        tools: sortedProviderTools(profile.tools),
        toolChoice: profile.toolChoice,
      },
    }),
  );

const exactAudioFormat = (value: unknown): boolean => {
  const format = record(value);
  return (
    format !== undefined &&
    exactKeys(format, ["type", "rate"]) &&
    format.type === "audio/pcm" &&
    format.rate === 24_000
  );
};

const exactTranscription = (value: unknown): boolean => {
  const transcription = record(value);
  return (
    transcription !== undefined &&
    exactKeys(transcription, ["model", "language", "prompt"]) &&
    transcription.model === SARAH_LIVEKIT_TRANSCRIPTION_MODEL &&
    transcription.language === undefined &&
    transcription.prompt === undefined
  );
};

const exactTurnDetection = (value: unknown): boolean => {
  const turnDetection = record(value);
  return (
    turnDetection !== undefined &&
    exactKeys(turnDetection, [
      "type",
      "eagerness",
      "create_response",
      "interrupt_response",
      "threshold",
      "prefix_padding_ms",
      "silence_duration_ms",
    ]) &&
    turnDetection.type === "semantic_vad" &&
    turnDetection.eagerness === "high" &&
    turnDetection.create_response === true &&
    turnDetection.interrupt_response === true &&
    turnDetection.threshold === undefined &&
    turnDetection.prefix_padding_ms === undefined &&
    turnDetection.silence_duration_ms === undefined
  );
};

const providerTool = (value: unknown): SarahRealtimeProviderTool | undefined => {
  const tool = record(value);
  if (
    tool === undefined ||
    !exactKeys(tool, ["type", "name", "description", "parameters"]) ||
    tool.type !== "function" ||
    typeof tool.name !== "string" ||
    typeof tool.description !== "string" ||
    record(tool.parameters) === undefined
  ) {
    return undefined;
  }
  return {
    type: "function",
    name: tool.name,
    description: tool.description,
    parameters: tool.parameters,
  };
};

const exactProviderProfile = (
  session: Readonly<Record<string, unknown>>,
  expected: SarahRealtimeProviderProfile,
): boolean => {
  if (
    session.instructions !== expected.instructions ||
    session.tool_choice !== expected.toolChoice ||
    !Array.isArray(session.tools)
  ) {
    return false;
  }
  const tools = session.tools.map(providerTool);
  if (tools.some((tool) => tool === undefined)) return false;
  const observed = sortedProviderTools(tools as ReadonlyArray<SarahRealtimeProviderTool>);
  const expectedTools = sortedProviderTools(expected.tools);
  if (
    new Set(observed.map((tool) => tool.name)).size !== observed.length ||
    new Set(expectedTools.map((tool) => tool.name)).size !== expectedTools.length
  ) {
    return false;
  }
  return canonicalJson(observed) === canonicalJson(expectedTools);
};

export type AdmittedRealtimeProvider = Readonly<{
  providerSessionRefDigest: string;
  providerConfigurationDigest: string;
}>;

export const admittedRealtimeProvider = (
  event: unknown,
  expectedProviderSessionRefDigest: string | undefined,
  expectedProfile: SarahRealtimeProviderProfile,
): AdmittedRealtimeProvider | false | undefined => {
  const envelope = record(event);
  if (envelope?.type !== "session.updated") return undefined;
  const session = record(envelope.session);
  const audio = record(session?.audio);
  const inputAudio = record(audio?.input);
  const outputAudio = record(audio?.output);
  const sessionRef = providerSessionRef(event);
  if (
    session === undefined ||
    audio === undefined ||
    inputAudio === undefined ||
    outputAudio === undefined ||
    sessionRef === undefined
  ) {
    return false;
  }
  const providerSessionRefDigest = digest(sessionRef);
  if (
    expectedProviderSessionRefDigest === undefined ||
    providerSessionRefDigest !== expectedProviderSessionRefDigest ||
    session.model !== SARAH_LIVEKIT_MODEL ||
    !Array.isArray(session.output_modalities) ||
    session.output_modalities.length !== 1 ||
    session.output_modalities[0] !== "audio" ||
    !exactAudioFormat(inputAudio.format) ||
    (inputAudio.noise_reduction !== undefined && inputAudio.noise_reduction !== null) ||
    !exactTranscription(inputAudio.transcription) ||
    !exactTurnDetection(inputAudio.turn_detection) ||
    !exactAudioFormat(outputAudio.format) ||
    outputAudio.voice !== SARAH_LIVEKIT_VOICE
  ) {
    return false;
  }
  if (!exactProviderProfile(session, expectedProfile)) return undefined;
  return {
    providerSessionRefDigest,
    providerConfigurationDigest: sarahProviderConfigurationDigest(expectedProfile),
  };
};

export class SarahProviderAccounting {
  readonly #activeResponseRefs = new Set<string>();
  readonly #waiters = new Set<() => void>();
  #providerSessionRefDigest: string | undefined;
  #disconnected = false;

  get providerSessionRefDigest(): string | undefined {
    return this.#providerSessionRefDigest;
  }

  get disconnected(): boolean {
    return this.#disconnected;
  }

  observe(event: unknown, terminalUsageObserved: boolean): void {
    const envelope = record(event);
    if (envelope?.type === "session.created") {
      const sessionRef = providerSessionRef(event);
      if (sessionRef !== undefined) this.#providerSessionRefDigest = digest(sessionRef);
      return;
    }
    const response = record(envelope?.response);
    const responseRef = boundedProviderRef(response?.id, "");
    if (envelope?.type === "response.created" && responseRef !== undefined) {
      this.#activeResponseRefs.add(responseRef);
      return;
    }
    if (envelope?.type === "response.done" && responseRef !== undefined && terminalUsageObserved) {
      this.#activeResponseRefs.delete(responseRef);
      this.#notifyIfTerminal();
    }
  }

  disconnect(): void {
    this.#disconnected = true;
    for (const resolve of this.#waiters) resolve();
    this.#waiters.clear();
  }

  async waitForTerminalResponses(
    timeoutMs: number,
    wait: (timeoutMs: number) => Promise<void> = (delayMs) =>
      new Promise((resolve) => {
        setTimeout(resolve, delayMs);
      }),
  ): Promise<boolean> {
    if (this.#activeResponseRefs.size === 0) return true;
    if (this.#disconnected) return false;
    let notify: (() => void) | undefined;
    const terminal = new Promise<void>((resolve) => {
      notify = resolve;
      this.#waiters.add(resolve);
    });
    await Promise.race([terminal, wait(timeoutMs)]);
    if (notify !== undefined) this.#waiters.delete(notify);
    return this.#activeResponseRefs.size === 0;
  }

  #notifyIfTerminal(): void {
    if (this.#activeResponseRefs.size !== 0) return;
    for (const resolve of this.#waiters) resolve();
    this.#waiters.clear();
  }
}

export const waitForAdmissionUntil = async <Value>(
  waitForAdmission: () => Promise<Value>,
  expiresAtMs: number,
  abortSignal: AbortSignal,
  now: () => number = Date.now,
): Promise<Value> => {
  const remainingMs = expiresAtMs - now();
  if (remainingMs <= 0) throw new Error("Sarah LiveKit admission expired");
  return new Promise<Value>((resolve, reject) => {
    let settled = false;
    const settle = (operation: () => void) => {
      if (settled) return;
      settled = true;
      clearTimeout(timeout);
      abortSignal.removeEventListener("abort", onAbort);
      operation();
    };
    const onAbort = () => settle(() => reject(new Error("Sarah LiveKit admission was aborted")));
    const timeout = setTimeout(
      () => settle(() => reject(new Error("Sarah LiveKit admission expired"))),
      remainingMs,
    );
    abortSignal.addEventListener("abort", onAbort, { once: true });
    waitForAdmission().then(
      (value) => settle(() => resolve(value)),
      (error) => settle(() => reject(error)),
    );
  });
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
  const providerResponseRef = boundedProviderRef(response?.id, "response:");
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
  const providerTranscriptionRef = boundedProviderRef(envelope.item_id, "transcription:");
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
  #providerEventRevision = 0;
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

  failAccounting(): void {
    if (this.#sealed) return;
    this.#settled = true;
    this.#closeReason = "worker_error";
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

  observeProviderEvent(): void {
    if (!this.#sealed) this.#providerEventRevision += 1;
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

  async quiesce(
    waitForIdle: () => Promise<void> = () =>
      new Promise((resolve) => {
        setTimeout(resolve, 25);
      }),
    maximumIdleChecks = 4,
  ): Promise<void> {
    for (let check = 0; check < maximumIdleChecks; check += 1) {
      const revision = this.#providerEventRevision;
      // eslint-disable-next-line no-await-in-loop
      await this.drain();
      // eslint-disable-next-line no-await-in-loop
      await waitForIdle();
      // eslint-disable-next-line no-await-in-loop
      await this.drain();
      if (revision === this.#providerEventRevision) {
        this.seal();
        return;
      }
    }
    this.seal();
    await this.drain();
  }
}

export const closeAfterProviderAccounting = async (
  fence: SarahGenerationFence,
  accounting: SarahProviderAccounting,
  requestProviderDrain: () => Promise<void>,
  closeProvider: () => Promise<void>,
  closeGeneration: (accountingStatus: "exact" | "uncertain") => Promise<void>,
  waitForIdle?: () => Promise<void>,
): Promise<void> => {
  await requestProviderDrain();
  const terminal = await accounting.waitForTerminalResponses(10_000);
  if (!terminal) {
    fence.failAccounting();
  }
  await closeProvider();
  await fence.quiesce(waitForIdle);
  await closeGeneration(terminal ? "exact" : "uncertain");
};
