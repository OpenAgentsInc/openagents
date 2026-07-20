import {
  DESKTOP_SOURCE_SAFE_POINT_CONTROL_SCHEMA_VERSION,
  DesktopSourceSafePointControlRequestSchema,
  DesktopSourceSafePointRendezvousSchema,
  type DesktopSourceSafePointControlRequest,
  type DesktopSourceSafePointControlResponse,
  type DesktopSourceSafePointRendezvous,
} from "@openagentsinc/portable-session-contract";
import { createHash, randomBytes, randomUUID, timingSafeEqual } from "node:crypto";
import { lstatSync, readFileSync, unlinkSync } from "node:fs";
import { createServer, type IncomingMessage, type ServerResponse } from "node:http";
import { chmod, lstat, mkdir, rename, unlink, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { Option, Schema } from "effect";

import type { DesktopSourceSafePoint } from "./desktop-source-safe-point.ts";

const CONTROL_DIRECTORY = "desktop-source-safe-point";
const RENDEZVOUS_FILE = "control.json";
const CONTROL_PATH = "/v1/source-safe-point";
const MAX_BODY_BYTES = 16 * 1024;
const REQUEST_KEYS = [
  "schema",
  "operationRef",
  "commandRef",
  "commandExecutionClaimRef",
  "ownerRef",
  "pylonRef",
  "targetRef",
  "sessionRef",
  "attachmentRef",
  "attachmentGeneration",
  "expiresAt",
] as const;

export const desktopSourceSafePointRendezvousPath = (pylonHome: string): string =>
  join(pylonHome, CONTROL_DIRECTORY, RENDEZVOUS_FILE);

export type DesktopSourceSafePointControlAuthority = Readonly<{
  ownerRef: string;
  pylonRef: string;
  targetRef: string;
}>;

export type DesktopSourceSafePointControlServer = Readonly<{
  desktopInstanceRef: string;
  rendezvousPath: string;
  stop: () => Promise<void>;
}>;

export class DesktopSourceSafePointControlServerError extends Error {
  override readonly name = "DesktopSourceSafePointControlServerError";
  constructor(readonly reason: "invalid_custody" | "invalid_identity" | "listen_failed") {
    super(`Desktop source safe-point control failed closed: ${reason}`);
  }
}

const decodeRequest = Schema.decodeUnknownOption(DesktopSourceSafePointControlRequestSchema);
const decodeRendezvous = Schema.decodeUnknownOption(DesktopSourceSafePointRendezvousSchema);
const safeRef = /^[a-zA-Z0-9][a-zA-Z0-9._:-]{2,255}$/u;

const currentUid = (): number | null =>
  typeof process.getuid === "function" ? process.getuid() : null;

const missing = (error: unknown): boolean =>
  error !== null && typeof error === "object" && "code" in error && error.code === "ENOENT";

const assertPrivateNode = async (path: string, kind: "directory" | "file"): Promise<void> => {
  const info = await lstat(path);
  const expected = kind === "directory" ? info.isDirectory() : info.isFile();
  const uid = currentUid();
  if (
    !expected ||
    info.isSymbolicLink() ||
    (info.mode & 0o077) !== 0 ||
    (uid !== null && info.uid !== uid)
  ) {
    throw new DesktopSourceSafePointControlServerError("invalid_custody");
  }
};

const exactObject = (value: unknown): value is Record<string, unknown> => {
  if (value === null || typeof value !== "object" || Array.isArray(value)) return false;
  const keys = Object.keys(value).sort();
  return (
    keys.length === REQUEST_KEYS.length &&
    keys.every((key, index) => key === [...REQUEST_KEYS].sort()[index])
  );
};

const bearerMatches = (received: string | undefined, expected: string): boolean => {
  const prefix = "Bearer ";
  if (received === undefined || !received.startsWith(prefix)) return false;
  const actual = Buffer.from(received.slice(prefix.length));
  const wanted = Buffer.from(expected);
  return actual.length === wanted.length && timingSafeEqual(actual, wanted);
};

const loopbackAddress = (address: string | undefined): boolean =>
  address === "127.0.0.1" || address === "::1" || address === "::ffff:127.0.0.1";

const json = (response: ServerResponse, status: number, body: unknown): void => {
  response.writeHead(status, { "content-type": "application/json", "cache-control": "no-store" });
  response.end(JSON.stringify(body));
};

const readJsonBody = (request: IncomingMessage): Promise<unknown> =>
  new Promise((resolve, reject) => {
    let size = 0;
    const chunks: Buffer[] = [];
    request.on("data", (chunk: Buffer) => {
      size += chunk.byteLength;
      if (size > MAX_BODY_BYTES) {
        reject(new Error("request body is too large"));
        request.destroy();
        return;
      }
      chunks.push(chunk);
    });
    request.on("end", () => {
      try {
        resolve(JSON.parse(Buffer.concat(chunks).toString("utf8")));
      } catch (error) {
        reject(error);
      }
    });
    request.on("error", reject);
  });

const requestFingerprint = (request: DesktopSourceSafePointControlRequest): string =>
  createHash("sha256").update(JSON.stringify(request)).digest("hex");

const refuse = (
  desktopInstanceRef: string,
  operationRef: string,
  reasonRef: string,
): DesktopSourceSafePointControlResponse => ({
  schema: DESKTOP_SOURCE_SAFE_POINT_CONTROL_SCHEMA_VERSION,
  desktopInstanceRef,
  operationRef,
  state: "refused",
  reasonRef,
  evidenceRefs: [],
  remoteExecution: "not_claimed",
});

export const startDesktopSourceSafePointControlServer = async (
  options: Readonly<{
    pylonHome: string;
    pylonRef: string;
    safePoint: DesktopSourceSafePoint;
    currentAuthority: () => DesktopSourceSafePointControlAuthority | null;
    hostname?: "127.0.0.1" | "::1";
    port?: number;
    now?: () => Date;
  }>,
): Promise<DesktopSourceSafePointControlServer> => {
  if (!safeRef.test(options.pylonRef)) {
    throw new DesktopSourceSafePointControlServerError("invalid_identity");
  }
  const hostname = options.hostname ?? "127.0.0.1";
  const now = options.now ?? (() => new Date());
  const directory = join(options.pylonHome, CONTROL_DIRECTORY);
  const rendezvousPath = join(directory, RENDEZVOUS_FILE);
  try {
    await assertPrivateNode(directory, "directory");
  } catch (error) {
    if (!missing(error)) throw error;
    await mkdir(directory, { recursive: true, mode: 0o700 });
  }
  await assertPrivateNode(directory, "directory");

  const desktopInstanceRef = `desktop.instance.${randomUUID()}`;
  const bearerToken = randomBytes(32).toString("hex");
  const operations = new Map<
    string,
    Readonly<{
      fingerprint: string;
      result: Promise<DesktopSourceSafePointControlResponse>;
    }>
  >();

  const execute = (
    request: DesktopSourceSafePointControlRequest,
  ): Promise<DesktopSourceSafePointControlResponse> => {
    const fingerprint = requestFingerprint(request);
    const existing = operations.get(request.operationRef);
    if (existing !== undefined) {
      return existing.fingerprint === fingerprint
        ? existing.result
        : Promise.resolve(
            refuse(
              desktopInstanceRef,
              request.operationRef,
              "desktop.safe-point.operation-conflict",
            ),
          );
    }
    const result = (async (): Promise<DesktopSourceSafePointControlResponse> => {
      const liveAuthority = options.currentAuthority();
      if (
        liveAuthority === null ||
        liveAuthority.ownerRef !== request.ownerRef ||
        liveAuthority.pylonRef !== request.pylonRef ||
        liveAuthority.targetRef !== request.targetRef
      ) {
        return refuse(
          desktopInstanceRef,
          request.operationRef,
          "desktop.safe-point.authority-mismatch",
        );
      }
      if (Date.parse(request.expiresAt) <= now().getTime()) {
        return refuse(
          desktopInstanceRef,
          request.operationRef,
          "desktop.safe-point.request-expired",
        );
      }
      const binding = options.safePoint.currentBinding();
      if (
        binding === null ||
        binding.sessionRef !== request.sessionRef ||
        binding.attachmentRef !== request.attachmentRef ||
        binding.generation !== request.attachmentGeneration
      ) {
        return refuse(
          desktopInstanceRef,
          request.operationRef,
          "desktop.safe-point.binding-mismatch",
        );
      }
      const settled = await options.safePoint.quiesce(binding);
      return {
        schema: DESKTOP_SOURCE_SAFE_POINT_CONTROL_SCHEMA_VERSION,
        desktopInstanceRef,
        operationRef: request.operationRef,
        state:
          settled.state === "quiescent"
            ? "quiescent"
            : settled.state === "not_quiescent"
              ? "not_quiescent"
              : "refused",
        reasonRef:
          settled.state === "refused"
            ? `desktop.safe-point.${settled.reason.replaceAll("_", "-")}`
            : null,
        evidenceRefs: settled.outcomes.map((outcome) => outcome.evidenceRef),
        remoteExecution: "not_claimed",
      };
    })();
    operations.set(request.operationRef, { fingerprint, result });
    return result;
  };

  const server = createServer((request, response) => {
    void (async () => {
      if (!loopbackAddress(request.socket.remoteAddress))
        return json(response, 403, { error: "loopback_required" });
      if (request.method !== "POST" || request.url !== CONTROL_PATH)
        return json(response, 405, { error: "method_not_allowed" });
      if (!bearerMatches(request.headers.authorization, bearerToken))
        return json(response, 401, { error: "unauthorized" });
      if (
        request.headers["content-type"]?.split(";", 1)[0]?.trim().toLowerCase() !==
        "application/json"
      ) {
        return json(response, 415, { error: "content_type_required" });
      }
      let unknown: unknown;
      try {
        unknown = await readJsonBody(request);
      } catch {
        return json(response, 400, { error: "invalid_request" });
      }
      if (!exactObject(unknown)) return json(response, 400, { error: "invalid_request" });
      const decoded = decodeRequest(unknown);
      if (Option.isNone(decoded)) return json(response, 400, { error: "invalid_request" });
      return json(response, 200, await execute(decoded.value));
    })().catch(() => json(response, 500, { error: "safe_point_failed" }));
  });

  await new Promise<void>((resolve, reject) => {
    const onError = () => reject(new DesktopSourceSafePointControlServerError("listen_failed"));
    server.once("error", onError);
    server.listen(options.port ?? 0, hostname, () => {
      server.off("error", onError);
      resolve();
    });
  });
  const address = server.address();
  if (address === null || typeof address === "string") {
    server.close();
    throw new DesktopSourceSafePointControlServerError("listen_failed");
  }
  const rendezvous: DesktopSourceSafePointRendezvous = {
    schema: DESKTOP_SOURCE_SAFE_POINT_CONTROL_SCHEMA_VERSION,
    desktopInstanceRef,
    pylonRef: options.pylonRef,
    url: `http://${hostname === "::1" ? "[::1]" : hostname}:${address.port}${CONTROL_PATH}`,
    bearerToken,
    issuedAt: now().toISOString(),
  };
  const temporary = `${rendezvousPath}.${desktopInstanceRef}.tmp`;
  try {
    await writeFile(temporary, `${JSON.stringify(rendezvous)}\n`, { mode: 0o600, flag: "wx" });
    await chmod(temporary, 0o600);
    await assertPrivateNode(temporary, "file");
    try {
      await assertPrivateNode(rendezvousPath, "file");
    } catch (error) {
      if (!missing(error)) throw error;
    }
    await rename(temporary, rendezvousPath);
    await assertPrivateNode(rendezvousPath, "file");
  } catch (error) {
    server.close();
    try {
      await assertPrivateNode(temporary, "file");
      await unlink(temporary);
    } catch {
      // Delete only the private temporary file that this instance created.
    }
    throw error;
  }

  let stopped = false;
  const removeOwnRendezvous = (): void => {
    try {
      const info = lstatSync(rendezvousPath);
      const uid = currentUid();
      if (
        !info.isFile() ||
        info.isSymbolicLink() ||
        (info.mode & 0o077) !== 0 ||
        (uid !== null && info.uid !== uid)
      )
        return;
      const decoded = decodeRendezvous(JSON.parse(readFileSync(rendezvousPath, "utf8")));
      if (
        Option.isSome(decoded) &&
        decoded.value.desktopInstanceRef === desktopInstanceRef &&
        decoded.value.bearerToken === bearerToken
      )
        unlinkSync(rendezvousPath);
    } catch {
      // A missing or replaced rendezvous does not grant deletion authority.
    }
  };
  return {
    desktopInstanceRef,
    rendezvousPath,
    stop: async () => {
      if (stopped) return;
      stopped = true;
      // This synchronous, bounded deletion runs before the first await so an
      // Electron before-quit listener cannot leave a live bearer on disk.
      removeOwnRendezvous();
      await new Promise<void>((resolve) => server.close(() => resolve()));
    },
  };
};
