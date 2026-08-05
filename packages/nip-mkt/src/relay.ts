import { Effect, Schema } from "effect";

import { CONTRACT_IDENTITY, CONTRACT_SOURCE_COMMIT } from "./generated.js";

export const NIP_MKT_SDK_VERSION = "0.1.2" as const;
export const IMMORTAL_RELAY_SOFTWARE = "https://github.com/OpenAgentsInc/immortal" as const;
export const IMMORTAL_RELAY_EXTENSION = "nip-mkt" as const;

const RelayProbeCodeSchema = Schema.Literals([
  "nip11_unavailable",
  "invalid_nip11",
  "not_immortal",
  "contract_version_mismatch",
  "nip_mkt_unavailable",
  "websocket_failed",
]);
export type RelayProbeCode = typeof RelayProbeCodeSchema.Type;

export class RelayProbeError extends Schema.TaggedErrorClass<RelayProbeError>()("RelayProbeError", {
  code: RelayProbeCodeSchema,
  message: Schema.String,
}) {}

const RelayInformationSchema = Schema.Struct({
  name: Schema.optional(Schema.String),
  software: Schema.String,
  version: Schema.String,
  supported_nips: Schema.Array(Schema.Number),
  supported_extensions: Schema.Array(Schema.String),
});
const decodeRelayInformation = Schema.decodeUnknownSync(RelayInformationSchema);

export interface ImmortalRelayProbe {
  readonly relayUrl: string;
  readonly relayName?: string;
  readonly relaySoftware: string;
  readonly relayVersion: string;
  readonly supportedNips: readonly number[];
  readonly supportedExtensions: readonly string[];
  readonly sdkVersion: typeof NIP_MKT_SDK_VERSION;
  readonly contractSourceCommit: typeof CONTRACT_SOURCE_COMMIT;
  readonly contractVersion: string;
  readonly websocketConnected: true;
  readonly snapshotComplete: true;
  readonly nip11LatencyMs: number;
  readonly websocketLatencyMs: number;
  readonly totalLatencyMs: number;
  readonly checkedAt: string;
}

export interface ImmortalRelayProbeOptions {
  readonly timeoutMs?: number;
}

function relayProbeError(code: RelayProbeCode, cause: unknown): RelayProbeError {
  return new RelayProbeError({
    code,
    message: cause instanceof Error ? cause.message : String(cause),
  });
}

function relayInformationUrl(relayUrl: string): string {
  const url = new URL(relayUrl);
  if (url.protocol === "wss:") url.protocol = "https:";
  else if (url.protocol === "ws:") url.protocol = "http:";
  else throw new Error("Relay URL must use ws:// or wss://");
  return url.toString();
}

async function fetchRelayInformation(relayUrl: string, timeoutMs: number): Promise<unknown> {
  const response = await fetch(relayInformationUrl(relayUrl), {
    headers: { Accept: "application/nostr+json" },
    signal: AbortSignal.timeout(timeoutMs),
  });
  if (!response.ok) {
    throw new Error(`NIP-11 returned ${response.status} ${response.statusText}`.trim());
  }
  const contentType = response.headers.get("content-type") ?? "";
  if (!contentType.toLowerCase().includes("application/nostr+json")) {
    throw new Error(`NIP-11 returned unsupported content type ${contentType || "missing"}`);
  }
  return response.json();
}

function randomHex(bytes: number): string {
  return Array.from(globalThis.crypto.getRandomValues(new Uint8Array(bytes)), (byte) =>
    byte.toString(16).padStart(2, "0"),
  ).join("");
}

