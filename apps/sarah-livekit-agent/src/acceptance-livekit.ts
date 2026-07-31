import {
  AudioFrame,
  AudioSource,
  AudioStream,
  LocalAudioTrack,
  Room,
  RoomEvent,
  TrackKind,
  TrackPublishOptions,
  TrackSource,
  type RemoteParticipant,
  type RemoteTrack,
  type RemoteTrackPublication,
} from "@livekit/rtc-node";
import {
  SARAH_VOICE_ADMISSION_PATH,
  SARAH_VOICE_ADMISSION_PROTOCOL_VERSION,
  SARAH_VOICE_CONNECT_PATH,
  SARAH_VOICE_PROTOCOL_VERSION,
  SARAH_VOICE_SESSION_PATH,
  SARAH_VOICE_SETTLEMENT_PATH,
  decodeSarahVoiceAdmissionResponse,
  decodeSarahVoiceServerControl,
  decodeSarahVoiceSessionResponse,
  decodeSarahVoiceSettlementResponse,
  type VoiceIdentity,
} from "@openagentsinc/audio-contract";
import WebSocket, { type ClientOptions, type RawData } from "ws";
import {
  MAX_ACCEPTANCE_INTERRUPT_AUDIO_TAIL_MS,
  digestSettlementReceipt,
  type SarahLiveKitAcceptanceScenario,
  type SarahLiveKitScenarioObservation,
} from "./acceptance-harness.js";
import { classifySelectedIcePath, type RtcStatsEntry } from "./ice-classification.js";
import { mintProductionSubscriberGrant } from "./acceptance-deployment.js";

const API_ORIGIN = "https://openagents.com";
const LIVEKIT_ORIGIN = "wss://livekit.openagents.com";
const TRANSCRIPTION_TOPIC = "lk.transcription";
const SCENARIO_TIMEOUT_MS = 60_000;
const SETTLEMENT_TIMEOUT_MS = 45_000;
const INTERRUPT_AUDIO_QUIET_WINDOW_MS = 250;
const COMMUNITY_ROOM_JOIN_PATH = "/api/sarah/livekit/room/join";
const COMMUNITY_ROOM_MEMBER_FLOOR_PATH = "/api/sarah/livekit/room/floor/member";

type Clock = Readonly<{
  now: () => number;
  sleep: (durationMs: number) => Promise<void>;
}>;

type Http = (input: string | URL, init?: RequestInit) => Promise<Response>;

export const measureSarahInterruptAudioTail = async (
  acknowledgedAtMs: number,
  lastAudibleAt: () => number | undefined,
  clock: Clock,
): Promise<number> => {
  const deadline =
    acknowledgedAtMs + MAX_ACCEPTANCE_INTERRUPT_AUDIO_TAIL_MS + INTERRUPT_AUDIO_QUIET_WINDOW_MS;
  while (clock.now() <= deadline) {
    const lastAudible = Math.max(lastAudibleAt() ?? acknowledgedAtMs, acknowledgedAtMs);
    if (clock.now() - lastAudible >= INTERRUPT_AUDIO_QUIET_WINDOW_MS) {
      const tailMs = Math.max(0, lastAudible - acknowledgedAtMs);
      if (tailMs <= MAX_ACCEPTANCE_INTERRUPT_AUDIO_TAIL_MS) return tailMs;
      break;
    }
    // eslint-disable-next-line no-await-in-loop
    await clock.sleep(50);
  }
  throw new Error("Sarah audio did not become quiet after the explicit interrupt");
};

export type SarahLiveKitAcceptanceControlSocket = Readonly<{
  onOpen: (listener: () => void) => void;
  onMessage: (listener: (data: RawData, isBinary: boolean) => void) => void;
  onError: (listener: () => void) => void;
  onClose: (listener: () => void) => void;
  send: (message: string) => void;
  terminate: () => void;
}>;

export type SarahLiveKitAcceptanceControlSocketFactory = (
  url: string,
  options: ClientOptions,
) => SarahLiveKitAcceptanceControlSocket;

export type SarahLiveKitLiveDependencies = Readonly<{
  clock?: Clock;
  fetch?: Http;
  mintSubscriberGrant?: (roomRef: string, subscriberRef: string) => Promise<string>;
  controlSocketFactory?: SarahLiveKitAcceptanceControlSocketFactory;
}>;

const defaultControlSocketFactory: SarahLiveKitAcceptanceControlSocketFactory = (url, options) => {
  const socket = new WebSocket(url, options);
  return {
    onOpen: (listener) => {
      socket.on("open", listener);
    },
    onMessage: (listener) => {
      socket.on("message", listener);
    },
    onError: (listener) => {
      socket.on("error", listener);
    },
    onClose: (listener) => {
      socket.on("close", listener);
    },
    send: (message) => {
      socket.send(message);
    },
    terminate: () => {
      socket.terminate();
    },
  };
};

const responseError = async (response: Response, operation: string): Promise<Error> => {
  let code = "unknown";
  try {
    const body = (await response.json()) as unknown;
    if (
      typeof body === "object" &&
      body !== null &&
      "error" in body &&
      typeof body.error === "string" &&
      /^[a-z0-9_]{1,128}$/u.test(body.error)
    ) {
      code = body.error;
    }
  } catch {
    code = "invalid_response";
  }
  return new Error(`${operation} failed with HTTP ${response.status} (${code})`);
};

