import {
  DesktopSourceSafePointControlResponseSchema,
  DesktopSourceSafePointRendezvousSchema,
  type DesktopSourceSafePointControlRequest,
  type DesktopSourceSafePointControlResponse,
  type DesktopSourceSafePointRendezvous,
} from "@openagentsinc/portable-session-contract";
import { lstat, readFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { Option, Schema } from "effect";

const RENDEZVOUS_PATH = join("desktop-source-safe-point", "control.json");
const RENDEZVOUS_KEYS = [
  "schema",
  "desktopInstanceRef",
  "pylonRef",
  "url",
  "bearerToken",
  "issuedAt",
] as const;
const RESPONSE_KEYS = [
  "schema",
  "desktopInstanceRef",
  "operationRef",
  "state",
  "reasonRef",
  "evidenceRefs",
  "remoteExecution",
] as const;

export type PylonDesktopSourceSafePointClient = Readonly<{
  quiesce: (
    request: DesktopSourceSafePointControlRequest,
    signal?: AbortSignal,
  ) => Promise<DesktopSourceSafePointControlResponse>;
}>;

export class PylonDesktopSourceSafePointClientError extends Error {
  override readonly name = "PylonDesktopSourceSafePointClientError";
  constructor(
    readonly reason:
      | "authority_mismatch"
      | "desktop_restarted"
      | "invalid_custody"
      | "invalid_rendezvous"
      | "invalid_response"
      | "request_expired"
      | "transport_failed",
  ) {
    super(`Pylon Desktop source safe-point failed closed: ${reason}`);
  }
}

const decodeRendezvous = Schema.decodeUnknownOption(DesktopSourceSafePointRendezvousSchema);
const decodeResponse = Schema.decodeUnknownOption(DesktopSourceSafePointControlResponseSchema);

const exactKeys = (
  value: unknown,
  expected: ReadonlyArray<string>,
): value is Record<string, unknown> => {
  if (value === null || typeof value !== "object" || Array.isArray(value)) return false;
  const actual = Object.keys(value).sort();
  const wanted = [...expected].sort();
  return actual.length === wanted.length && actual.every((key, index) => key === wanted[index]);
};

const currentUid = (): number | null =>
  typeof process.getuid === "function" ? process.getuid() : null;

const assertPrivateNode = async (path: string, kind: "directory" | "file"): Promise<void> => {
  try {
    const info = await lstat(path);
    const expected = kind === "directory" ? info.isDirectory() : info.isFile();
    const uid = currentUid();
    if (
      !expected ||
      info.isSymbolicLink() ||
      (info.mode & 0o077) !== 0 ||
      (uid !== null && info.uid !== uid)
    ) {
      throw new Error("unsafe node");
    }
  } catch {
    throw new PylonDesktopSourceSafePointClientError("invalid_custody");
  }
};

const loopbackUrl = (value: string): boolean => {
  try {
    const url = new URL(value);
    return (
      url.protocol === "http:" &&
      (url.hostname === "127.0.0.1" || url.hostname === "[::1]" || url.hostname === "::1") &&
      url.pathname === "/v1/source-safe-point" &&
      url.username === "" &&
      url.password === "" &&
      url.search === "" &&
      url.hash === ""
    );
  } catch {
    return false;
  }
};

const readRendezvous = async (
  pylonHome: string,
  pylonRef: string,
): Promise<DesktopSourceSafePointRendezvous> => {
  const path = join(pylonHome, RENDEZVOUS_PATH);
  await assertPrivateNode(dirname(path), "directory");
  await assertPrivateNode(path, "file");
  let unknown: unknown;
  try {
    unknown = JSON.parse(await readFile(path, "utf8"));
  } catch {
    throw new PylonDesktopSourceSafePointClientError("invalid_rendezvous");
  }
  if (!exactKeys(unknown, RENDEZVOUS_KEYS)) {
    throw new PylonDesktopSourceSafePointClientError("invalid_rendezvous");
  }
  const decoded = decodeRendezvous(unknown);
  if (
    Option.isNone(decoded) ||
    decoded.value.pylonRef !== pylonRef ||
    !loopbackUrl(decoded.value.url)
  ) {
    throw new PylonDesktopSourceSafePointClientError("invalid_rendezvous");
  }
  return decoded.value;
};

export const makePylonDesktopSourceSafePointClient = (
  options: Readonly<{
    pylonHome: string;
    pylonRef: string;
    fetchImpl?: typeof globalThis.fetch;
    now?: () => Date;
  }>,
): PylonDesktopSourceSafePointClient => {
  const fetchImpl = options.fetchImpl ?? globalThis.fetch;
  const now = options.now ?? (() => new Date());
  return {
    quiesce: async (request, signal) => {
      if (request.pylonRef !== options.pylonRef) {
        throw new PylonDesktopSourceSafePointClientError("authority_mismatch");
      }
      if (Date.parse(request.expiresAt) <= now().getTime()) {
        throw new PylonDesktopSourceSafePointClientError("request_expired");
      }
      const rendezvous = await readRendezvous(options.pylonHome, options.pylonRef);
      let response: Response;
      try {
        response = await fetchImpl(rendezvous.url, {
          method: "POST",
          headers: {
            authorization: `Bearer ${rendezvous.bearerToken}`,
            "content-type": "application/json",
          },
          body: JSON.stringify(request),
          signal,
        });
      } catch {
        throw new PylonDesktopSourceSafePointClientError("transport_failed");
      }
      if (!response.ok) throw new PylonDesktopSourceSafePointClientError("transport_failed");
      let unknown: unknown;
      try {
        unknown = await response.json();
      } catch {
        throw new PylonDesktopSourceSafePointClientError("invalid_response");
      }
      if (!exactKeys(unknown, RESPONSE_KEYS)) {
        throw new PylonDesktopSourceSafePointClientError("invalid_response");
      }
      const decoded = decodeResponse(unknown);
      if (Option.isNone(decoded) || decoded.value.operationRef !== request.operationRef) {
        throw new PylonDesktopSourceSafePointClientError("invalid_response");
      }
      if (decoded.value.desktopInstanceRef !== rendezvous.desktopInstanceRef) {
        throw new PylonDesktopSourceSafePointClientError("desktop_restarted");
      }
      return decoded.value;
    },
  };
};
