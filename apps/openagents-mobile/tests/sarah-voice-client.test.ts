import { createHash } from "node:crypto";
import { describe, expect, test } from "vite-plus/test";

import {
  AUDIO_MEDIA_MAGIC,
  AUDIO_PROTOCOL_VERSION,
  OMEGA_NOSTR_DEVICE_LINK_CHALLENGE_PATH,
  OMEGA_NOSTR_DEVICE_LINK_CHALLENGE_PROTOCOL_VERSION,
  OMEGA_NOSTR_DEVICE_LINK_PATH,
  OMEGA_NOSTR_DEVICE_LINK_PROTOCOL_VERSION,
  SARAH_VOICE_NOSTR_CHALLENGE_PROTOCOL_VERSION,
  SARAH_VOICE_PROTOCOL_VERSION,
  type VoiceIdentity,
} from "@openagentsinc/audio-contract";
import type { Issue31NostrSigner } from "@openagentsinc/sarah/issue31-nostr";

import { SarahVoiceClient, type SarahVoiceSocket } from "../src/sarah-voice/client.ts";
import { makeSarahVoiceDeviceLinkRecovery } from "../src/sarah-voice/device-link.ts";
import {
  OPENAGENTS_NATIVE_SESSION_EPOCH,
  OPENAGENTS_NATIVE_SESSION_KEY,
  type NativeSessionSecureStore,
} from "../src/auth/native-session-vault.ts";
import type {
  SarahVoiceSessionVault,
  SarahVoiceStoredSession,
} from "../src/sarah-voice/session-vault.ts";

const publicKeyHex = "a".repeat(64);
const ownerRef = "user-1";
const challenge = `challenge_${"c".repeat(32)}`;
const deviceLinkChallenge = `device_link_${"f".repeat(32)}`;
const accessToken = `oa_omega_${"b".repeat(43)}`;
const canonicalAccessToken = `oa_access_${"c".repeat(32)}`;
const canonicalRefreshToken = `oa_refresh_${"d".repeat(32)}`;
const sha256 = async (bytes: Uint8Array): Promise<Uint8Array> =>
  new Uint8Array(createHash("sha256").update(bytes).digest());
const tick = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 0));

class FixtureSocket implements SarahVoiceSocket {
  binaryType = "";
  readyState = 0;
  onopen: (() => void) | null = null;
  onmessage: ((event: Readonly<{ data: unknown }>) => void) | null = null;
  onerror: (() => void) | null = null;
  onclose: ((event: Readonly<{ code: number; reason: string }>) => void) | null = null;
  readonly sent: Array<string | ArrayBuffer> = [];
  readonly closes: Array<Readonly<{ code?: number; reason?: string }>> = [];

  constructor(
    readonly url: string,
    readonly headers: Readonly<Record<string, string>>,
  ) {}

  send(data: string | ArrayBuffer): void {
    this.sent.push(data);
  }

  close(code?: number, reason?: string): void {
    this.readyState = 3;
    this.closes.push({ code, reason });
  }

  open(): void {
    this.readyState = 1;
    this.onopen?.();
  }

  serverControl(frame: unknown): void {
    this.onmessage?.({ data: JSON.stringify(frame) });
  }

  serverClose(code = 1011, reason = "transport_error"): void {
    this.readyState = 3;
    this.onclose?.({ code, reason });
  }
}

const signer: Issue31NostrSigner = {
  getPublicKey: async () => publicKeyHex,
  signEvent: async (event) => ({
    id: "d".repeat(64),
    pubkey: publicKeyHex,
    created_at: event.created_at ?? 0,
    kind: event.kind,
    tags: event.tags,
    content: event.content,
    sig: "e".repeat(128),
  }),
  nip44Encrypt: async () => "unused",
  nip44Decrypt: async () => "unused",
};

const makeVault = (initial: SarahVoiceStoredSession | null = null) => {
  let record = initial;
  let clearCount = 0;
  const vault: SarahVoiceSessionVault = {
    read: async () => record,
    write: async (next) => {
      record = next;
    },
    clear: async () => {
      clearCount += 1;
      record = null;
    },
  };
  return { clearCount: () => clearCount, read: () => record, vault };
};

const canonicalSessionRecord = (
  input: Readonly<{
    accessToken?: string;
    refreshToken?: string;
  }> = {},
): string =>
  JSON.stringify({
    schemaVersion: 1,
    credentialEpoch: OPENAGENTS_NATIVE_SESSION_EPOCH,
    ownerUserId: ownerRef,
    accessToken: input.accessToken ?? canonicalAccessToken,
    refreshToken: input.refreshToken ?? canonicalRefreshToken,
  });

