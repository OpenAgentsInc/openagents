import { Effect, Schema } from "effect";
import { sha256 } from "@noble/hashes/sha2";
import { bytesToHex, utf8ToBytes } from "@noble/hashes/utils";

import type {
  Issue31DeviceIdentity,
  Issue31SecureStore,
  Issue31SecureStoreOptions,
} from "./issue31-device-key-vault.ts";

export const OMEGA_DEVICE_BRIDGE_PROTOCOL = "openagents.omega.device_bridge.v1" as const;
export const OMEGA_DEVICE_BRIDGE_STORE_KEY = "openagents.omega.device-bridge.v1" as const;
export const OMEGA_DEVICE_BRIDGE_KEYCHAIN_SERVICE =
  "com.openagents.mobile.omega-device-bridge" as const;
export const OMEGA_DEVICE_BRIDGE_MAX_FRAME_BYTES = 64 * 1024;
export const OMEGA_DEVICE_BRIDGE_ADMISSION_TIMEOUT_MS = 5_000;

const NonNegativeInteger = Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0));
const PositiveInteger = Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0));
const Hex32 = Schema.String.check(Schema.isPattern(/^[0-9a-f]{64}$/));
const Hex64 = Schema.String.check(Schema.isPattern(/^[0-9a-f]{128}$/));
const NonEmptyString = Schema.String.check(Schema.isMinLength(1));

export const OmegaDeviceBridgeCursorSchema = Schema.Struct({
  generation: NonNegativeInteger,
  sequence: NonNegativeInteger,
});
export type OmegaDeviceBridgeCursor = Schema.Schema.Type<typeof OmegaDeviceBridgeCursorSchema>;

export const OmegaExecutorDisclosureSchema = Schema.Struct({
  executorId: NonEmptyString,
  executorName: NonEmptyString,
  modelId: Schema.NullOr(Schema.String),
  modelName: Schema.NullOr(Schema.String),
});
export type OmegaExecutorDisclosure = Schema.Schema.Type<typeof OmegaExecutorDisclosureSchema>;

export const OmegaMirrorThreadSchema = Schema.Struct({
  threadRef: NonEmptyString,
  title: NonEmptyString,
  executor: OmegaExecutorDisclosureSchema,
  state: Schema.Literals(["idle", "queued", "running", "waiting", "completed", "failed"]),
  transcript: Schema.Array(
    Schema.Struct({
      messageRef: NonEmptyString,
      role: Schema.Literals(["user", "assistant", "system", "tool"]),
      text: Schema.String,
      createdAt: NonNegativeInteger,
    }),
  ),
  updatedAt: NonNegativeInteger,
});
export type OmegaMirrorThread = Schema.Schema.Type<typeof OmegaMirrorThreadSchema>;

export const OmegaMirrorRunSchema = Schema.Struct({
  runRef: NonEmptyString,
  title: NonEmptyString,
  lane: Schema.String,
  state: Schema.Literals(["queued", "running", "paused", "completed", "failed", "cancelled"]),
  receiptRefs: Schema.Array(NonEmptyString),
  updatedAt: NonNegativeInteger,
});
export type OmegaMirrorRun = Schema.Schema.Type<typeof OmegaMirrorRunSchema>;

export const OmegaMirrorHealthSchema = Schema.Struct({
  engineUp: Schema.Boolean,
  engineGeneration: NonNegativeInteger,
  laneReady: Schema.Boolean,
  observedAt: NonNegativeInteger,
});
export type OmegaMirrorHealth = Schema.Schema.Type<typeof OmegaMirrorHealthSchema>;

export const OmegaMirrorSnapshotSchema = Schema.Struct({
  desktopName: NonEmptyString,
  generation: NonNegativeInteger,
  sequence: NonNegativeInteger,
  threads: Schema.Array(OmegaMirrorThreadSchema),
  runs: Schema.Array(OmegaMirrorRunSchema),
  health: OmegaMirrorHealthSchema,
  projectedAt: NonNegativeInteger,
});
export type OmegaMirrorSnapshot = Schema.Schema.Type<typeof OmegaMirrorSnapshotSchema>;

const ThreadUpsertDelta = Schema.Struct({
  type: Schema.Literal("thread_upsert"),
  thread: OmegaMirrorThreadSchema,
});
const ThreadRemoveDelta = Schema.Struct({
  type: Schema.Literal("thread_remove"),
  threadRef: NonEmptyString,
});
const TranscriptAppendDelta = Schema.Struct({
  type: Schema.Literal("transcript_append"),
  threadRef: NonEmptyString,
  message: Schema.Struct({
    messageRef: NonEmptyString,
    role: Schema.Literals(["user", "assistant", "system", "tool"]),
    text: Schema.String,
    createdAt: NonNegativeInteger,
  }),
  updatedAt: NonNegativeInteger,
});
const RunUpsertDelta = Schema.Struct({
  type: Schema.Literal("run_upsert"),
  run: OmegaMirrorRunSchema,
});
const RunRemoveDelta = Schema.Struct({
  type: Schema.Literal("run_remove"),
  runRef: NonEmptyString,
});
const HealthDelta = Schema.Struct({
  type: Schema.Literal("health"),
  health: OmegaMirrorHealthSchema,
});

