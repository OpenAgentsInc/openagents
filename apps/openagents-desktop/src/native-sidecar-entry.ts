import { timingSafeEqual } from "node:crypto";
import { createServer, type IncomingMessage, type ServerResponse } from "node:http";

import {
  DesktopNativeSidecarFrameLimit,
  decodeDesktopNativeSidecarBootstrapRequest,
  decodeDesktopNativeSidecarReadyReceipt,
  decodeDesktopNativeSidecarRpcRequest,
  openDesktopNativeSidecarService,
} from "./native-sidecar-contract.ts";

const readBoundedStream = async (
  stream: NodeJS.ReadableStream,
  limit = DesktopNativeSidecarFrameLimit,
): Promise<Buffer> => {
  const chunks: Buffer[] = [];
  let length = 0;
  for await (const chunk of stream) {
    const bytes = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk);
    length += bytes.length;
    if (length > limit) throw new Error("Native sidecar frame exceeded its bound.");
    chunks.push(bytes);
  }
  return Buffer.concat(chunks, length);
};

const writeJson = (response: ServerResponse, status: number, value: unknown): void => {
  const body = JSON.stringify(value);
  response.writeHead(status, {
    "content-type": "application/json; charset=utf-8",
    "content-length": Buffer.byteLength(body),
    "cache-control": "no-store",
    "x-content-type-options": "nosniff",
  });
  response.end(body);
};

const authorized = (candidate: string | undefined, token: string): boolean => {
  if (candidate === undefined || !candidate.startsWith("Bearer ")) return false;
  const supplied = Buffer.from(candidate.slice("Bearer ".length), "utf8");
  const expected = Buffer.from(token, "utf8");
  return supplied.length === expected.length && timingSafeEqual(supplied, expected);
};

const main = async (): Promise<void> => {
  const bootstrapBytes = await readBoundedStream(process.stdin);
  let bootstrapValue: unknown;
  try {
    bootstrapValue = JSON.parse(bootstrapBytes.toString("utf8"));
  } catch {
    throw new Error("Native sidecar bootstrap was not valid JSON.");
  }
  const bootstrap = decodeDesktopNativeSidecarBootstrapRequest(bootstrapValue);
  if (bootstrap === null) throw new Error("Native sidecar bootstrap failed its closed schema.");

  const service = await openDesktopNativeSidecarService(bootstrap);
  const token = bootstrap.transportToken;
  let queue: Promise<unknown> = Promise.resolve();
  let expectedHost = "";
  const server = createServer((request: IncomingMessage, response: ServerResponse) => {
    void (async () => {
      const contentLength = Number(request.headers["content-length"] ?? "0");
      const contentType = request.headers["content-type"]?.split(";", 1)[0]?.trim().toLowerCase();
      if (
        request.method !== "POST" ||
        request.url !== "/v1/coding" ||
        request.socket.localAddress !== "127.0.0.1" ||
        request.socket.remoteAddress !== "127.0.0.1" ||
        request.headers.host !== expectedHost ||
        request.headers.origin !== undefined ||
        !authorized(request.headers.authorization, token) ||
        contentType !== "application/json" ||
        !Number.isSafeInteger(contentLength) ||
        contentLength <= 0 ||
        contentLength > DesktopNativeSidecarFrameLimit
      ) {
        writeJson(response, 404, { error: "unavailable" });
        return;
      }
      let value: unknown;
      try {
        const bytes = await readBoundedStream(request);
        if (bytes.length !== contentLength) throw new Error("Native sidecar body length mismatch.");
        value = JSON.parse(bytes.toString("utf8"));
      } catch {
        writeJson(response, 400, { error: "invalid_request" });
        return;
      }
      const rpc = decodeDesktopNativeSidecarRpcRequest(value);
      if (rpc === null) {
        writeJson(response, 400, { error: "invalid_request" });
        return;
      }
      const operation = queue.then(() => service.execute(rpc));
      queue = operation.catch(() => undefined);
      try {
        writeJson(response, 200, await operation);
      } catch {
        writeJson(response, 409, { error: "operation_refused" });
      }
    })().catch(() => {
      if (!response.headersSent) writeJson(response, 500, { error: "service_unavailable" });
      else response.destroy();
    });
  });
  server.headersTimeout = 5_000;
  server.requestTimeout = 5_000;
  server.keepAliveTimeout = 1_000;
  server.maxConnections = 8;

  try {
    await new Promise<void>((resolve, reject) => {
      server.once("error", reject);
      server.listen({ host: "127.0.0.1", port: 0, exclusive: true }, () => resolve());
    });
  } catch (cause) {
    service.dispose();
    throw cause;
  }
  const address = server.address();
  if (address === null || typeof address === "string") {
    server.close();
    service.dispose();
    throw new Error("Native sidecar loopback transport is unavailable.");
  }
  expectedHost = `127.0.0.1:${address.port}`;
  const ready = {
    ...service.receipt,
    transport: {
      kind: "loopback_http" as const,
      host: "127.0.0.1" as const,
      port: address.port,
    },
  };
  if (decodeDesktopNativeSidecarReadyReceipt(ready) === null) {
    server.close();
    service.dispose();
    throw new Error("Native sidecar ready receipt failed its closed schema.");
  }
  process.stdout.write(`${JSON.stringify(ready)}\n`);

  await new Promise<void>((resolve) => {
    let closing = false;
    const parentPid = process.ppid;
    let parentWatch: NodeJS.Timeout | null = null;
    const close = (): void => {
      if (closing) return;
      closing = true;
      if (parentWatch !== null) clearInterval(parentWatch);
      server.close(() => {
        service.dispose();
        resolve();
      });
    };
    parentWatch = setInterval(() => {
      try {
        process.kill(parentPid, 0);
      } catch {
        close();
      }
    }, 500);
    parentWatch.unref();
    process.once("SIGTERM", close);
    process.once("SIGINT", close);
  });
};

void main().catch(() => {
  process.stderr.write("[openagents-native-sidecar] service failed closed.\n");
  process.exitCode = 1;
});
