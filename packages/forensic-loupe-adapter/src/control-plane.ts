import { createHash } from "node:crypto";

import { forensicCanonicalJson, forensicSha256Digest } from "@openagentsinc/forensic-contract";

export const LOUPE_CONTROL_PLANE_TRANSCRIPT_VERSION =
  "openagents.loupe_control_plane_transcript.v1" as const;

/**
 * The two managed-sandbox control-plane routes the verifier is allowed to call.
 *
 * Naming them as an enum rather than accepting a URL is deliberate: a transport
 * cannot be pointed at some other service that would answer more agreeably.
 */
export type LoupeControlPlaneRoute = "runtime_operation" | "guest_io";

const ROUTE_PATHS: Record<LoupeControlPlaneRoute, string> = {
  runtime_operation: "/v1/managed-sandbox/runtime/operations",
  guest_io: "/v1/managed-sandbox/runtime/io",
};

/**
 * How much a result derived through this transport is allowed to claim.
 *
 * `live` is an authenticated managed-sandbox control plane over the network.
 * `recorded` replays the wire responses a `live` transport returned, matched
 * against the requests that produced them.
 * `conformance` is an in-memory simulation. It exercises the driver and the
 * evaluator and can never carry a confirmation out of this library, however
 * agreeable its answers are.
 */
export type LoupeControlPlaneKind = "live" | "recorded" | "conformance";

export interface LoupeControlPlaneResponse {
  readonly status: number;
  readonly body: unknown;
}

/**
 * The verifier's single injected boundary.
 *
 * Everything the verifier concludes is derived from what this returns: the
 * evidence receipts it writes, the admitted-worker receipts that authorize
 * them, and the provenance it is allowed to claim. A caller supplies a
 * transport; it does not supply evidence, and it does not supply the authority
 * that validates evidence, because both now come from the same origin and the
 * verifier does the deriving.
 */
export interface LoupeControlPlaneTransport {
  readonly kind: LoupeControlPlaneKind;
  readonly originRef: string;
  readonly post: (
    route: LoupeControlPlaneRoute,
    body: Record<string, unknown>,
  ) => Promise<LoupeControlPlaneResponse>;
  /**
   * The driver's only clock.
   *
   * Every timestamp the verifier writes — request fields, evidence
   * `observedAt`, the verdict lock, completion — is read from here, so a
   * recorded run replays with the times it actually observed rather than the
   * times of the replay. Without this, a replay would relock the durable
   * ledger with a different `lockedAt` and could not reproduce its own run.
   */
  readonly now: () => string;
}

/**
 * Request fields excluded from the digest a recorded transcript matches on.
 *
 * It is empty, and that is the point: because the driver reads every timestamp
 * from {@link LoupeControlPlaneTransport.now}, a replayed request is identical
 * to the recorded one down to the millisecond, so nothing needs excusing. The
 * field is retained in the transcript so that if an exclusion is ever needed it
 * is recorded rather than implicit.
 */
export const LOUPE_VOLATILE_REQUEST_FIELDS: ReadonlyArray<string> = [];

const stripVolatile = (value: unknown): unknown => {
  if (Array.isArray(value)) return value.map(stripVolatile);
  if (value !== null && typeof value === "object") {
    const entries = Object.entries(value as Record<string, unknown>)
      .filter(([key]) => !LOUPE_VOLATILE_REQUEST_FIELDS.includes(key))
      .map(([key, member]) => [key, stripVolatile(member)] as const);
    return Object.fromEntries(entries);
  }
  return value;
};

/**
 * Identifies a request by everything about it that is not a clock reading.
 * A replay whose driver would have asked a different question fails here
 * instead of silently answering the old one.
 */
export const loupeControlPlaneRequestDigest = (
  route: LoupeControlPlaneRoute,
  body: Record<string, unknown>,
): string => forensicSha256Digest({ route, request: stripVolatile(body) });

export interface LoupeControlPlaneExchange {
  readonly index: number;
  readonly route: LoupeControlPlaneRoute;
  readonly action: string;
  readonly stableRequestDigest: string;
  readonly status: number;
  readonly body: unknown;
}

export interface LoupeControlPlaneTranscript {
  readonly schema: typeof LOUPE_CONTROL_PLANE_TRANSCRIPT_VERSION;
  readonly recordedOriginRef: string;
  readonly recordedOriginKind: LoupeControlPlaneKind;
  readonly volatileRequestFields: ReadonlyArray<string>;
  readonly clockReadings: ReadonlyArray<string>;
  readonly exchanges: ReadonlyArray<LoupeControlPlaneExchange>;
}

const isoMillis = (millis: number): string =>
  new Date(millis).toISOString().replace(/\.(\d{3})\d*Z$/, ".$1Z");

const actionOf = (body: Record<string, unknown>): string =>
  typeof body.action === "string" ? body.action : "unknown";

/**
 * An authenticated managed-sandbox control plane.
 *
 * `originRef` binds the host the verifier talked to and a fingerprint of the
 * credential it presented, so a result records which authority it trusted.
 * The credential itself never appears in the ref, the transcript, or a result.
 */
