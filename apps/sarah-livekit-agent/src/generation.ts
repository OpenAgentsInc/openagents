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
  sessionType: "realtime",
  model: SARAH_LIVEKIT_MODEL,
  outputModalities: ["audio"],
  include: [],
  maxOutputTokens: "inf",
  prompt: null,
  tracing: null,
  truncation: "auto",
  reasoning: null,
  inputAudio: {
    format: { type: "audio/pcm", rate: 24_000 },
    noiseReduction: null,
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
    speed: 1,
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

const exactInputAudio = (value: unknown): boolean => {
  const inputAudio = record(value);
  return (
    inputAudio !== undefined &&
    exactKeys(inputAudio, ["format", "noise_reduction", "transcription", "turn_detection"]) &&
    exactAudioFormat(inputAudio.format) &&
    (inputAudio.noise_reduction === undefined || inputAudio.noise_reduction === null) &&
    exactTranscription(inputAudio.transcription) &&
    exactTurnDetection(inputAudio.turn_detection)
  );
};

const exactOutputAudio = (value: unknown): boolean => {
  const outputAudio = record(value);
  return (
    outputAudio !== undefined &&
    exactKeys(outputAudio, ["format", "speed", "voice"]) &&
    exactAudioFormat(outputAudio.format) &&
    outputAudio.speed === 1 &&
    outputAudio.voice === SARAH_LIVEKIT_VOICE
  );
};

const nullish = (value: unknown): boolean => value === undefined || value === null;

const exactProviderPolicy = (session: Readonly<Record<string, unknown>>): boolean =>
  session.type === "realtime" &&
  session.max_output_tokens === "inf" &&
  nullish(session.prompt) &&
  nullish(session.tracing) &&
  nullish(session.reasoning) &&
  (session.truncation === undefined || session.truncation === "auto") &&
  (session.include === undefined ||
    session.include === null ||
    (Array.isArray(session.include) && session.include.length === 0)) &&
  session.temperature === undefined &&
  session.modalities === undefined &&
  session.voice === undefined &&
  session.input_audio_format === undefined &&
  session.output_audio_format === undefined &&
  session.input_audio_transcription === undefined &&
  session.turn_detection === undefined &&
  session.max_response_output_tokens === undefined &&
  session.speed === undefined;

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

const providerProfileMatch = (
  session: Readonly<Record<string, unknown>>,
  expected: SarahRealtimeProviderProfile,
  allowToolLoadingTransition: boolean,
): "exact" | "loading_tools" | "mismatch" => {
  if (
    session.instructions !== expected.instructions ||
    session.tool_choice !== expected.toolChoice ||
    !Array.isArray(session.tools)
  ) {
    return "mismatch";
  }
  const tools = session.tools.map(providerTool);
  if (tools.some((tool) => tool === undefined)) return "mismatch";
  const observed = sortedProviderTools(tools as ReadonlyArray<SarahRealtimeProviderTool>);
  const expectedTools = sortedProviderTools(expected.tools);
  if (allowToolLoadingTransition && observed.length === 0 && expectedTools.length > 0) {
    return "loading_tools";
  }
  if (
    new Set(observed.map((tool) => tool.name)).size !== observed.length ||
    new Set(expectedTools.map((tool) => tool.name)).size !== expectedTools.length
  ) {
    return "mismatch";
  }
  return canonicalJson(observed) === canonicalJson(expectedTools) ? "exact" : "mismatch";
};

export type AdmittedRealtimeProvider = Readonly<{
  providerSessionRefDigest: string;
  providerConfigurationDigest: string;
}>;

export const admittedRealtimeProvider = (
  event: unknown,
  expectedProviderSessionRefDigest: string | undefined,
  expectedProfile: SarahRealtimeProviderProfile,
  allowToolLoadingTransition = true,
): AdmittedRealtimeProvider | false | undefined => {
  const envelope = record(event);
  if (envelope?.type !== "session.updated") return undefined;
  const session = record(envelope.session);
  const audio = record(session?.audio);
  const sessionRef = providerSessionRef(event);
  if (session === undefined || audio === undefined || sessionRef === undefined) {
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
    !exactKeys(audio, ["input", "output"]) ||
    !exactInputAudio(audio.input) ||
    !exactOutputAudio(audio.output) ||
    !exactProviderPolicy(session)
  ) {
    return false;
  }
  const profileMatch = providerProfileMatch(session, expectedProfile, allowToolLoadingTransition);
  if (profileMatch === "loading_tools") return undefined;
  if (profileMatch === "mismatch") return false;
  return {
    providerSessionRefDigest,
    providerConfigurationDigest: sarahProviderConfigurationDigest(expectedProfile),
  };
};

const sameAdmission = (left: AdmittedRealtimeProvider, right: AdmittedRealtimeProvider): boolean =>
  left.providerSessionRefDigest === right.providerSessionRefDigest &&
  left.providerConfigurationDigest === right.providerConfigurationDigest;

export type SarahProviderAttestationObservation =
  | Readonly<{ state: "pending" }>
  | Readonly<{ state: "candidate"; admission: AdmittedRealtimeProvider }>
  | Readonly<{ state: "confirmed"; admission: AdmittedRealtimeProvider }>
  | Readonly<{ state: "mismatch" }>
  | Readonly<{ state: "drift" }>;

export class SarahProviderAttestation {
  #candidate: AdmittedRealtimeProvider | undefined;
  #durable: AdmittedRealtimeProvider | undefined;

  observe(
    event: unknown,
    expectedProviderSessionRefDigest: string | undefined,
    expectedProfile: SarahRealtimeProviderProfile,
  ): SarahProviderAttestationObservation {
    const observed = admittedRealtimeProvider(
      event,
      expectedProviderSessionRefDigest,
      expectedProfile,
      this.#candidate === undefined && this.#durable === undefined,
    );
    if (observed === undefined) return { state: "pending" };
    if (observed === false) {
      this.#candidate = undefined;
      return { state: this.#durable === undefined ? "mismatch" : "drift" };
    }
    if (this.#durable !== undefined) {
      return sameAdmission(this.#durable, observed)
        ? { state: "confirmed", admission: observed }
        : { state: "drift" };
    }
    this.#candidate = observed;
    return { state: "candidate", admission: observed };
  }

  markDurable(admission: AdmittedRealtimeProvider): boolean {
    if (this.#candidate === undefined || !sameAdmission(this.#candidate, admission)) {
      return false;
    }
    this.#durable = admission;
    return true;
  }
}

export class SarahProviderAccounting {
  readonly #activeResponseRefs = new Set<string>();
  readonly #waiters = new Set<() => void>();
  #providerSessionRefDigest: string | undefined;
  #disconnected = false;
  #deliveryUncertain = false;

  get providerSessionRefDigest(): string | undefined {
    return this.#providerSessionRefDigest;
  }

  get disconnected(): boolean {
    return this.#disconnected;
  }

  get exact(): boolean {
    return !this.#disconnected && !this.#deliveryUncertain && this.#activeResponseRefs.size === 0;
  }

  observe(event: unknown, terminalUsageObserved: boolean): string | undefined {
    const envelope = record(event);
    if (envelope?.type === "session.created") {
      const sessionRef = providerSessionRef(event);
      if (sessionRef !== undefined) this.#providerSessionRefDigest = digest(sessionRef);
      return undefined;
    }
    const response = record(envelope?.response);
    const responseRef = boundedProviderRef(response?.id, "");
    if (envelope?.type === "response.created" && responseRef !== undefined) {
      this.#activeResponseRefs.add(responseRef);
      return undefined;
    }
    if (envelope?.type === "response.done" && responseRef !== undefined) {
      this.#activeResponseRefs.add(responseRef);
      if (terminalUsageObserved) return responseRef;
      this.failTerminalUsageDelivery();
    }
    return undefined;
  }

  acknowledgeTerminalUsage(responseRef: string): void {
    this.#activeResponseRefs.delete(responseRef);
    this.#notifyIfTerminal();
  }

  failTerminalUsageDelivery(): void {
    this.#deliveryUncertain = true;
    for (const resolve of this.#waiters) resolve();
    this.#waiters.clear();
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
    if (this.#disconnected || this.#deliveryUncertain) return false;
    if (this.#activeResponseRefs.size === 0) return true;
    let notify: (() => void) | undefined;
    const terminal = new Promise<void>((resolve) => {
      notify = resolve;
      this.#waiters.add(resolve);
    });
    await Promise.race([terminal, wait(timeoutMs)]);
    if (notify !== undefined) this.#waiters.delete(notify);
    return !this.#disconnected && !this.#deliveryUncertain && this.#activeResponseRefs.size === 0;
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

export const retrySarahLiveKitWorkerClaim = async <Value>(
  claim: () => Promise<Value>,
  wait: () => Promise<void> = () =>
    new Promise((resolve) => {
      setTimeout(resolve, 100);
    }),
  maximumAttempts = 300,
): Promise<Value> => {
  let lastError: unknown = new Error("Sarah LiveKit claim was unavailable");
  for (let attempt = 0; attempt < maximumAttempts; attempt += 1) {
    try {
      // eslint-disable-next-line no-await-in-loop
      return await claim();
    } catch (error) {
      lastError = error;
      if (attempt + 1 < maximumAttempts) {
        // eslint-disable-next-line no-await-in-loop
        await wait();
      }
    }
  }
  throw lastError;
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
  await closeGeneration(terminal && accounting.exact ? "exact" : "uncertain");
};