const postJson = async (
  http: Http,
  path: string,
  bearer: string,
  deviceRef: string,
  body: unknown,
): Promise<unknown> => {
  const response = await http(`${API_ORIGIN}${path}`, {
    method: "POST",
    headers: {
      authorization: `Bearer ${bearer}`,
      "content-type": "application/json",
      "x-openagents-omega-device-ref": deviceRef,
    },
    body: JSON.stringify(body),
    redirect: "error",
  });
  if (!response.ok) throw await responseError(response, path);
  return response.json();
};

const record = (value: unknown): Readonly<Record<string, unknown>> | undefined =>
  typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as Readonly<Record<string, unknown>>)
    : undefined;

export const acquireSarahLiveKitCommunityFloor = async (
  input: Readonly<{
    fetch: Http;
    bearer: string;
    deviceRef: string;
    communityRef: string;
    channelRef: string;
    roomRef: string;
    participantRef: string;
  }>,
): Promise<void> => {
  const joined = record(
    await postJson(input.fetch, COMMUNITY_ROOM_JOIN_PATH, input.bearer, input.deviceRef, {
      communityRef: input.communityRef,
      channelRef: input.channelRef,
    }),
  );
  const authority = record(joined?.authority);
  const presenceLeaseRef = joined?.presenceLeaseRef;
  const revision = authority?.revision;
  if (
    joined?.roomRef !== input.roomRef ||
    joined.participantRef !== input.participantRef ||
    typeof presenceLeaseRef !== "string" ||
    presenceLeaseRef.trim() !== presenceLeaseRef ||
    presenceLeaseRef.length < 1 ||
    presenceLeaseRef.length > 256 ||
    !Number.isSafeInteger(revision) ||
    (revision as number) < 1
  ) {
    throw new Error("community Sarah room join authority did not match the admitted session");
  }
  const floor = record(
    await postJson(input.fetch, COMMUNITY_ROOM_MEMBER_FLOOR_PATH, input.bearer, input.deviceRef, {
      action: "acquire",
      presenceLeaseRef,
      expectedRevision: revision,
      nonce: crypto.randomUUID().replaceAll("-", ""),
      requestedLeaseMs: 30_000,
    }),
  );
  if (
    !Number.isSafeInteger(floor?.revision) ||
    (floor?.revision as number) <= (revision as number)
  ) {
    throw new Error("community Sarah speaking floor was not acquired");
  }
};

const timeout = async <T>(operation: Promise<T>, timeoutMs: number, label: string): Promise<T> => {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const deadline = new Promise<never>((_, reject) => {
    timer = setTimeout(() => reject(new Error(`${label} timed out`)), timeoutMs);
  });
  try {
    return await Promise.race([operation, deadline]);
  } finally {
    if (timer !== undefined) clearTimeout(timer);
  }
};

const deferred = <T>() => {
  let resolve: ((value: T) => void) | undefined;
  let reject: ((error: Error) => void) | undefined;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return {
    promise,
    resolve: (value: T) => resolve?.(value),
    reject: (error: Error) => reject?.(error),
  };
};

const sameVoiceIdentity = (left: VoiceIdentity, right: VoiceIdentity): boolean =>
  left.ownerRef === right.ownerRef &&
  left.deviceRef === right.deviceRef &&
  left.threadRef === right.threadRef &&
  left.sessionRef === right.sessionRef &&
  left.generation === right.generation;

const controlText = (data: RawData): string => {
  if (Buffer.isBuffer(data)) return data.toString("utf8");
  if (Array.isArray(data)) return Buffer.concat(data).toString("utf8");
  return Buffer.from(data).toString("utf8");
};

const validGatewayUrl = (value: string): boolean => {
  try {
    const url = new URL(value);
    return (
      url.protocol === "wss:" &&
      url.hostname === "openagents.com" &&
      url.port === "" &&
      url.username === "" &&
      url.password === "" &&
      url.pathname === SARAH_VOICE_CONNECT_PATH &&
      url.search === "" &&
      url.hash === ""
    );
  } catch {
    return false;
  }
};

/**
 * The first terminal signal the control channel observed.
 *
 * The acceptance path never needs this: it closes the channel itself and any
 * other termination is a failure it reports through the rejected `ready`,
 * `interrupt`, or `close` promise. A failure drill needs the opposite reading.
 * Termination is the event under test, so the instant and the server's own
 * stated reason are evidence rather than an error, and they have to be
 * preserved even when the transport dies before a `closing` frame arrives.
 *
 * `terminal` therefore resolves and never rejects. It is purely additive: the
 * existing failure semantics above are untouched, so the two-room acceptance
 * receipt keeps failing closed on exactly the conditions it always did.
 */
export type SarahLiveKitControlTerminal = Readonly<{
  atMs: number;
  kind: "closing" | "error" | "transport_closed" | "transport_error";
  /**
   * The server's typed `closing.reason` or `error.code` when one arrived, and
   * otherwise a public-safe name for the transport event observed instead.
   */
  reason: string;
}>;

