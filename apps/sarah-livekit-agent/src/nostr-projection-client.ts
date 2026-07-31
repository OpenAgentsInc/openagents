import {
  SARAH_SIGNING_REQUEST_SCHEMA,
  SarahSigningResponseSchema,
  reconstructSarahSignedEvent,
  type SarahSignerTemplate,
} from "@openagentsinc/sarah/nostr-signing-boundary";
import {
  canonicalSarahLiveKitRoomPresenceAuthority,
  type SarahLiveKitRoomPresenceLease,
} from "@openagentsinc/audio-contract";
import { Schema as S } from "effect";
import { createHash } from "node:crypto";
import WebSocket from "ws";

const METADATA_IDENTITY_URL =
  "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/identity";
const TOKEN_CACHE_MS = 45 * 60_000;
const MAX_RESPONSE_BYTES = 8_192;

export type SarahNostrProjectionConfig = Readonly<{
  signerUrl: string;
  signerAudience: string;
  expectedPubkey: string;
  relayUrl: string;
}>;

type Fetch = typeof fetch;
type WebSocketConstructor = typeof WebSocket;
const sha256 = (value: string): string => createHash("sha256").update(value).digest("hex");

export const sarahPresenceTemplateFromLease = (
  lease: SarahLiveKitRoomPresenceLease,
  status: "active" | "inactive",
  nowSeconds = Math.floor(Date.now() / 1_000),
): Extract<SarahSignerTemplate, { type: "presence" }> => ({
  type: "presence",
  createdAt: status === "active" ? Math.floor(lease.issuedAtMs / 1_000) : nowSeconds,
  expiresAt: status === "active" ? Math.floor(lease.expiresAtMs / 1_000) : nowSeconds,
  groupRef: lease.communityRef,
  channelRef: lease.channelRef,
  presenceLeaseRef: lease.leaseRef,
  roomEpochDigest: sha256(`${lease.roomRef}\n${lease.roomEpoch}`),
  sessionDigest: sha256(lease.sessionRef),
  generation: lease.generation,
  membershipRevision: lease.membershipRevision,
  e2eeKeyRevision: lease.e2eeKeyRevision,
  admissionDigest: lease.admissionDigest,
  authorityDigest: sha256(canonicalSarahLiveKitRoomPresenceAuthority(lease)),
  status,
});

export const sarahKind9TemplateFromMessage = (
  lease: SarahLiveKitRoomPresenceLease,
  input: Readonly<{ messageRef: string; content: string; createdAtMs: number }>,
): Extract<SarahSignerTemplate, { type: "kind9_projection" }> => ({
  type: "kind9_projection",
  createdAt: Math.floor(input.createdAtMs / 1_000),
  groupRef: lease.communityRef,
  channelRef: lease.channelRef,
  messageRef: input.messageRef,
  presenceLeaseRef: lease.leaseRef,
  generation: lease.generation,
  content: input.content,
});

const normalizedUrl = (value: string, protocol: "https:" | "wss:"): string => {
  const url = new URL(value);
  if (
    url.protocol !== protocol ||
    url.username !== "" ||
    url.password !== "" ||
    url.search !== "" ||
    url.hash !== ""
  ) {
    throw new Error(`Sarah Nostr endpoint must use ${protocol}`);
  }
  url.pathname = url.pathname.replace(/\/+$/u, "");
  return url.toString().replace(/\/$/u, "");
};

export const readSarahNostrProjectionConfig = (
  environment: NodeJS.ProcessEnv = process.env,
): SarahNostrProjectionConfig => {
  const required = (name: string): string => {
    const value = environment[name]?.trim();
    if (value === undefined || value === "") throw new Error(`${name} is required`);
    return value;
  };
  const expectedPubkey = required("SARAH_NOSTR_EXPECTED_PUBKEY");
  if (!/^[a-f0-9]{64}$/u.test(expectedPubkey)) {
    throw new Error("SARAH_NOSTR_EXPECTED_PUBKEY must be lowercase 64-hex");
  }
  return {
    signerUrl: normalizedUrl(required("SARAH_NOSTR_SIGNER_URL"), "https:"),
    signerAudience: normalizedUrl(required("SARAH_NOSTR_SIGNER_AUDIENCE"), "https:"),
    expectedPubkey,
    relayUrl: normalizedUrl(required("SARAH_NOSTR_RELAY_URL"), "wss:"),
  };
};

const responseText = async (response: Response): Promise<string> => {
  const declaredLength = Number(response.headers.get("content-length") ?? "0");
  if (!Number.isFinite(declaredLength) || declaredLength > MAX_RESPONSE_BYTES) {
    throw new Error("Sarah Nostr service returned an oversized response");
  }
  const text = await response.text();
  if (new TextEncoder().encode(text).byteLength > MAX_RESPONSE_BYTES) {
    throw new Error("Sarah Nostr service returned an oversized response");
  }
  return text;
};

