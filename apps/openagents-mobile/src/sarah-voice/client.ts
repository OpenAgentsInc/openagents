import {
  SARAH_VOICE_NOSTR_AUTH_METHOD,
  SARAH_VOICE_NOSTR_CHALLENGE_PATH,
  SARAH_VOICE_NOSTR_CHALLENGE_PROTOCOL_VERSION,
  SARAH_VOICE_PROTOCOL_VERSION,
  SARAH_VOICE_SESSION_PATH,
  decodeSarahVoiceNostrChallengeResponse,
  decodeSarahVoiceServerControl,
  decodeSarahVoiceSessionResponse,
  type SarahVoiceServerControl,
  type VoiceIdentity,
} from "@openagentsinc/audio-contract";

import type { Issue31NostrSigner } from "@openagentsinc/sarah/issue31-nostr";

import {
  decodeServerAudioFrame,
  encodeClientAudioFrame,
  encodeNip98Authorization,
  sha256Hex,
  type Sha256,
} from "./protocol";
import type { SarahVoiceSessionVault } from "./session-vault";

const DISCLOSURE_REF = "openagents.mobile.sarah.voice.v1";
const MAX_RECONNECT_ATTEMPTS = 2;
const MAX_PENDING_AUDIO_FRAMES = 8;

type ClientControlPayload =
  | Readonly<{ _tag: "session_hello"; disclosureRef: string }>
  | Readonly<{
      _tag: "interrupt";
      providerItemRef?: string;
      playedAudioMs?: number;
    }>
  | Readonly<{
      _tag: "close";
      reason: "user_stop" | "app_backgrounded" | "transport_error";
    }>
  | Readonly<{ _tag: "heartbeat" }>;

export type SarahVoicePhase =
  | "idle"
  | "connecting"
  | "listening"
  | "thinking"
  | "speaking"
  | "interrupted"
  | "reconnecting"
  | "ended"
  | "error";

export type SarahVoiceTranscript = Readonly<{
  utteranceRef: string;
  source: "user" | "assistant";
  text: string;
  final: boolean;
}>;

export type SarahVoiceSnapshot = Readonly<{
  phase: SarahVoicePhase;
  muted: boolean;
  message: string | null;
  retryable: boolean;
  transcripts: ReadonlyArray<SarahVoiceTranscript>;
  reservedCreditMsat: number | null;
}>;

export interface SarahVoiceSocket {
  binaryType: string;
  readyState: number;
  onopen: (() => void) | null;
  onmessage: ((event: Readonly<{ data: unknown }>) => void) | null;
  onerror: (() => void) | null;
  onclose: ((event: Readonly<{ code: number; reason: string }>) => void) | null;
  send: (data: string | ArrayBuffer) => void;
  close: (code?: number, reason?: string) => void;
}

export type SarahVoiceSocketFactory = (
  url: string,
  headers: Readonly<Record<string, string>>,
) => SarahVoiceSocket;

export type SarahVoiceClientDependencies = Readonly<{
  baseUrl: string;
  publicKeyHex: string;
  signer: Issue31NostrSigner;
  vault: SarahVoiceSessionVault;
  fetch: typeof globalThis.fetch;
  createSocket: SarahVoiceSocketFactory;
  sha256: Sha256;
  randomUuid: () => string;
  now: () => number;
  setTimeout: (callback: () => void, delayMs: number) => ReturnType<typeof setTimeout>;
  clearTimeout: (handle: ReturnType<typeof setTimeout>) => void;
}>;

const initialSnapshot: SarahVoiceSnapshot = {
  phase: "idle",
  muted: false,
  message: null,
  retryable: false,
  transcripts: [],
  reservedCreditMsat: null,
};

const safeBaseUrl = (value: string): string => {
  const url = new URL(value);
  if (!["http:", "https:"].includes(url.protocol) || url.username !== "" || url.password !== "") {
    throw new Error("The OpenAgents voice service URL is invalid.");
  }
  url.pathname = "/";
  url.search = "";
  url.hash = "";
  return url.toString().replace(/\/$/u, "");
};