export type SarahLiveKitAcceptanceControlChannel = Readonly<{
  ready: Promise<number>;
  terminal: Promise<SarahLiveKitControlTerminal>;
  interrupt: () => Promise<Readonly<{ acknowledgedAtMs: number; interruptedAtMs: number }>>;
  close: () => Promise<void>;
  dispose: () => void;
}>;

export const openSarahLiveKitAcceptanceControlChannel = (
  input: Readonly<{
    gatewayUrl: string;
    ticket: string;
    identity: VoiceIdentity;
    disclosureRef: string;
  }>,
  now: () => number,
  socketFactory: SarahLiveKitAcceptanceControlSocketFactory = defaultControlSocketFactory,
): SarahLiveKitAcceptanceControlChannel => {
  if (!validGatewayUrl(input.gatewayUrl) || !/^[A-Za-z0-9_-]{32,256}$/u.test(input.ticket)) {
    throw new Error("Sarah LiveKit acceptance control grant is invalid");
  }
  const socket = socketFactory(input.gatewayUrl, {
    headers: {
      "x-openagents-sarah-voice-session": input.identity.sessionRef,
      "x-openagents-sarah-voice-ticket": input.ticket,
    },
    handshakeTimeout: 10_000,
    maxPayload: 32_768,
  });
  const ready = deferred<number>();
  const terminal = deferred<SarahLiveKitControlTerminal>();
  let terminalObserved = false;
  const observeTerminal = (kind: SarahLiveKitControlTerminal["kind"], reason: string): void => {
    if (terminalObserved) return;
    terminalObserved = true;
    terminal.resolve({ atMs: now(), kind, reason });
  };
  let interrupt:
    | ReturnType<typeof deferred<Readonly<{ acknowledgedAtMs: number; interruptedAtMs: number }>>>
    | undefined;
  let close: ReturnType<typeof deferred<void>> | undefined;
  let expectedServerSequence = 0;
  let clientSequence = 0;
  let readyObserved = false;
  let interruptAcknowledgedAtMs: number | undefined;
  let interruptedAtMs: number | undefined;
  let closeRequested = false;
  let closed = false;
  let failed = false;

  const fail = (message: string): void => {
    if (failed || closed) return;
    failed = true;
    const error = new Error(message);
    ready.reject(error);
    interrupt?.reject(error);
    close?.reject(error);
    socket.terminate();
  };
  const sendControl = (control: Readonly<Record<string, unknown>>): void => {
    if (failed || closed) {
      throw new Error("Sarah LiveKit acceptance control channel is unavailable");
    }
    try {
      socket.send(
        JSON.stringify({
          schema: SARAH_VOICE_PROTOCOL_VERSION,
          identity: input.identity,
          sequence: clientSequence,
          ...control,
        }),
      );
      clientSequence += 1;
    } catch {
      fail("Sarah LiveKit acceptance control send failed");
      throw new Error("Sarah LiveKit acceptance control send failed");
    }
  };
  const resolveInterrupt = (): void => {
    if (
      interrupt !== undefined &&
      interruptAcknowledgedAtMs !== undefined &&
      interruptedAtMs !== undefined
    ) {
      interrupt.resolve({
        acknowledgedAtMs: interruptAcknowledgedAtMs,
        interruptedAtMs,
      });
    }
  };

  socket.onOpen(() => {
    try {
      sendControl({
        _tag: "session_hello",
        disclosureRef: input.disclosureRef,
      });
    } catch {
      // sendControl already failed the channel with a public-safe error.
    }
  });
  socket.onMessage((data, isBinary) => {
    if (isBinary) {
      fail("Sarah LiveKit acceptance control channel received binary media");
      return;
    }
    try {
      const control = decodeSarahVoiceServerControl(JSON.parse(controlText(data)) as unknown);
      if (
        !sameVoiceIdentity(control.identity, input.identity) ||
        control.sequence !== expectedServerSequence
      ) {
        fail("Sarah LiveKit acceptance control authority or sequence disagreed");
        return;
      }
      expectedServerSequence += 1;
      if (control["_tag"] === "closing") {
        observeTerminal("closing", control.reason);
      } else if (control["_tag"] === "error") {
        observeTerminal("error", control.code);
        fail(`Sarah LiveKit acceptance control returned ${control.code}`);
      } else if (control["_tag"] === "session_ready") {
        readyObserved = true;
        ready.resolve(now());
      } else if (control["_tag"] === "interrupt_ack") {
        interruptAcknowledgedAtMs = now();
        resolveInterrupt();
      } else if (control["_tag"] === "lifecycle" && control.state === "interrupted") {
        interruptedAtMs = now();
        resolveInterrupt();
      }
    } catch {
      fail("Sarah LiveKit acceptance control frame was invalid");
    }
  });
  socket.onError(() => {
    observeTerminal("transport_error", "control_transport_error");
    fail("Sarah LiveKit acceptance control transport failed");
  });
  socket.onClose(() => {
    observeTerminal(
      "transport_closed",
      closeRequested ? "user_stop" : "closed_without_terminal_frame",
    );
    if (closeRequested) {
      closed = true;
      close?.resolve();
      return;
    }
    fail("Sarah LiveKit acceptance control channel closed early");
  });

  return {
    ready: ready.promise,
    terminal: terminal.promise,
    interrupt: () => {
      if (!readyObserved || interrupt !== undefined) {
        return Promise.reject(
          new Error("Sarah LiveKit acceptance control interrupt is not available"),
        );
      }
      interrupt = deferred();
      sendControl({ _tag: "interrupt" });
      return interrupt.promise;
    },
    close: () => {
      if (close !== undefined) return close.promise;
      close = deferred();
      closeRequested = true;
      sendControl({ _tag: "close", reason: "user_stop" });
      return close.promise;
    },
    dispose: () => {
      if (closed) return;
      closed = true;
      socket.terminate();
    },
  };
};