export const httpLoupeControlPlane = (config: {
  readonly baseUrl: string;
  readonly token: string;
  readonly fetchImpl?: typeof fetch;
  readonly timeoutMillis?: number;
}): LoupeControlPlaneTransport => {
  const baseUrl = config.baseUrl.replace(/\/$/, "");
  const host = new URL(baseUrl).host.replace(/[^A-Za-z0-9._:-]/g, "-");
  const credentialFingerprint = createHash("sha256")
    .update(config.token)
    .digest("hex")
    .slice(0, 16);
  const call = config.fetchImpl ?? fetch;
  return {
    kind: "live",
    originRef: `control-plane.live.${host}.${credentialFingerprint}`,
    now: () => isoMillis(Date.now()),
    post: async (route, body) => {
      const response = await call(`${baseUrl}${ROUTE_PATHS[route]}`, {
        method: "POST",
        headers: {
          authorization: `Bearer ${config.token}`,
          "content-type": "application/json",
        },
        ...(config.timeoutMillis === undefined
          ? {}
          : { signal: AbortSignal.timeout(config.timeoutMillis) }),
        body: JSON.stringify(body),
      });
      const text = await response.text();
      let parsed: unknown;
      try {
        parsed = text.length === 0 ? null : (JSON.parse(text) as unknown);
      } catch {
        parsed = { error: text.slice(0, 600) };
      }
      return { status: response.status, body: parsed };
    },
  };
};

export interface RecordingLoupeControlPlane {
  readonly transport: LoupeControlPlaneTransport;
  readonly transcript: () => LoupeControlPlaneTranscript;
}

/** Wraps a transport and records every exchange in order. */
export const recordingLoupeControlPlane = (
  inner: LoupeControlPlaneTransport,
): RecordingLoupeControlPlane => {
  const exchanges: Array<LoupeControlPlaneExchange> = [];
  const clockReadings: Array<string> = [];
  return {
    transport: {
      kind: inner.kind,
      originRef: inner.originRef,
      now: () => {
        const reading = inner.now();
        clockReadings.push(reading);
        return reading;
      },
      post: async (route, body) => {
        const response = await inner.post(route, body);
        exchanges.push({
          index: exchanges.length + 1,
          route,
          action: actionOf(body),
          stableRequestDigest: loupeControlPlaneRequestDigest(route, body),
          status: response.status,
          body: response.body,
        });
        return response;
      },
    },
    transcript: () => ({
      schema: LOUPE_CONTROL_PLANE_TRANSCRIPT_VERSION,
      recordedOriginRef: inner.originRef,
      recordedOriginKind: inner.kind,
      volatileRequestFields: LOUPE_VOLATILE_REQUEST_FIELDS,
      clockReadings: [...clockReadings],
      exchanges: [...exchanges],
    }),
  };
};

/**
 * Replays a recorded transcript.
 *
 * Order and request identity are both enforced. A driver that would have issued
 * a different request, or the same requests in a different order, is refused
 * rather than handed the old answer. That is what keeps a transcript a
 * measurement of a specific run instead of a bag of agreeable responses.
 */
export const recordedLoupeControlPlane = (
  transcript: LoupeControlPlaneTranscript,
): LoupeControlPlaneTransport => {
  if (transcript.schema !== LOUPE_CONTROL_PLANE_TRANSCRIPT_VERSION) {
    throw new Error("unknown Loupe control-plane transcript schema");
  }
  if (transcript.recordedOriginKind !== "live") {
    throw new Error("a replayable control-plane transcript must have been recorded from a live origin");
  }
  let cursor = 0;
  let clockCursor = 0;
  return {
    kind: "recorded",
    originRef: transcript.recordedOriginRef,
    now: () => {
      const reading = transcript.clockReadings[clockCursor];
      clockCursor += 1;
      if (reading === undefined) {
        throw new Error("the recorded control plane has no clock reading left for this run");
      }
      return reading;
    },
    post: async (route, body) => {
      const exchange = transcript.exchanges[cursor];
      cursor += 1;
      if (exchange === undefined) {
        throw new Error("the recorded control plane has no response left for this request");
      }
      if (exchange.route !== route) {
        throw new Error(
          `the recorded control plane expected a ${exchange.route} request and was asked for ${route}`,
        );
      }
      if (exchange.stableRequestDigest !== loupeControlPlaneRequestDigest(route, body)) {
        throw new Error(
          `the recorded control plane holds no response for this ${route} ${actionOf(body)} request`,
        );
      }
      return { status: exchange.status, body: exchange.body };
    },
  };
};

/**
 * An in-memory control plane for conformance runs.
 *
 * It is a simulation, so it is capped by `kind: "conformance"` and no result
 * derived through it can reach the independently verified tier. It exists so
 * the driver's ordering, derivation and refusal behaviour can be falsified
 * without spending money.
 */
export const conformanceLoupeControlPlane = (options: {
  readonly handle: (
    route: LoupeControlPlaneRoute,
    body: Record<string, unknown>,
  ) => Promise<LoupeControlPlaneResponse> | LoupeControlPlaneResponse;
  readonly startedAt: string;
  readonly stepMillis?: number;
}): LoupeControlPlaneTransport => {
  let millis = Date.parse(options.startedAt);
  if (Number.isNaN(millis)) throw new Error("a conformance control plane requires a start time");
  const step = options.stepMillis ?? 1_000;
  return {
    kind: "conformance",
    originRef: `control-plane.conformance.${createHash("sha256")
      .update(forensicCanonicalJson({ startedAt: options.startedAt, step }))
      .digest("hex")
      .slice(0, 16)}`,
    now: () => {
      const reading = isoMillis(millis);
      millis += step;
      return reading;
    },
    post: async (route, body) => await options.handle(route, body),
  };
};
