import {
  SARAH_LIVEKIT_JOB_CLAIM_PATH,
  SARAH_LIVEKIT_JOB_EVENT_PATH,
  SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
  decodeSarahLiveKitJobClaimResponse,
  type SarahLiveKitDispatchMetadata,
  type SarahLiveKitJobClaimResponse,
  type SarahLiveKitJobEvent,
} from "@openagentsinc/audio-contract";

export type SarahLiveKitControlConfig = Readonly<{
  baseUrl: string;
  workerRef: string;
}>;

export type SarahLiveKitClaimInput = Readonly<{
  dispatch: SarahLiveKitDispatchMetadata;
  dispatchRef: string;
  jobRef: string;
  roomSid: string;
}>;

export type SarahLiveKitEventResult = Readonly<{
  accepted: true;
  stopReason?: "hold_exhausted" | "membership_revoked" | "operator_stop";
}>;

const normalizedBaseUrl = (value: string): string => {
  const url = new URL(value);
  if (
    url.protocol !== "https:" ||
    url.username !== "" ||
    url.password !== "" ||
    url.search !== "" ||
    url.hash !== ""
  ) {
    throw new Error("The Sarah LiveKit control URL must be an HTTPS origin");
  }
  url.pathname = url.pathname.replace(/\/+$/u, "");
  return url.toString().replace(/\/$/u, "");
};

const noStoreHeaders = (token: string) => ({
  authorization: `Bearer ${token}`,
  "cache-control": "no-store",
  "content-type": "application/json",
});

const boundedBody = async (response: Response): Promise<unknown> => {
  const contentLength = Number(response.headers.get("content-length") ?? "0");
  if (Number.isFinite(contentLength) && contentLength > 16_384) {
    throw new Error("The Sarah LiveKit control response was too large");
  }
  const text = await response.text();
  if (new TextEncoder().encode(text).byteLength > 16_384) {
    throw new Error("The Sarah LiveKit control response was too large");
  }
  return JSON.parse(text) as unknown;
};

export const makeSarahLiveKitControlClient = (
  config: SarahLiveKitControlConfig,
  fetcher: typeof fetch = fetch,
) => {
  const baseUrl = normalizedBaseUrl(config.baseUrl);

  const claim = async (input: SarahLiveKitClaimInput): Promise<SarahLiveKitJobClaimResponse> => {
    const response = await fetcher(`${baseUrl}${SARAH_LIVEKIT_JOB_CLAIM_PATH}`, {
      method: "POST",
      headers: noStoreHeaders(input.dispatch.controlToken),
      body: JSON.stringify({
        schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
        workerRef: config.workerRef,
        jobRef: input.jobRef,
        dispatchRef: input.dispatchRef,
        roomSid: input.roomSid,
        dispatch: {
          sessionRef: input.dispatch.sessionRef,
          generation: input.dispatch.generation,
          roomRef: input.dispatch.roomRef,
          roomEpoch: input.dispatch.roomEpoch,
          participantRef: input.dispatch.participantRef,
          sarahParticipantRef: input.dispatch.sarahParticipantRef,
          sarahPresenceLeaseRef: input.dispatch.sarahPresenceLeaseRef,
          capabilityProfile: input.dispatch.capabilityProfile,
          roomContext: input.dispatch.roomContext,
        },
      }),
      redirect: "error",
      signal: AbortSignal.timeout(5_000),
    });
    if (!response.ok) {
      throw new Error(`Sarah LiveKit job claim refused (${response.status})`);
    }
    const claimReceipt = decodeSarahLiveKitJobClaimResponse(await boundedBody(response));
    if (
      claimReceipt.sessionRef !== input.dispatch.sessionRef ||
      claimReceipt.generation !== input.dispatch.generation
    ) {
      throw new Error("The Sarah LiveKit claim disagreed with dispatch");
    }
    return claimReceipt;
  };

  const event = async (
    token: string,
    value: SarahLiveKitJobEvent,
  ): Promise<SarahLiveKitEventResult> => {
    let response: Response | undefined;
    for (let attempt = 0; attempt < 5; attempt += 1) {
      try {
        // eslint-disable-next-line no-await-in-loop
        response = await fetcher(`${baseUrl}${SARAH_LIVEKIT_JOB_EVENT_PATH}`, {
          method: "POST",
          headers: noStoreHeaders(token),
          body: JSON.stringify(value),
          redirect: "error",
          signal: AbortSignal.timeout(5_000),
        });
      } catch (error) {
        if (attempt === 4) throw error;
      }
      if (response !== undefined && ![409, 503].includes(response.status)) {
        break;
      }
      if (attempt < 4) {
        // A worker can observe provider usage immediately before the API's
        // admission transaction becomes visible to another database session.
        // eslint-disable-next-line no-await-in-loop
        await new Promise((resolve) => setTimeout(resolve, 200));
      }
    }
    if (response === undefined) {
      throw new Error("Sarah LiveKit event delivery failed");
    }
    if (!response.ok) {
      throw new Error(`Sarah LiveKit event refused (${response.status})`);
    }
    const body = await boundedBody(response);
    if (
      typeof body !== "object" ||
      body === null ||
      !("accepted" in body) ||
      body.accepted !== true
    ) {
      throw new Error("The Sarah LiveKit event receipt was invalid");
    }
    const stopReason =
      "stopReason" in body &&
      (body.stopReason === "hold_exhausted" ||
        body.stopReason === "membership_revoked" ||
        body.stopReason === "operator_stop")
        ? body.stopReason
        : undefined;
    return stopReason === undefined ? { accepted: true } : { accepted: true, stopReason };
  };

  return { claim, event } as const;
};