const pcmSamples = (pcm: Uint8Array): Int16Array => {
  const view = new DataView(pcm.buffer, pcm.byteOffset, pcm.byteLength);
  const samples = new Int16Array(pcm.byteLength / 2);
  for (let index = 0; index < samples.length; index += 1) {
    samples[index] = view.getInt16(index * 2, true);
  }
  return samples;
};

const publishMicrophone = async (
  room: Room,
  pcm: Uint8Array,
): Promise<Readonly<{ track: LocalAudioTrack; source: AudioSource; publicationSid: string }>> => {
  const source = new AudioSource(24_000, 1, 100);
  const track = LocalAudioTrack.createAudioTrack("sarah-acceptance-microphone", source);
  const options = new TrackPublishOptions();
  options.source = TrackSource.SOURCE_MICROPHONE;
  const participant = room.localParticipant;
  if (participant === undefined) {
    await track.close();
    throw new Error("LiveKit local participant is unavailable");
  }
  try {
    const publication = await participant.publishTrack(track, options);
    if (publication.sid === undefined) throw new Error("LiveKit microphone publication has no SID");

    const samples = pcmSamples(pcm);
    const samplesPerFrame = 240;
    for (let offset = 0; offset < samples.length; offset += samplesPerFrame) {
      const length = Math.min(samplesPerFrame, samples.length - offset);
      const frame = AudioFrame.create(24_000, 1, samplesPerFrame);
      frame.data.set(samples.subarray(offset, offset + length));
      // A single native source must receive frames in playout order.
      // eslint-disable-next-line no-await-in-loop
      await source.captureFrame(frame);
    }
    const silenceFrames = 150;
    for (let index = 0; index < silenceFrames; index += 1) {
      // The semantic VAD needs ordered trailing silence to close the spoken turn.
      // eslint-disable-next-line no-await-in-loop
      await source.captureFrame(AudioFrame.create(24_000, 1, samplesPerFrame));
    }
    return { track, source, publicationSid: publication.sid };
  } catch (error) {
    await track.close();
    throw error;
  }
};

const decodeAdmission = (value: unknown) => {
  try {
    return decodeSarahVoiceAdmissionResponse(value);
  } catch {
    throw new Error("Sarah admission returned an invalid response");
  }
};

const decodeSession = (value: unknown) => {
  try {
    return decodeSarahVoiceSessionResponse(value);
  } catch {
    throw new Error("Sarah session returned an invalid response");
  }
};

const decodeSettlement = (value: unknown) => {
  try {
    return decodeSarahVoiceSettlementResponse(value);
  } catch {
    throw new Error("Sarah settlement returned an invalid response");
  }
};

const observeSarahOutputs = (room: Room, sarahParticipantRef: string, clock: Clock) => {
  const audio = deferred<number>();
  const transcription = deferred<number>();
  let audioAttached = false;
  let cancelAudio: (() => Promise<void>) | undefined;
  let lastAudibleAtMs: number | undefined;

  const attachAudio = (
    track: RemoteTrack,
    _publication: RemoteTrackPublication,
    participant: RemoteParticipant,
  ) => {
    if (
      audioAttached ||
      participant.identity !== sarahParticipantRef ||
      track.kind !== TrackKind.KIND_AUDIO
    ) {
      return;
    }
    audioAttached = true;
    const stream = new AudioStream(track, { sampleRate: 24_000, numChannels: 1 });
    void (async () => {
      const reader = stream.getReader();
      cancelAudio = async () => {
        await reader.cancel();
      };
      try {
        while (true) {
          // Audibility depends on the next frame from this one ordered stream.
          // eslint-disable-next-line no-await-in-loop
          const frame = await reader.read();
          if (frame.done) throw new Error("Sarah audio track ended before audible output");
          let audible = false;
          for (const sample of frame.value.data) {
            if (Math.abs(sample) > 8) {
              audible = true;
              break;
            }
          }
          if (audible) {
            lastAudibleAtMs = clock.now();
            audio.resolve(lastAudibleAtMs);
          }
        }
      } catch (error) {
        audio.reject(error instanceof Error ? error : new Error("Sarah audio observation failed"));
      } finally {
        reader.releaseLock();
      }
    })();
  };

  room.on(RoomEvent.TrackSubscribed, attachAudio);
  room.registerTextStreamHandler(TRANSCRIPTION_TOPIC, (reader, participant) => {
    void (async () => {
      let observedBytes = 0;
      try {
        for await (const chunk of reader) {
          observedBytes += Buffer.byteLength(chunk, "utf8");
        }
        if (participant.identity === sarahParticipantRef && observedBytes > 0) {
          transcription.resolve(clock.now());
        }
      } catch (error) {
        if (participant.identity === sarahParticipantRef) {
          transcription.reject(
            error instanceof Error ? error : new Error("Sarah transcription observation failed"),
          );
        }
      }
    })();
  });

  const attachExisting = () => {
    const sarah = room.remoteParticipants.get(sarahParticipantRef);
    sarah?.trackPublications.forEach((publication) => {
      if (publication.track !== undefined) {
        attachAudio(publication.track, publication, sarah);
      }
    });
  };

  return {
    audio: audio.promise,
    transcription: transcription.promise,
    attachExisting,
    measureInterruptTail: (acknowledgedAtMs: number) =>
      measureSarahInterruptAudioTail(acknowledgedAtMs, () => lastAudibleAtMs, clock),
    close: async () => {
      room.off(RoomEvent.TrackSubscribed, attachAudio);
      room.unregisterTextStreamHandler(TRANSCRIPTION_TOPIC);
      await cancelAudio?.().catch(() => {});
    },
  };
};

