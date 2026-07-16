import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { constants, createReadStream } from "node:fs";
import { access, realpath } from "node:fs/promises";
import { basename, delimiter, isAbsolute, resolve } from "node:path";
import { promisify } from "node:util";

import {
  admitAcpPeerProfile,
  createDefaultAcpTrustedPeerProfileRegistry,
  type AcpConformanceEvidenceRecord,
  type AcpExecutableProbe,
  type AcpPeerAdmissionDecision,
} from "@openagentsinc/agent-client-protocol/profiles";
import { GROK_ACP_PROFILE } from "@openagentsinc/agent-client-protocol/extensions/grok";
import {
  AcpSessionRuntime,
  createAdmittedAcpSessionTransport,
  type AcpMcpReference,
  type AcpRuntimeEvidence,
  type AcpLifecycleOutcome,
  type AcpSessionTransportPort,
  type AcpSessionRuntimeOptions,
  type AcpSessionUpdateRecord,
} from "@openagentsinc/agent-client-runtime-bridge";
import type {
  AgentStdioReverseHandler,
  AgentStdioTransportLimits,
} from "@openagentsinc/agent-stdio-transport";

const execFileAsync = promisify(execFile);

export type GrokAcpTransport = AcpSessionTransportPort &
  Readonly<{
    registerReverseHandler(method: string, handler: AgentStdioReverseHandler): () => void;
  }>;

export type GrokAcpAuthorityInstallation = Readonly<{
  install(transport: GrokAcpTransport): void | (() => void);
}>;

export type GrokAuthInteraction = Readonly<{
  methodId: "grok.com" | "oidc";
  kind: "external-browser";
  state: "login-required";
  label?: string;
}>;

export type CreateGrokAcpPeerRuntimeOptions = Readonly<{
  cwd: string;
  environment?: Readonly<Record<string, string | undefined>>;
  apiKeyConfigured?: boolean;
  evidence?: ReadonlyArray<AcpConformanceEvidenceRecord>;
  now?: Date;
  authorizeLogin?: (interaction: GrokAuthInteraction) => Promise<"continue" | "cancel">;
  authority?: GrokAcpAuthorityInstallation;
  installVendorHandlers?: (transport: GrokAcpTransport) => void | (() => void);
  materializeMcp?: AcpSessionRuntimeOptions["materializeMcp"];
  onUpdate?: (record: AcpSessionUpdateRecord) => void | Promise<void>;
  settleTurn?: AcpSessionRuntimeOptions["settleTurn"];
  limits?: Partial<AgentStdioTransportLimits>;
  requestTimeoutMs?: number;
  cancelGraceMs?: number;
  probe?: AcpExecutableProbe;
  admission?: Extract<AcpPeerAdmissionDecision, { _tag: "PeerAdmitted" }>;
  createTransport?: (
    admission: Extract<AcpPeerAdmissionDecision, { _tag: "PeerAdmitted" }>,
  ) => Promise<GrokAcpTransport>;
}>;

const instrumentRegistrations = (
  transport: GrokAcpTransport,
  methods: Set<string>,
): GrokAcpTransport =>
  new Proxy(transport, {
    get(target, property) {
      if (property === "registerReverseHandler")
        return (method: string, handler: AgentStdioReverseHandler) => {
          const unregister = target.registerReverseHandler(method, handler);
          methods.add(method);
          return () => {
            methods.delete(method);
            unregister();
          };
        };
      const value = Reflect.get(target, property, target) as unknown;
      return typeof value === "function" ? value.bind(target) : value;
    },
  });

const findOnPath = async (executable: string, pathValue: string | undefined): Promise<string> => {
  for (const directory of (pathValue ?? "").split(delimiter)) {
    if (directory.length === 0) continue;
    const candidate = resolve(directory, executable);
    try {
      await access(candidate, constants.X_OK);
      return candidate;
    } catch {
      // Continue through the bounded PATH list.
    }
  }
  throw Object.assign(new Error("Grok CLI is not installed or not on PATH"), {
    kind: "missing_executable",
  });
};

const sha256File = (path: string): Promise<string> =>
  new Promise((resolveHash, reject) => {
    const hash = createHash("sha256");
    const stream = createReadStream(path);
    stream.on("data", (chunk) => hash.update(chunk));
    stream.on("error", reject);
    stream.on("end", () => resolveHash(hash.digest("hex")));
  });

