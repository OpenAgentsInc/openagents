import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { basename } from "node:path";

import { AgentStdioTransport } from "@openagentsinc/agent-stdio-transport";

export type LiveProfile = "grok" | "cursor";
export type AcpLiveProbeResult = Readonly<{
  proofClass: "diagnostic-live";
  peer: LiveProfile;
  command: ReadonlyArray<string>;
  resolvedExecutableName?: string;
  resolvedExecutableSha256?: string;
  binaryVersion: string;
  schemaRelease: "schema-v1.19.0";
  protocolVersion: 1;
  result: "pass" | "fail" | "skipped";
  initialize?: unknown;
  errorKind?: string;
}>;

const profiles = {
  grok: {
    arm: "GROK_ACP_LIVE",
    executable: "grok",
    args: ["agent", "stdio"],
    versionArgs: ["version"],
  },
  cursor: {
    arm: "CURSOR_ACP_LIVE",
    executable: "agent",
    args: ["acp"],
    versionArgs: ["--version"],
  },
} as const;

const object = (value: unknown): Record<string, unknown> =>
  value !== null && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};

/** Deliberately excludes provider metadata, hostnames, IDs, paths, and model context details. */
export const summarizeSafeInitialize = (value: unknown): unknown => {
  const initialize = object(value);
  const info = object(initialize.agentInfo);
  const capabilities = object(initialize.agentCapabilities);
  const sessionCapabilities = object(capabilities.sessionCapabilities);
  return Object.freeze({
    protocolVersion: initialize.protocolVersion,
    ...(typeof info.name === "string" || typeof info.version === "string"
      ? {
          agentInfo: {
            ...(typeof info.name === "string" ? { name: info.name.slice(0, 128) } : {}),
            ...(typeof info.version === "string" ? { version: info.version.slice(0, 64) } : {}),
          },
        }
      : {}),
    advertisedCapabilityKeys: Object.keys(capabilities)
      .filter((key) => key !== "_meta")
      .toSorted(),
    advertisedSessionCapabilityKeys: Object.keys(sessionCapabilities).toSorted(),
    authMethodIds: Array.isArray(initialize.authMethods)
      ? initialize.authMethods
          .map(object)
          .flatMap((method) => (typeof method.id === "string" ? [method.id.slice(0, 128)] : []))
      : [],
  });
};

export const runAcpLiveProbe = async (peer: LiveProfile): Promise<AcpLiveProbeResult> => {
  const profile = profiles[peer];
  const base = {
    proofClass: "diagnostic-live" as const,
    peer,
    command: [profile.executable, ...profile.args],
    schemaRelease: "schema-v1.19.0" as const,
    protocolVersion: 1 as const,
  };
  if (process.env[profile.arm] !== "1")
    return { ...base, binaryVersion: "not-probed", result: "skipped" };
  const version = spawnSync(profile.executable, profile.versionArgs, {
    encoding: "utf8",
    timeout: 5_000,
  });
  const binaryVersion = `${version.stdout}${version.stderr}`
    .trim()
    .slice(0, 256)
    .replace(/(?:token|key|secret)=\S+/gi, "[REDACTED]");
  if (version.error !== undefined || version.status !== 0 || binaryVersion.length === 0) {
    return {
      ...base,
      binaryVersion: binaryVersion || "version-probe-failed",
      result: "fail",
      errorKind: version.error?.name ?? `version-exit-${String(version.status)}`,
    };
  }
  let transport: AgentStdioTransport | undefined;
  const executableIdentity = () => {
    if (transport === undefined) return {};
    const resolved = transport.getReceipt().resolvedExecutable;
    return {
      resolvedExecutableName: basename(resolved),
      resolvedExecutableSha256: createHash("sha256").update(readFileSync(resolved)).digest("hex"),
    };
  };
  try {
    transport = await AgentStdioTransport.start({
      executable: profile.executable,
      args: profile.args,
      limits: { requestTimeoutMs: 10_000 },
    });
    const initialize = await transport.request("initialize", {
      protocolVersion: 1,
      clientCapabilities: { fs: { readTextFile: false, writeTextFile: false }, terminal: false },
      clientInfo: { name: "openagents-live-probe", version: "0.1.0" },
    });
    return {
      ...base,
      binaryVersion,
      ...executableIdentity(),
      result: "pass",
      initialize: summarizeSafeInitialize(initialize),
    };
  } catch (error) {
    return {
      ...base,
      binaryVersion,
      ...executableIdentity(),
      result: "fail",
      errorKind: error instanceof Error ? error.name : "unknown",
    };
  } finally {
    await transport?.dispose();
  }
};