export type SarahLiveKitScenarioRtcStats = Readonly<{
  publisherStats: readonly RtcStatsEntry[];
  subscriberStats: readonly RtcStatsEntry[];
}>;

/**
 * Classify the selected ICE path on both peer connections, or fail closed.
 *
 * The connectivity release-gate row needs the path named, not merely counted, so
 * an unclassifiable pair is a scenario failure and the typed reason travels in
 * the error rather than degrading to an unattributed pass.
 */
export const classifySarahLiveKitScenarioIcePaths = (
  kind: SarahLiveKitAcceptanceScenario["kind"],
  stats: SarahLiveKitScenarioRtcStats,
): SarahLiveKitScenarioObservation["selectedIcePath"] => {
  const classify = (peer: "publisher" | "subscriber", entries: readonly RtcStatsEntry[]) => {
    const result = classifySelectedIcePath(entries);
    if (!result.classified) {
      throw new Error(`${kind} ${peer} selected ICE path was not classifiable (${result.reason})`);
    }
    return result.path;
  };
  return {
    publisher: classify("publisher", stats.publisherStats),
    subscriber: classify("subscriber", stats.subscriberStats),
  };
};

/**
 * One terminal settlement reading, with the two instants a bound is measured
 * against.
 *
 * `terminalObservedAtMs` is when this poller saw the settlement, not when the
 * authority made it terminal — those differ by at most one poll interval. A
 * drill measuring a bound from a fault must use the observed instant, which
 * overstates the interval and therefore fails closed: a run recorded as within
 * bound really was within bound. `lastNonTerminalObservedAtMs` publishes the
 * width of that uncertainty rather than hiding it.
 */
export type SarahLiveKitSettlementReading = Readonly<{
  settlement: ReturnType<typeof decodeSettlement>;
  terminalObservedAtMs: number;
  lastNonTerminalObservedAtMs: number;
}>;

export const SARAH_LIVEKIT_SETTLEMENT_POLL_INTERVAL_MS = 500;

/**
 * Poll the terminal settlement authority until it answers or the window closes.
 *
 * This reads plain authenticated HTTP against the API origin, so it survives the
 * loss of the media transport, the SFU, or the worker. That independence is what
 * makes it usable as the settlement observer for a failure drill whose whole
 * premise is that the LiveKit path is gone.
 *
 * Returns `null` when the window closes with no terminal settlement. A caller
 * proving a bound records that as a contradiction; a caller that merely needs
 * the settlement raises it as an error.
 */
export const pollSarahLiveKitSettlement = async (
  http: Http,
  clock: Clock,
  scenario: Readonly<{ bearer: string; sessionRef: string }>,
  window: Readonly<{ windowMs: number; intervalMs: number }> = {
    windowMs: SETTLEMENT_TIMEOUT_MS,
    intervalMs: SARAH_LIVEKIT_SETTLEMENT_POLL_INTERVAL_MS,
  },
): Promise<SarahLiveKitSettlementReading | null> => {
  const deadline = clock.now() + window.windowMs;
  let lastNonTerminalObservedAtMs = clock.now();
  while (clock.now() < deadline) {
    const attemptedAtMs = clock.now();
    // Each retry is authorized only after the preceding settlement read is non-terminal.
    // eslint-disable-next-line no-await-in-loop
    const response = await http(`${API_ORIGIN}${SARAH_VOICE_SETTLEMENT_PATH}`, {
      method: "GET",
      headers: {
        authorization: `Bearer ${scenario.bearer}`,
        "x-openagents-sarah-voice-session": scenario.sessionRef,
        "x-openagents-sarah-livekit-acceptance": "live-observation-v1",
      },
      redirect: "error",
    });
    if (response.ok) {
      // eslint-disable-next-line no-await-in-loop
      const settlement = decodeSettlement(await response.json());
      return { settlement, terminalObservedAtMs: clock.now(), lastNonTerminalObservedAtMs };
    }
    if (response.status !== 404) {
      // eslint-disable-next-line no-await-in-loop
      throw await responseError(response, SARAH_VOICE_SETTLEMENT_PATH);
    }
    lastNonTerminalObservedAtMs = attemptedAtMs;
    // Do not overlap reads of the same terminal accounting authority.
    // eslint-disable-next-line no-await-in-loop
    await clock.sleep(window.intervalMs);
  }
  return null;
};