export const probeGrokAcpExecutable = async (
  environment: Readonly<Record<string, string | undefined>> = process.env,
  candidatePath?: string,
): Promise<AcpExecutableProbe> => {
  if (candidatePath !== undefined && (!isAbsolute(candidatePath) || basename(candidatePath) !== "grok")) {
    throw Object.assign(new Error("Alternate Grok executable must be an absolute path to grok"), { kind: "identity_mismatch" });
  }
  const resolvedPath = candidatePath ?? (await findOnPath("grok", environment.PATH));
  const realPath = await realpath(resolvedPath);
  const [{ stdout, stderr }, sha256] = await Promise.all([
    execFileAsync(realPath, ["version"], { timeout: 5_000, maxBuffer: 16_384 }),
    sha256File(realPath),
  ]);
  const reportedVersion = `${stdout}${stderr}`.trim().slice(0, 256);
  if (reportedVersion.length === 0) throw new Error("Grok version probe returned no version");
  return Object.freeze({
    requestedExecutable: "grok",
    resolvedPath,
    realPath,
    sha256,
    reportedVersion,
    platform: Object.freeze({ os: process.platform, arch: process.arch }),
  });
};

export const admitGrokAcpPeer = async (
  options: Pick<CreateGrokAcpPeerRuntimeOptions, "environment" | "evidence" | "now" | "probe"> = {},
): Promise<Extract<AcpPeerAdmissionDecision, { _tag: "PeerAdmitted" }>> => {
  const registry = createDefaultAcpTrustedPeerProfileRegistry();
  if (registry._tag !== "RegistryReady") throw new Error(registry.detail);
  const probe = options.probe ?? (await probeGrokAcpExecutable(options.environment));
  const decision = admitAcpPeerProfile({
    registry: registry.registry,
    profileId: "grok-cli",
    probe,
    evidence: options.evidence ?? [],
    now: options.now ?? new Date(),
  });
  if (decision._tag !== "PeerAdmitted")
    throw Object.assign(new Error(decision.detail), { kind: decision.reason });
  return decision;
};

export type GrokAcpPeerRuntime = Readonly<{
  admission: Extract<AcpPeerAdmissionDecision, { _tag: "PeerAdmitted" }>;
  evidence(): AcpRuntimeEvidence | undefined;
  receipts: AcpSessionRuntime["receipts"];
  sessions: AcpSessionRuntime["sessions"];
  start(): Promise<AcpLifecycleOutcome<AcpRuntimeEvidence>>;
  newSession(
    input: Readonly<{
      cwd: string;
      canonicalThreadSeed: string;
      mcpRefs?: ReadonlyArray<AcpMcpReference>;
      scopeRef?: string;
    }>,
  ): ReturnType<AcpSessionRuntime["newSession"]>;
  loadSession: AcpSessionRuntime["loadSession"];
  resumeSession: AcpSessionRuntime["resumeSession"];
  listSessions: AcpSessionRuntime["listSessions"];
  closeSession: AcpSessionRuntime["closeSession"];
  deleteSession: AcpSessionRuntime["deleteSession"];
  logout: AcpSessionRuntime["logout"];
  prompt: AcpSessionRuntime["prompt"];
  cancel: AcpSessionRuntime["cancel"];
  setMode: AcpSessionRuntime["setMode"];
  setConfigOption: AcpSessionRuntime["setConfigOption"];
  recover: AcpSessionRuntime["recover"];
  shutdown: AcpSessionRuntime["shutdown"];
}>;

