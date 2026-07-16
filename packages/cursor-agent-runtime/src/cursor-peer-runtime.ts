import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { constants, createReadStream } from "node:fs";
import { access, lstat, readdir, realpath } from "node:fs/promises";
import { basename, delimiter, dirname, isAbsolute, join, relative, resolve } from "node:path";
import { promisify } from "node:util";

import {
  CURSOR_ACP_PROFILE,
  decodeCursorListAvailableModelsResponse,
  type CursorListAvailableModelsResponse,
} from "@openagentsinc/agent-client-protocol/extensions/cursor";
import {
  admitAcpPeerProfile,
  createDefaultAcpTrustedPeerProfileRegistry,
  type AcpConformanceEvidenceRecord,
  type AcpExecutableProbe,
  type AcpPeerAdmissionDecision,
} from "@openagentsinc/agent-client-protocol/profiles";
import {
  AcpSessionRuntime,
  classifyAcpLifecycleFailure,
  type AcpLifecycleOutcome,
  type AcpLifecycleFailure,
  type AcpMcpReference,
  type AcpRuntimeEvidence,
  type AcpSessionRuntimeOptions,
  type AcpSessionTransportPort,
  type AcpSessionUpdateRecord,
} from "@openagentsinc/agent-client-runtime-bridge";
import {
  AgentStdioTransport,
  type AgentStdioReverseHandler,
  type AgentStdioTransportLimits,
} from "@openagentsinc/agent-stdio-transport";

const execFileAsync = promisify(execFile);

export type CursorAcpTransport = AcpSessionTransportPort &
  Readonly<{
    registerReverseHandler(method: string, handler: AgentStdioReverseHandler): () => void;
  }>;

export type CursorAuthInteraction = Readonly<{
  methodId: "cursor_login";
  kind: "external-browser";
  state: "login-required";
}>;

export type CursorModelDiscovery = Readonly<{
  provenance: "cursor/list_available_models";
  profileVersion: 1;
  runtimeGeneration: number;
  response: CursorListAvailableModelsResponse;
  models: ReadonlyArray<
    Readonly<{
      value: string;
      name: string;
      configOptions?: CursorListAvailableModelsResponse["models"][number]["configOptions"];
      sources: ReadonlyArray<"stable-config" | "cursor/list_available_models">;
    }>
  >;
}>;

export type CreateCursorAcpPeerRuntimeOptions = Readonly<{
  cwd: string;
  environment?: Readonly<Record<string, string | undefined>>;
  evidence?: ReadonlyArray<AcpConformanceEvidenceRecord>;
  now?: Date;
  authorizeLogin?: (interaction: CursorAuthInteraction) => Promise<"continue" | "cancel">;
  authority?: Readonly<{
    install(transport: CursorAcpTransport): void | (() => void);
  }>;
  installVendorHandlers?: (transport: CursorAcpTransport) => void | (() => void);
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
  ) => Promise<CursorAcpTransport>;
}>;