export const OmegaMirrorChangeSchema = Schema.Union([
  ThreadUpsertDelta,
  ThreadRemoveDelta,
  TranscriptAppendDelta,
  RunUpsertDelta,
  RunRemoveDelta,
  HealthDelta,
]);
export type OmegaMirrorChange = Schema.Schema.Type<typeof OmegaMirrorChangeSchema>;

const SignedDeviceProofSchema = Schema.Struct({
  id: Hex32,
  pubkey: Hex32,
  created_at: NonNegativeInteger,
  kind: PositiveInteger,
  tags: Schema.Array(Schema.Array(Schema.String)),
  content: Schema.String,
  sig: Hex64,
});

const HelloFrameSchema = Schema.Struct({
  type: Schema.Literal("hello"),
  protocol: Schema.Literal(OMEGA_DEVICE_BRIDGE_PROTOCOL),
  devicePublicKeyHex: Hex32,
  hostPublicKeyHex: Hex32,
  grantRef: Schema.NullOr(Schema.String),
  pairingSecret: Schema.NullOr(Schema.String),
  resumeCursor: Schema.NullOr(OmegaDeviceBridgeCursorSchema),
  proof: SignedDeviceProofSchema,
});

const GrantAcceptedFrameSchema = Schema.Struct({
  type: Schema.Literal("grant"),
  admitted: Schema.Literal(true),
  grantRef: NonEmptyString,
  hostPublicKeyHex: Hex32,
  devicePublicKeyHex: Hex32,
  expiresAt: NonNegativeInteger,
  generation: NonNegativeInteger,
});
const GrantRefusedFrameSchema = Schema.Struct({
  type: Schema.Literal("grant"),
  admitted: Schema.Literal(false),
  reason: Schema.Literals([
    "invalid_proof",
    "grant_missing",
    "grant_expired",
    "grant_revoked",
    "protocol_unsupported",
    "pairing_expired",
    "pairing_refused",
  ]),
});
const SnapshotFrameSchema = Schema.Struct({
  type: Schema.Literal("snapshot"),
  snapshot: OmegaMirrorSnapshotSchema,
});
const DeltaFrameSchema = Schema.Struct({
  type: Schema.Literal("delta"),
  generation: NonNegativeInteger,
  sequence: PositiveInteger,
  change: OmegaMirrorChangeSchema,
});
const HeartbeatFrameSchema = Schema.Struct({
  type: Schema.Literal("heartbeat"),
  generation: NonNegativeInteger,
  sequence: NonNegativeInteger,
  sentAt: NonNegativeInteger,
});
const ByeFrameSchema = Schema.Struct({
  type: Schema.Literal("bye"),
  reason: Schema.Literals([
    "client_closed",
    "host_shutdown",
    "grant_revoked",
    "grant_expired",
    "protocol_error",
    "resnapshot_required",
  ]),
});

export const OmegaDeviceBridgeServerFrameSchema = Schema.Union([
  GrantAcceptedFrameSchema,
  GrantRefusedFrameSchema,
  SnapshotFrameSchema,
  DeltaFrameSchema,
  HeartbeatFrameSchema,
  ByeFrameSchema,
]);
export type OmegaDeviceBridgeServerFrame = Schema.Schema.Type<
  typeof OmegaDeviceBridgeServerFrameSchema
>;

export const OmegaBridgeEndpointSchema = Schema.Struct({
  url: Schema.String.check(Schema.isPattern(/^wss?:\/\/[^\s]+$/)),
  source: Schema.Literals(["cached", "announcement", "qr", "manual"]),
  hostPublicKeyHex: Hex32,
  generation: NonNegativeInteger,
  expiresAt: Schema.NullOr(NonNegativeInteger),
  pairingSecret: Schema.NullOr(Schema.String),
});
export type OmegaBridgeEndpoint = Schema.Schema.Type<typeof OmegaBridgeEndpointSchema>;

export const OmegaBridgeAnnouncementSchema = Schema.Struct({
  hostPublicKeyHex: Hex32,
  generation: NonNegativeInteger,
  expiresAt: NonNegativeInteger,
  endpoints: Schema.Array(
    Schema.Struct({
      url: Schema.String.check(Schema.isPattern(/^wss?:\/\/[^\s]+$/)),
      protocol: Schema.Literal(OMEGA_DEVICE_BRIDGE_PROTOCOL),
    }),
  ),
});
export type OmegaBridgeAnnouncement = Schema.Schema.Type<typeof OmegaBridgeAnnouncementSchema>;

export const OmegaBridgePairingBootstrapSchema = Schema.Struct({
  endpoint: Schema.String.check(Schema.isPattern(/^wss?:\/\/[^\s]+$/)),
  hostPublicKeyHex: Hex32,
  pairingSecret: NonEmptyString,
  expiresAt: NonNegativeInteger,
});
export type OmegaBridgePairingBootstrap = Schema.Schema.Type<
  typeof OmegaBridgePairingBootstrapSchema
>;

const decodePairingBootstrap = Schema.decodeUnknownSync(OmegaBridgePairingBootstrapSchema);

export const decodeOmegaBridgePairingBootstrap = (input: unknown): OmegaBridgePairingBootstrap =>
  decodePairingBootstrap(input, {
    onExcessProperty: "error",
  });