const makeNativeSessionStore = (initial: string | null) => {
  let record = initial;
  const requireCanonicalKey = (key: string): void => {
    if (key !== OPENAGENTS_NATIVE_SESSION_KEY) {
      throw new Error(`Unexpected native session key: ${key}`);
    }
  };
  const store: NativeSessionSecureStore = {
    AFTER_FIRST_UNLOCK_THIS_DEVICE_ONLY: "device-only",
    getItemAsync: async (key) => {
      requireCanonicalKey(key);
      return record;
    },
    setItemAsync: async (key, value) => {
      requireCanonicalKey(key);
      record = value;
    },
    deleteItemAsync: async (key) => {
      requireCanonicalKey(key);
      record = null;
    },
  };
  return { read: () => record, store };
};

const sessionResponse = (
  identity: VoiceIdentity,
  ticket: string,
  withAuth: boolean,
): Record<string, unknown> => ({
  schema: SARAH_VOICE_PROTOCOL_VERSION,
  sessionRef: identity.sessionRef,
  model: "gpt-realtime-2.1",
  gatewayUrl: "wss://openagents.com/api/omega/sarah/voice/connect",
  ticket,
  ticketExpiresAtMs: 20_000,
  sessionExpiresAtMs: 600_000,
  reservedCreditMsat: 25_000,
  maxDurationSeconds: 600,
  clientProfile: "mobile_voice_only",
  inputAudio: { codec: "pcm_s16le", sampleRateHz: 24_000, channels: 1 },
  outputAudio: { codec: "pcm_s16le", sampleRateHz: 24_000, channels: 1 },
  ...(withAuth
    ? {
        auth: {
          method: "nostr_nip98",
          accessToken,
          expiresIn: 900,
        },
      }
    : {}),
});

const normalSessionResponse = (
  provider: "nostr" | "github" | "email" = "nostr",
): Record<string, unknown> => ({
  accessToken,
  expiresIn: 900,
  user: {
    userId: ownerRef,
    provider,
  },
});

const control = (
  identity: VoiceIdentity,
  sequence: number,
  value: Record<string, unknown>,
): Record<string, unknown> => ({
  schema: SARAH_VOICE_PROTOCOL_VERSION,
  identity,
  sequence,
  ...value,
});

const serverAudioFrame = (
  identity: VoiceIdentity,
  sequence: number,
  itemRef: string,
): ArrayBuffer => {
  const pcm = Uint8Array.from([sequence + 1, 0]);
  const header = new TextEncoder().encode(
    JSON.stringify({
      schema: AUDIO_PROTOCOL_VERSION,
      kind: "server_tts",
      identity,
      sequence,
      turnRef: itemRef,
      speechRef: itemRef,
      codec: "pcm_s16le",
      sampleRateHz: 24_000,
      channels: 1,
      payloadLength: pcm.byteLength,
      sha256: createHash("sha256").update(pcm).digest("hex"),
    }),
  );
  const frame = new Uint8Array(8 + header.byteLength + pcm.byteLength);
  frame.set(new TextEncoder().encode(AUDIO_MEDIA_MAGIC), 0);
  new DataView(frame.buffer).setUint32(4, header.byteLength);
  frame.set(header, 8);
  frame.set(pcm, 8 + header.byteLength);
  return frame.buffer;
};

