import { createHash, randomUUID } from "node:crypto";
import { chmodSync, lstatSync, mkdirSync, realpathSync } from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";

import { Exit, Schema } from "@effect-native/core/effect";
import { CodingRef } from "@openagentsinc/khala-sync";

import {
  DesktopCodingCatalogProjectionSchema,
  projectDesktopCodingCatalog,
  type DesktopCodingCatalogProjection,
} from "./coding-catalog-contract.ts";
import {
  openAdmittedDesktopWorkspace,
  type AdmittedDesktopWorkspace,
} from "./desktop-workspace-admission.ts";
import { openDesktopSyncHost } from "./desktop-sync-host.ts";
import {
  DesktopRuntimeGatewayProtocolVersion,
  DesktopRuntimeGatewayResponseSchema,
  decodeDesktopRuntimeGatewayResponse,
  type DesktopRuntimeGatewayResponse,
} from "./runtime-gateway-contract.ts";
import { createDesktopRuntimeGateway, type DesktopRuntimeGateway } from "./runtime-gateway.ts";

export const DesktopNativeSidecarProtocol = "openagents.desktop.native-sidecar.v2" as const;
export const DesktopNativeSidecarRpcProtocol = "openagents.desktop.native-sidecar-rpc.v1" as const;
export const DesktopNativeSidecarNodeVersion = "24.13.1" as const;
export const DesktopNativeSidecarFrameLimit = 64 * 1024;
export const DesktopNativeSidecarProjectionLimit = 48 * 1024;

const GenerationSchema = Schema.Number.check(
  Schema.isInt(),
  Schema.isGreaterThan(0),
  Schema.isLessThanOrEqualTo(Number.MAX_SAFE_INTEGER),
);
const NonceSchema = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(80),
  Schema.isPattern(/^[A-Za-z0-9._-]+$/),
);
const RequestIdSchema = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(160),
  Schema.isPattern(/^[A-Za-z0-9._:-]+$/),
);
const Base64PathSchema = Schema.String.check(
  Schema.isMinLength(4),
  Schema.isMaxLength(8 * 1024),
  Schema.isPattern(/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/),
);
const DigestSchema = Schema.String.check(Schema.isPattern(/^sha256:[a-f0-9]{64}$/));
const PortSchema = Schema.Number.check(
  Schema.isInt(),
  Schema.isGreaterThanOrEqualTo(1024),
  Schema.isLessThanOrEqualTo(65_535),
);
const TokenSchema = Schema.String.check(Schema.isPattern(/^[a-f0-9]{64}$/));

export const DesktopNativeSidecarBootstrapRequestSchema = Schema.Struct({
  protocol: Schema.Literal(DesktopNativeSidecarProtocol),
  generation: GenerationSchema,
  nonce: NonceSchema,
  stateRootBase64: Base64PathSchema,
  transportToken: TokenSchema,
});
export type DesktopNativeSidecarBootstrapRequest =
  typeof DesktopNativeSidecarBootstrapRequestSchema.Type;

export const DesktopNativeSidecarBootstrapReceiptSchema = Schema.Struct({
  protocol: Schema.Literal(DesktopNativeSidecarProtocol),
  generation: GenerationSchema,
  nonce: NonceSchema,
  pid: Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0)),
  nodeVersion: Schema.Literal(DesktopNativeSidecarNodeVersion),
  gatewayProtocolVersion: Schema.Literal(DesktopRuntimeGatewayProtocolVersion),
  requestId: Schema.Literal("native-sidecar.bootstrap"),
  response: DesktopRuntimeGatewayResponseSchema,
});
export type DesktopNativeSidecarBootstrapReceipt =
  typeof DesktopNativeSidecarBootstrapReceiptSchema.Type;