/** The schema tag the desktop stamps into the JSON its pairing QR carries. */
export const OMEGA_DESKTOP_PAIRING_BOOTSTRAP_SCHEMA =
  "openagents.omega.device_pairing.v1" as const;

/**
 * The HTTPS base the desktop pairing QR may wrap its payload in, so the iOS
 * Camera app opens this app through a Universal Link instead of dead-ending in
 * a browser. The payload rides in the URL fragment, which never reaches the
 * openagents.com server or its logs.
 */
export const OMEGA_PAIRING_LINK_BASE = "https://openagents.com/pair" as const;

const MagicDnsName = Schema.String.check(
  Schema.isPattern(/^[a-z0-9]([a-z0-9.-]{0,251}[a-z0-9])?$/),
);

/**
 * What the desktop actually serializes into the QR: `PairingBootstrap` in
 * `crates/omega_device_bridge`. It names the host by MagicDNS name and port
 * rather than a dialable URL, so admission maps it onto the bridge's plain
 * WebSocket endpoint.
 */
const DesktopPairingBootstrapSchema = Schema.Struct({
  schema: Schema.Literal(OMEGA_DESKTOP_PAIRING_BOOTSTRAP_SCHEMA),
  magicDnsName: MagicDnsName,
  port: PositiveInteger.check(Schema.isLessThanOrEqualTo(65_535)),
  protocol: Schema.Literal(OMEGA_DEVICE_BRIDGE_PROTOCOL),
  hostPublicKeyHex: Hex32,
  pairingSecret: NonEmptyString,
  generation: PositiveInteger,
  issuedAt: NonNegativeInteger,
  expiresAt: NonNegativeInteger,
});

const decodeDesktopPairingBootstrap = Schema.decodeUnknownSync(DesktopPairingBootstrapSchema);

const pairingFromJsonValue = (value: unknown): OmegaBridgePairingBootstrap => {
  if (typeof value === "object" && value !== null && "schema" in value) {
    const desktop = decodeDesktopPairingBootstrap(value, { onExcessProperty: "error" });
    return decodeOmegaBridgePairingBootstrap({
      endpoint: `ws://${desktop.magicDnsName}:${desktop.port}`,
      hostPublicKeyHex: desktop.hostPublicKeyHex,
      pairingSecret: desktop.pairingSecret,
      expiresAt: desktop.expiresAt,
    });
  }
  return decodeOmegaBridgePairingBootstrap(value);
};

/** Whether a URL is this app's pairing Universal Link with a payload fragment. */
export const isOmegaPairingLink = (text: string): boolean =>
  omegaPairingLinkFragment(text.trim()) !== null;

const omegaPairingLinkFragment = (text: string): string | null => {
  const hashIndex = text.indexOf("#");
  if (hashIndex < 0) return null;
  const base = text.slice(0, hashIndex);
  if (base !== OMEGA_PAIRING_LINK_BASE && base !== `${OMEGA_PAIRING_LINK_BASE}/`) return null;
  const fragment = text.slice(hashIndex + 1);
  return fragment === "" ? null : fragment;
};

// Hand-rolled on purpose: Hermes has bitten this app before with APIs that
// exist in Node but not on device (`toSorted`, and the missing platform
// implementations `crypto-random-values.ts` documents). Base64 and UTF-8
// decoding stay dependency-free here so the scanner path cannot die that way.
const BASE64URL_ALPHABET = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

const base64UrlToBytes = (encoded: string): Uint8Array | null => {
  if (encoded.length % 4 === 1) return null;
  const bytes: Array<number> = [];
  let buffer = 0;
  let bits = 0;
  for (const character of encoded) {
    if (character === "=") break;
    const value = BASE64URL_ALPHABET.indexOf(
      character === "+" ? "-" : character === "/" ? "_" : character,
    );
    if (value < 0) return null;
    buffer = (buffer << 6) | value;
    bits += 6;
    if (bits >= 8) {
      bits -= 8;
      bytes.push((buffer >> bits) & 0xff);
    }
  }
  return Uint8Array.from(bytes);
};

const bytesToText = (bytes: Uint8Array): string | null => {
  let percentEncoded = "";
  for (const byte of bytes) {
    percentEncoded += `%${byte.toString(16).padStart(2, "0")}`;
  }
  try {
    return decodeURIComponent(percentEncoded);
  } catch {
    return null;
  }
};

/**
 * Decode whatever text a pairing source hands over: the desktop QR's raw
 * bootstrap JSON, this client's own bootstrap JSON, or the
 * `https://openagents.com/pair#<base64url>` Universal Link wrapping either.
 * Accepting every format at once is what keeps an old desktop QR and a new
 * link-format QR both scannable from the same build.
 */
export const decodeOmegaBridgePairingText = (text: string): OmegaBridgePairingBootstrap => {
  const trimmed = text.trim();
  const fragment = omegaPairingLinkFragment(trimmed);
  if (fragment === null) {
    return pairingFromJsonValue(JSON.parse(trimmed) as unknown);
  }
  const bytes = base64UrlToBytes(fragment);
  const json = bytes === null ? null : bytesToText(bytes);
  if (json === null) {
    throw new Error("The pairing link does not carry a readable payload.");
  }
  return pairingFromJsonValue(JSON.parse(json) as unknown);
};

