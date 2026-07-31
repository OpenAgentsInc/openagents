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
  toolChoice: "auto" | "none";
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
    nullish(transcription.language) &&
    nullish(transcription.prompt)
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

const normalizedProviderTool = (
  tool: SarahRealtimeProviderTool,
): SarahRealtimeProviderTool | undefined => {
  const parameters = record(tool.parameters);
  if (parameters === undefined) return undefined;
  const { $schema, ...normalizedParameters } = parameters;
  if ($schema !== undefined && $schema !== "http://json-schema.org/draft-07/schema#") {
    return undefined;
  }
  return {
    ...tool,
    parameters: normalizedParameters,
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
  const observed = sortedProviderTools(tools as ReadonlyArray<SarahRealtimeProviderTool>).map(
    normalizedProviderTool,
  );
  const expectedTools = sortedProviderTools(expected.tools).map(normalizedProviderTool);
  if (
    observed.some((tool) => tool === undefined) ||
    expectedTools.some((tool) => tool === undefined)
  ) {
    return "mismatch";
  }
  if (allowToolLoadingTransition && observed.length === 0 && expectedTools.length > 0) {
    return "loading_tools";
  }
  if (
    new Set(observed.map((tool) => tool?.name)).size !== observed.length ||
    new Set(expectedTools.map((tool) => tool?.name)).size !== expectedTools.length
  ) {
    return "mismatch";
  }
  return canonicalJson(observed) === canonicalJson(expectedTools) ? "exact" : "mismatch";
};

const providerTransportMatch = (
  event: unknown,
  expectedProviderSessionRefDigest: string | undefined,
): Readonly<Record<string, unknown>> | undefined => {
  const envelope = record(event);
  if (envelope?.type !== "session.updated") return undefined;
  const session = record(envelope.session);
  const audio = record(session?.audio);
  const sessionRef = providerSessionRef(event);
  if (session === undefined || audio === undefined || sessionRef === undefined) {
    return undefined;
  }
  const observedProviderSessionRefDigest = digest(sessionRef);
  if (
    expectedProviderSessionRefDigest === undefined ||
    observedProviderSessionRefDigest !== expectedProviderSessionRefDigest ||
    session.model !== SARAH_LIVEKIT_MODEL ||
    !Array.isArray(session.output_modalities) ||
    session.output_modalities.length !== 1 ||
    session.output_modalities[0] !== "audio" ||
    !exactKeys(audio, ["input", "output"]) ||
    !exactInputAudio(audio.input) ||
    !exactOutputAudio(audio.output) ||
    !exactProviderPolicy(session)
  ) {
    return undefined;
  }
  return session;
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
  if (expectedProviderSessionRefDigest === undefined) return false;
  const session = providerTransportMatch(event, expectedProviderSessionRefDigest);
  if (session === undefined) return false;
  const profileMatch = providerProfileMatch(session, expectedProfile, allowToolLoadingTransition);
  if (profileMatch === "loading_tools") return undefined;
  if (profileMatch === "mismatch") return false;
  return {
    providerSessionRefDigest: expectedProviderSessionRefDigest,
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

type SarahProviderTransition =
  | "startup_base"
  | "startup_instructions"
  | "startup_tools"
  | "tool_choice_none"
  | "tool_choice_auto";

const exactClientSessionUpdate = (
  event: unknown,
  keys: ReadonlyArray<string>,
): Readonly<Record<string, unknown>> | undefined => {
  const envelope = record(event);
  const session = record(envelope?.session);
  if (
    envelope?.type !== "session.update" ||
    session === undefined ||
    !exactKeys(envelope, ["type", "session", "event_id"]) ||
    !exactKeys(session, keys)
  ) {
    return undefined;
  }
  return session;
};

const startupBaseClientUpdate = (event: unknown): boolean => {
  const session = exactClientSessionUpdate(event, [
    "type",
    "model",
    "output_modalities",
    "audio",
    "max_output_tokens",
    "tool_choice",
    "tracing",
    "instructions",
  ]);
  const audio = record(session?.audio);
  return (
    session !== undefined &&
    session.type === "realtime" &&
    session.model === SARAH_LIVEKIT_MODEL &&
    Array.isArray(session.output_modalities) &&
    session.output_modalities.length === 1 &&
    session.output_modalities[0] === "audio" &&
    session.max_output_tokens === "inf" &&
    session.tool_choice === "auto" &&
    session.tracing === null &&
    session.instructions === undefined &&
    audio !== undefined &&
    exactKeys(audio, ["input", "output"]) &&
    exactInputAudio(audio.input) &&
    exactOutputAudio(audio.output)
  );
};

const startupInstructionsClientUpdate = (
  event: unknown,
  expectedProfile: SarahRealtimeProviderProfile,
): boolean => {
  const session = exactClientSessionUpdate(event, ["type", "instructions"]);
  return (
    session !== undefined &&
    session.type === "realtime" &&
    session.instructions === expectedProfile.instructions
  );
};

const startupToolsClientUpdate = (
  event: unknown,
  expectedProfile: SarahRealtimeProviderProfile,
): boolean => {
  const session = exactClientSessionUpdate(event, ["type", "model", "tools"]);
  return (
    session !== undefined &&
    session.type === "realtime" &&
    session.model === SARAH_LIVEKIT_MODEL &&
    providerProfileMatch(
      {
        instructions: expectedProfile.instructions,
        tool_choice: expectedProfile.toolChoice,
        tools: session.tools,
      },
      expectedProfile,
      false,
    ) === "exact"
  );
};

const toolChoiceClientUpdate = (event: unknown, toolChoice: "auto" | "none"): boolean => {
  const session = exactClientSessionUpdate(event, ["type", "tool_choice"]);
  return session !== undefined && session.type === "realtime" && session.tool_choice === toolChoice;
};

export class SarahProviderAttestation {
  #candidate: AdmittedRealtimeProvider | undefined;
  #durable: AdmittedRealtimeProvider | undefined;
  #mismatchPhase: string | undefined;
  readonly #expectedProviderTransitions: SarahProviderTransition[] = [];
  #clientPhase: "startup_base" | "startup_instructions" | "startup_tools" | "steady" =
    "startup_base";
  #commandedToolChoice: "auto" | "none" = "auto";

  get mismatchPhase(): string | undefined {
    return this.#mismatchPhase;
  }

  observeClientEvent(event: unknown, expectedProfile: SarahRealtimeProviderProfile): boolean {
    const envelope = record(event);
    if (envelope?.type !== "session.update") return true;
    if (this.#clientPhase === "startup_base") {
      if (!startupBaseClientUpdate(event)) {
        this.#mismatchPhase = "client_startup_base";
        return false;
      }
      this.#expectedProviderTransitions.push("startup_base");
      this.#clientPhase = "startup_instructions";
      return true;
    }
    if (this.#clientPhase === "startup_instructions") {
      if (!startupInstructionsClientUpdate(event, expectedProfile)) {
        this.#mismatchPhase = "client_startup_instructions";
        return false;
      }
      this.#expectedProviderTransitions.push("startup_instructions");
      this.#clientPhase = "startup_tools";
      return true;
    }
    if (this.#clientPhase === "startup_tools") {
      if (!startupToolsClientUpdate(event, expectedProfile)) {
        this.#mismatchPhase = "client_startup_tools";
        return false;
      }
      this.#expectedProviderTransitions.push("startup_tools");
      this.#clientPhase = "steady";
      return true;
    }
    if (this.#durable === undefined) {
      this.#mismatchPhase = "client_before_durable_admission";
      return false;
    }
    if (this.#commandedToolChoice === "auto" && toolChoiceClientUpdate(event, "none")) {
      this.#expectedProviderTransitions.push("tool_choice_none");
      this.#commandedToolChoice = "none";
      return true;
    }
    if (this.#commandedToolChoice === "none" && toolChoiceClientUpdate(event, "auto")) {
      this.#expectedProviderTransitions.push("tool_choice_auto");
      this.#commandedToolChoice = "auto";
      return true;
    }
    this.#mismatchPhase = "client_uncommanded_update";
    return false;
  }

  observe(
    event: unknown,
    expectedProviderSessionRefDigest: string | undefined,
    expectedProfile: SarahRealtimeProviderProfile,
  ): SarahProviderAttestationObservation {
    const envelope = record(event);
    if (envelope?.type !== "session.updated") return { state: "pending" };
    const transition = this.#expectedProviderTransitions[0];
    if (transition !== undefined) {
      const transitionProfile =
        transition === "tool_choice_none"
          ? ({ ...expectedProfile, toolChoice: "none" } as const)
          : expectedProfile;
      const observed =
        transition === "startup_base"
          ? this.#observeStartupBase(event, expectedProviderSessionRefDigest, expectedProfile)
          : transition === "startup_instructions"
            ? admittedRealtimeProvider(
                event,
                expectedProviderSessionRefDigest,
                { ...expectedProfile, tools: [] },
                false,
              )
            : admittedRealtimeProvider(
                event,
                expectedProviderSessionRefDigest,
                transitionProfile,
                false,
              );
      if (observed === false || observed === undefined) {
        if (
          transition === "startup_instructions" &&
          this.#expectedProviderTransitions[1] === "startup_tools"
        ) {
          const coalesced = admittedRealtimeProvider(
            event,
            expectedProviderSessionRefDigest,
            expectedProfile,
            false,
          );
          if (coalesced !== false && coalesced !== undefined) {
            this.#expectedProviderTransitions.shift();
            this.#expectedProviderTransitions.shift();
            this.#candidate = coalesced;
            return { state: "candidate", admission: coalesced };
          }
        }
        this.#candidate = undefined;
        this.#mismatchPhase = `provider_${transition}`;
        return { state: this.#durable === undefined ? "mismatch" : "drift" };
      }
      this.#expectedProviderTransitions.shift();
      if (transition === "startup_base" || transition === "startup_instructions") {
        return { state: "pending" };
      }
      if (transition === "tool_choice_none" || transition === "tool_choice_auto") {
        return { state: "confirmed", admission: this.#durable ?? observed };
      }
      this.#candidate = observed;
      return { state: "candidate", admission: observed };
    }
    const observed = admittedRealtimeProvider(
      event,
      expectedProviderSessionRefDigest,
      expectedProfile,
      this.#candidate === undefined && this.#durable === undefined,
    );
    if (observed === undefined) return { state: "pending" };
    if (observed === false) {
      this.#candidate = undefined;
      this.#mismatchPhase = "provider_steady";
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

  #observeStartupBase(
    event: unknown,
    expectedProviderSessionRefDigest: string | undefined,
    expectedProfile: SarahRealtimeProviderProfile,
  ): AdmittedRealtimeProvider | false {
    if (expectedProviderSessionRefDigest === undefined) return false;
    const session = providerTransportMatch(event, expectedProviderSessionRefDigest);
    if (
      session === undefined ||
      typeof session.instructions !== "string" ||
      session.instructions.length === 0 ||
      session.tool_choice !== "auto" ||
      !Array.isArray(session.tools) ||
      session.tools.length !== 0
    ) {
      return false;
    }
    return {
      providerSessionRefDigest: expectedProviderSessionRefDigest,
      providerConfigurationDigest: sarahProviderConfigurationDigest(expectedProfile),
    };
  }
}

export type SarahProviderUsageRef = Readonly<{
  kind: "response" | "transcription";
  providerRef: string;
}>;

export class SarahProviderAccounting {
  readonly #activeResponseRefs = new Set<string>();
  readonly #activeTranscriptionRefs = new Set<string>();
  readonly #acknowledgedTranscriptionRefs = new Set<string>();
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
    return (
      !this.#disconnected &&
      !this.#deliveryUncertain &&
      this.#activeResponseRefs.size === 0 &&
      this.#activeTranscriptionRefs.size === 0
    );
  }

  observe(
    event: unknown,
    terminalUsageObserved: boolean | "response_usage" | "transcription_usage",
  ): SarahProviderUsageRef | undefined {
    const envelope = record(event);
    if (envelope?.type === "session.created") {
      const sessionRef = providerSessionRef(event);
      if (sessionRef !== undefined) this.#providerSessionRefDigest = digest(sessionRef);
      return undefined;
    }
    if (envelope?.type === "input_audio_buffer.committed") {
      const transcriptionRef = boundedProviderRef(envelope.item_id, "transcription:");
      if (transcriptionRef === undefined) {
        this.failTerminalUsageDelivery();
      } else if (!this.#acknowledgedTranscriptionRefs.has(transcriptionRef)) {
        this.#activeTranscriptionRefs.add(transcriptionRef);
      }
      return undefined;
    }
    if (
      envelope?.type === "conversation.item.input_audio_transcription.completed" ||
      envelope?.type === "conversation.item.input_audio_transcription.failed"
    ) {
      const transcriptionRef = boundedProviderRef(envelope.item_id, "transcription:");
      if (transcriptionRef === undefined) {
        this.failTerminalUsageDelivery();
        return undefined;
      }
      if (!this.#acknowledgedTranscriptionRefs.has(transcriptionRef)) {
        this.#activeTranscriptionRefs.add(transcriptionRef);
      }
      if (
        envelope.type === "conversation.item.input_audio_transcription.completed" &&
        terminalUsageObserved === "transcription_usage"
      ) {
        return { kind: "transcription", providerRef: transcriptionRef };
      }
      this.failTerminalUsageDelivery();
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
      if (terminalUsageObserved === true || terminalUsageObserved === "response_usage") {
        return { kind: "response", providerRef: responseRef };
      }
      this.failTerminalUsageDelivery();
    }
    return undefined;
  }

  acknowledgeTerminalUsage(usageRef: SarahProviderUsageRef): void {
    if (usageRef.kind === "response") {
      this.#activeResponseRefs.delete(usageRef.providerRef);
    } else {
      this.#activeTranscriptionRefs.delete(usageRef.providerRef);
      this.#acknowledgedTranscriptionRefs.add(usageRef.providerRef);
    }
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
    return this.waitForTerminalUsage(timeoutMs, wait);
  }

  async waitForTerminalUsage(
    timeoutMs: number,
    wait: (timeoutMs: number) => Promise<void> = (delayMs) =>
      new Promise((resolve) => {
        setTimeout(resolve, delayMs);
      }),
  ): Promise<boolean> {
    if (this.#disconnected || this.#deliveryUncertain) return false;
    if (this.#activeResponseRefs.size === 0 && this.#activeTranscriptionRefs.size === 0)
      return true;
    let notify: (() => void) | undefined;
    const terminal = new Promise<void>((resolve) => {
      notify = resolve;
      this.#waiters.add(resolve);
    });
    await Promise.race([terminal, wait(timeoutMs)]);
    if (notify !== undefined) this.#waiters.delete(notify);
    return this.exact;
  }

  #notifyIfTerminal(): void {
    if (this.#activeResponseRefs.size !== 0 || this.#activeTranscriptionRefs.size !== 0) return;
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
  const terminal = await accounting.waitForTerminalUsage(10_000);
  if (!terminal) {
    fence.failAccounting();
  }
  await closeProvider();
  await fence.quiesce(waitForIdle);
  await closeGeneration(terminal && accounting.exact ? "exact" : "uncertain");
};