const readSettlement = async (
  http: Http,
  clock: Clock,
  scenario: SarahLiveKitAcceptanceScenario,
) => {
  const reading = await pollSarahLiveKitSettlement(http, clock, scenario);
  if (reading === null) throw new Error("Sarah settlement did not become terminal");
  return reading.settlement;
};

type SarahOutputObserver = ReturnType<typeof observeSarahOutputs>;

/** The instants the live bring-up produced, before any scenario-specific step. */
export type SarahLiveKitLiveSessionTimings = Readonly<{
  startedAtMs: number;
  admissionLatencyMs: number;
  sessionLatencyMs: number;
  roomConnectLatencyMs: number;
  microphonePublishLatencyMs: number;
  /** When the owner room finished connecting. The room is live from here. */
  activeRoomStartedAtMs: number;
  firstSarahAudioAtMs: number;
  firstSarahAudioLatencyMs: number;
}>;

/**
 * One live Sarah generation, held open for the caller to do something to.
 *
 * This is the bring-up half of the production acceptance path, factored out
 * rather than reimplemented. Everything through the first audible Sarah frame is
 * identical for an acceptance capture and for a failure drill: the same
 * admission, the same `livekit_room_v1` session, the same room-scoped grant, the
 * same ticketed control channel, the same microphone publication. The two
 * diverge only in what happens next — the acceptance interrupts and settles
 * cleanly, the drill injects a fault — so only that tail belongs to either
 * caller.
 *
 * A drill driver that stood up its own session would prove something about the
 * drill driver. Holding the production path open is what makes a drill evidence
 * about production.
 *
 * The caller owns the handle and must `release()` it, normally in a `finally`.
 * `release()` is idempotent.
 */
export type SarahLiveKitLiveSession = Readonly<{
  kind: SarahLiveKitAcceptanceScenario["kind"];
  identity: VoiceIdentity;
  /** Room addressing, for targeting a fault. Private: never enters a receipt. */
  roomRef: string;
  participantRef: string;
  sarahParticipantRef: string;
  timings: SarahLiveKitLiveSessionTimings;
  room: Room;
  subscriberRoom: Room;
  control: SarahLiveKitAcceptanceControlChannel;
  output: SarahOutputObserver;
  subscriberOutput: SarahOutputObserver;
  /** Resolves once both the owner and the secondary subscriber heard Sarah. */
  fanoutAudio: Promise<readonly [number, number]>;
  clock: Clock;
  http: Http;
  unpublishMicrophone: () => Promise<void>;
  release: () => Promise<void>;
}>;