describe("managed Sarah mobile voice client", () => {
  test("authenticates with protected NIP-98 identity and refuses mobile device tools", async () => {
    const sockets: FixtureSocket[] = [];
    const vault = makeVault();
    let sessionIdentity: VoiceIdentity | null = null;
    let sessionBody: Record<string, unknown> | null = null;
    const requests: Array<Readonly<{ url: string; init?: RequestInit }>> = [];
    const fetch = async (input: string | URL | Request, init?: RequestInit): Promise<Response> => {
      const url = String(input);
      requests.push({ url, init });
      if (url.endsWith("/api/omega/auth/session")) {
        return Response.json(normalSessionResponse());
      }
      sessionBody = JSON.parse(String(init?.body)) as Record<string, unknown>;
      sessionIdentity = sessionBody.identity as VoiceIdentity;
      return Response.json(sessionResponse(sessionIdentity, "t".repeat(43), false), {
        status: 201,
      });
    };
    const client = new SarahVoiceClient({
      baseUrl: "https://openagents.com",
      publicKeyHex,
      signer,
      vault: vault.vault,
      fetch: fetch as typeof globalThis.fetch,
      createSocket: (url, headers) => {
        const socket = new FixtureSocket(url, headers);
        sockets.push(socket);
        return socket;
      },
      sha256,
      randomUuid: () => "voice-1",
      now: () => 10_000,
      setTimeout,
      clearTimeout,
    });
    const audioItems: string[] = [];
    client.onAudio(({ itemRef }) => audioItems.push(itemRef));

    await client.start();
    expect(requests).toHaveLength(2);
    expect(sessionBody).toMatchObject({
      schema: SARAH_VOICE_PROTOCOL_VERSION,
      disclosureRef: "openagents.mobile.sarah.voice.v1",
      clientProfile: "mobile_voice_only",
    });
    expect(requests[0]?.init).toMatchObject({
      method: "POST",
      headers: {
        authorization: expect.stringMatching(/^Nostr /u),
      },
    });
    expect(new Uint8Array(requests[0]?.init?.body as Uint8Array)).toHaveLength(0);
    expect(requests[1]?.init?.headers).toMatchObject({
      authorization: `Bearer ${accessToken}`,
      "x-openagents-omega-device-ref": `omega-mobile-${publicKeyHex.slice(0, 24)}`,
    });
    expect(vault.read()).toMatchObject({ publicKeyHex, ownerRef, accessToken });

    const socket = sockets[0]!;
    expect(socket.binaryType).toBe("arraybuffer");
    expect(socket.url).not.toContain("ticket");
    expect(socket.url).not.toContain("t".repeat(43));
    expect(socket.headers).toEqual({
      "x-openagents-sarah-voice-session": "sarah.voice.voice-1",
      "x-openagents-sarah-voice-ticket": "t".repeat(43),
    });
    socket.open();
    expect(JSON.parse(socket.sent[0] as string)).toMatchObject({
      _tag: "session_hello",
      disclosureRef: "openagents.mobile.sarah.voice.v1",
      sequence: 0,
    });

    const identity = sessionIdentity!;
    socket.serverControl(control(identity, 0, { _tag: "lifecycle", state: "listening" }));
    await tick();
    client.sendAudio(Uint8Array.from([1, 0, 2, 0]), 24_000, 1);
    await tick();
    expect(socket.sent.some((entry) => entry instanceof ArrayBuffer)).toBe(true);

    socket.onmessage?.({
      data: serverAudioFrame(identity, 0, "provider-item-1"),
    });
    socket.onmessage?.({
      data: serverAudioFrame(identity, 1, "provider-item-1"),
    });
    await tick();
    await tick();
    expect(audioItems).toEqual(["provider-item-1", "provider-item-1"]);

    socket.serverControl(
      control(identity, 1, {
        _tag: "transcript_delta",
        source: "user",
        utteranceRef: "utterance-1",
        text: "Hello",
      }),
    );
    socket.serverControl(
      control(identity, 2, {
        _tag: "transcript_final",
        source: "user",
        utteranceRef: "utterance-1",
        text: "Hello Sarah.",
      }),
    );
    await tick();
    expect(client.snapshot().transcripts).toEqual([
      {
        utteranceRef: "utterance-1",
        source: "user",
        text: "Hello Sarah.",
        final: true,
      },
    ]);

    socket.serverControl(
      control(identity, 3, {
        _tag: "tool_execute",
        proposalRef: "proposal-1",
        proposalDigest: "f".repeat(64),
        command: {
          _tag: "open_path",
          target: { workspaceRef: "workspace-1", path: "secret.txt" },
        },
      }),
    );
    await tick();
    expect(client.snapshot()).toMatchObject({
      phase: "error",
      message: "Mobile Sarah voice refused an unsupported device action.",
    });
    expect(socket.closes.at(-1)).toEqual({
      code: 1011,
      reason: "transport_error",
    });
  });

  test("accepts a device-linked GitHub canonical account session", async () => {
    const sockets: FixtureSocket[] = [];
    const vault = makeVault();
    let sessionIdentity: VoiceIdentity | null = null;
    const client = new SarahVoiceClient({
      baseUrl: "https://openagents.com",
      publicKeyHex,
      signer,
      vault: vault.vault,
      fetch: (async (input, init) => {
        const url = String(input);
        if (url.endsWith("/api/omega/auth/session")) {
          return Response.json(normalSessionResponse("github"));
        }
        const body = JSON.parse(String(init?.body)) as Record<string, unknown>;
        sessionIdentity = body.identity as VoiceIdentity;
        return Response.json(sessionResponse(sessionIdentity, "t".repeat(43), false), {
          status: 201,
        });
      }) as typeof globalThis.fetch,
      createSocket: (url, headers) => {
        const socket = new FixtureSocket(url, headers);
        sockets.push(socket);
        return socket;
      },
      sha256,
      randomUuid: () => "voice-github",
      now: () => 10_000,
      setTimeout,
      clearTimeout,
    });

    await client.start();
    expect(vault.read()).toMatchObject({ publicKeyHex, ownerRef, accessToken });
    expect((sessionIdentity as VoiceIdentity | null)?.ownerRef).toBe(ownerRef);
    expect(client.snapshot().phase).toBe("connecting");
    expect(sockets).toHaveLength(1);
  });

  test("uses the bounded Sarah challenge when automatic account sessions are unavailable", async () => {
    const vault = makeVault();
    const requests: string[] = [];
    let sessionBody: Record<string, unknown> | null = null;
    const client = new SarahVoiceClient({
      baseUrl: "https://openagents.com",
      publicKeyHex,
      signer,
      vault: vault.vault,
      fetch: (async (input, init) => {
        const url = String(input);
        requests.push(url);
        if (url.endsWith("/api/omega/auth/session")) {
          return Response.json({ error: "omega_nostr_auth_unavailable" }, { status: 503 });
        }
        if (url.endsWith("/auth/challenge")) {
          return Response.json({
            schema: SARAH_VOICE_NOSTR_CHALLENGE_PROTOCOL_VERSION,
            challenge,
            expiresAtMs: 20_000,
            ownerRef,
          });
        }
        sessionBody = JSON.parse(String(init?.body)) as Record<string, unknown>;
        return Response.json(
          sessionResponse(sessionBody.identity as VoiceIdentity, "t".repeat(43), true),
          { status: 201 },
        );
      }) as typeof globalThis.fetch,
      createSocket: (url, headers) => new FixtureSocket(url, headers),
      sha256,
      randomUuid: () => "voice-challenge",
      now: () => 10_000,
      setTimeout,
      clearTimeout,
    });

    await client.start();
    expect(requests).toEqual([
      "https://openagents.com/api/omega/auth/session",
      "https://openagents.com/api/omega/sarah/voice/auth/challenge",
      "https://openagents.com/api/omega/sarah/voice/session",
    ]);
    expect(sessionBody).toMatchObject({
      auth: {
        method: "nostr_nip98",
        challenge,
      },
      clientProfile: "mobile_voice_only",
    });
    expect(vault.read()).toMatchObject({ publicKeyHex, ownerRef, accessToken });
  });

  test("accepts a linked canonical GitHub account after fresh device link", async () => {
    const vault = makeVault();
    const nativeSession = makeNativeSessionStore(canonicalSessionRecord());
    const requests: Array<Readonly<{ url: string; init?: RequestInit }>> = [];
    let omegaAttempts = 0;
    let voiceIdentity: VoiceIdentity | null = null;
    const rotatedAccessToken = `oa_access_${"e".repeat(32)}`;
    const rotatedRefreshToken = `oa_refresh_${"f".repeat(32)}`;
    const client = new SarahVoiceClient({
      baseUrl: "https://openagents.com",
      publicKeyHex,
      signer,
      vault: vault.vault,
      fetch: (async (input, init) => {
        const url = String(input);
        requests.push({ url, init });
        if (url.endsWith("/api/omega/auth/session")) {
          omegaAttempts += 1;
          return omegaAttempts === 1
            ? Response.json({ error: "mobile_session_required" }, { status: 401 })
            : Response.json(normalSessionResponse("github"));
        }
        if (url.endsWith("/api/mobile/auth/session")) {
          return Response.json({
            authenticated: true,
            user: { userId: ownerRef },
            tokens: {
              access: rotatedAccessToken,
              refresh: rotatedRefreshToken,
              expiresIn: 900,
            },
          });
        }
        if (url.endsWith(OMEGA_NOSTR_DEVICE_LINK_CHALLENGE_PATH)) {
          return Response.json({
            schema: OMEGA_NOSTR_DEVICE_LINK_CHALLENGE_PROTOCOL_VERSION,
            challenge: deviceLinkChallenge,
            expiresAtMs: 20_000,
            ownerRef,
          });
        }
        if (url.endsWith(OMEGA_NOSTR_DEVICE_LINK_PATH)) {
          return Response.json({
            schema: OMEGA_NOSTR_DEVICE_LINK_PROTOCOL_VERSION,
            linked: true,
            ownerRef,
          });
        }
        const body = JSON.parse(String(init?.body)) as Record<string, unknown>;
        voiceIdentity = body.identity as VoiceIdentity;
        return Response.json(sessionResponse(voiceIdentity, "t".repeat(43), false), {
          status: 201,
        });
      }) as typeof globalThis.fetch,
      createSocket: (url, headers) => new FixtureSocket(url, headers),
      sha256,
      randomUuid: () => "voice-fresh-device",
      now: () => 10_000,
      setTimeout,
      clearTimeout,
      recoverDeviceLink: makeSarahVoiceDeviceLinkRecovery(nativeSession.store),
    });

    await client.start();

    expect(client.snapshot()).toMatchObject({
      phase: "connecting",
      message: null,
    });
    expect(requests.map(({ url }) => url)).toEqual([
      "https://openagents.com/api/omega/auth/session",
      "https://openagents.com/api/mobile/auth/session",
      `https://openagents.com${OMEGA_NOSTR_DEVICE_LINK_CHALLENGE_PATH}`,
      `https://openagents.com${OMEGA_NOSTR_DEVICE_LINK_PATH}`,
      "https://openagents.com/api/omega/auth/session",
      "https://openagents.com/api/omega/sarah/voice/session",
    ]);
    expect(requests[1]?.init?.headers).toEqual({
      authorization: `Bearer ${canonicalAccessToken}`,
      "x-openagents-refresh-token": canonicalRefreshToken,
    });
    const challengeBody = String(requests[2]?.init?.body);
    expect(JSON.parse(challengeBody)).toEqual({
      schema: OMEGA_NOSTR_DEVICE_LINK_CHALLENGE_PROTOCOL_VERSION,
      pubkey: publicKeyHex,
      deviceRef: `omega-mobile-${publicKeyHex.slice(0, 24)}`,
    });
    expect(requests[2]?.init?.headers).toEqual({
      authorization: `Bearer ${rotatedAccessToken}`,
      "content-type": "application/json",
      "x-openagents-refresh-token": rotatedRefreshToken,
    });
    const linkBody = String(requests[3]?.init?.body);
    expect(JSON.parse(linkBody)).toEqual({
      schema: OMEGA_NOSTR_DEVICE_LINK_PROTOCOL_VERSION,
      challenge: deviceLinkChallenge,
      ownerRef,
      deviceRef: `omega-mobile-${publicKeyHex.slice(0, 24)}`,
    });
    expect(requests[3]?.init?.headers).toEqual({
      authorization: expect.stringMatching(/^Nostr /u),
      "content-type": "application/json",
      "x-openagents-omega-device-ref": `omega-mobile-${publicKeyHex.slice(0, 24)}`,
    });
    const linkHeaders = requests[3]?.init?.headers as Record<string, string>;
    const proof = JSON.parse(atob(linkHeaders.authorization!.slice(6))) as Readonly<{
      tags: ReadonlyArray<ReadonlyArray<string>>;
    }>;
    expect(proof.tags).toEqual([
      ["u", `https://openagents.com${OMEGA_NOSTR_DEVICE_LINK_PATH}`],
      ["method", "POST"],
      ["payload", createHash("sha256").update(linkBody).digest("hex")],
    ]);
    const rotatedRecord = JSON.parse(nativeSession.read() ?? "{}") as Record<string, unknown>;
    expect(rotatedRecord).toMatchObject({
      ownerUserId: ownerRef,
      accessToken: rotatedAccessToken,
      refreshToken: rotatedRefreshToken,
    });
    expect(vault.read()).toMatchObject({ ownerRef, accessToken });
    expect(voiceIdentity).toMatchObject({
      ownerRef,
      deviceRef: `omega-mobile-${publicKeyHex.slice(0, 24)}`,
    });
  });

  test("does not run device-link recovery for an already linked device", async () => {
    const vault = makeVault();
    let recoveryCount = 0;
    const requests: string[] = [];
    const client = new SarahVoiceClient({
      baseUrl: "https://openagents.com",
      publicKeyHex,
      signer,
      vault: vault.vault,
      fetch: (async (input, init) => {
        const url = String(input);
        requests.push(url);
        if (url.endsWith("/api/omega/auth/session")) {
          return Response.json(normalSessionResponse());
        }
        const body = JSON.parse(String(init?.body)) as Record<string, unknown>;
        return Response.json(
          sessionResponse(body.identity as VoiceIdentity, "t".repeat(43), false),
          { status: 201 },
        );
      }) as typeof globalThis.fetch,
      createSocket: (url, headers) => new FixtureSocket(url, headers),
      sha256,
      randomUuid: () => "voice-linked-device",
      now: () => 10_000,
      setTimeout,
      clearTimeout,
      recoverDeviceLink: async () => {
        recoveryCount += 1;
        return { _tag: "Success", ownerRef };
      },
    });

    await client.start();

    expect(recoveryCount).toBe(0);
    expect(requests).toEqual([
      "https://openagents.com/api/omega/auth/session",
      "https://openagents.com/api/omega/sarah/voice/session",
    ]);
  });

  test("accepts an already linked canonical email account", async () => {
    const vault = makeVault();
    const client = new SarahVoiceClient({
      baseUrl: "https://openagents.com",
      publicKeyHex,
      signer,
      vault: vault.vault,
      fetch: (async (input, init) => {
        const url = String(input);
        if (url.endsWith("/api/omega/auth/session")) {
          return Response.json(normalSessionResponse("email"));
        }
        const body = JSON.parse(String(init?.body)) as Readonly<{ identity: VoiceIdentity }>;
        return Response.json(sessionResponse(body.identity, "t".repeat(43), false), {
          status: 201,
        });
      }) as typeof globalThis.fetch,
      createSocket: (url, headers) => new FixtureSocket(url, headers),
      sha256,
      randomUuid: () => "voice-linked-email",
      now: () => 10_000,
      setTimeout,
      clearTimeout,
    });

    await client.start();

    expect(client.snapshot()).toMatchObject({ phase: "connecting", message: null });
    expect(vault.read()).toMatchObject({ ownerRef, accessToken });
  });

  test("does not expose a canonical session decoder error", async () => {
    const vault = makeVault();
    const client = new SarahVoiceClient({
      baseUrl: "https://openagents.com",
      publicKeyHex,
      signer,
      vault: vault.vault,
      fetch: (async () =>
        Response.json({
          accessToken,
          expiresIn: 900,
          user: { userId: ownerRef, provider: "unsupported" },
        })) as typeof globalThis.fetch,
      createSocket: () => {
        throw new Error("An invalid session must not open a socket.");
      },
      sha256,
      randomUuid: () => "voice-invalid-session",
      now: () => 10_000,
      setTimeout,
      clearTimeout,
    });

    await client.start();

    expect(client.snapshot()).toMatchObject({
      phase: "error",
      message: "Sarah voice could not verify the OpenAgents session. Try again.",
      retryable: true,
    });
    expect(client.snapshot().message).not.toContain("provider");
  });

  test("asks a signed-out user to sign in before Sarah voice", async () => {
    const vault = makeVault();
    const nativeSession = makeNativeSessionStore(null);
    const requests: string[] = [];
    const client = new SarahVoiceClient({
      baseUrl: "https://openagents.com",
      publicKeyHex,
      signer,
      vault: vault.vault,
      fetch: (async (input) => {
        requests.push(String(input));
        return Response.json({ error: "mobile_session_required" }, { status: 401 });
      }) as typeof globalThis.fetch,
      createSocket: () => {
        throw new Error("A signed-out client must not open a socket.");
      },
      sha256,
      randomUuid: () => "voice-signed-out",
      now: () => 10_000,
      setTimeout,
      clearTimeout,
      recoverDeviceLink: makeSarahVoiceDeviceLinkRecovery(nativeSession.store),
    });

    await client.start();

    expect(requests).toEqual(["https://openagents.com/api/omega/auth/session"]);
    expect(client.snapshot()).toMatchObject({
      phase: "error",
      message: "Sign in to OpenAgents to use Sarah voice.",
      retryable: false,
    });
  });

  test.each([
    {
      status: 403,
      error: "invalid_nostr_device_link_proof",
      message: "Sarah could not verify this device. Try again.",
      retryable: true,
    },
    {
      status: 409,
      error: "nostr_identity_link_conflict",
      message: "This device is linked to another OpenAgents account.",
      retryable: false,
    },
  ])(
    "shows an actionable device-link error for $error",
    async ({ status, error, message, retryable }) => {
      const vault = makeVault();
      const nativeSession = makeNativeSessionStore(canonicalSessionRecord());
      const requests: string[] = [];
      const client = new SarahVoiceClient({
        baseUrl: "https://openagents.com",
        publicKeyHex,
        signer,
        vault: vault.vault,
        fetch: (async (input) => {
          const url = String(input);
          requests.push(url);
          if (url.endsWith("/api/omega/auth/session")) {
            return Response.json({ error: "mobile_session_required" }, { status: 401 });
          }
          if (url.endsWith("/api/mobile/auth/session")) {
            return Response.json({
              authenticated: true,
              user: { userId: ownerRef },
            });
          }
          if (url.endsWith(OMEGA_NOSTR_DEVICE_LINK_CHALLENGE_PATH)) {
            return Response.json({
              schema: OMEGA_NOSTR_DEVICE_LINK_CHALLENGE_PROTOCOL_VERSION,
              challenge: deviceLinkChallenge,
              expiresAtMs: 20_000,
              ownerRef,
            });
          }
          return Response.json({ error }, { status });
        }) as typeof globalThis.fetch,
        createSocket: () => {
          throw new Error("A rejected device link must not open a socket.");
        },
        sha256,
        randomUuid: () => "voice-link-error",
        now: () => 10_000,
        setTimeout,
        clearTimeout,
        recoverDeviceLink: makeSarahVoiceDeviceLinkRecovery(nativeSession.store),
      });

      await client.start();

      expect(requests).toEqual([
        "https://openagents.com/api/omega/auth/session",
        "https://openagents.com/api/mobile/auth/session",
        `https://openagents.com${OMEGA_NOSTR_DEVICE_LINK_CHALLENGE_PATH}`,
        `https://openagents.com${OMEGA_NOSTR_DEVICE_LINK_PATH}`,
      ]);
      expect(client.snapshot()).toMatchObject({
        phase: "error",
        message,
        retryable,
      });
    },
  );

  test.each([
    { status: 402, error: "insufficient_credit" },
    { status: 403, error: "sarah_voice_not_available" },
  ])(
    "refreshes the scoped session and retries voice once after $status",
    async ({ status, error }) => {
      const priorAccessToken = `oa_omega_${"p".repeat(43)}`;
      const stored: SarahVoiceStoredSession = {
        schemaVersion: 1,
        publicKeyHex,
        ownerRef,
        accessToken: priorAccessToken,
        expiresAtMs: 100_000,
      };
      const vault = makeVault(stored);
      const requests: Array<Readonly<{ url: string; authorization: string }>> = [];
      let voiceAttempts = 0;
      const uuids = ["voice-policy-denied", "voice-policy-restored"];
      const client = new SarahVoiceClient({
        baseUrl: "https://openagents.com",
        publicKeyHex,
        signer,
        vault: vault.vault,
        fetch: (async (input, init) => {
          const url = String(input);
          requests.push({
            url,
            authorization: String((init?.headers as Record<string, string>)?.authorization ?? ""),
          });
          if (url.endsWith("/api/omega/auth/session")) {
            return Response.json(normalSessionResponse());
          }
          voiceAttempts += 1;
          if (voiceAttempts === 1) return Response.json({ error }, { status });
          const body = JSON.parse(String(init?.body)) as Readonly<{ identity: VoiceIdentity }>;
          return Response.json(sessionResponse(body.identity, "r".repeat(43), false), {
            status: 201,
          });
        }) as typeof globalThis.fetch,
        createSocket: (url, headers) => new FixtureSocket(url, headers),
        sha256,
        randomUuid: () => uuids.shift() ?? "unexpected-policy-retry",
        now: () => 10_000,
        setTimeout,
        clearTimeout,
      });

      await client.start();

      expect(requests.map(({ url }) => url)).toEqual([
        "https://openagents.com/api/omega/sarah/voice/session",
        "https://openagents.com/api/omega/auth/session",
        "https://openagents.com/api/omega/sarah/voice/session",
      ]);
      expect(requests[0]?.authorization).toBe(`Bearer ${priorAccessToken}`);
      expect(requests[2]?.authorization).toBe(`Bearer ${accessToken}`);
      expect(vault.read()?.accessToken).toBe(accessToken);
      expect(client.snapshot()).toMatchObject({
        phase: "connecting",
        message: null,
      });
    },
  );

  test("preserves a real credit denial after one scoped-session retry", async () => {
    const stored: SarahVoiceStoredSession = {
      schemaVersion: 1,
      publicKeyHex,
      ownerRef,
      accessToken,
      expiresAtMs: 100_000,
    };
    const vault = makeVault(stored);
    const requests: string[] = [];
    const client = new SarahVoiceClient({
      baseUrl: "https://openagents.com",
      publicKeyHex,
      signer,
      vault: vault.vault,
      fetch: (async (input) => {
        const url = String(input);
        requests.push(url);
        if (url.endsWith("/api/omega/auth/session")) {
          return Response.json(normalSessionResponse());
        }
        return Response.json({ error: "insufficient_credit" }, { status: 402 });
      }) as typeof globalThis.fetch,
      createSocket: () => {
        throw new Error("A denied session must not open a socket.");
      },
      sha256,
      randomUuid: () => "voice-credit",
      now: () => 10_000,
      setTimeout,
      clearTimeout,
    });

    await client.start();
    expect(requests).toEqual([
      "https://openagents.com/api/omega/sarah/voice/session",
      "https://openagents.com/api/omega/auth/session",
      "https://openagents.com/api/omega/sarah/voice/session",
    ]);
    expect(vault.clearCount()).toBe(0);
    expect(client.snapshot()).toMatchObject({
      phase: "error",
      message: "Sarah voice needs available OpenAgents credits. Add credits, then retry voice.",
      retryable: true,
    });
  });

  test("reconnects with a new one-use session and ticket", async () => {
    const sockets: FixtureSocket[] = [];
    const vault = makeVault();
    const sessionRequests: Array<Record<string, unknown>> = [];
    const timers: Array<() => void> = [];
    const uuids = ["voice-1", "voice-2"];
    const fetch = async (input: string | URL | Request, init?: RequestInit): Promise<Response> => {
      const url = String(input);
      if (url.endsWith("/api/omega/auth/session")) {
        return Response.json(normalSessionResponse());
      }
      const body = JSON.parse(String(init?.body)) as Record<string, unknown>;
      sessionRequests.push(body);
      const identity = body.identity as VoiceIdentity;
      return Response.json(
        sessionResponse(identity, `${sessionRequests.length}`.repeat(43), false),
        { status: 201 },
      );
    };
    const client = new SarahVoiceClient({
      baseUrl: "https://openagents.com",
      publicKeyHex,
      signer,
      vault: vault.vault,
      fetch: fetch as typeof globalThis.fetch,
      createSocket: (url, headers) => {
        const socket = new FixtureSocket(url, headers);
        sockets.push(socket);
        return socket;
      },
      sha256,
      randomUuid: () => uuids.shift() ?? "unexpected",
      now: () => 10_000,
      setTimeout: ((callback: () => void) => {
        timers.push(callback);
        return 1 as unknown as ReturnType<typeof setTimeout>;
      }) as typeof setTimeout,
      clearTimeout: () => undefined,
    });

    await client.start();
    sockets[0]!.serverClose();
    expect(client.snapshot().phase).toBe("reconnecting");
    expect(timers).toHaveLength(1);
    timers[0]!();
    await tick();

    expect(sessionRequests).toHaveLength(2);
    expect((sessionRequests[0]!.identity as VoiceIdentity).sessionRef).not.toBe(
      (sessionRequests[1]!.identity as VoiceIdentity).sessionRef,
    );
    expect(sockets).toHaveLength(2);
    expect(sockets[0]!.headers["x-openagents-sarah-voice-ticket"]).not.toBe(
      sockets[1]!.headers["x-openagents-sarah-voice-ticket"],
    );
    expect(sockets[1]!.url).not.toContain("ticket");
  });

  test("drops decoded audio when the app leaves the foreground during digest validation", async () => {
    const sockets: FixtureSocket[] = [];
    const vault = makeVault();
    let sessionIdentity: VoiceIdentity | null = null;
    let deferDigest = false;
    const digestGate: { release?: () => void } = {};
    const deferredSha256 = async (bytes: Uint8Array): Promise<Uint8Array> => {
      if (deferDigest) {
        await new Promise<void>((resolve) => {
          digestGate.release = resolve;
        });
      }
      return sha256(bytes);
    };
    const fetch = async (input: string | URL | Request, init?: RequestInit): Promise<Response> => {
      if (String(input).endsWith("/api/omega/auth/session")) {
        return Response.json(normalSessionResponse());
      }
      const body = JSON.parse(String(init?.body)) as Record<string, unknown>;
      sessionIdentity = body.identity as VoiceIdentity;
      return Response.json(sessionResponse(sessionIdentity, "t".repeat(43), false), {
        status: 201,
      });
    };
    const client = new SarahVoiceClient({
      baseUrl: "https://openagents.com",
      publicKeyHex,
      signer,
      vault: vault.vault,
      fetch: fetch as typeof globalThis.fetch,
      createSocket: (url, headers) => {
        const socket = new FixtureSocket(url, headers);
        sockets.push(socket);
        return socket;
      },
      sha256: deferredSha256,
      randomUuid: () => "voice-background",
      now: () => 10_000,
      setTimeout,
      clearTimeout,
    });
    const audioItems: string[] = [];
    client.onAudio(({ itemRef }) => audioItems.push(itemRef));

    await client.start();
    const socket = sockets[0]!;
    socket.open();
    socket.serverControl(
      control(sessionIdentity!, 0, {
        _tag: "lifecycle",
        state: "listening",
      }),
    );
    await tick();

    deferDigest = true;
    socket.onmessage?.({
      data: serverAudioFrame(sessionIdentity!, 0, "provider-item-background"),
    });
    await tick();
    client.setForeground(false);
    digestGate.release?.();
    await tick();

    expect(audioItems).toEqual([]);
    expect(client.snapshot().phase).toBe("ended");
    expect(socket.closes.at(-1)).toEqual({
      code: 1000,
      reason: "app_backgrounded",
    });
  });
});
