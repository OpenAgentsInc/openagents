import { Effect, Schema } from "effect";
import type { Event } from "nostr-effect/pure";
import { fetchRelayInformation } from "nostr-effect/nip11";
import { Relay, type Subscription } from "nostr-effect/relay-client";

import { CONTRACT_IDENTITY, CONTRACT_SOURCE_COMMIT, PUBLIC_MKT_KINDS } from "./generated.js";
import { validatePublicHead } from "./validation.js";

export const NIP_MKT_SDK_VERSION = "0.1.1" as const;
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
  readonly validatedEvents: number;
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

async function probeSnapshot(
  relayUrl: string,
  timeoutMs: number,
): Promise<{
  readonly websocketLatencyMs: number;
  readonly validatedEvents: number;
}> {
  const relay = new Relay(relayUrl);
  const startedAt = Date.now();
  let subscription: Subscription | undefined;

  try {
    await relay.connect(timeoutMs);
    const connectedAt = Date.now();
    const validatedEvents = await new Promise<number>((resolve, reject) => {
      let count = 0;
      let settled = false;
      const finish = (result: { readonly count?: number; readonly error?: Error }) => {
        if (settled) return;
        settled = true;
        clearTimeout(timer);
        subscription?.close();
        if (result.error !== undefined) reject(result.error);
        else resolve(result.count ?? 0);
      };
      const timer = setTimeout(
        () => finish({ error: new Error(`EOSE timeout after ${timeoutMs}ms`) }),
        timeoutMs,
      );

      subscription = relay.subscribe([{ kinds: [...PUBLIC_MKT_KINDS], limit: 100 }], {
        onevent: (event) => {
          try {
            validatePublicHead(event as Event);
            count += 1;
          } catch (cause) {
            finish({
              error: new Error(
                `Relay returned an invalid NIP-MKT public event: ${cause instanceof Error ? cause.message : String(cause)}`,
              ),
            });
          }
        },
        oneose: () => finish({ count }),
        onclose: (reason) => finish({ error: new Error(reason) }),
      });
    });

    return { websocketLatencyMs: connectedAt - startedAt, validatedEvents };
  } finally {
    subscription?.close();
    relay.close();
  }
}

export const probeImmortalRelay = Effect.fn("NipMkt.probeImmortalRelay")(function* (
  relayUrl: string,
  options: ImmortalRelayProbeOptions = {},
) {
  const timeoutMs = options.timeoutMs ?? 10_000;
  const startedAt = Date.now();
  const relayInformation = yield* Effect.tryPromise({
    try: () => fetchRelayInformation(relayUrl),
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
    validatedEvents: snapshot.validatedEvents,
    nip11LatencyMs: nip11CompletedAt - startedAt,
    websocketLatencyMs: snapshot.websocketLatencyMs,
    totalLatencyMs: completedAt - startedAt,
    checkedAt: new Date(completedAt).toISOString(),
  } satisfies ImmortalRelayProbe;
});