const safeGatewayUrl = (value: string, baseUrl: string): string => {
  const gateway = new URL(value);
  const base = new URL(baseUrl);
  const expectedProtocol = base.protocol === "https:" ? "wss:" : "ws:";
  if (
    gateway.protocol !== expectedProtocol ||
    gateway.username !== "" ||
    gateway.password !== "" ||
    gateway.origin !== `${expectedProtocol}//${base.host}`
  ) {
    throw new Error("The OpenAgents voice gateway URL is invalid.");
  }
  return gateway.toString();
};

const statusMessage = (status: number, error: string | undefined): Readonly<{
  message: string;
  retryable: boolean;
}> => {
  if (status === 401) {
    return {
      message: "This protected device identity is not linked to an OpenAgents session.",
      retryable: false,
    };
  }
  if (status === 402 || error === "insufficient_credit") {
    return { message: "This account needs more OpenAgents credits for voice.", retryable: false };
  }
  if (status === 403) {
    return { message: "Sarah voice is not enabled for this OpenAgents account.", retryable: false };
  }
  if (status === 409) {
    return { message: "Another Sarah voice session is still closing. Try again.", retryable: true };
  }
  if (status === 429) {
    return { message: "Voice sign-in is busy. Wait a moment, then retry.", retryable: true };
  }
  return { message: "Sarah voice is unavailable. Check the network and try again.", retryable: true };
};

const readError = async (response: Response): Promise<string | undefined> => {
  try {
    const value = (await response.json()) as unknown;
    if (typeof value === "object" && value !== null && "error" in value) {
      return typeof value.error === "string" ? value.error : undefined;
    }
  } catch {
    // A non-JSON service response stays product-safe.
  }
  return undefined;
};

const sameIdentity = (left: VoiceIdentity, right: VoiceIdentity): boolean =>
  left.ownerRef === right.ownerRef &&
  left.deviceRef === right.deviceRef &&
  left.threadRef === right.threadRef &&
  left.sessionRef === right.sessionRef &&
  left.generation === right.generation;

export class SarahVoiceClient {
  private snapshotValue: SarahVoiceSnapshot = initialSnapshot;
  private readonly listeners = new Set<(snapshot: SarahVoiceSnapshot) => void>();
  private readonly audioListeners = new Set<
    (audio: Readonly<{ itemRef: string; pcm: Uint8Array }>) => void
  >();
  private socket: SarahVoiceSocket | null = null;
  private identity: VoiceIdentity | null = null;
  private controlSequence = 0;
  private serverControlSequence = 0;
  private audioSequence = 0;
  private serverAudioSequence = 0;
  private pendingAudioFrames = 0;
  private audioChain: Promise<void> = Promise.resolve();
  private messageChain: Promise<void> = Promise.resolve();
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private shouldRun = false;
  private foreground = true;
  private attemptGeneration = 0;
  private currentProviderItemRef: string | null = null;

  constructor(private readonly dependencies: SarahVoiceClientDependencies) {}

  snapshot = (): SarahVoiceSnapshot => this.snapshotValue;

  subscribe = (listener: (snapshot: SarahVoiceSnapshot) => void): (() => void) => {
    this.listeners.add(listener);
    listener(this.snapshotValue);
    return () => this.listeners.delete(listener);
  };

  onAudio = (
    listener: (audio: Readonly<{ itemRef: string; pcm: Uint8Array }>) => void,
  ): (() => void) => {
    this.audioListeners.add(listener);
    return () => this.audioListeners.delete(listener);
  };

  start = async (): Promise<void> => {
    if (!this.foreground || ["connecting", "listening", "thinking", "speaking"].includes(
      this.snapshotValue.phase,
    )) {
      return;
    }
    this.shouldRun = true;
    this.update({
      ...initialSnapshot,
      phase: "connecting",
      transcripts: this.snapshotValue.transcripts,
    });
    await this.connect(0);
  };

  retry = async (): Promise<void> => {
    await this.end("transport_error", false);
    await this.start();
  };