export const OmegaDeviceBridgeStoredStateSchema = Schema.Struct({
  schemaVersion: Schema.Literal(1),
  endpoint: Schema.NullOr(
    Schema.Struct({
      url: Schema.String.check(Schema.isPattern(/^wss?:\/\/[^\s]+$/)),
      hostPublicKeyHex: Hex32,
    }),
  ),
  grant: Schema.NullOr(
    Schema.Struct({
      grantRef: NonEmptyString,
      hostPublicKeyHex: Hex32,
      devicePublicKeyHex: Hex32,
      expiresAt: NonNegativeInteger,
      generation: Schema.optional(NonNegativeInteger),
    }),
  ),
  cursor: Schema.NullOr(OmegaDeviceBridgeCursorSchema),
});
export type OmegaDeviceBridgeStoredState = Schema.Schema.Type<
  typeof OmegaDeviceBridgeStoredStateSchema
>;

export interface OmegaDeviceBridgeStore {
  readonly load: () => Effect.Effect<OmegaDeviceBridgeStoredState | null, OmegaDeviceBridgeError>;
  readonly save: (
    state: OmegaDeviceBridgeStoredState,
  ) => Effect.Effect<void, OmegaDeviceBridgeError>;
  readonly clearGrant: () => Effect.Effect<void, OmegaDeviceBridgeError>;
}

export interface OmegaDeviceBridgeWebSocket {
  readonly readyState: number;
  readonly send: (data: string) => void;
  readonly close: (code?: number, reason?: string) => void;
  readonly addEventListener: (
    type: "open" | "message" | "close" | "error",
    listener: (event: { readonly data?: unknown }) => void,
  ) => void;
}

export type OmegaDeviceBridgeConnection = Readonly<{
  state: "direct" | "relay" | "offline";
  endpoint: string | null;
  heartbeatAt: number | null;
  relayObservedAt: number | null;
  staleSince: number | null;
}>;

export type OmegaDeviceBridgeState = Readonly<{
  paired: boolean;
  connection: OmegaDeviceBridgeConnection;
  mirror: OmegaMirrorSnapshot | null;
  recovery: "none" | "resnapshot_requested";
  refusal: string | null;
}>;

export class OmegaDeviceBridgeError extends Error {
  readonly _tag = "OmegaDeviceBridgeError";
  override readonly name = "OmegaDeviceBridgeError";

  constructor(
    readonly reason:
      | "invalid_frame"
      | "frame_too_large"
      | "connection_failed"
      | "all_endpoints_failed"
      | "storage_failed"
      | "closed",
    message: string,
  ) {
    super(message);
  }
}

const decodeServerFrame = Schema.decodeUnknownSync(OmegaDeviceBridgeServerFrameSchema);
const decodeStoredState = Schema.decodeUnknownSync(OmegaDeviceBridgeStoredStateSchema);
const encodeFrame = (frame: unknown): string => {
  const encoded = JSON.stringify(frame);
  if (new TextEncoder().encode(encoded).byteLength > OMEGA_DEVICE_BRIDGE_MAX_FRAME_BYTES) {
    throw new OmegaDeviceBridgeError("frame_too_large", "The Omega bridge frame is too large.");
  }
  return encoded;
};

