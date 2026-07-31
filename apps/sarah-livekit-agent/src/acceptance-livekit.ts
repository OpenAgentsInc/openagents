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
  SARAH_VOICE_PROTOCOL_VERSION,
  SARAH_VOICE_SESSION_PATH,
  SARAH_VOICE_SETTLEMENT_PATH,
  decodeSarahVoiceAdmissionResponse,
  decodeSarahVoiceSessionResponse,
  decodeSarahVoiceSettlementResponse,
} from "@openagentsinc/audio-contract";
import {
  digestSettlementReceipt,
  type SarahLiveKitAcceptanceScenario,
  type SarahLiveKitScenarioObservation,
} from "./acceptance-harness.js";

const API_ORIGIN = "https://openagents.com";
const LIVEKIT_ORIGIN = "wss://livekit.openagents.com";
const TRANSCRIPTION_TOPIC = "lk.transcription";
const SCENARIO_TIMEOUT_MS = 60_000;
const SETTLEMENT_TIMEOUT_MS = 45_000;

type Clock = Readonly<{
  now: () => number;
  sleep: (durationMs: number) => Promise<void>;
}>;

type Http = (input: string | URL, init?: RequestInit) => Promise<Response>;

export type SarahLiveKitLiveDependencies = Readonly<{
  clock?: Clock;
  fetch?: Http;
}>;

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

const observeSarahOutputs = (room: Room, sarahParticipantRef: string, now: () => number) => {
  const audio = deferred<number>();
  const transcription = deferred<number>();
  let audioAttached = false;

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
            audio.resolve(now());
            // Cancel only after the first audible frame has been consumed.
            // eslint-disable-next-line no-await-in-loop
            await reader.cancel();
            return;
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
          transcription.resolve(now());
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
    close: () => {
      room.off(RoomEvent.TrackSubscribed, attachAudio);
      room.unregisterTextStreamHandler(TRANSCRIPTION_TOPIC);
    },
  };
};

type RtcStat = Readonly<{
  stats?: Readonly<{
    case?: string;
    value?: Readonly<{
      rtc?: Readonly<{ id?: string }>;
      transport?: Readonly<{ selectedCandidatePairId?: string }>;
      candidatePair?: Readonly<{
        nominated?: boolean;
        packetsSent?: bigint;
        packetsReceived?: bigint;
      }>;
    }>;
  }>;
}>;

const selectedIcePathObserved = (entries: readonly RtcStat[]): boolean => {
  const selectedPairIds = new Set(
    entries.flatMap((entry) =>
      entry.stats?.case === "transport" &&
      entry.stats.value?.transport?.selectedCandidatePairId !== undefined
        ? [entry.stats.value.transport.selectedCandidatePairId]
        : [],
    ),
  );
  return entries.some((entry) => {
    if (entry.stats?.case !== "candidatePair") return false;
    const pair = entry.stats.value?.candidatePair;
    return (
      entry.stats.value?.rtc?.id !== undefined &&
      selectedPairIds.has(entry.stats.value.rtc.id) &&
      pair?.nominated === true &&
      ((pair.packetsSent ?? 0n) > 0n || (pair.packetsReceived ?? 0n) > 0n)
    );
  });
};

const iceObservation = (
  stats: Readonly<{ publisherStats: readonly RtcStat[]; subscriberStats: readonly RtcStat[] }>,
) => {
  const publisherIceStatsObserved = selectedIcePathObserved(stats.publisherStats);
  const subscriberIceStatsObserved = selectedIcePathObserved(stats.subscriberStats);
  return {
    publisherIceStatsObserved,
    subscriberIceStatsObserved,
    selectedIcePathObserved: publisherIceStatsObserved && subscriberIceStatsObserved,
  };
};

const readSettlement = async (
  http: Http,
  clock: Clock,
  scenario: SarahLiveKitAcceptanceScenario,
) => {
  const deadline = clock.now() + SETTLEMENT_TIMEOUT_MS;
  while (clock.now() < deadline) {
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
      return decodeSettlement(await response.json());
    }
    if (response.status !== 404) {
      // eslint-disable-next-line no-await-in-loop
      throw await responseError(response, SARAH_VOICE_SETTLEMENT_PATH);
    }
    // Do not overlap reads of the same terminal accounting authority.
    // eslint-disable-next-line no-await-in-loop
    await clock.sleep(500);
  }
  throw new Error("Sarah settlement did not become terminal");
};