export const DesktopNativeSidecarReadyReceiptSchema = Schema.Struct({
  protocol: Schema.Literal(DesktopNativeSidecarProtocol),
  generation: GenerationSchema,
  nonce: NonceSchema,
  pid: Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0)),
  nodeVersion: Schema.Literal(DesktopNativeSidecarNodeVersion),
  gatewayProtocolVersion: Schema.Literal(DesktopRuntimeGatewayProtocolVersion),
  requestId: Schema.Literal("native-sidecar.bootstrap"),
  response: DesktopRuntimeGatewayResponseSchema,
  transport: Schema.Struct({
    kind: Schema.Literal("loopback_http"),
    host: Schema.Literal("127.0.0.1"),
    port: PortSchema,
  }),
});
export type DesktopNativeSidecarReadyReceipt = typeof DesktopNativeSidecarReadyReceiptSchema.Type;

const RpcBase = {
  protocol: Schema.Literal(DesktopNativeSidecarRpcProtocol),
  generation: GenerationSchema,
  nonce: NonceSchema,
  requestId: RequestIdSchema,
} as const;

export const DesktopNativeSidecarRpcRequestSchema = Schema.Union([
  Schema.Struct({ ...RpcBase, operation: Schema.Literal("coding.snapshot") }),
  Schema.Struct({
    ...RpcBase,
    operation: Schema.Literal("coding.admit"),
    rootBase64: Base64PathSchema,
  }),
]);
export type DesktopNativeSidecarRpcRequest = typeof DesktopNativeSidecarRpcRequestSchema.Type;

const DesktopWorkspaceAdmissionSchema = Schema.Struct({
  grantRef: CodingRef,
  projectRef: CodingRef,
  repositoryRef: CodingRef,
  worktreeRef: CodingRef,
  workContextRef: CodingRef,
  sessionRef: CodingRef,
});

const RpcResponseBase = {
  protocol: Schema.Literal(DesktopNativeSidecarRpcProtocol),
  generation: GenerationSchema,
  nonce: NonceSchema,
  requestId: RequestIdSchema,
} as const;
const ProjectionFields = {
  projection: DesktopCodingCatalogProjectionSchema,
  projectionDigest: DigestSchema,
} as const;

export const DesktopNativeSidecarRpcResponseSchema = Schema.Union([
  Schema.Struct({
    ...RpcResponseBase,
    result: Schema.Struct({
      kind: Schema.Literal("coding.snapshot"),
      ...ProjectionFields,
    }),
  }),
  Schema.Struct({
    ...RpcResponseBase,
    result: Schema.Struct({
      kind: Schema.Literal("coding.admitted"),
      ...ProjectionFields,
      admission: DesktopWorkspaceAdmissionSchema,
    }),
  }),
  Schema.Struct({
    ...RpcResponseBase,
    result: Schema.Struct({
      kind: Schema.Literal("coding.refused"),
      reason: Schema.Literals(["not_repository", "request_conflict", "request_limit"]),
      ...ProjectionFields,
    }),
  }),
]);
export type DesktopNativeSidecarRpcResponse = typeof DesktopNativeSidecarRpcResponseSchema.Type;

const decode = <A>(schema: any, value: unknown): A | null => {
  const result = Schema.decodeUnknownExit(schema)(value, { onExcessProperty: "error" });
  return Exit.isSuccess(result) ? (result.value as A) : null;
};

export const decodeDesktopNativeSidecarBootstrapRequest = (
  value: unknown,
): DesktopNativeSidecarBootstrapRequest | null =>
  decode(DesktopNativeSidecarBootstrapRequestSchema, value);

export const decodeDesktopNativeSidecarBootstrapReceipt = (
  value: unknown,
): DesktopNativeSidecarBootstrapReceipt | null =>
  decode(DesktopNativeSidecarBootstrapReceiptSchema, value);

export const decodeDesktopNativeSidecarReadyReceipt = (
  value: unknown,
): DesktopNativeSidecarReadyReceipt | null => decode(DesktopNativeSidecarReadyReceiptSchema, value);