async function probeSnapshot(
  relayUrl: string,
  timeoutMs: number,
): Promise<{
  readonly websocketLatencyMs: number;
}> {
  return new Promise((resolve, reject) => {
    const startedAt = Date.now();
    const subscriptionId = `mkt-probe-${globalThis.crypto.randomUUID()}`;
    const impossibleEventId = randomHex(32);
    let socket: WebSocket | undefined;
    let connectedAt: number | undefined;
    let settled = false;

    const finish = (result: { readonly error?: Error }) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      try {
        socket?.send(JSON.stringify(["CLOSE", subscriptionId]));
      } catch {
        // The socket may already be closed.
      }
      try {
        socket?.close();
      } catch {
        // Cleanup must not replace the handshake result.
      }
      if (result.error !== undefined) reject(result.error);
      else resolve({ websocketLatencyMs: (connectedAt ?? Date.now()) - startedAt });
    };
    const timer = setTimeout(
      () => finish({ error: new Error(`WebSocket or EOSE timeout after ${timeoutMs}ms`) }),
      timeoutMs,
    );

    try {
      socket = new WebSocket(relayUrl);
    } catch (cause) {
      finish({ error: cause instanceof Error ? cause : new Error(String(cause)) });
      return;
    }

    socket.addEventListener("open", () => {
      connectedAt = Date.now();
      try {
        socket?.send(
          JSON.stringify(["REQ", subscriptionId, { ids: [impossibleEventId], limit: 1 }]),
        );
      } catch (cause) {
        finish({ error: cause instanceof Error ? cause : new Error(String(cause)) });
      }
    });
    socket.addEventListener("error", () =>
      finish({ error: new Error(`WebSocket error from ${relayUrl}`) }),
    );
    socket.addEventListener("close", () => {
      if (!settled) finish({ error: new Error("Relay closed before EOSE") });
    });
    socket.addEventListener("message", (event) => {
      if (typeof event.data !== "string") {
        finish({ error: new Error("Relay returned a non-text Nostr frame") });
        return;
      }
      let message: unknown;
      try {
        message = JSON.parse(event.data);
      } catch {
        finish({ error: new Error("Relay returned invalid JSON") });
        return;
      }
      if (!Array.isArray(message) || message[1] !== subscriptionId) return;
      if (message[0] === "EVENT") {
        finish({ error: new Error("Relay returned an event for an exact random event ID") });
      } else if (message[0] === "EOSE") {
        finish({});
      } else if (message[0] === "CLOSED") {
        finish({ error: new Error(String(message[2] ?? "Relay closed the subscription")) });
      }
    });
  });
}

export const probeImmortalRelay = Effect.fn("NipMkt.probeImmortalRelay")(function* (
  relayUrl: string,
  options: ImmortalRelayProbeOptions = {},
) {
  const timeoutMs = options.timeoutMs ?? 10_000;
  const startedAt = Date.now();
  const relayInformation = yield* Effect.tryPromise({
    try: () => fetchRelayInformation(relayUrl, timeoutMs),
    catch: (cause) => relayProbeError("nip11_unavailable", cause),
  });
  const information = yield* Effect.try({
    try: () => decodeRelayInformation(relayInformation),
    catch: (cause) => relayProbeError("invalid_nip11", cause),
  });
  const nip11CompletedAt = Date.now();

  if (information.software !== IMMORTAL_RELAY_SOFTWARE) {
    return yield* relayProbeError(
      "not_immortal",
      `Expected ${IMMORTAL_RELAY_SOFTWARE}, received ${information.software}`,
    );
  }
  if (information.version !== CONTRACT_IDENTITY.crate_version) {
    return yield* relayProbeError(
      "contract_version_mismatch",
      `SDK contract ${CONTRACT_IDENTITY.crate_version} does not match relay ${information.version}`,
    );
  }
  if (!information.supported_extensions.includes(IMMORTAL_RELAY_EXTENSION)) {
    return yield* relayProbeError(
      "nip_mkt_unavailable",
      `Relay does not advertise ${IMMORTAL_RELAY_EXTENSION}`,
    );
  }

  const snapshot = yield* Effect.tryPromise({
    try: () => probeSnapshot(relayUrl, timeoutMs),
    catch: (cause) => relayProbeError("websocket_failed", cause),
  });
  const completedAt = Date.now();

  return {
    relayUrl,
    ...(information.name === undefined ? {} : { relayName: information.name }),
    relaySoftware: information.software,
    relayVersion: information.version,
    supportedNips: information.supported_nips,
    supportedExtensions: information.supported_extensions,
    sdkVersion: NIP_MKT_SDK_VERSION,
    contractSourceCommit: CONTRACT_SOURCE_COMMIT,
    contractVersion: CONTRACT_IDENTITY.crate_version,
    websocketConnected: true,
    snapshotComplete: true,
    nip11LatencyMs: nip11CompletedAt - startedAt,
    websocketLatencyMs: snapshot.websocketLatencyMs,
    totalLatencyMs: completedAt - startedAt,
    checkedAt: new Date(completedAt).toISOString(),
  } satisfies ImmortalRelayProbe;
});
