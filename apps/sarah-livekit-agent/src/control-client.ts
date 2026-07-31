import {
  SARAH_LIVEKIT_JOB_CLAIM_PATH,
  SARAH_LIVEKIT_JOB_EVENT_PATH,
  SARAH_LIVEKIT_TOOL_PROPOSAL_PATH,
  SARAH_LIVEKIT_TOOL_STATE_PATH,
  SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
  canonicalSarahLiveKitDispatchAuthority,
  decodeSarahLiveKitJobClaimResponse,
  decodeSarahLiveKitToolProposalResponse,
  decodeSarahLiveKitToolStateResponse,
  type SarahLiveKitDispatchMetadata,
  type SarahLiveKitEditorCommand,
  type SarahLiveKitJobClaimResponse,
  type SarahLiveKitJobEvent,
  type SarahLiveKitToolProposal,
  type SarahLiveKitToolStateResponse,
} from "@openagentsinc/audio-contract";
import { createHmac } from "node:crypto";

export type SarahLiveKitControlConfig = Readonly<{
  baseUrl: string;
  workerRef: string;
  controlRoot: string;
}>;

export type SarahLiveKitClaimInput = Readonly<{
  dispatch: SarahLiveKitDispatchMetadata;
  dispatchRef: string;
  jobRef: string;
  roomSid: string;
}>;

export type SarahLiveKitEventResult = Readonly<{
  accepted: true;
  stopReason?: "hold_exhausted" | "membership_revoked" | "operator_stop" | "session_expired";
}>;

export type SarahLiveKitToolProposalInput = Readonly<{
  sessionRef: string;
  generation: number;
  jobRef: string;
  eventRef: string;
  providerCallRef: string;
  command: SarahLiveKitEditorCommand;
}>;

export type SarahLiveKitToolStateInput = Readonly<{
  sessionRef: string;
  generation: number;
  jobRef: string;
  proposalRef: string;
  proposalDigest: string;
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

const parseControlRoot = (value: string): Buffer => {
  if (!/^[A-Za-z0-9_-]{64,128}$/u.test(value)) {
    throw new Error("The Sarah LiveKit control root is invalid");
  }
  return Buffer.from(value, "utf8");
};

export const deriveSarahLiveKitControlToken = (
  controlRoot: string,
  dispatch: SarahLiveKitDispatchMetadata,
): string =>
  `oa_sarah_lk_${createHmac("sha256", parseControlRoot(controlRoot))
    .update(canonicalSarahLiveKitDispatchAuthority(dispatch))
    .digest("base64url")}`;

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

const retryableControlStatus = (status: number): boolean =>
  [409, 429, 502, 503, 504].includes(status);

const postControlWithRetry = async <Result>(
  fetcher: typeof fetch,
  url: string,
  token: string,
  body: string,
  refused: (status: number) => Error,
  decode: (body: unknown) => Result,
): Promise<Result> => {
  for (let attempt = 0; attempt < 5; attempt += 1) {
    try {
      // Every retried body carries the same durable event/proposal identity.
      // eslint-disable-next-line no-await-in-loop
      const response = await fetcher(url, {
        method: "POST",
        headers: noStoreHeaders(token),
        body,
        redirect: "error",
        signal: AbortSignal.timeout(5_000),
      });
      if (!response.ok) {
        if (!retryableControlStatus(response.status)) {
          throw refused(response.status);
        }
      } else {
        // Body reads and decodes are part of the retryable operation: a reset
        // after 200 headers is still an unknown durable delivery outcome.
        // eslint-disable-next-line no-await-in-loop
        return decode(await boundedBody(response));
      }
    } catch (error) {
      if (error instanceof Error && error.name === "SarahControlRefusedError") {
        throw error;
      }
      if (attempt === 4) throw error;
    }
    if (attempt < 4) {
      // eslint-disable-next-line no-await-in-loop
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
  }
  throw new Error("Sarah LiveKit control delivery failed");
};

const refusedControlResponse = (message: string, status: number): Error => {
  const error = new Error(`${message} (${status})`);
  error.name = "SarahControlRefusedError";
  return error;
};

const decodeSarahLiveKitEventResult = (body: unknown): SarahLiveKitEventResult => {
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
      body.stopReason === "operator_stop" ||
      body.stopReason === "session_expired")
      ? body.stopReason
      : undefined;
  return stopReason === undefined ? { accepted: true } : { accepted: true, stopReason };
};

export const makeSarahLiveKitControlClient = (
  config: SarahLiveKitControlConfig,
  fetcher: typeof fetch = fetch,
) => {
  const baseUrl = normalizedBaseUrl(config.baseUrl);
  const controlRoot = parseControlRoot(config.controlRoot);
  const controlToken = (dispatch: SarahLiveKitDispatchMetadata): string =>
    `oa_sarah_lk_${createHmac("sha256", controlRoot)
      .update(canonicalSarahLiveKitDispatchAuthority(dispatch))
      .digest("base64url")}`;

  const claim = async (input: SarahLiveKitClaimInput): Promise<SarahLiveKitJobClaimResponse> => {
    const response = await fetcher(`${baseUrl}${SARAH_LIVEKIT_JOB_CLAIM_PATH}`, {
      method: "POST",
      headers: noStoreHeaders(controlToken(input.dispatch)),
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
    dispatch: SarahLiveKitDispatchMetadata,
    value: SarahLiveKitJobEvent,
  ): Promise<SarahLiveKitEventResult> => {
    const body = JSON.stringify(value);
    return postControlWithRetry(
      fetcher,
      `${baseUrl}${SARAH_LIVEKIT_JOB_EVENT_PATH}`,
      controlToken(dispatch),
      body,
      (status) => refusedControlResponse("Sarah LiveKit event refused", status),
      decodeSarahLiveKitEventResult,
    );
  };

  const proposeTool = async (
    dispatch: SarahLiveKitDispatchMetadata,
    input: SarahLiveKitToolProposalInput,
  ): Promise<SarahLiveKitToolProposal> => {
    return postControlWithRetry(
      fetcher,
      `${baseUrl}${SARAH_LIVEKIT_TOOL_PROPOSAL_PATH}`,
      controlToken(dispatch),
      JSON.stringify({
        schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
        ...input,
      }),
      (status) => refusedControlResponse("Sarah LiveKit tool proposal refused", status),
      (responseBody) => decodeSarahLiveKitToolProposalResponse(responseBody).proposal,
    );
  };

  const readToolState = async (
    dispatch: SarahLiveKitDispatchMetadata,
    input: SarahLiveKitToolStateInput,
  ): Promise<SarahLiveKitToolStateResponse> => {
    return postControlWithRetry(
      fetcher,
      `${baseUrl}${SARAH_LIVEKIT_TOOL_STATE_PATH}`,
      controlToken(dispatch),
      JSON.stringify({
        schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
        ...input,
      }),
      (status) => refusedControlResponse("Sarah LiveKit tool state refused", status),
      decodeSarahLiveKitToolStateResponse,
    );
  };

  return { claim, event, proposeTool, readToolState } as const;
};