  setMuted = (muted: boolean): void => {
    this.update({ ...this.snapshotValue, muted });
  };

  setForeground = (foreground: boolean): void => {
    this.foreground = foreground;
    if (!foreground) {
      void this.end("app_backgrounded");
    }
  };

  sendAudio = (pcm: Uint8Array, sampleRate: number, channels: number): void => {
    if (
      this.snapshotValue.phase !== "listening" ||
      this.snapshotValue.muted ||
      sampleRate !== 24_000 ||
      channels !== 1 ||
      this.socket?.readyState !== 1 ||
      this.identity === null
    ) {
      return;
    }
    if (this.pendingAudioFrames >= MAX_PENDING_AUDIO_FRAMES) {
      this.transportFailure("The network cannot keep up with microphone audio.");
      return;
    }
    this.pendingAudioFrames += 1;
    const sequence = this.audioSequence;
    this.audioSequence += 1;
    const socket = this.socket;
    const identity = this.identity;
    const generation = this.attemptGeneration;
    this.audioChain = this.audioChain.then(async () => {
      if (
        socket !== this.socket ||
        socket.readyState !== 1 ||
        generation !== this.attemptGeneration
      ) return;
      const frame = await encodeClientAudioFrame({
        identity,
        sequence,
        pcm,
        sha256: this.dependencies.sha256,
      });
      if (
        socket !== this.socket ||
        socket.readyState !== 1 ||
        generation !== this.attemptGeneration ||
        !this.shouldRun ||
        !this.foreground ||
        this.identity === null ||
        !sameIdentity(this.identity, identity)
      ) return;
      socket.send(frame);
    }).catch(() => {
      if (
        socket === this.socket &&
        generation === this.attemptGeneration &&
        this.shouldRun
      ) {
        this.transportFailure("The microphone audio could not be sent.");
      }
    });
  };

  interrupt = (playedAudioMs: number): void => {
    this.sendControl({
      _tag: "interrupt",
      ...(this.currentProviderItemRef === null
        ? {}
        : {
            providerItemRef: this.currentProviderItemRef,
            playedAudioMs: Math.max(0, Math.floor(playedAudioMs)),
          }),
    });
    this.update({ ...this.snapshotValue, phase: "interrupted" });
  };