export const openLiveSarahLiveKitSession = async (
  scenario: SarahLiveKitAcceptanceScenario,
  dependencies: SarahLiveKitLiveDependencies = {},
): Promise<SarahLiveKitLiveSession> => {
  const clock =
    dependencies.clock ??
    ({
      now: Date.now,
      sleep: (durationMs) => new Promise((resolve) => setTimeout(resolve, durationMs)),
    } satisfies Clock);
  const http = dependencies.fetch ?? fetch;
  const mintSubscriberGrant = dependencies.mintSubscriberGrant ?? mintProductionSubscriberGrant;
  const controlSocketFactory = dependencies.controlSocketFactory ?? defaultControlSocketFactory;
  const startedAtMs = clock.now();
  const identity = {
    ownerRef: scenario.ownerRef,
    deviceRef: scenario.deviceRef,
    threadRef: scenario.threadRef,
    sessionRef: scenario.sessionRef,
    generation: scenario.generation,
  } as const;
  const requestBase = {
    identity,
    disclosureRef: "disclosure.sarah_livekit_acceptance.v1",
    clientProfile: "omega_editor" as const,
    requestedTransport: "livekit_room_v1" as const,
    roomContext: scenario.roomContext,
  };

  const admissionStartedAtMs = clock.now();
  const admission = decodeAdmission(
    await postJson(http, SARAH_VOICE_ADMISSION_PATH, scenario.bearer, scenario.deviceRef, {
      schema: SARAH_VOICE_ADMISSION_PROTOCOL_VERSION,
      ...requestBase,
    }),
  );
  const admissionCompletedAtMs = clock.now();
  if (!admission.admitted || admission.admissionRef === undefined) {
    throw new Error(`${scenario.kind} Sarah LiveKit admission was refused`);
  }

  const sessionStartedAtMs = clock.now();
  const session = decodeSession(
    await postJson(http, SARAH_VOICE_SESSION_PATH, scenario.bearer, scenario.deviceRef, {
      schema: SARAH_VOICE_PROTOCOL_VERSION,
      ...requestBase,
      admissionRef: admission.admissionRef,
    }),
  );
  const sessionCompletedAtMs = clock.now();
  if (
    session.transport?.kind !== "livekit_room_v1" ||
    session.transport.livekitUrl !== LIVEKIT_ORIGIN ||
    session.transport.roomRef === "" ||
    session.transport.participantGrant === "" ||
    session.transport.permissions.canPublish !== true ||
    session.transport.permissions.canSubscribe !== true ||
    session.transport.sarahParticipantRef !== "principal.sarah"
  ) {
    throw new Error(`${scenario.kind} Sarah session did not return the production LiveKit grant`);
  }
  if (scenario.roomContext.kind === "community") {
    await acquireSarahLiveKitCommunityFloor({
      fetch: http,
      bearer: scenario.bearer,
      deviceRef: scenario.deviceRef,
      communityRef: scenario.roomContext.communityRef,
      channelRef: scenario.roomContext.channelRef,
      roomRef: session.transport.roomRef,
      participantRef: session.transport.participantRef,
    });
  }

  const control = openSarahLiveKitAcceptanceControlChannel(
    {
      gatewayUrl: session.gatewayUrl,
      ticket: session.ticket,
      identity,
      disclosureRef: requestBase.disclosureRef,
    },
    clock.now,
    controlSocketFactory,
  );
  const room = new Room();
  const output = observeSarahOutputs(room, session.transport.sarahParticipantRef, clock);
  const subscriberRoom = new Room();
  const subscriberOutput = observeSarahOutputs(
    subscriberRoom,
    session.transport.sarahParticipantRef,
    clock,
  );
  let microphone: Awaited<ReturnType<typeof publishMicrophone>> | undefined;
  let released = false;
  const release = async (): Promise<void> => {
    if (released) return;
    released = true;
    control.dispose();
    await output.close();
    await subscriberOutput.close();
    if (room.isConnected) await room.disconnect();
    if (subscriberRoom.isConnected) await subscriberRoom.disconnect();
    await microphone?.track.close();
    microphone = undefined;
  };

  try {
    const roomConnectStartedAtMs = clock.now();
    await timeout(
      room.connect(session.transport.livekitUrl, session.transport.participantGrant, {
        autoSubscribe: true,
        dynacast: false,
      }),
      SCENARIO_TIMEOUT_MS,
      `${scenario.kind} LiveKit room connect`,
    );
    const roomConnectedAtMs = clock.now();
    if (
      room.name !== session.transport.roomRef ||
      room.localParticipant?.identity !== session.transport.participantRef
    ) {
      throw new Error(`${scenario.kind} LiveKit room identity did not match the server grant`);
    }
    await timeout(control.ready, SCENARIO_TIMEOUT_MS, `${scenario.kind} Sarah control readiness`);
    const subscriberGrant =
      scenario.subscriberGrant ??
      (await mintSubscriberGrant(session.transport.roomRef, scenario.subscriberRef));
    await timeout(
      subscriberRoom.connect(session.transport.livekitUrl, subscriberGrant, {
        autoSubscribe: true,
        dynacast: false,
      }),
      SCENARIO_TIMEOUT_MS,
      `${scenario.kind} secondary subscriber connect`,
    );
    if (
      subscriberRoom.name !== session.transport.roomRef ||
      subscriberRoom.localParticipant?.identity !== scenario.subscriberRef ||
      scenario.subscriberRef === session.transport.participantRef
    ) {
      throw new Error(`${scenario.kind} secondary subscriber grant did not match the room`);
    }
    output.attachExisting();
    subscriberOutput.attachExisting();
    const microphonePublishStartedAtMs = clock.now();
    microphone = await timeout(
      publishMicrophone(room, scenario.pcm),
      SCENARIO_TIMEOUT_MS,
      `${scenario.kind} microphone publication`,
    );
    const microphonePublishedAtMs = clock.now();
    const fanoutAudio = Promise.all([output.audio, subscriberOutput.audio]);
    // A rejected fanout is the caller's to observe. Attaching a no-op handler
    // here keeps an unobserved rejection from crashing a drill that has
    // deliberately destroyed the transport carrying that audio.
    fanoutAudio.catch(() => {});
    const firstSarahAudioAtMs = await timeout(
      Promise.race([output.audio, subscriberOutput.audio]),
      SCENARIO_TIMEOUT_MS,
      `${scenario.kind} first Sarah audio`,
    );

    return {
      kind: scenario.kind,
      identity,
      roomRef: session.transport.roomRef,
      participantRef: session.transport.participantRef,
      sarahParticipantRef: session.transport.sarahParticipantRef,
      timings: {
        startedAtMs,
        admissionLatencyMs: admissionCompletedAtMs - admissionStartedAtMs,
        sessionLatencyMs: sessionCompletedAtMs - sessionStartedAtMs,
        roomConnectLatencyMs: roomConnectedAtMs - roomConnectStartedAtMs,
        microphonePublishLatencyMs: microphonePublishedAtMs - microphonePublishStartedAtMs,
        activeRoomStartedAtMs: roomConnectedAtMs,
        firstSarahAudioAtMs,
        firstSarahAudioLatencyMs: firstSarahAudioAtMs - startedAtMs,
      },
      room,
      subscriberRoom,
      control,
      output,
      subscriberOutput,
      fanoutAudio,
      clock,
      http,
      unpublishMicrophone: async () => {
        const published = microphone;
        if (published === undefined) return;
        microphone = undefined;
        await room.localParticipant?.unpublishTrack(published.publicationSid, true);
      },
      release,
    };
  } catch (error) {
    await release();
    throw error;
  }
};