export const createGrokAcpPeerRuntime = async (
  options: CreateGrokAcpPeerRuntimeOptions,
): Promise<GrokAcpPeerRuntime> => {
  const admission = options.admission ?? (await admitGrokAcpPeer(options));
  if (!isAbsolute(options.cwd)) throw new TypeError("Grok process cwd must be absolute");
  const authorizedWorkspace = await realpath(resolve(options.cwd));
  const sourceEnvironment = options.environment ?? process.env;
  const hasLiveFeatureEvidence = (suiteId: string): boolean =>
    (options.evidence ?? []).some((record) => {
      const ageMs = (options.now ?? new Date()).getTime() - Date.parse(record.recordedAt);
      return (
        record.suiteId === suiteId &&
        record.kind === "live" &&
        record.result === "pass" &&
        record.peerVersion === admission.peerVersion &&
        record.executableSha256 === admission.identityPin.sha256 &&
        Number.isFinite(ageMs) &&
        ageMs >= 0 &&
        ageMs <= 30 * 86_400_000
      );
    });
  const apiKeyEnabled =
    options.apiKeyConfigured === true &&
    hasLiveFeatureEvidence("grok-api-key-auth") &&
    typeof sourceEnvironment.XAI_API_KEY === "string" &&
    sourceEnvironment.XAI_API_KEY.length > 0;
  const environment = Object.freeze({
    ...(typeof sourceEnvironment.HOME === "string" ? { HOME: sourceEnvironment.HOME } : {}),
    ...(apiKeyEnabled ? { XAI_API_KEY: sourceEnvironment.XAI_API_KEY } : {}),
  });
  const authority = options.authority;
  const authorityEnabled =
    admission.supportState === "supported" &&
    hasLiveFeatureEvidence("grok-authority-reverse") &&
    authority !== undefined;
  const extensionsEligible =
    admission.supportState === "supported" &&
    hasLiveFeatureEvidence("grok-question-extensions") &&
    options.installVendorHandlers !== undefined &&
    GROK_ACP_PROFILE.methods.every((entry) =>
      admission.grants.vendorExtensionMethods.includes(entry.method),
    );
  const clientCapabilities = {
    fs: { readTextFile: false, writeTextFile: false },
    terminal: false,
  };
  const extensionMethods: string[] = [];
  const runtime = new AcpSessionRuntime({
    profile: "grok",
    extensionMethods,
    clientCapabilities,
    clientInfo: { name: "openagents", title: "OpenAgents", version: "0.1.0" },
    peerIdentityFallback: { name: "grok", version: admission.peerVersion },
    expectedPeerIdentity: { namePrefix: "grok", version: admission.peerVersion },
    selectAuthMethod: async (advertised) => {
      const ids = new Set(advertised.map((method) => method.id));
      if (apiKeyEnabled && ids.has("xai.api_key")) return "xai.api_key";
      if (ids.has("cached_token")) return "cached_token";
      const interactive = advertised.find(
        (method): method is typeof method & { id: "grok.com" | "oidc" } =>
          method.id === "grok.com" || method.id === "oidc",
      );
      if (interactive === undefined) return undefined;
      const decision =
        (await options.authorizeLogin?.({
          methodId: interactive.id,
          kind: "external-browser",
          state: "login-required",
          ...(interactive.name === undefined ? {} : { label: interactive.name }),
        })) ?? "cancel";
      return decision === "continue" ? interactive.id : undefined;
    },
    authenticateMeta: { headless: true },
    createTransport: async () => {
      clientCapabilities.fs.readTextFile = false;
      clientCapabilities.fs.writeTextFile = false;
      clientCapabilities.terminal = false;
      extensionMethods.splice(0);
      const transport =
        options.createTransport === undefined
          ? await createAdmittedAcpSessionTransport(admission, {
              cwd: authorizedWorkspace,
              environment,
              ...(options.limits === undefined ? {} : { limits: options.limits }),
              methodKinds: extensionsEligible
                ? GROK_ACP_PROFILE.methods.map((entry) => ({
                    method: entry.method,
                    kind: entry.kind,
                  }))
                : [],
            })
          : await options.createTransport(admission);
      const disposers: Array<() => void> = [];
      let cleaned = false;
      const cleanup = (): void => {
        if (cleaned) return;
        cleaned = true;
        for (const dispose of disposers.toReversed()) {
          try {
            dispose();
          } catch {
            // Every scoped registration still gets a best-effort teardown.
          }
        }
      };
      try {
        if (authorityEnabled) {
          const methods = new Set<string>();
          const dispose = authority.install(instrumentRegistrations(transport, methods));
          if (typeof dispose === "function") disposers.push(dispose);
          clientCapabilities.fs.readTextFile =
            admission.grants.fsReadTextFile && methods.has("fs/read_text_file");
          clientCapabilities.fs.writeTextFile =
            admission.grants.fsWriteTextFile && methods.has("fs/write_text_file");
          clientCapabilities.terminal =
            admission.grants.terminal &&
            [
              "terminal/create",
              "terminal/output",
              "terminal/release",
              "terminal/wait_for_exit",
              "terminal/kill",
            ].every((method) => methods.has(method));
        }
        if (extensionsEligible) {
          const methods = new Set<string>();
          const dispose = options.installVendorHandlers(
            instrumentRegistrations(transport, methods),
          );
          if (typeof dispose === "function") disposers.push(dispose);
          if (GROK_ACP_PROFILE.methods.every((entry) => methods.has(entry.method)))
            extensionMethods.push(...GROK_ACP_PROFILE.methods.map((entry) => entry.method));
        }
      } catch (error) {
        cleanup();
        await transport.dispose().catch(() => undefined);
        throw error;
      }
      return {
        get generation() {
          return transport.generation;
        },
        get state() {
          return transport.state;
        },
        request: (method, params, requestOptions) =>
          transport.request(method, params, requestOptions),
        notify: (method, params) => transport.notify(method, params),
        onNotification: (method, handler) => transport.onNotification(method, handler),
        cancelReverseRequests: (sessionId) => transport.cancelReverseRequests(sessionId),
        drainAcceptedInbound: (maxTurns) => transport.drainAcceptedInbound(maxTurns),
        waitForExit: () => transport.waitForExit(),
        shutdown: async (sessionIds) => {
          cleanup();
          await transport.shutdown(sessionIds);
        },
        dispose: async () => {
          cleanup();
          await transport.dispose();
        },
      };
    },
    ...(options.materializeMcp === undefined ? {} : { materializeMcp: options.materializeMcp }),
    ...(options.onUpdate === undefined ? {} : { onUpdate: options.onUpdate }),
    ...(options.settleTurn === undefined ? {} : { settleTurn: options.settleTurn }),
    ...(options.requestTimeoutMs === undefined
      ? {}
      : { requestTimeoutMs: options.requestTimeoutMs }),
  });
  const workspaceAllowed = async (cwd: string): Promise<boolean> => {
    if (!isAbsolute(cwd)) return false;
    try {
      return (await realpath(resolve(cwd))) === authorizedWorkspace;
    } catch {
      return false;
    }
  };
  const workspaceFailure = (method: string) =>
    Promise.resolve({
      ok: false as const,
      reason: "invalid_value" as const,
      safeDetail: "Grok workspace is outside the authorized runtime root",
      receipt: {
        at: new Date().toISOString(),
        runtimeGeneration: runtime.evidence?.runtimeGeneration ?? 0,
        method,
        outcome: "invalid_value" as const,
        evidenceRefs: [] as ReadonlyArray<string>,
      },
    });
  const activePrompts = new Map<string, ReturnType<AcpSessionRuntime["prompt"]>>();
  const facade: GrokAcpPeerRuntime = {
    admission,
    evidence: () => runtime.evidence,
    receipts: () => runtime.receipts(),
    sessions: () => runtime.sessions(),
    start: () => runtime.start(),
    async newSession(input) {
      if (!(await workspaceAllowed(input.cwd))) return workspaceFailure("session/new");
      return runtime.newSession({ ...input, cwd: authorizedWorkspace });
    },
    loadSession: async (input) =>
      (await workspaceAllowed(input.cwd))
        ? runtime.loadSession({ ...input, cwd: authorizedWorkspace })
        : workspaceFailure("session/load"),
    resumeSession: async (input) =>
      (await workspaceAllowed(input.cwd))
        ? runtime.resumeSession({ ...input, cwd: authorizedWorkspace })
        : workspaceFailure("session/resume"),
    listSessions: (input) => runtime.listSessions(input),
    closeSession: (sessionId) => runtime.closeSession(sessionId),
    deleteSession: (sessionId) => runtime.deleteSession(sessionId),
    logout: () => runtime.logout(),
    prompt: (sessionId, prompt) => {
      const pending = runtime.prompt(sessionId, prompt);
      activePrompts.set(sessionId, pending);
      void pending.then(
        () => {
          if (activePrompts.get(sessionId) === pending) activePrompts.delete(sessionId);
        },
        () => {
          if (activePrompts.get(sessionId) === pending) activePrompts.delete(sessionId);
        },
      );
      return pending;
    },
    async cancel(sessionId, source) {
      const pending = activePrompts.get(sessionId);
      const outcome = await runtime.cancel(sessionId, source, { abortLocal: false });
      if (!outcome.ok) return outcome;
      if (pending === undefined) return outcome;
      let graceTimer: ReturnType<typeof setTimeout> | undefined;
      const confirmed = await Promise.race([
        pending.then(
          (result) =>
            result.ok &&
            result.value.terminal === "cancelled" &&
            result.value.stopReason === "cancelled",
          () => false,
        ),
        new Promise<false>(
          (resolveWait) =>
            (graceTimer = setTimeout(() => resolveWait(false), options.cancelGraceMs ?? 1_500)),
        ),
      ]);
      if (graceTimer !== undefined) clearTimeout(graceTimer);
      if (!confirmed) {
        await runtime.cancel(sessionId, source);
        await runtime.shutdown();
      }
      return outcome;
    },
    setMode: (sessionId, modeId) => runtime.setMode(sessionId, modeId),
    setConfigOption: (sessionId, configId, value) =>
      runtime.setConfigOption(sessionId, configId, value),
    recover: async (input, recoverOptions) =>
      (await workspaceAllowed(input.cwd))
        ? runtime.recover({ ...input, cwd: authorizedWorkspace }, recoverOptions)
        : workspaceFailure("recover"),
    shutdown: () => runtime.shutdown(),
  };
  return Object.freeze(facade);
};
