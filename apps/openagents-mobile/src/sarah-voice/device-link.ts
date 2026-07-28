import type { Issue31NostrSigner } from "@openagentsinc/sarah/issue31-nostr";
import {
  OMEGA_NOSTR_DEVICE_LINK_CHALLENGE_PATH,
  OMEGA_NOSTR_DEVICE_LINK_CHALLENGE_PROTOCOL_VERSION,
  OMEGA_NOSTR_DEVICE_LINK_PATH,
  OMEGA_NOSTR_DEVICE_LINK_PROTOCOL_VERSION,
  OmegaNostrDeviceLinkChallengeResponseSchema,
  OmegaNostrDeviceLinkResponseSchema,
} from "@openagentsinc/audio-contract";
import { Schema } from "effect";

import {
  clearNativeSessionCredential,
  loadNativeSessionCredential,
  saveNativeSessionCredential,
  type NativeSessionCredential,
  type NativeSessionSecureStore,
} from "../auth/native-session-vault";
import { encodeNip98Authorization, sha256Hex, type Sha256 } from "./protocol";

export const OPENAGENTS_MOBILE_AUTH_SESSION_PATH = "/api/mobile/auth/session";
export const OPENAGENTS_NATIVE_REFRESH_HEADER = "x-openagents-refresh-token";
export const OPENAGENTS_OMEGA_DEVICE_REF_HEADER = "x-openagents-omega-device-ref";

const RotatedTokensSchema = Schema.Struct({
  access: Schema.String,
  refresh: Schema.String,
  expiresIn: Schema.Number,
});

const VerifiedNativeSessionSchema = Schema.Struct({
  authenticated: Schema.Literal(true),
  user: Schema.Struct({ userId: Schema.String }),
  tokens: Schema.optional(RotatedTokensSchema),
});

export type SarahVoiceDeviceLinkFailure = Readonly<{
  message: string;
  retryable: boolean;
}>;

export type SarahVoiceDeviceLinkRecovery = (
  input: Readonly<{
    baseUrl: string;
    deviceRef: string;
    publicKeyHex: string;
    signer: Issue31NostrSigner;
    fetch: typeof globalThis.fetch;
    sha256: Sha256;
    now: () => number;
  }>,
) => Promise<
  | Readonly<{ _tag: "Success"; ownerRef: string }>
  | Readonly<{ _tag: "Failure"; failure: SarahVoiceDeviceLinkFailure }>
>;

const signedOut = (): SarahVoiceDeviceLinkFailure => ({
  message: "Sign in to OpenAgents to use Sarah voice.",
  retryable: false,
});

const invalidProof = (): SarahVoiceDeviceLinkFailure => ({
  message: "Sarah could not verify this device. Try again.",
  retryable: true,
});

const conflict = (): SarahVoiceDeviceLinkFailure => ({
  message: "This device is linked to another OpenAgents account.",
  retryable: false,
});

const unavailable = (): SarahVoiceDeviceLinkFailure => ({
  message: "Sarah could not link this device. Check the network and try again.",
  retryable: true,
});

const readError = async (response: Response): Promise<string | undefined> => {
  try {
    const value = (await response.json()) as unknown;
    return typeof value === "object" &&
      value !== null &&
      "error" in value &&
      typeof value.error === "string"
      ? value.error
      : undefined;
  } catch {
    return undefined;
  }
};

const normalizedRotation = (
  current: NativeSessionCredential,
  tokens: typeof RotatedTokensSchema.Type,
): NativeSessionCredential | null => {
  const accessToken = tokens.access.trim();
  const refreshToken = tokens.refresh.trim();
  if (
    accessToken === "" ||
    refreshToken === "" ||
    !Number.isFinite(tokens.expiresIn) ||
    tokens.expiresIn <= 0
  ) {
    return null;
  }
  return { ...current, accessToken, refreshToken };
};

const failure = (
  value: SarahVoiceDeviceLinkFailure,
): Readonly<{ _tag: "Failure"; failure: SarahVoiceDeviceLinkFailure }> => ({
  _tag: "Failure",
  failure: value,
});