  end = async (
    reason: "user_stop" | "app_backgrounded" | "transport_error" = "user_stop",
    replaceState = true,
  ): Promise<void> => {
    this.shouldRun = false;
    this.attemptGeneration += 1;
    if (this.reconnectTimer !== null) {
      this.dependencies.clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    if (this.socket?.readyState === 1) {
      this.sendControl({ _tag: "close", reason });
    }
    const socket = this.socket;
    this.socket = null;
    socket?.close(1000, reason);
    this.identity = null;
    this.pendingAudioFrames = 0;
    this.currentProviderItemRef = null;
    if (replaceState) {
      this.update({
        ...this.snapshotValue,
        phase: "ended",
        message: null,
        retryable: false,
        reservedCreditMsat: null,
      });
    }
  };

  private update(snapshot: SarahVoiceSnapshot): void {
    this.snapshotValue = snapshot;
    for (const listener of this.listeners) listener(snapshot);
  }

  private async connect(reconnectAttempt: number): Promise<void> {
    const generation = ++this.attemptGeneration;
    try {
      const session = await this.requestSession();
      if (!this.shouldRun || !this.foreground || generation !== this.attemptGeneration) return;
      this.identity = session.identity;
      this.controlSequence = 0;
      this.serverControlSequence = 0;
      this.audioSequence = 0;
      this.serverAudioSequence = 0;
      this.pendingAudioFrames = 0;
      this.messageChain = Promise.resolve();
      const socket = this.dependencies.createSocket(session.gatewayUrl, {
        "x-openagents-sarah-voice-session": session.identity.sessionRef,
        "x-openagents-sarah-voice-ticket": session.ticket,
      });
      socket.binaryType = "arraybuffer";
      this.socket = socket;
      socket.onopen = () => {
        if (socket !== this.socket) return;
        this.sendControl({ _tag: "session_hello", disclosureRef: DISCLOSURE_REF });
        this.update({
          ...this.snapshotValue,
          phase: "connecting",
          reservedCreditMsat: session.reservedCreditMsat,
        });
      };
      socket.onmessage = (event) => {
        if (socket !== this.socket) return;
        this.messageChain = this.messageChain.then(async () => {
          if (socket !== this.socket) return;
          await this.handleMessage(event.data, socket, generation);
        });
      };
      socket.onerror = () => {
        if (socket === this.socket) socket.close(1011, "transport_error");
      };
      socket.onclose = () => {
        if (socket !== this.socket) return;
        this.socket = null;
        this.identity = null;
        this.pendingAudioFrames = 0;
        if (!this.shouldRun || !this.foreground) return;
        if (reconnectAttempt >= MAX_RECONNECT_ATTEMPTS) {
          this.update({
            ...this.snapshotValue,
            phase: "error",
            message: "Sarah voice disconnected. Retry when the network is ready.",
            retryable: true,
          });
          return;
        }
        this.update({ ...this.snapshotValue, phase: "reconnecting", message: null });
        this.reconnectTimer = this.dependencies.setTimeout(() => {
          this.reconnectTimer = null;
          void this.connect(reconnectAttempt + 1);
        }, reconnectAttempt === 0 ? 500 : 1_500);
      };
    } catch (error: unknown) {
      if (generation !== this.attemptGeneration) return;
      const failure =
        typeof error === "object" &&
        error !== null &&
        "message" in error &&
        "retryable" in error
          ? (error as Readonly<{ message: string; retryable: boolean }>)
          : {
              message: error instanceof Error ? error.message : "Sarah voice is unavailable.",
              retryable: true,
            };
      this.shouldRun = false;
      this.update({
        ...this.snapshotValue,
        phase: "error",
        message: failure.message,
        retryable: failure.retryable,
      });
    }
  }

  private async requestSession(): Promise<Readonly<{
    identity: VoiceIdentity;
    gatewayUrl: string;
    ticket: string;
    reservedCreditMsat: number;
  }>> {
    const baseUrl = safeBaseUrl(this.dependencies.baseUrl);
    const deviceRef = `omega-mobile-${this.dependencies.publicKeyHex.slice(0, 24)}`;
    const stored = await this.dependencies.vault.read(
      this.dependencies.publicKeyHex,
      this.dependencies.now(),
    );
    if (stored !== null) {
      const bearer = await this.createSession({
        baseUrl,
        deviceRef,
        ownerRef: stored.ownerRef,
        authorization: `Bearer ${stored.accessToken}`,
      });
      if (bearer._tag === "Success") return bearer.value;
      if (bearer.status !== 401) throw bearer.failure;
      await this.dependencies.vault.clear();
    }

    const challengeResponse = await this.dependencies.fetch(
      `${baseUrl}${SARAH_VOICE_NOSTR_CHALLENGE_PATH}`,
      {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          schema: SARAH_VOICE_NOSTR_CHALLENGE_PROTOCOL_VERSION,
          deviceRef,
          pubkey: this.dependencies.publicKeyHex,
        }),
      },
    );
    if (!challengeResponse.ok) {
      const error = await readError(challengeResponse);
      throw statusMessage(challengeResponse.status, error);
    }
    const challenge = decodeSarahVoiceNostrChallengeResponse(
      await challengeResponse.json(),
    );
    const result = await this.createSession({
      baseUrl,
      deviceRef,
      ownerRef: challenge.ownerRef,
      challenge: challenge.challenge,
    });
    if (result._tag === "Failure") throw result.failure;
    if (result.accessToken !== undefined) {
      await this.dependencies.vault.write({
        schemaVersion: 1,
        publicKeyHex: this.dependencies.publicKeyHex,
        ownerRef: challenge.ownerRef,
        accessToken: result.accessToken,
        expiresAtMs: this.dependencies.now() + result.expiresIn * 1_000,
      });
    }
    return result.value;
  }

  private async createSession(input: Readonly<{
    baseUrl: string;
    deviceRef: string;
    ownerRef: string;
    authorization?: string;
    challenge?: string;
  }>): Promise<
    | Readonly<{
        _tag: "Success";
        value: Readonly<{
          identity: VoiceIdentity;
          gatewayUrl: string;
          ticket: string;
          reservedCreditMsat: number;
        }>;
        accessToken?: string;
        expiresIn: number;
      }>
    | Readonly<{
        _tag: "Failure";
        status: number;
        failure: Readonly<{ message: string; retryable: boolean }>;
      }>
  > {
    const identity: VoiceIdentity = {
      ownerRef: input.ownerRef,
      deviceRef: input.deviceRef,
      threadRef: `thread.sarah.mobile.${this.dependencies.publicKeyHex.slice(0, 24)}`,
      sessionRef: `sarah.voice.${this.dependencies.randomUuid()}`,
      generation: 1,
    };
    const body = JSON.stringify({
      schema: SARAH_VOICE_PROTOCOL_VERSION,
      identity,
      disclosureRef: DISCLOSURE_REF,
      clientProfile: "mobile_voice_only",
      ...(input.challenge === undefined
        ? {}
        : {
            auth: {
              method: SARAH_VOICE_NOSTR_AUTH_METHOD,
              challenge: input.challenge,
            },
          }),
    });
    let authorization = input.authorization;
    if (input.challenge !== undefined) {
      const url = `${input.baseUrl}${SARAH_VOICE_SESSION_PATH}`;
      const signed = await this.dependencies.signer.signEvent({
        kind: 27_235,
        created_at: Math.floor(this.dependencies.now() / 1_000),
        tags: [
          ["u", url],
          ["method", "POST"],
          ["payload", await sha256Hex(new TextEncoder().encode(body), this.dependencies.sha256)],
        ],
        content: "",
      });
      authorization = encodeNip98Authorization(signed);
    }
    const response = await this.dependencies.fetch(
      `${input.baseUrl}${SARAH_VOICE_SESSION_PATH}`,
      {
        method: "POST",
        headers: {
          authorization: authorization ?? "",
          "content-type": "application/json",
          "x-openagents-omega-device-ref": input.deviceRef,
        },
        body,
      },
    );
    if (!response.ok) {
      const error = await readError(response);
      return {
        _tag: "Failure",
        status: response.status,
        failure: statusMessage(response.status, error),
      };
    }
    const session = decodeSarahVoiceSessionResponse(await response.json());
    if (
      session.clientProfile !== "mobile_voice_only" ||
      session.sessionRef !== identity.sessionRef
    ) {
      return {
        _tag: "Failure",
        status: 500,
        failure: {
          message: "The voice service did not preserve the mobile safety profile.",
          retryable: false,
        },
      };
    }
    return {
      _tag: "Success",
      value: {
        identity,
        gatewayUrl: safeGatewayUrl(session.gatewayUrl, input.baseUrl),
        ticket: session.ticket,
        reservedCreditMsat: session.reservedCreditMsat,
      },
      ...(session.auth === undefined
        ? {}
        : {
            accessToken: session.auth.accessToken,
            expiresIn: session.auth.expiresIn,
          }),
      expiresIn: session.auth?.expiresIn ?? 0,
    };
  }

  private sendControl(
    frame: ClientControlPayload,
  ): void {
    if (this.socket?.readyState !== 1 || this.identity === null) return;
    this.socket.send(
      JSON.stringify({
        schema: SARAH_VOICE_PROTOCOL_VERSION,
        identity: this.identity,
        sequence: this.controlSequence,
        ...frame,
      }),
    );
    this.controlSequence += 1;
  }

  private async handleMessage(
    data: unknown,
    socket: SarahVoiceSocket,
    generation: number,
  ): Promise<void> {
    try {
      if (socket !== this.socket || generation !== this.attemptGeneration) return;
      if (typeof data === "string") {
        const control = decodeSarahVoiceServerControl(JSON.parse(data) as unknown);
        if (socket !== this.socket || generation !== this.attemptGeneration) return;
        this.handleControl(control);
        return;
      }
      const identity = this.identity;
      if (!(data instanceof ArrayBuffer) || identity === null) {
        throw new Error("The Sarah voice transport returned an unsupported frame.");
      }
      const audio = await decodeServerAudioFrame({
        frame: data,
        identity,
        expectedSequence: this.serverAudioSequence,
        sha256: this.dependencies.sha256,
      });
      if (
        socket !== this.socket ||
        generation !== this.attemptGeneration ||
        !this.shouldRun ||
        !this.foreground ||
        this.identity === null ||
        !sameIdentity(this.identity, identity)
      ) return;
      this.serverAudioSequence += 1;
      this.currentProviderItemRef = audio.itemRef;
      for (const listener of this.audioListeners) listener(audio);
    } catch {
      if (
        socket === this.socket &&
        generation === this.attemptGeneration &&
        this.shouldRun
      ) {
        this.transportFailure("Sarah voice returned an invalid transport frame.");
      }
    }
  }

  private handleControl(control: SarahVoiceServerControl): void {
    if (
      this.identity === null ||
      !sameIdentity(control.identity, this.identity) ||
      control.sequence !== this.serverControlSequence
    ) {
      this.transportFailure("Sarah voice lost transport sequence.");
      return;
    }
    this.serverControlSequence += 1;
    switch (control._tag) {
      case "session_ready":
        this.update({
          ...this.snapshotValue,
          phase: "connecting",
          reservedCreditMsat: control.reservedCreditMsat,
        });
        return;
      case "lifecycle":
        this.update({
          ...this.snapshotValue,
          phase: control.state === "closing" ? "ended" : control.state,
        });
        return;
      case "transcript_delta":
      case "transcript_final": {
        const next = this.snapshotValue.transcripts.filter(
          (entry) => entry.utteranceRef !== control.utteranceRef,
        );
        next.push({
          utteranceRef: control.utteranceRef,
          source: control.source,
          text: control.text,
          final: control._tag === "transcript_final",
        });
        this.update({ ...this.snapshotValue, transcripts: next.slice(-40) });
        return;
      }
      case "audio_ack":
        this.pendingAudioFrames = Math.max(0, this.pendingAudioFrames - 1);
        return;
      case "interrupt_ack":
        this.update({ ...this.snapshotValue, phase: "listening" });
        return;
      case "heartbeat":
      case "tool_outcome_ref":
        return;
      case "tool_proposal":
      case "tool_execute":
        this.shouldRun = false;
        this.transportFailure(
          "Mobile Sarah voice refused an unsupported device action.",
          false,
        );
        return;
      case "error":
        if (control.code === "credit_limit") {
          this.shouldRun = false;
          this.update({
            ...this.snapshotValue,
            phase: "error",
            message: "This voice session reached its OpenAgents credit limit.",
            retryable: false,
          });
          this.socket?.close(1008, "credit_limit");
          return;
        }
        if (!control.retryable) {
          this.shouldRun = false;
        }
        this.transportFailure(
          control.retryable
            ? "Sarah voice had a temporary service error."
            : "Sarah voice stopped because the session was invalid.",
          control.retryable,
        );
        return;
      case "closing":
        this.shouldRun = false;
        this.update({
          ...this.snapshotValue,
          phase: control.reason === "credit_limit" ? "error" : "ended",
          message:
            control.reason === "credit_limit"
              ? "This voice session reached its OpenAgents credit limit."
              : null,
          retryable: control.reason === "provider_error" || control.reason === "transport_error",
        });
        return;
    }
  }

  private transportFailure(message: string, retryable = true): void {
    this.update({ ...this.snapshotValue, phase: "error", message, retryable });
    if (this.socket !== null) {
      this.socket.close(1011, "transport_error");
      return;
    }
    this.shouldRun = false;
  }
}