const findCursorOnPath = async (pathValue: string | undefined): Promise<string> => {
  for (const directory of (pathValue ?? "").split(delimiter)) {
    if (directory.length === 0) continue;
    const candidate = resolve(directory, "agent");
    try {
      await access(candidate, constants.X_OK);
      const target = await realpath(candidate);
      if (basename(target) === "cursor-agent") return target;
    } catch {
      // Continue through the bounded PATH list.
    }
  }
  throw Object.assign(new Error("Cursor Agent CLI is not installed or not on PATH"), {
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

const sha256InstallationClosure = async (root: string): Promise<string> => {
  const files: string[] = [];
  const visit = async (directory: string): Promise<void> => {
    for (const entry of (await readdir(directory, { withFileTypes: true })).toSorted((a, b) =>
      a.name.localeCompare(b.name),
    )) {
      const path = join(directory, entry.name);
      if (entry.isDirectory()) await visit(path);
      else if (entry.isFile()) files.push(path);
      else if (entry.isSymbolicLink() && directory === root && entry.name === "agent") continue;
      else throw new Error("Cursor installation closure contains an unsupported link or device");
    }
  };
  await visit(root);
  const aggregate = createHash("sha256");
  for (const path of files) {
    const metadata = await lstat(path);
    aggregate.update(relative(root, path));
    aggregate.update("\0");
    aggregate.update(String(metadata.mode & 0o777));
    aggregate.update("\0");
    aggregate.update(await sha256File(path));
    aggregate.update("\0");
  }
  return aggregate.digest("hex");
};

export const probeCursorAcpExecutable = async (
  environment: Readonly<Record<string, string | undefined>> = process.env,
  candidatePath?: string,
): Promise<AcpExecutableProbe> => {
  if (candidatePath !== undefined && !isAbsolute(candidatePath)) {
    throw Object.assign(new Error("Alternate Cursor executable path must be absolute"), { kind: "identity_mismatch" });
  }
  const realPath = candidatePath === undefined ? await findCursorOnPath(environment.PATH) : await realpath(candidatePath);
  if (basename(realPath) !== "cursor-agent") {
    throw Object.assign(new Error("Alternate Cursor executable must resolve to cursor-agent"), { kind: "identity_mismatch" });
  }
  const resolvedPath = realPath;
  const [{ stdout, stderr }, sha256, closureSha256] = await Promise.all([
    execFileAsync(realPath, ["--version"], { timeout: 5_000, maxBuffer: 16_384 }),
    sha256File(realPath),
    sha256InstallationClosure(dirname(realPath)),
  ]);
  const reportedVersion = `${stdout}${stderr}`.trim().slice(0, 256);
  if (reportedVersion.length === 0) throw new Error("Cursor version probe returned no version");
  return Object.freeze({
    requestedExecutable: "agent",
    resolvedPath,
    realPath,
    sha256,
    closureSha256,
    reportedVersion,
    platform: Object.freeze({ os: process.platform, arch: process.arch }),
  });
};

export const admitCursorAcpPeer = async (
  options: Pick<
    CreateCursorAcpPeerRuntimeOptions,
    "environment" | "evidence" | "now" | "probe"
  > = {},
): Promise<Extract<AcpPeerAdmissionDecision, { _tag: "PeerAdmitted" }>> => {
  const registry = createDefaultAcpTrustedPeerProfileRegistry();
  if (registry._tag !== "RegistryReady") throw new Error(registry.detail);
  const probe = options.probe ?? (await probeCursorAcpExecutable(options.environment));
  const decision = admitAcpPeerProfile({
    registry: registry.registry,
    profileId: "cursor-agent",
    probe,
    evidence: options.evidence ?? [],
    now: options.now ?? new Date(),
  });
  if (decision._tag !== "PeerAdmitted")
    throw Object.assign(new Error(decision.detail), { kind: decision.reason });
  return decision;
};

const instrumentRegistrations = (
  transport: CursorAcpTransport,
  methods: Set<string>,
): CursorAcpTransport =>
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
      if (property === "onNotification")
        return (method: string, handler: (params: unknown) => void) => {
          const unregister = target.onNotification(method, handler);
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

export type CursorAcpPeerRuntime = Readonly<{
  admission: Extract<AcpPeerAdmissionDecision, { _tag: "PeerAdmitted" }>;
  evidence(): AcpRuntimeEvidence | undefined;
  receipts: AcpSessionRuntime["receipts"];
  sessions: AcpSessionRuntime["sessions"];
  start(): Promise<AcpLifecycleOutcome<AcpRuntimeEvidence>>;
  newSession(input: {
    cwd: string;
    canonicalThreadSeed: string;
    mcpRefs?: ReadonlyArray<AcpMcpReference>;
    scopeRef?: string;
  }): ReturnType<AcpSessionRuntime["newSession"]>;
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
  listAvailableModels(signal?: AbortSignal): Promise<AcpLifecycleOutcome<CursorModelDiscovery>>;
  recover: AcpSessionRuntime["recover"];
  shutdown: AcpSessionRuntime["shutdown"];
}>;

export const createCursorAcpPeerRuntime = async (
  options: CreateCursorAcpPeerRuntimeOptions,
): Promise<CursorAcpPeerRuntime> => {
  const admission = options.admission ?? (await admitCursorAcpPeer(options));
  if (!isAbsolute(options.cwd)) throw new TypeError("Cursor process cwd must be absolute");
  const authorizedWorkspace = await realpath(resolve(options.cwd));
  const sourceEnvironment = options.environment ?? process.env;
  const environment = Object.freeze({
    ...(typeof sourceEnvironment.HOME === "string" ? { HOME: sourceEnvironment.HOME } : {}),
    // Cursor's signed launcher invokes only these base-system utilities before
    // execing its sibling Node binary. Never forward the caller's PATH.
    PATH: "/usr/bin:/bin",
  });
  const hasLiveFeatureEvidence = (suiteId: string): boolean =>
    (options.evidence ?? []).some((record) => {
      const ageMs = (options.now ?? new Date()).getTime() - Date.parse(record.recordedAt);
      return (
        record.suiteId === suiteId &&
        record.kind === "live" &&
        record.result === "pass" &&
        record.peerVersion === admission.peerVersion &&
        record.executableSha256 === admission.identityPin.sha256 &&
        record.installationClosureSha256 === admission.identityPin.closureSha256 &&
        Number.isFinite(ageMs) &&
        ageMs >= 0 &&
        ageMs <= 30 * 86_400_000
      );
    });
  const authorityEnabled =
    admission.supportState === "supported" &&
    hasLiveFeatureEvidence("cursor-authority-reverse") &&
    options.authority !== undefined;
  const extensionsEnabled =
    admission.supportState === "supported" &&
    hasLiveFeatureEvidence("cursor-vendor-extensions") &&
    options.installVendorHandlers !== undefined &&
    CURSOR_ACP_PROFILE.methods.every((entry) =>
      admission.grants.vendorExtensionMethods.includes(entry.method),
    );
  const modelsEnabled = extensionsEnabled && hasLiveFeatureEvidence("cursor-model-discovery");
  const parameterizedModelPicker = hasLiveFeatureEvidence("cursor-parameterized-model-picker");
  const clientCapabilities = {
    fs: { readTextFile: false, writeTextFile: false },
    terminal: false,
    ...(parameterizedModelPicker
      ? { _meta: { parameterizedModelPicker: true } as Readonly<Record<string, unknown>> }
      : {}),
  };
  const extensionMethods: string[] = [];
  let activeTransport: CursorAcpTransport | undefined;
  const runtime = new AcpSessionRuntime({
    profile: "cursor",
    extensionMethods,
    clientCapabilities,
    clientInfo: { name: "openagents", title: "OpenAgents", version: "0.1.0" },
    peerIdentityFallback: { name: "cursor-agent", version: admission.peerVersion },
    expectedPeerIdentity: { namePrefix: "cursor", version: admission.peerVersion },
    selectAuthMethod: async (advertised) => {
      if (!advertised.some((method) => method.id === "cursor_login")) return undefined;
      const decision =
        (await options.authorizeLogin?.({
          methodId: "cursor_login",
          kind: "external-browser",
          state: "login-required",
        })) ?? "cancel";
      return decision === "continue" ? "cursor_login" : undefined;
    },
    createTransport: async () => {
      clientCapabilities.fs.readTextFile = false;
      clientCapabilities.fs.writeTextFile = false;
      clientCapabilities.terminal = false;
      extensionMethods.splice(0);
      activeTransport = undefined;
      const transport =
        options.createTransport === undefined
          ? await (async () => {
              if (admission.identityPin.closureSha256 === undefined)
                throw new Error("Cursor installation closure is not pinned");
              const currentClosure = await sha256InstallationClosure(
                dirname(admission.identityPin.realPath),
              );
              if (currentClosure !== admission.identityPin.closureSha256)
                throw Object.assign(
                  new Error("Cursor installation closure changed after admission"),
                  {
                    kind: "path_replacement",
                  },
                );
              return AgentStdioTransport.start({
                executable: admission.identityPin.realPath,
                args: admission.launchPlan.args,
                versionProbeArgs: admission.launchPlan.versionProbeArgs,
                env: environment,
                identityPin: admission.identityPin,
                cwd: authorizedWorkspace,
                ...(options.limits === undefined ? {} : { limits: options.limits }),
                methodKinds: extensionsEnabled
                  ? CURSOR_ACP_PROFILE.methods
                      .filter((entry) => entry.direction === "agent-to-client")
                      .map((entry) => ({ method: entry.method, kind: entry.kind }))
                  : [],
              });
            })()
          : await options.createTransport(admission);
      const disposers: Array<() => void> = [];
      const cleanup = (): void => {
        activeTransport = undefined;
        for (const dispose of disposers.splice(0).toReversed()) {
          try {
            dispose();
          } catch {
            // Complete all scoped cleanup even if one handler is faulty.
          }
        }
      };
      try {
        if (authorityEnabled) {
          const methods = new Set<string>();
          const dispose = options.authority.install(instrumentRegistrations(transport, methods));
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
        if (extensionsEnabled) {
          const methods = new Set<string>();
          const dispose = options.installVendorHandlers(
            instrumentRegistrations(transport, methods),
          );
          if (typeof dispose === "function") disposers.push(dispose);
          const inbound = CURSOR_ACP_PROFILE.methods.filter(
            (entry) => entry.direction === "agent-to-client",
          );
          if (inbound.every((entry) => methods.has(entry.method))) {
            extensionMethods.push(...inbound.map((entry) => entry.method));
            if (modelsEnabled) extensionMethods.push("cursor/list_available_models");
          }
        }
        activeTransport = transport;
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
  const failure = (method: string, reason: AcpLifecycleFailure, detail: string) => ({
    ok: false as const,
    reason,
    safeDetail: detail,
    receipt: {
      at: new Date().toISOString(),
      runtimeGeneration: runtime.evidence?.runtimeGeneration ?? 0,
      method,
      outcome: reason,
      evidenceRefs: [] as ReadonlyArray<string>,
    },
  });
  const activePrompts = new Map<string, ReturnType<AcpSessionRuntime["prompt"]>>();
  return Object.freeze({
    admission,
    evidence: () => runtime.evidence,
    receipts: () => runtime.receipts(),
    sessions: () => runtime.sessions(),
    start: () => runtime.start(),
    async newSession(input) {
      if (!(await workspaceAllowed(input.cwd)))
        return failure(
          "session/new",
          "invalid_value",
          "Cursor workspace is outside the authorized root",
        );
      return runtime.newSession({ ...input, cwd: authorizedWorkspace });
    },
    loadSession: async (input) =>
      (await workspaceAllowed(input.cwd))
        ? runtime.loadSession({ ...input, cwd: authorizedWorkspace })
        : failure(
            "session/load",
            "invalid_value",
            "Cursor workspace is outside the authorized root",
          ),
    resumeSession: async (input) =>
      (await workspaceAllowed(input.cwd))
        ? runtime.resumeSession({ ...input, cwd: authorizedWorkspace })
        : failure(
            "session/resume",
            "invalid_value",
            "Cursor workspace is outside the authorized root",
          ),
    listSessions: (input) => runtime.listSessions(input),
    closeSession: (sessionId) => runtime.closeSession(sessionId),
    deleteSession: (sessionId) => runtime.deleteSession(sessionId),
    logout: () => runtime.logout(),
    prompt: (sessionId, prompt) => {
      const pending = runtime.prompt(sessionId, prompt);
      activePrompts.set(sessionId, pending);
      void pending.finally(() => {
        if (activePrompts.get(sessionId) === pending) activePrompts.delete(sessionId);
      });
      return pending;
    },
    async cancel(sessionId, source) {
      const pending = activePrompts.get(sessionId);
      const outcome = await runtime.cancel(sessionId, source, { abortLocal: false });
      if (!outcome.ok || pending === undefined) return outcome;
      let timer: ReturnType<typeof setTimeout> | undefined;
      const confirmed = await Promise.race([
        pending.then(
          (result) =>
            result.ok &&
            result.value.terminal === "cancelled" &&
            result.value.stopReason === "cancelled",
          () => false,
        ),
        new Promise<false>((resolveWait) => {
          timer = setTimeout(() => resolveWait(false), options.cancelGraceMs ?? 1_500);
        }),
      ]);
      if (timer !== undefined) clearTimeout(timer);
      if (!confirmed) {
        await runtime.cancel(sessionId, source);
        await runtime.shutdown();
        return failure(
          "session/cancel",
          "protocol_failure",
          "Cursor did not confirm cancellation before the peer was stopped",
        );
      }
      return outcome;
    },
    setMode: (sessionId, modeId) => runtime.setMode(sessionId, modeId),
    setConfigOption: (sessionId, configId, value) =>
      runtime.setConfigOption(sessionId, configId, value),
    async listAvailableModels(signal) {
      const method = "cursor/list_available_models";
      const evidence = runtime.evidence;
      const transport = activeTransport;
      if (
        !modelsEnabled ||
        evidence === undefined ||
        !evidence.extensionMethods.includes(method) ||
        transport === undefined ||
        transport.generation !== evidence.runtimeGeneration
      )
        return failure(
          method,
          "unsupported",
          "Cursor model discovery is not proven for this peer generation",
        );
      try {
        const raw = await transport.request(
          method,
          {},
          {
            ...(options.requestTimeoutMs === undefined
              ? {}
              : { timeoutMs: options.requestTimeoutMs }),
            ...(signal === undefined ? {} : { signal }),
          },
        );
        const response = decodeCursorListAvailableModelsResponse(raw);
        if (response === undefined)
          return failure(
            method,
            "invalid_value",
            "Cursor returned an invalid bounded model response",
          );
        const merged = new Map<
          string,
          {
            value: string;
            name: string;
            configOptions?: CursorListAvailableModelsResponse["models"][number]["configOptions"];
            sources: Array<"stable-config" | "cursor/list_available_models">;
          }
        >();
        for (const model of response.models)
          merged.set(model.value, {
            ...model,
            sources: ["cursor/list_available_models"],
          });
        for (const session of runtime.sessions()) {
          for (const option of session.configOptions) {
            if (
              (option.category !== "model" && !option.id.toLowerCase().includes("model")) ||
              !("options" in option) ||
              !Array.isArray(option.options)
            )
              continue;
            for (const stableModel of option.options) {
              const existing = merged.get(stableModel.value);
              if (existing === undefined)
                merged.set(stableModel.value, {
                  value: stableModel.value,
                  name: stableModel.name,
                  sources: ["stable-config"],
                });
              else if (!existing.sources.includes("stable-config"))
                existing.sources.push("stable-config");
            }
          }
        }
        const models = Object.freeze(
          [...merged.values()]
            .toSorted((left, right) => left.value.localeCompare(right.value))
            .map((model) =>
              Object.freeze({ ...model, sources: Object.freeze([...model.sources]) }),
            ),
        );
        return {
          ok: true,
          value: Object.freeze({
            provenance: method,
            profileVersion: 1,
            runtimeGeneration: evidence.runtimeGeneration,
            response,
            models,
          }),
          receipt: {
            at: new Date().toISOString(),
            runtimeGeneration: evidence.runtimeGeneration,
            method,
            outcome: "succeeded" as const,
            evidenceRefs: [],
          },
        };
      } catch (error) {
        const reason = signal?.aborted ? "cancelled" : classifyAcpLifecycleFailure(error);
        const detail: Partial<Record<AcpLifecycleFailure, string>> = {
          cancelled: "Cursor model discovery was cancelled",
          timed_out: "Cursor model discovery timed out",
          auth_lost: "Cursor authentication was lost during model discovery",
          process_exit: "Cursor exited during model discovery",
          refused: "Cursor refused model discovery",
          protocol_failure: "Cursor model discovery failed at the protocol boundary",
        };
        return failure(method, reason, detail[reason] ?? "Cursor model discovery failed");
      }
    },
    recover: async (input, recoverOptions) =>
      (await workspaceAllowed(input.cwd))
        ? runtime.recover({ ...input, cwd: authorizedWorkspace }, recoverOptions)
        : failure("recover", "invalid_value", "Cursor workspace is outside the authorized root"),
    shutdown: () => runtime.shutdown(),
  });
};