export const makeSarahVoiceDeviceLinkRecovery =
  (store: NativeSessionSecureStore): SarahVoiceDeviceLinkRecovery =>
  async (input) => {
    let credential: NativeSessionCredential | null;
    try {
      credential = await loadNativeSessionCredential(store);
    } catch {
      return failure(unavailable());
    }
    if (credential === null) return failure(signedOut());

    let sessionResponse: Response;
    try {
      sessionResponse = await input.fetch(
        `${input.baseUrl}${OPENAGENTS_MOBILE_AUTH_SESSION_PATH}`,
        {
          method: "GET",
          headers: {
            authorization: `Bearer ${credential.accessToken}`,
            [OPENAGENTS_NATIVE_REFRESH_HEADER]: credential.refreshToken,
          },
        },
      );
    } catch {
      return failure(unavailable());
    }

    if (sessionResponse.status === 401 || sessionResponse.status === 403) {
      try {
        await clearNativeSessionCredential(store);
      } catch {
        return failure(unavailable());
      }
      return failure(signedOut());
    }
    if (!sessionResponse.ok) return failure(unavailable());

    let verified: typeof VerifiedNativeSessionSchema.Type;
    try {
      verified = Schema.decodeUnknownSync(VerifiedNativeSessionSchema)(
        await sessionResponse.json(),
      );
    } catch {
      return failure(unavailable());
    }
    const ownerRef = verified.user.userId.trim();
    if (ownerRef === "" || ownerRef !== credential.ownerUserId) {
      try {
        await clearNativeSessionCredential(store);
      } catch {
        return failure(unavailable());
      }
      return failure(signedOut());
    }

    if (verified.tokens !== undefined) {
      const rotated = normalizedRotation(credential, verified.tokens);
      if (rotated === null) return failure(unavailable());
      try {
        await saveNativeSessionCredential(store, rotated);
      } catch {
        return failure(unavailable());
      }
      credential = rotated;
    }

    const challengeBody = JSON.stringify({
      schema: OMEGA_NOSTR_DEVICE_LINK_CHALLENGE_PROTOCOL_VERSION,
      pubkey: input.publicKeyHex,
      deviceRef: input.deviceRef,
    });
    const challengeUrl = `${input.baseUrl}${OMEGA_NOSTR_DEVICE_LINK_CHALLENGE_PATH}`;
    let challengeResponse: Response;
    try {
      challengeResponse = await input.fetch(challengeUrl, {
        method: "POST",
        headers: {
          authorization: `Bearer ${credential.accessToken}`,
          "content-type": "application/json",
          [OPENAGENTS_NATIVE_REFRESH_HEADER]: credential.refreshToken,
        },
        body: challengeBody,
      });
    } catch {
      return failure(unavailable());
    }

    if (!challengeResponse.ok) {
      const error = await readError(challengeResponse);
      if (challengeResponse.status === 401 || error === "unauthorized") {
        try {
          await clearNativeSessionCredential(store);
        } catch {
          return failure(unavailable());
        }
        return failure(signedOut());
      }
      if (challengeResponse.status === 429 || error === "nostr_device_link_rate_limited") {
        return failure({
          message: "Sarah sign-in is busy. Wait a moment, then try again.",
          retryable: true,
        });
      }
      return failure(unavailable());
    }

    let challenge: typeof OmegaNostrDeviceLinkChallengeResponseSchema.Type;
    try {
      challenge = Schema.decodeUnknownSync(OmegaNostrDeviceLinkChallengeResponseSchema)(
        await challengeResponse.json(),
      );
      if (
        challenge.ownerRef !== ownerRef ||
        challenge.challenge.trim() === "" ||
        challenge.expiresAtMs <= input.now()
      ) {
        return failure(unavailable());
      }
    } catch {
      return failure(unavailable());
    }

    const body = JSON.stringify({
      schema: OMEGA_NOSTR_DEVICE_LINK_PROTOCOL_VERSION,
      challenge: challenge.challenge,
      ownerRef,
      deviceRef: input.deviceRef,
    });
    const url = `${input.baseUrl}${OMEGA_NOSTR_DEVICE_LINK_PATH}`;
    let nostrAuthorization: string;
    try {
      const signed = await input.signer.signEvent({
        kind: 27_235,
        created_at: Math.floor(input.now() / 1_000),
        tags: [
          ["u", url],
          ["method", "POST"],
          ["payload", await sha256Hex(new TextEncoder().encode(body), input.sha256)],
        ],
        content: "",
      });
      nostrAuthorization = encodeNip98Authorization(signed);
    } catch {
      return failure(invalidProof());
    }

    let linkResponse: Response;
    try {
      linkResponse = await input.fetch(url, {
        method: "POST",
        headers: {
          authorization: nostrAuthorization,
          "content-type": "application/json",
          [OPENAGENTS_OMEGA_DEVICE_REF_HEADER]: input.deviceRef,
        },
        body,
      });
    } catch {
      return failure(unavailable());
    }

    if (!linkResponse.ok) {
      const error = await readError(linkResponse);
      if (linkResponse.status === 401 || linkResponse.status === 403) {
        return failure(invalidProof());
      }
      if (error === "nostr_identity_link_conflict") return failure(conflict());
      if (error === "nostr_device_link_replayed" || error === "omega_nostr_proof_replayed") {
        return failure(invalidProof());
      }
      return failure(unavailable());
    }

    try {
      const linked = Schema.decodeUnknownSync(OmegaNostrDeviceLinkResponseSchema)(
        await linkResponse.json(),
      );
      return linked.ownerRef === ownerRef ? { _tag: "Success", ownerRef } : failure(unavailable());
    } catch {
      return failure(unavailable());
    }
  };