export const omegaBridgeDialLadder = (
  input: Readonly<{
    stored: OmegaDeviceBridgeStoredState | null;
    announcements: ReadonlyArray<OmegaBridgeAnnouncement>;
    pairing: OmegaBridgePairingBootstrap | null;
    manualMagicDns: string | null;
    now: number;
    defaultPort: number;
  }>,
): ReadonlyArray<OmegaBridgeEndpoint> => {
  const candidates: Array<OmegaBridgeEndpoint> = [];
  if (input.pairing !== null && input.pairing.expiresAt > input.now) {
    candidates.push({
      url: input.pairing.endpoint,
      source: "qr",
      hostPublicKeyHex: input.pairing.hostPublicKeyHex,
      generation: 0,
      expiresAt: input.pairing.expiresAt,
      pairingSecret: input.pairing.pairingSecret,
    });
  }
  if (input.stored?.endpoint !== null && input.stored?.endpoint !== undefined) {
    candidates.push({
      ...input.stored.endpoint,
      source: "cached",
      generation: input.stored.cursor?.generation ?? 0,
      expiresAt: null,
      pairingSecret: null,
    });
  }
  // `Array.prototype.toSorted` is ES2023 and Hermes does not implement it. A
  // call throws a TypeError, which Effect reports as a defect rather than a
  // typed failure, so the dial ladder died silently on device while every
  // Node-run test passed. Sort a copy instead.
  for (const announcement of input.announcements
    .filter((entry) => entry.expiresAt > input.now)
    .slice()
    .sort((left, right) => right.generation - left.generation)) {
    for (const endpoint of announcement.endpoints) {
      candidates.push({
        url: endpoint.url,
        source: "announcement",
        hostPublicKeyHex: announcement.hostPublicKeyHex,
        generation: announcement.generation,
        expiresAt: announcement.expiresAt,
        pairingSecret: null,
      });
    }
  }
  const magicDns = input.manualMagicDns?.trim();
  if (magicDns !== undefined && magicDns !== "") {
    const hostPublicKeyHex =
      input.pairing?.hostPublicKeyHex ??
      input.announcements.find((entry) => entry.expiresAt > input.now)?.hostPublicKeyHex ??
      input.stored?.endpoint?.hostPublicKeyHex;
    if (hostPublicKeyHex !== undefined) {
      candidates.push({
        url: `wss://${magicDns}:${input.defaultPort}`,
        source: "manual",
        hostPublicKeyHex,
        generation: 0,
        expiresAt: null,
        pairingSecret: null,
      });
    }
  }
  const seen = new Set<string>();
  return candidates.filter((candidate) => {
    const key = `${candidate.hostPublicKeyHex}\u0000${candidate.url}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
};

const applyMirrorChange = (
  snapshot: OmegaMirrorSnapshot,
  change: OmegaMirrorChange,
  sequence: number,
): OmegaMirrorSnapshot => {
  switch (change.type) {
    case "thread_upsert":
      return {
        ...snapshot,
        sequence,
        threads: [
          change.thread,
          ...snapshot.threads.filter((thread) => thread.threadRef !== change.thread.threadRef),
        ],
        projectedAt: change.thread.updatedAt,
      };
    case "thread_remove":
      return {
        ...snapshot,
        sequence,
        threads: snapshot.threads.filter((thread) => thread.threadRef !== change.threadRef),
      };
    case "transcript_append":
      return {
        ...snapshot,
        sequence,
        threads: snapshot.threads.map((thread) =>
          thread.threadRef === change.threadRef
            ? {
                ...thread,
                transcript: [...thread.transcript, change.message],
                updatedAt: change.updatedAt,
              }
            : thread,
        ),
        projectedAt: change.updatedAt,
      };
    case "run_upsert":
      return {
        ...snapshot,
        sequence,
        runs: [change.run, ...snapshot.runs.filter((run) => run.runRef !== change.run.runRef)],
        projectedAt: change.run.updatedAt,
      };
    case "run_remove":
      return {
        ...snapshot,
        sequence,
        runs: snapshot.runs.filter((run) => run.runRef !== change.runRef),
      };
    case "health":
      return {
        ...snapshot,
        sequence,
        health: change.health,
        projectedAt: change.health.observedAt,
      };
  }
};

const initialState = (): OmegaDeviceBridgeState => ({
  paired: false,
  connection: {
    state: "offline",
    endpoint: null,
    heartbeatAt: null,
    relayObservedAt: null,
    staleSince: null,
  },
  mirror: null,
  recovery: "none",
  refusal: null,
});

/**
 * The host binds the proof to the pairing secret it issued, so the signed
 * content must carry that secret's digest. Without it the host reads
 * `pairingSecretDigest: null` against its own `Some(digest)` and refuses every
 * QR pairing with `invalid_proof`.
 */
const proofContent = (
  input: Readonly<{
    hostPublicKeyHex: string;
    grantRef: string | null;
    pairingSecret: string | null;
    resumeCursor: OmegaDeviceBridgeCursor | null;
    nonce: string;
  }>,
): string =>
  JSON.stringify({
    protocol: OMEGA_DEVICE_BRIDGE_PROTOCOL,
    hostPublicKeyHex: input.hostPublicKeyHex,
    grantRef: input.grantRef,
    pairingSecretDigest:
      input.pairingSecret === null
        ? null
        : bytesToHex(sha256(utf8ToBytes(input.pairingSecret))),
    resumeCursor: input.resumeCursor,
    nonce: input.nonce,
  });

export interface OmegaDeviceBridgeClient {
  readonly state: () => OmegaDeviceBridgeState;
  readonly subscribe: (listener: (state: OmegaDeviceBridgeState) => void) => () => void;
  readonly observeRelay: (observedAt: number) => Effect.Effect<void>;
  readonly connect: (
    input: Readonly<{
      announcements: ReadonlyArray<OmegaBridgeAnnouncement>;
      pairing: OmegaBridgePairingBootstrap | null;
      manualMagicDns: string | null;
    }>,
  ) => Effect.Effect<void, OmegaDeviceBridgeError>;
  readonly close: () => Effect.Effect<void>;
}

export const createOmegaDeviceBridgeClient = (
  input: Readonly<{
    identity: Issue31DeviceIdentity;
    store: OmegaDeviceBridgeStore;
    createSocket: (url: string) => OmegaDeviceBridgeWebSocket;
    now: () => number;
    randomNonce: () => string;
    defaultPort: number;
    admissionTimeoutMs?: number;
    scheduleAdmissionDeadline?: (onDeadline: () => void, delayMs: number) => () => void;
  }>,
): OmegaDeviceBridgeClient => {
  let state = initialState();
  let closed = false;
  let socket: OmegaDeviceBridgeWebSocket | null = null;
  const listeners = new Set<(state: OmegaDeviceBridgeState) => void>();
  const scheduleAdmissionDeadline =
    input.scheduleAdmissionDeadline ??
    ((onDeadline: () => void, delayMs: number): (() => void) => {
      const timer = setTimeout(onDeadline, delayMs);
      return () => clearTimeout(timer);
    });

  const publish = (next: OmegaDeviceBridgeState): void => {
    state = next;
    for (const listener of listeners) listener(state);
  };

  const persist = (effect: Effect.Effect<void, OmegaDeviceBridgeError>): void => {
    void Effect.runPromise(effect).catch((error: OmegaDeviceBridgeError) => {
      publish({ ...state, refusal: error.message });
    });
  };

  const connectionWithoutDirect = (): OmegaDeviceBridgeConnection => {
    const relayObservedAt = state.connection.relayObservedAt;
    return {
      state: relayObservedAt === null ? "offline" : "relay",
      endpoint: null,
      heartbeatAt: null,
      relayObservedAt,
      staleSince: state.mirror === null ? null : state.mirror.projectedAt,
    };
  };

  const connectEndpoint = (
    endpoint: OmegaBridgeEndpoint,
    stored: OmegaDeviceBridgeStoredState | null,
  ): Effect.Effect<void, OmegaDeviceBridgeError> =>
    Effect.tryPromise({
      try: () =>
        new Promise<void>((resolve, reject) => {
          if (closed) {
            reject(new OmegaDeviceBridgeError("closed", "The Omega bridge is closed."));
            return;
          }
          let settled = false;
          let abandoned = false;
          let currentStored = stored;
          let cancelAdmissionDeadline: (() => void) | null = null;
          let candidateSocket: OmegaDeviceBridgeWebSocket;
          try {
            candidateSocket = input.createSocket(endpoint.url);
          } catch (error) {
            reject(
              new OmegaDeviceBridgeError(
                "connection_failed",
                error instanceof Error ? error.message : "The Omega bridge connection failed.",
              ),
            );
            return;
          }
          socket = candidateSocket;

          const finishSuccess = (): void => {
            if (settled) return;
            cancelAdmissionDeadline?.();
            settled = true;
            resolve();
          };
          const finishFailure = (
            error: OmegaDeviceBridgeError,
            closeCode: number | null = 1000,
            closeReason = "admission failed",
          ): void => {
            if (settled) return;
            cancelAdmissionDeadline?.();
            settled = true;
            abandoned = true;
            if (closeCode !== null) candidateSocket.close(closeCode, closeReason);
            reject(error);
          };
          cancelAdmissionDeadline = scheduleAdmissionDeadline(() => {
            finishFailure(
              new OmegaDeviceBridgeError(
                "connection_failed",
                `The connection to ${endpoint.url} did not admit this device before the deadline.`,
              ),
              1008,
              "admission timeout",
            );
          }, input.admissionTimeoutMs ?? OMEGA_DEVICE_BRIDGE_ADMISSION_TIMEOUT_MS);

          const sendHello = (resumeCursor: OmegaDeviceBridgeCursor | null): void => {
            const grant =
              endpoint.pairingSecret === null &&
              currentStored?.grant?.hostPublicKeyHex === endpoint.hostPublicKeyHex &&
              currentStored.grant.devicePublicKeyHex === input.identity.publicKeyHex &&
              currentStored.grant.expiresAt > input.now()
                ? currentStored.grant
                : null;
            const content = proofContent({
              hostPublicKeyHex: endpoint.hostPublicKeyHex,
              grantRef: grant?.grantRef ?? null,
              pairingSecret: endpoint.pairingSecret,
              resumeCursor,
              nonce: input.randomNonce(),
            });
            void input.identity.signer
              .signEvent({
                kind: 27_272,
                created_at: Math.floor(input.now() / 1000),
                tags: [
                  ["p", endpoint.hostPublicKeyHex],
                  ["protocol", OMEGA_DEVICE_BRIDGE_PROTOCOL],
                ],
                content,
              })
              .then(
                (proof) => {
                  if (abandoned || closed || socket !== candidateSocket) return;
                  candidateSocket.send(
                    encodeFrame({
                      type: "hello",
                      protocol: OMEGA_DEVICE_BRIDGE_PROTOCOL,
                      devicePublicKeyHex: input.identity.publicKeyHex,
                      hostPublicKeyHex: endpoint.hostPublicKeyHex,
                      grantRef: grant?.grantRef ?? null,
                      pairingSecret: endpoint.pairingSecret,
                      resumeCursor,
                      proof,
                    } satisfies Schema.Schema.Type<typeof HelloFrameSchema>),
                  );
                },
                (error) =>
                  finishFailure(
                    new OmegaDeviceBridgeError(
                      "connection_failed",
                      error instanceof Error ? error.message : "The device proof failed.",
                    ),
                    1011,
                    "device proof failed",
                  ),
              );
          };

          candidateSocket.addEventListener("open", () => {
            if (abandoned || socket !== candidateSocket) return;
            sendHello(currentStored?.cursor ?? null);
          });
          candidateSocket.addEventListener("message", (event) => {
            if (abandoned || socket !== candidateSocket) return;
            if (typeof event.data !== "string") {
              finishFailure(
                new OmegaDeviceBridgeError("invalid_frame", "The Omega bridge frame is not text."),
                1002,
                "text frames required",
              );
              return;
            }
            if (
              new TextEncoder().encode(event.data).byteLength > OMEGA_DEVICE_BRIDGE_MAX_FRAME_BYTES
            ) {
              finishFailure(
                new OmegaDeviceBridgeError(
                  "frame_too_large",
                  "The Omega bridge frame is too large.",
                ),
                1009,
                "frame too large",
              );
              return;
            }
            let frame: OmegaDeviceBridgeServerFrame;
            try {
              frame = decodeServerFrame(JSON.parse(event.data), { onExcessProperty: "error" });
            } catch {
              finishFailure(
                new OmegaDeviceBridgeError("invalid_frame", "The Omega bridge frame is invalid."),
                1002,
                "invalid frame",
              );
              return;
            }
            if (frame.type === "grant") {
              if (!frame.admitted) {
                publish({
                  ...state,
                  paired:
                    frame.reason === "grant_revoked" || frame.reason === "grant_expired"
                      ? false
                      : state.paired,
                  refusal: frame.reason,
                });
                if (frame.reason === "grant_revoked" || frame.reason === "grant_expired") {
                  persist(input.store.clearGrant());
                }
                finishFailure(
                  new OmegaDeviceBridgeError(
                    "connection_failed",
                    `The Omega bridge refused this device: ${frame.reason}.`,
                  ),
                  1008,
                  frame.reason,
                );
                return;
              }
              currentStored = {
                schemaVersion: 1,
                endpoint: {
                  url: endpoint.url,
                  hostPublicKeyHex: endpoint.hostPublicKeyHex,
                },
                grant: {
                  grantRef: frame.grantRef,
                  hostPublicKeyHex: frame.hostPublicKeyHex,
                  devicePublicKeyHex: frame.devicePublicKeyHex,
                  expiresAt: frame.expiresAt,
                  generation: frame.generation,
                },
                cursor: currentStored?.cursor ?? null,
              };
              publish({ ...state, paired: true, refusal: null });
              persist(input.store.save(currentStored));
              finishSuccess();
              return;
            }
            if (frame.type === "snapshot") {
              currentStored = {
                schemaVersion: 1,
                endpoint: {
                  url: endpoint.url,
                  hostPublicKeyHex: endpoint.hostPublicKeyHex,
                },
                grant: currentStored?.grant ?? null,
                cursor: {
                  generation: frame.snapshot.generation,
                  sequence: frame.snapshot.sequence,
                },
              };
              persist(input.store.save(currentStored));
              publish({
                paired: currentStored.grant !== null,
                connection: {
                  state: "direct",
                  endpoint: endpoint.url,
                  heartbeatAt: input.now(),
                  relayObservedAt: state.connection.relayObservedAt,
                  staleSince: null,
                },
                mirror: frame.snapshot,
                recovery: "none",
                refusal: null,
              });
              finishSuccess();
              return;
            }
            if (frame.type === "delta") {
              const mirror = state.mirror;
              if (
                mirror === null ||
                frame.generation !== mirror.generation ||
                frame.sequence !== mirror.sequence + 1
              ) {
                publish({ ...state, recovery: "resnapshot_requested" });
                currentStored = currentStored === null ? null : { ...currentStored, cursor: null };
                if (currentStored !== null) persist(input.store.save(currentStored));
                sendHello(null);
                return;
              }
              const nextMirror = applyMirrorChange(mirror, frame.change, frame.sequence);
              currentStored =
                currentStored === null
                  ? null
                  : {
                      ...currentStored,
                      cursor: {
                        generation: nextMirror.generation,
                        sequence: nextMirror.sequence,
                      },
                    };
              if (currentStored !== null) persist(input.store.save(currentStored));
              publish({
                ...state,
                mirror: nextMirror,
                recovery: "none",
                connection: {
                  ...state.connection,
                  state: "direct",
                  endpoint: endpoint.url,
                  heartbeatAt: input.now(),
                  staleSince: null,
                },
              });
              return;
            }
            if (frame.type === "heartbeat") {
              candidateSocket.send(
                encodeFrame({
                  type: "heartbeat",
                  generation: frame.generation,
                  sequence: frame.sequence,
                  sentAt: input.now(),
                }),
              );
              publish({
                ...state,
                connection: {
                  ...state.connection,
                  state: "direct",
                  endpoint: endpoint.url,
                  heartbeatAt: input.now(),
                  staleSince: null,
                },
              });
              return;
            }
            if (frame.type === "bye") {
              if (frame.reason === "grant_revoked" || frame.reason === "grant_expired") {
                persist(input.store.clearGrant());
              }
              publish({
                ...state,
                paired:
                  frame.reason === "grant_revoked" || frame.reason === "grant_expired"
                    ? false
                    : state.paired,
                connection: connectionWithoutDirect(),
                refusal: frame.reason,
              });
              candidateSocket.close(1000, frame.reason);
            }
          });
          candidateSocket.addEventListener("error", () => {
            if (!settled) {
              finishFailure(
                new OmegaDeviceBridgeError(
                  "connection_failed",
                  `Could not connect to ${endpoint.url}.`,
                ),
                1001,
                "connection failed",
              );
            }
          });
          candidateSocket.addEventListener("close", () => {
            const wasCurrent = socket === candidateSocket;
            if (wasCurrent) socket = null;
            if (abandoned) return;
            if (wasCurrent) publish({ ...state, connection: connectionWithoutDirect() });
            if (!settled) {
              finishFailure(
                new OmegaDeviceBridgeError(
                  "connection_failed",
                  `The connection to ${endpoint.url} closed before admission.`,
                ),
                null,
              );
            }
          });
        }),
      catch: (error) =>
        error instanceof OmegaDeviceBridgeError
          ? error
          : new OmegaDeviceBridgeError(
              "connection_failed",
              error instanceof Error ? error.message : "The Omega bridge connection failed.",
            ),
    });

  return {
    state: () => state,
    subscribe: (listener) => {
      listeners.add(listener);
      listener(state);
      return () => listeners.delete(listener);
    },
    observeRelay: (observedAt) =>
      Effect.sync(() => {
        publish({
          ...state,
          connection: {
            ...state.connection,
            state: state.connection.state === "direct" ? "direct" : "relay",
            relayObservedAt: observedAt,
            staleSince:
              state.connection.state === "direct" || state.mirror === null
                ? null
                : state.mirror.projectedAt,
          },
        });
      }),
    connect: (request) =>
      Effect.gen(function* () {
        if (closed) {
          return yield* Effect.fail(
            new OmegaDeviceBridgeError("closed", "The Omega bridge is closed."),
          );
        }
        const stored = yield* input.store.load();
        publish({
          ...state,
          paired:
            stored?.grant !== null &&
            stored?.grant !== undefined &&
            stored.grant.expiresAt > input.now(),
        });
        const endpoints = omegaBridgeDialLadder({
          stored,
          announcements: request.announcements,
          pairing: request.pairing,
          manualMagicDns: request.manualMagicDns,
          now: input.now(),
          defaultPort: input.defaultPort,
        });
        let lastFailureMessage: string | null = null;
        for (const endpoint of endpoints) {
          const connected = yield* connectEndpoint(endpoint, stored).pipe(
            Effect.map(() => true),
            Effect.catch((error) =>
              Effect.sync(() => {
                lastFailureMessage = error.message;
                return false;
              }),
            ),
          );
          if (connected) return;
        }
        return yield* Effect.fail(
          new OmegaDeviceBridgeError(
            "all_endpoints_failed",
            lastFailureMessage ?? "No Omega bridge endpoint is available.",
          ),
        );
      }),
    close: () =>
      Effect.sync(() => {
        if (closed) return;
        closed = true;
        if (socket?.readyState === 1) {
          socket.send(encodeFrame({ type: "bye", reason: "client_closed" }));
        }
        socket?.close(1000, "client_closed");
        socket = null;
        publish({ ...state, connection: connectionWithoutDirect() });
        input.identity.close();
        listeners.clear();
      }),
  };
};

export const createMemoryOmegaDeviceBridgeStore = (
  initial: OmegaDeviceBridgeStoredState | null = null,
): OmegaDeviceBridgeStore & { readonly inspect: () => OmegaDeviceBridgeStoredState | null } => {
  let state = initial === null ? null : decodeStoredState(initial, { onExcessProperty: "error" });
  return {
    load: () => Effect.succeed(state),
    save: (next) =>
      Effect.sync(() => {
        state = decodeStoredState(next, { onExcessProperty: "error" });
      }),
    clearGrant: () =>
      Effect.sync(() => {
        if (state !== null) state = { ...state, grant: null, cursor: null };
      }),
    inspect: () => state,
  };
};

const secureBridgeStoreOptions = (store: Issue31SecureStore): Issue31SecureStoreOptions => ({
  keychainService: OMEGA_DEVICE_BRIDGE_KEYCHAIN_SERVICE,
  ...(store.AFTER_FIRST_UNLOCK_THIS_DEVICE_ONLY === undefined
    ? { requireAuthentication: false }
    : { keychainAccessible: store.AFTER_FIRST_UNLOCK_THIS_DEVICE_ONLY }),
});

export const createSecureOmegaDeviceBridgeStore = (
  store: Issue31SecureStore,
): OmegaDeviceBridgeStore => {
  const options = secureBridgeStoreOptions(store);
  const load = (): Effect.Effect<OmegaDeviceBridgeStoredState | null, OmegaDeviceBridgeError> =>
    Effect.tryPromise({
      try: async () => {
        const value = await store.getItemAsync(OMEGA_DEVICE_BRIDGE_STORE_KEY, options);
        if (value === null) return null;
        return decodeStoredState(JSON.parse(value), { onExcessProperty: "error" });
      },
      catch: () =>
        new OmegaDeviceBridgeError(
          "storage_failed",
          "The Omega bridge connection record is unavailable.",
        ),
    });
  const save = (state: OmegaDeviceBridgeStoredState): Effect.Effect<void, OmegaDeviceBridgeError> =>
    Effect.tryPromise({
      try: () =>
        store.setItemAsync(
          OMEGA_DEVICE_BRIDGE_STORE_KEY,
          JSON.stringify(decodeStoredState(state, { onExcessProperty: "error" })),
          options,
        ),
      catch: () =>
        new OmegaDeviceBridgeError(
          "storage_failed",
          "The Omega bridge connection record could not be saved.",
        ),
    });
  return {
    load,
    save,
    clearGrant: () =>
      Effect.gen(function* () {
        const state = yield* load();
        if (state === null) return;
        yield* save({ ...state, grant: null, cursor: null });
      }),
  };
};

declare const require: (id: string) => unknown;

export const openExpoOmegaDeviceBridgeStore = (): OmegaDeviceBridgeStore =>
  createSecureOmegaDeviceBridgeStore(require("expo-secure-store") as Issue31SecureStore);