export const decodeDesktopNativeSidecarRpcRequest = (
  value: unknown,
): DesktopNativeSidecarRpcRequest | null => decode(DesktopNativeSidecarRpcRequestSchema, value);

export const decodeDesktopNativeSidecarRpcResponse = (
  value: unknown,
): DesktopNativeSidecarRpcResponse | null => decode(DesktopNativeSidecarRpcResponseSchema, value);

export const encodeDesktopNativeSidecarPath = (value: string): string =>
  Buffer.from(value, "utf8").toString("base64");

const decodePrivatePath = (encoded: string): string => {
  const bytes = Buffer.from(encoded, "base64");
  if (bytes.length === 0 || bytes.length > 4_096 || bytes.toString("base64") !== encoded) {
    throw new Error("Native sidecar path encoding is invalid.");
  }
  const value = bytes.toString("utf8");
  if (
    !Buffer.from(value, "utf8").equals(bytes) ||
    value.includes("\0") ||
    !path.isAbsolute(value)
  ) {
    throw new Error("Native sidecar path must be an absolute UTF-8 host path.");
  }
  return value;
};

const sha256 = (value: string): string =>
  `sha256:${createHash("sha256").update(value).digest("hex")}`;

const strictProjection = (value: unknown): DesktopCodingCatalogProjection => {
  const decoded = decode<DesktopCodingCatalogProjection>(
    DesktopCodingCatalogProjectionSchema,
    value,
  );
  if (decoded === null) throw new Error("Native coding projection failed its closed schema.");
  return decoded;
};

const isRepositoryRoot = (candidate: string): string | null => {
  try {
    const root = realpathSync(candidate);
    if (!lstatSync(root).isDirectory()) return null;
    const result = spawnSync(
      "git",
      ["-C", root, "rev-parse", "--is-inside-work-tree", "--show-toplevel"],
      {
        encoding: "utf8",
        timeout: 5_000,
        maxBuffer: 8 * 1024,
        stdio: ["ignore", "pipe", "ignore"],
        env: {
          PATH: process.env.PATH,
          HOME: process.env.HOME,
          SystemRoot: process.env.SystemRoot,
          TMPDIR: process.env.TMPDIR,
          TEMP: process.env.TEMP,
          TMP: process.env.TMP,
          GIT_CONFIG_NOSYSTEM: "1",
          GIT_CONFIG_GLOBAL: process.platform === "win32" ? "NUL" : "/dev/null",
          GIT_OPTIONAL_LOCKS: "0",
        },
      },
    );
    if (result.status !== 0 || result.error !== undefined) return null;
    const [insideWorkTree, rawTopLevel, ...excess] = result.stdout.trim().split(/\r?\n/u);
    if (insideWorkTree !== "true" || rawTopLevel === undefined || excess.length > 0) return null;
    const topLevel = realpathSync(rawTopLevel);
    return topLevel === root ? root : null;
  } catch {
    return null;
  }
};

const openRuntimeGateway = async (): Promise<
  Readonly<{
    gateway: DesktopRuntimeGateway;
    response: DesktopRuntimeGatewayResponse;
  }>
> => {
  const gateway = createDesktopRuntimeGateway();
  gateway.start();
  try {
    const response = await gateway.request({
      kind: "query",
      requestId: "native-sidecar.bootstrap",
      query: { id: "runtime.bootstrap" },
    });
    const decodedResponse = decodeDesktopRuntimeGatewayResponse(response);
    if (
      decodedResponse === null ||
      decodedResponse.kind !== "query_result" ||
      decodedResponse.requestId !== "native-sidecar.bootstrap" ||
      decodedResponse.result.kind !== "runtime.bootstrap" ||
      decodedResponse.result.lifecycle !== "ready" ||
      decodedResponse.result.protocolVersion !== DesktopRuntimeGatewayProtocolVersion
    ) {
      throw new Error("Production Desktop runtime gateway bootstrap failed closed.");
    }
    return { gateway, response: decodedResponse };
  } catch (cause) {
    gateway.dispose();
    throw cause;
  }
};