export const runLiveSarahLiveKitScenario = async (
  scenario: SarahLiveKitAcceptanceScenario,
  dependencies: SarahLiveKitLiveDependencies = {},
): Promise<SarahLiveKitScenarioObservation> => {
  const clock =
    dependencies.clock ??
    ({
      now: Date.now,
      sleep: (durationMs) => new Promise((resolve) => setTimeout(resolve, durationMs)),
    } satisfies Clock);
  const http = dependencies.fetch ?? fetch;
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

  const room = new Room();
  const output = observeSarahOutputs(room, session.transport.sarahParticipantRef, clock.now);
  const subscriberRoom = new Room();
  const subscriberOutput = observeSarahOutputs(
    subscriberRoom,
    session.transport.sarahParticipantRef,
    clock.now,
  );
  let microphone: Awaited<ReturnType<typeof publishMicrophone>> | undefined;
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
    await timeout(
      subscriberRoom.connect(session.transport.livekitUrl, scenario.subscriberGrant, {
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
    const [firstSarahAudioAtMs, firstSarahTranscriptionAtMs] = await timeout(
      Promise.all([output.audio, output.transcription, subscriberOutput.audio]).then(
        ([audioAtMs, transcriptionAtMs]) => [audioAtMs, transcriptionAtMs] as const,
      ),
      SCENARIO_TIMEOUT_MS,
      `${scenario.kind} Sarah audio and transcription`,
    );
    const rtcStats = (await room.getRtcStats()) as unknown as Readonly<{
      publisherStats: readonly RtcStat[];
      subscriberStats: readonly RtcStat[];
    }>;
    const ice = iceObservation(rtcStats);
    if (!ice.selectedIcePathObserved) {
      throw new Error(`${scenario.kind} selected ICE path was not observable`);
    }

    const activeRoomEndedAtMs = clock.now();
    await room.localParticipant?.unpublishTrack(microphone.publicationSid, true);
    microphone = undefined;
    await room.disconnect();
    await subscriberRoom.disconnect();
    const settlement = await readSettlement(http, clock, scenario);
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
      evidence.providerSessionCount !== 1
    ) {
      throw new Error(`${scenario.kind} terminal LiveKit acceptance evidence was incomplete`);
    }
    const endedAtMs = clock.now();
    return {
      kind: scenario.kind,
      startedAtMs,
      endedAtMs,
      activeRoomStartedAtMs: roomConnectedAtMs,
      activeRoomEndedAtMs,
      admissionLatencyMs: admissionCompletedAtMs - admissionStartedAtMs,
      sessionLatencyMs: sessionCompletedAtMs - sessionStartedAtMs,
      roomConnectLatencyMs: roomConnectedAtMs - roomConnectStartedAtMs,
      microphonePublishLatencyMs: microphonePublishedAtMs - microphonePublishStartedAtMs,
      firstSarahAudioLatencyMs: firstSarahAudioAtMs - startedAtMs,
      firstSarahTranscriptionLatencyMs: firstSarahTranscriptionAtMs - startedAtMs,
      ...ice,
      microphonePublished: true,
      sarahAudioObserved: true,
      sarahTranscriptionObserved: true,
      principalSarahObserved: true,
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
      },
      identityIsolationObserved: true,
      exactProviderUsageObserved: true,
      subscriberFanoutCount: 2,
      audibleFanoutObserved: true,
      settlementState: settlement.state,
      settlementCreditMode: settlement.creditMode,
      finalChargeMsat: settlement.finalChargeMsat,
      settlementReceiptDigest: digestSettlementReceipt(settlement.receiptRef),
    };
  } finally {
    output.close();
    subscriberOutput.close();
    if (room.isConnected) await room.disconnect();
    if (subscriberRoom.isConnected) await subscriberRoom.disconnect();
    await microphone?.track.close();
  }
};