export const runLiveSarahLiveKitScenario = async (
  scenario: SarahLiveKitAcceptanceScenario,
  dependencies: SarahLiveKitLiveDependencies = {},
): Promise<SarahLiveKitScenarioObservation> => {
  const session = await openLiveSarahLiveKitSession(scenario, dependencies);
  const { clock, http, control, room, subscriberRoom, output, subscriberOutput } = session;
  const { startedAtMs, activeRoomStartedAtMs, firstSarahAudioAtMs } = session.timings;
  try {
    const interruptStartedAtMs = clock.now();
    const interruption = await timeout(
      control.interrupt(),
      SCENARIO_TIMEOUT_MS,
      `${scenario.kind} Sarah explicit interrupt`,
    );
    await timeout(
      session.fanoutAudio,
      SCENARIO_TIMEOUT_MS,
      `${scenario.kind} Sarah audible fanout`,
    );
    const postInterruptAudioTailMs = Math.max(
      ...(await Promise.all([
        output.measureInterruptTail(interruption.acknowledgedAtMs),
        subscriberOutput.measureInterruptTail(interruption.acknowledgedAtMs),
      ])),
    );
    const firstSarahTranscriptionAtMs = await timeout(
      output.transcription,
      SCENARIO_TIMEOUT_MS,
      `${scenario.kind} Sarah transcription`,
    );
    const rtcStats = (await room.getRtcStats()) as unknown as SarahLiveKitScenarioRtcStats;
    const selectedIcePath = classifySarahLiveKitScenarioIcePaths(scenario.kind, rtcStats);

    await timeout(control.close(), SCENARIO_TIMEOUT_MS, `${scenario.kind} Sarah control close`);
    await session.unpublishMicrophone();
    const settlement = await readSettlement(http, clock, scenario);
    await room.disconnect();
    await subscriberRoom.disconnect();
    const activeRoomEndedAtMs = clock.now();
    const evidence = (
      settlement as typeof settlement & {
        acceptanceEvidence?: Readonly<{
          principal: "principal.sarah";
          identityDigests: SarahLiveKitScenarioObservation["identityDigests"];
          usage: SarahLiveKitScenarioObservation["providerUsage"] &
            Readonly<{ cancelledResponseCount: number }>;
          providerAccountingStatus: "exact";
          workerJobCount: number;
          providerSessionCount: number;
        }>;
      }
    ).acceptanceEvidence;
    if (
      evidence === undefined ||
      evidence.principal !== "principal.sarah" ||
      evidence.providerAccountingStatus !== "exact" ||
      evidence.workerJobCount !== 1 ||
      evidence.providerSessionCount !== 1 ||
      evidence.usage.cancelledResponseCount < 1 ||
      evidence.usage.cancelledResponseCount > evidence.usage.responseCount
    ) {
      throw new Error(`${scenario.kind} terminal LiveKit acceptance evidence was incomplete`);
    }
    const endedAtMs = clock.now();
    return {
      kind: scenario.kind,
      startedAtMs,
      endedAtMs,
      activeRoomStartedAtMs,
      activeRoomEndedAtMs,
      admissionLatencyMs: session.timings.admissionLatencyMs,
      sessionLatencyMs: session.timings.sessionLatencyMs,
      roomConnectLatencyMs: session.timings.roomConnectLatencyMs,
      microphonePublishLatencyMs: session.timings.microphonePublishLatencyMs,
      firstSarahAudioLatencyMs: firstSarahAudioAtMs - startedAtMs,
      firstSarahTranscriptionLatencyMs: firstSarahTranscriptionAtMs - startedAtMs,
      interruptAckLatencyMs: interruption.acknowledgedAtMs - interruptStartedAtMs,
      postInterruptAudioTailMs,
      // Classification already failed closed above, so both paths are observed.
      selectedIcePathObserved: true,
      publisherIceStatsObserved: true,
      subscriberIceStatsObserved: true,
      selectedIcePath,
      microphonePublished: true,
      sarahAudioObserved: true,
      sarahTranscriptionObserved: true,
      principalSarahObserved: true,
      controlChannelReady: true,
      interruptAckObserved: true,
      interruptedLifecycleObserved: true,
      identityDigests: evidence.identityDigests,
      providerUsage: {
        inputTokens: evidence.usage.inputTokens,
        outputTokens: evidence.usage.outputTokens,
        cachedInputTokens: evidence.usage.cachedInputTokens,
        audioInputTokens: evidence.usage.audioInputTokens,
        audioOutputTokens: evidence.usage.audioOutputTokens,
        chargeMsat: evidence.usage.chargeMsat,
        responseCount: evidence.usage.responseCount,
        transcriptionCount: evidence.usage.transcriptionCount,
        cancelledResponseCount: evidence.usage.cancelledResponseCount,
      },
      // Measured from the terminal evidence, not asserted: this scenario bound
      // eight pairwise distinct authority identities. The cross-room half of the
      // claim belongs to the harness, which holds both scenarios.
      identityIsolationObserved:
        new Set(Object.values(evidence.identityDigests)).size ===
        Object.keys(evidence.identityDigests).length,
      exactProviderUsageObserved: true,
      subscriberFanoutCount: 2,
      audibleFanoutObserved: true,
      settlementState: settlement.state,
      settlementCreditMode: settlement.creditMode,
      finalChargeMsat: settlement.finalChargeMsat,
      settlementReceiptDigest: digestSettlementReceipt(settlement.receiptRef),
    };
  } finally {
    await session.release();
  }
};
