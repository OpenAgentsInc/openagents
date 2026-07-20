import {
  PortableCheckpointCustodyObjectManifestSchema,
  type PortableCheckpointCustodyObjectManifest,
} from "@openagentsinc/portable-session-contract";
import { createHash } from "node:crypto";
import { Effect, Schema } from "effect";

const RESPONSE_SCHEMA = "openagents.portable_checkpoint_artifact_transport.v1" as const;
const SAFE_REF = /^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$/u;
const SHA256 = /^sha256:[a-f0-9]{64}$/u;
const MAX_JSON_BYTES = 512 * 1024;

const canonical = (value: unknown): string => {
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`;
  if (value !== null && typeof value === "object") {
    return `{${Object.entries(value as Readonly<Record<string, unknown>>)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, item]) => `${JSON.stringify(key)}:${canonical(item)}`)
      .join(",")}}`;
  }
  return JSON.stringify(value);
};

const valueDigest = (value: unknown): `sha256:${string}` =>
  `sha256:${createHash("sha256").update(canonical(value)).digest("hex")}`;

const Ref = Schema.String.check(
  Schema.isMinLength(3),
  Schema.isMaxLength(256),
  Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
);
const Digest = Schema.String.check(Schema.isPattern(/^sha256:[a-f0-9]{64}$/));
const Timestamp = Schema.String.check(
  Schema.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$/),
);
const PrepareResponse = Schema.Struct({
  schema: Schema.Literal(RESPONSE_SCHEMA),
  status: Schema.Literal("prepared"),
  manifestDigest: Digest,
  objectDigest: Digest,
  byteLimit: Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0)),
  expiresAt: Timestamp,
  upload: Schema.Struct({
    transport: Schema.Literal("server_mediated"),
    method: Schema.Literal("PUT"),
    path: Schema.String,
    contentType: Schema.Literal("application/octet-stream"),
    operationRefHeader: Schema.Literal("x-openagents-operation-ref"),
    expiresAt: Timestamp,
  }),
});
const CommitResponse = Schema.Struct({
  schema: Schema.Literal(RESPONSE_SCHEMA),
  status: Schema.Literal("committed"),
  manifestDigest: Digest,
  objectDigest: Digest,
});
const RedeemResponse = Schema.Struct({
  schema: Schema.Literal(RESPONSE_SCHEMA),
  status: Schema.Literal("redeemed"),
  redemptionRef: Ref,
  expiresAt: Timestamp,
  manifest: PortableCheckpointCustodyObjectManifestSchema,
  download: Schema.Struct({
    transport: Schema.Literal("server_mediated"),
    method: Schema.Literal("POST"),
    path: Schema.String,
  }),
});

export class PylonPortableCheckpointArtifactTransportError extends Schema.TaggedErrorClass<PylonPortableCheckpointArtifactTransportError>()(
  "PylonPortableCheckpointArtifactTransportError",
  {
    reason: Schema.Literals([
      "bad_response",
      "cancelled",
      "conflict",
      "expired",
      "invalid_config",
      "network_failed",
      "not_authorized",
      "too_large",
      "unavailable",
    ]),
  },
) {}

export type PylonPortableCheckpointArtifactClient = Readonly<{
  publish: (input: Readonly<{
    operationRef: string
    manifest: PortableCheckpointCustodyObjectManifest
    bytes: Uint8Array
    signal?: AbortSignal
  }>) => Effect.Effect<Readonly<{ manifestDigest: string }>, PylonPortableCheckpointArtifactTransportError>
  redeem: (input: Readonly<{
    operationRef: string
    manifestDigest: string
    checkpointObjectRef: string
    checkpointDigest: string
    commandClaimRef: string
    signal?: AbortSignal
  }>) => Effect.Effect<
    Readonly<{ manifest: PortableCheckpointCustodyObjectManifest; bytes: Uint8Array }>,
    PylonPortableCheckpointArtifactTransportError
  >
}>;

export type MakePylonPortableCheckpointArtifactClientOptions = Readonly<{
  agentToken: string
  baseUrl: string
  pylonRef: string
  targetRef: string
  fetchImpl?: typeof globalThis.fetch
  requestTimeoutMs?: number
  now?: () => Date
}>;

const failure = (reason: PylonPortableCheckpointArtifactTransportError["reason"]) =>
  new PylonPortableCheckpointArtifactTransportError({ reason });

const baseUrl = (value: string): URL => {
  try {
    const parsed = new URL(value);
    const loopbackHttp =
      parsed.protocol === "http:" && ["localhost", "127.0.0.1", "[::1]"].includes(parsed.hostname);
    if (
      (parsed.protocol !== "https:" && !loopbackHttp) ||
      parsed.username !== "" ||
      parsed.password !== ""
    ) {
      throw failure("invalid_config");
    }
    return parsed;
  } catch (error) {
    if (error instanceof PylonPortableCheckpointArtifactTransportError) throw error;
    throw failure("invalid_config");
  }
};

const statusFailure = (status: number): PylonPortableCheckpointArtifactTransportError => {
  if (status === 400 || status === 413) return failure("bad_response");
  if (status === 401 || status === 403 || status === 404) return failure("not_authorized");
  if (status === 409 || status === 410) return failure("conflict");
  return failure("unavailable");
};

const decodeJson = <A>(schema: Schema.Decoder<A>, bytes: Uint8Array): A => {
  try {
    return Schema.decodeUnknownSync(Schema.fromJsonString(schema))(
      new TextDecoder().decode(bytes),
      { onExcessProperty: "error" },
    );
  } catch {
    throw failure("bad_response");
  }
};

const readBounded = async (response: Response, maximumBytes: number): Promise<Uint8Array> => {
  const declared = response.headers.get("content-length");
  if (declared !== null && (!/^\d+$/.test(declared) || Number(declared) > maximumBytes)) {
    throw failure("too_large");
  }
  if (response.body === null) throw failure("bad_response");
  const reader = response.body.getReader();
  const chunks: Array<Uint8Array> = [];
  let total = 0;
  try {
    for (;;) {
      const next = await reader.read();
      if (next.done) break;
      total += next.value.byteLength;
      if (total > maximumBytes) {
        await reader.cancel();
        throw failure("too_large");
      }
      chunks.push(next.value);
    }
  } finally {
    reader.releaseLock();
  }
  const bytes = new Uint8Array(total);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return bytes;
};

export const makePylonPortableCheckpointArtifactClient = (
  options: MakePylonPortableCheckpointArtifactClientOptions,
): PylonPortableCheckpointArtifactClient => {
  const origin = baseUrl(options.baseUrl);
  if (
    options.agentToken.trim() === "" ||
    !SAFE_REF.test(options.pylonRef) ||
    !SAFE_REF.test(options.targetRef)
  ) {
    throw failure("invalid_config");
  }
  const timeoutMs = options.requestTimeoutMs ?? 30_000;
  if (!Number.isSafeInteger(timeoutMs) || timeoutMs < 1_000 || timeoutMs > 120_000) {
    throw failure("invalid_config");
  }
  const fetchImpl = options.fetchImpl ?? globalThis.fetch;
  const now = options.now ?? (() => new Date());
  const route = `/api/pylons/${encodeURIComponent(options.pylonRef)}/portable-targets/${encodeURIComponent(options.targetRef)}/checkpoint-artifacts`;

  const send = Effect.fn("PortableCheckpointArtifactClient.send")(function* (
    path: string,
    init: RequestInit,
    maximumBytes: number,
    signal?: AbortSignal,
  ) {
    const timeout = AbortSignal.timeout(timeoutMs);
    const combined = signal === undefined ? timeout : AbortSignal.any([signal, timeout]);
    const response = yield* Effect.tryPromise({
      try: () =>
        fetchImpl(new URL(path, origin), {
          ...init,
          headers: {
            Authorization: `Bearer ${options.agentToken}`,
            ...init.headers,
          },
          signal: combined,
        }),
      catch: () => (signal?.aborted === true ? failure("cancelled") : failure("network_failed")),
    });
    if (!response.ok) return yield* statusFailure(response.status);
    return yield* Effect.tryPromise({
      try: () => readBounded(response, maximumBytes),
      catch: error =>
        error instanceof PylonPortableCheckpointArtifactTransportError
          ? error
          : failure("bad_response"),
    });
  });

  const publish = Effect.fn("PortableCheckpointArtifactClient.publish")(function* (input: {
    readonly operationRef: string
    readonly manifest: PortableCheckpointCustodyObjectManifest
    readonly bytes: Uint8Array
    readonly signal?: AbortSignal
  }) {
    if (
      !SAFE_REF.test(input.operationRef) ||
      input.bytes.byteLength <= 0 ||
      input.bytes.byteLength > input.manifest.byteLimit ||
      Date.parse(input.manifest.expiresAt) <= now().getTime()
    ) {
      return yield* failure("expired");
    }
    const prepared = decodeJson(
      PrepareResponse,
      yield* send(
        `${route}/prepare`,
        {
          method: "POST",
          headers: { Accept: "application/json", "Content-Type": "application/json" },
          body: JSON.stringify({ operationRef: input.operationRef, manifest: input.manifest }),
        },
        MAX_JSON_BYTES,
        input.signal,
      ),
    );
    const expectedPath = `${route}/${prepared.manifestDigest.slice(7)}/upload`;
    if (
      prepared.manifestDigest !== valueDigest(input.manifest) ||
      prepared.objectDigest !== input.manifest.objectDigest ||
      prepared.byteLimit !== input.manifest.byteLimit ||
      prepared.expiresAt !== input.manifest.expiresAt ||
      prepared.upload.path !== expectedPath ||
      prepared.upload.expiresAt !== input.manifest.expiresAt
    ) {
      return yield* failure("bad_response");
    }
    const uploadBytes = Uint8Array.from(input.bytes);
    const uploadResponse = yield* send(
      expectedPath,
      {
        method: "PUT",
        headers: {
          Accept: "application/json",
          "Content-Type": "application/octet-stream",
          "x-openagents-operation-ref": input.operationRef,
        },
        body: uploadBytes.buffer,
      },
      MAX_JSON_BYTES,
      input.signal,
    ).pipe(Effect.ensuring(Effect.sync(() => uploadBytes.fill(0))));
    decodeJson(
      Schema.Struct({
        schema: Schema.Literal(RESPONSE_SCHEMA),
        status: Schema.Literal("uploaded"),
        manifestDigest: Schema.Literal(prepared.manifestDigest),
        objectDigest: Schema.Literal(input.manifest.objectDigest),
      }),
      uploadResponse,
    );
    const commit = decodeJson(
      CommitResponse,
      yield* send(
        `${route}/${prepared.manifestDigest.slice(7)}/commit`,
        {
          method: "POST",
          headers: { Accept: "application/json", "Content-Type": "application/json" },
          body: JSON.stringify({ operationRef: input.operationRef }),
        },
        MAX_JSON_BYTES,
        input.signal,
      ),
    );
    if (
      commit.manifestDigest !== prepared.manifestDigest ||
      commit.objectDigest !== input.manifest.objectDigest
    ) {
      return yield* failure("bad_response");
    }
    return { manifestDigest: commit.manifestDigest };
  });

  const redeem = Effect.fn("PortableCheckpointArtifactClient.redeem")(function* (input: {
    readonly operationRef: string
    readonly manifestDigest: string
    readonly checkpointObjectRef: string
    readonly checkpointDigest: string
    readonly commandClaimRef: string
    readonly signal?: AbortSignal
  }) {
    if (
      ![input.operationRef, input.checkpointObjectRef, input.commandClaimRef].every(ref =>
        SAFE_REF.test(ref),
      ) ||
      !SHA256.test(input.manifestDigest) ||
      !SHA256.test(input.checkpointDigest)
    ) {
      return yield* failure("invalid_config");
    }
    const manifestHex = input.manifestDigest.slice("sha256:".length);
    const redeemPath = `${route}/${manifestHex}/redeem`;
    const redeemed = decodeJson(
      RedeemResponse,
      yield* send(
        redeemPath,
        {
          method: "POST",
          headers: { Accept: "application/json", "Content-Type": "application/json" },
          body: JSON.stringify({ operationRef: input.operationRef }),
        },
        MAX_JSON_BYTES,
        input.signal,
      ),
    );
    const expectedDownload = `${route}/${manifestHex}/download`;
    if (
      redeemed.download.path !== expectedDownload ||
      valueDigest(redeemed.manifest) !== input.manifestDigest ||
      redeemed.manifest.objectRef !== input.checkpointObjectRef ||
      redeemed.manifest.checkpointDigest !== input.checkpointDigest ||
      redeemed.manifest.targetRef !== options.targetRef ||
      redeemed.manifest.commandClaim.claimRef !== input.commandClaimRef ||
      Date.parse(redeemed.expiresAt) <= now().getTime() ||
      Date.parse(redeemed.manifest.expiresAt) <= now().getTime()
    ) {
      return yield* failure("bad_response");
    }
    const bytes = yield* send(
      expectedDownload,
      {
        method: "POST",
        headers: { Accept: "application/octet-stream", "Content-Type": "application/json" },
        body: JSON.stringify({
          operationRef: input.operationRef,
          redemptionRef: redeemed.redemptionRef,
        }),
      },
      redeemed.manifest.byteLimit,
      input.signal,
    );
    return { manifest: redeemed.manifest, bytes };
  });

  return { publish, redeem };
};