export const makeSarahNostrProjectionClient = (options: Readonly<{
  config: SarahNostrProjectionConfig;
  fetch?: Fetch;
  WebSocketImpl?: WebSocketConstructor;
  nowMs?: () => number;
  receiptTimeoutMs?: number;
}>) => {
  const fetchImpl = options.fetch ?? fetch;
  const WebSocketImpl = options.WebSocketImpl ?? WebSocket;
  const nowMs = options.nowMs ?? Date.now;
  const signerUrl = `${normalizedUrl(options.config.signerUrl, "https:")}/v1/sign`;
  const signerAudience = normalizedUrl(options.config.signerAudience, "https:");
  const relayUrl = normalizedUrl(options.config.relayUrl, "wss:");
  if (!/^[a-f0-9]{64}$/u.test(options.config.expectedPubkey)) {
    throw new Error("Sarah Nostr expected public key is invalid");
  }
  let cachedToken: Readonly<{ value: string; expiresAtMs: number }> | undefined;

  const identityToken = async (): Promise<string> => {
    const current = cachedToken;
    if (current !== undefined && current.expiresAtMs > nowMs()) return current.value;
    const metadataUrl = new URL(METADATA_IDENTITY_URL);
    metadataUrl.searchParams.set("audience", signerAudience);
    metadataUrl.searchParams.set("format", "full");
    const response = await fetchImpl(metadataUrl, {
      headers: { "metadata-flavor": "Google" },
      signal: AbortSignal.timeout(5_000),
    });
    if (!response.ok) throw new Error(`Sarah signer identity request failed: ${response.status}`);
    const token = (await responseText(response)).trim();
    if (token.length < 100 || token.length > MAX_RESPONSE_BYTES || /\s/u.test(token)) {
      throw new Error("Sarah signer identity response was invalid");
    }
    cachedToken = { value: token, expiresAtMs: nowMs() + TOKEN_CACHE_MS };
    return token;
  };

  const sign = async (template: SarahSignerTemplate) => {
    const token = await identityToken();
    const response = await fetchImpl(signerUrl, {
      method: "POST",
      headers: {
        authorization: `Bearer ${token}`,
        "cache-control": "no-store",
        "content-type": "application/json",
      },
      body: JSON.stringify({
        schemaVersion: SARAH_SIGNING_REQUEST_SCHEMA,
        template,
      }),
      signal: AbortSignal.timeout(10_000),
    });
    const raw = await responseText(response);
    if (!response.ok) throw new Error(`Sarah signer rejected the template: ${response.status}`);
    let decoded: unknown;
    try {
      decoded = JSON.parse(raw);
    } catch {
      throw new Error("Sarah signer returned invalid JSON");
    }
    const signature = S.decodeUnknownSync(SarahSigningResponseSchema)(decoded, {
      onExcessProperty: "error",
    });
    return reconstructSarahSignedEvent(
      template,
      signature,
      options.config.expectedPubkey,
    );
  };

  const publish = async (event: Awaited<ReturnType<typeof sign>>): Promise<void> =>
    new Promise<void>((resolve, reject) => {
      const socket = new WebSocketImpl(relayUrl);
      let authEventId: string | undefined;
      let authInFlight = false;
      const timer = setTimeout(() => {
        socket.close();
        reject(new Error("Sarah relay publish receipt timed out"));
      }, options.receiptTimeoutMs ?? 10_000);
      const fail = (error: Error) => {
        clearTimeout(timer);
        socket.close();
        reject(error);
      };
      socket.on("message", (data) => {
        let frame: unknown;
        try {
          frame = JSON.parse(data.toString());
        } catch {
          return;
        }
        if (
          Array.isArray(frame) &&
          frame[0] === "AUTH" &&
          typeof frame[1] === "string" &&
          !authInFlight &&
          authEventId === undefined
        ) {
          authInFlight = true;
          void sign({
            type: "relay_auth",
            createdAt: Math.floor(nowMs() / 1_000),
            relayUrl,
            challenge: frame[1],
          }).then(
            (authEvent) => {
              authEventId = authEvent.id;
              socket.send(JSON.stringify(["AUTH", authEvent]));
            },
            fail,
          );
          return;
        }
        if (
          !Array.isArray(frame) ||
          frame[0] !== "OK" ||
          typeof frame[2] !== "boolean"
        ) {
          return;
        }
        if (frame[1] === authEventId) {
          if (frame[2]) socket.send(JSON.stringify(["EVENT", event]));
          else fail(new Error(`Sarah relay rejected authentication: ${String(frame[3] ?? "")}`));
          return;
        }
        if (frame[1] !== event.id) return;
        clearTimeout(timer);
        socket.close();
        if (frame[2]) resolve();
        else reject(new Error(`Sarah relay rejected projection: ${String(frame[3] ?? "")}`));
      });
      socket.once("error", () => fail(new Error("Sarah relay connection failed")));
    });

  return {
    sign,
    publish,
    signAndPublish: async (template: SarahSignerTemplate) => {
      const event = await sign(template);
      await publish(event);
      return event;
    },
  };
};