export type DesktopNativeSidecarService = Readonly<{
  receipt: DesktopNativeSidecarBootstrapReceipt;
  execute: (request: DesktopNativeSidecarRpcRequest) => Promise<DesktopNativeSidecarRpcResponse>;
  dispose: () => void;
}>;

export const openDesktopNativeSidecarService = async (
  input: DesktopNativeSidecarBootstrapRequest,
  facts: Readonly<{ nodeVersion: string; pid: number }> = {
    nodeVersion: process.versions.node,
    pid: process.pid,
  },
): Promise<DesktopNativeSidecarService> => {
  const request = decodeDesktopNativeSidecarBootstrapRequest(input);
  if (request === null) throw new Error("Native sidecar bootstrap request is invalid.");
  if (facts.nodeVersion !== DesktopNativeSidecarNodeVersion) {
    throw new Error(
      `Native sidecar requires Node ${DesktopNativeSidecarNodeVersion}; observed ${facts.nodeVersion}.`,
    );
  }
  if (!Number.isSafeInteger(facts.pid) || facts.pid <= 0) {
    throw new Error("Native sidecar process identity is invalid.");
  }

  const stateRoot = decodePrivatePath(request.stateRootBase64);
  mkdirSync(stateRoot, { recursive: true, mode: 0o700 });
  if (process.platform !== "win32") chmodSync(stateRoot, 0o700);

  const runtime = await openRuntimeGateway();
  const response = runtime.response;
  const receipt: DesktopNativeSidecarBootstrapReceipt = {
    protocol: DesktopNativeSidecarProtocol,
    generation: request.generation,
    nonce: request.nonce,
    pid: facts.pid,
    nodeVersion: DesktopNativeSidecarNodeVersion,
    gatewayProtocolVersion: DesktopRuntimeGatewayProtocolVersion,
    requestId: "native-sidecar.bootstrap",
    response,
  };
  if (decodeDesktopNativeSidecarBootstrapReceipt(receipt) === null) {
    throw new Error("Native sidecar bootstrap receipt failed its output schema.");
  }

  let sync: ReturnType<typeof openDesktopSyncHost>;
  try {
    sync = openDesktopSyncHost({
      databasePath: path.join(stateRoot, "sync.sqlite"),
      randomId: randomUUID,
    });
  } catch (cause) {
    runtime.gateway.dispose();
    throw cause;
  }
  const catalog = sync.codingCatalog();
  if (catalog === null) {
    sync.close();
    runtime.gateway.dispose();
    throw new Error("Native sidecar coding catalog is unavailable.");
  }
  let activeWorkspace: AdmittedDesktopWorkspace | null = null;
  let disposed = false;
  const replay = new Map<
    string,
    Readonly<{
      requestDigest: string;
      response: DesktopNativeSidecarRpcResponse;
    }>
  >();

  try {
    const selectedRoot = catalog.selectedRoot();
    if (selectedRoot !== null) {
      const repositoryRoot = isRepositoryRoot(selectedRoot);
      if (repositoryRoot !== null) {
        activeWorkspace = openAdmittedDesktopWorkspace(catalog, repositoryRoot);
      }
    }
  } catch (cause) {
    sync.close();
    runtime.gateway.dispose();
    throw cause;
  }

  const execute = async (
    raw: DesktopNativeSidecarRpcRequest,
  ): Promise<DesktopNativeSidecarRpcResponse> => {
    if (disposed) throw new Error("Native sidecar service is closed.");
    const rpc = decodeDesktopNativeSidecarRpcRequest(raw);
    if (rpc === null || rpc.generation !== request.generation || rpc.nonce !== request.nonce) {
      throw new Error("Native sidecar RPC failed its generation fence.");
    }

    const projectionResult = () => {
      const projection = strictProjection(projectDesktopCodingCatalog(catalog.snapshot()));
      const projectionJson = JSON.stringify(projection);
      if (Buffer.byteLength(projectionJson, "utf8") > DesktopNativeSidecarProjectionLimit) {
        throw new Error("Native coding projection exceeded its output bound.");
      }
      const selectedRoot = catalog.selectedRoot();
      if (
        projectionJson.includes(stateRoot) ||
        (selectedRoot !== null && projectionJson.includes(selectedRoot))
      ) {
        throw new Error("Native coding projection contained a private host path.");
      }
      return { projection, projectionDigest: sha256(projectionJson) };
    };
    const requestDigest = sha256(JSON.stringify(rpc));
    const prior = replay.get(rpc.requestId);
    if (prior !== undefined) {
      if (prior.requestDigest === requestDigest) return prior.response;
      const result: DesktopNativeSidecarRpcResponse = {
        protocol: DesktopNativeSidecarRpcProtocol,
        generation: request.generation,
        nonce: request.nonce,
        requestId: rpc.requestId,
        result: { kind: "coding.refused", reason: "request_conflict", ...projectionResult() },
      };
      if (decodeDesktopNativeSidecarRpcResponse(result) === null) {
        throw new Error("Native sidecar RPC conflict response failed its closed schema.");
      }
      return result;
    }
    if (replay.size >= 256) {
      return {
        protocol: DesktopNativeSidecarRpcProtocol,
        generation: request.generation,
        nonce: request.nonce,
        requestId: rpc.requestId,
        result: { kind: "coding.refused", reason: "request_limit", ...projectionResult() },
      };
    }

    let response: DesktopNativeSidecarRpcResponse;
    if (rpc.operation === "coding.admit") {
      const repositoryRoot = isRepositoryRoot(decodePrivatePath(rpc.rootBase64));
      if (repositoryRoot === null) {
        response = {
          protocol: DesktopNativeSidecarRpcProtocol,
          generation: request.generation,
          nonce: request.nonce,
          requestId: rpc.requestId,
          result: { kind: "coding.refused", reason: "not_repository", ...projectionResult() },
        };
      } else {
        const admitted = openAdmittedDesktopWorkspace(catalog, repositoryRoot);
        const previous = activeWorkspace;
        activeWorkspace = admitted;
        previous?.workspace.dispose();
        response = {
          protocol: DesktopNativeSidecarRpcProtocol,
          generation: request.generation,
          nonce: request.nonce,
          requestId: rpc.requestId,
          result: {
            kind: "coding.admitted",
            ...projectionResult(),
            admission: admitted.admission,
          },
        };
      }
    } else {
      response = {
        protocol: DesktopNativeSidecarRpcProtocol,
        generation: request.generation,
        nonce: request.nonce,
        requestId: rpc.requestId,
        result: { kind: "coding.snapshot", ...projectionResult() },
      };
    }
    if (decodeDesktopNativeSidecarRpcResponse(response) === null) {
      throw new Error("Native sidecar RPC response failed its closed schema.");
    }
    replay.set(rpc.requestId, { requestDigest, response });
    return response;
  };

  return {
    receipt,
    execute,
    dispose: () => {
      if (disposed) return;
      disposed = true;
      activeWorkspace?.workspace.dispose();
      activeWorkspace = null;
      sync.close();
      runtime.gateway.dispose();
    },
  };
};

export const executeDesktopNativeSidecarBootstrap = async (
  input: DesktopNativeSidecarBootstrapRequest,
  facts?: Readonly<{ nodeVersion: string; pid: number }>,
): Promise<DesktopNativeSidecarBootstrapReceipt> => {
  const service = await openDesktopNativeSidecarService(input, facts);
  try {
    return service.receipt;
  } finally {
    service.dispose();
  }
};
