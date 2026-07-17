/** Main-only Grok/Cursor Agent Client Protocol host (#8895). */
import { randomUUID } from "node:crypto";

import {
  admitCursorAcpPeer,
  createCursorAcpPeerRuntime,
  probeCursorAcpExecutable,
  type CursorAcpPeerRuntime,
} from "@openagentsinc/cursor-agent-runtime";
import {
  admitGrokAcpPeer,
  createGrokAcpPeerRuntime,
  probeGrokAcpExecutable,
  type GrokAcpPeerRuntime,
} from "@openagentsinc/grok-harness";
import type {
  AcpConformanceEvidenceRecord,
  AcpExecutableProbe,
  AcpPeerAdmissionDecision,
} from "@openagentsinc/agent-client-protocol/profiles";
import { checkedAcpReleaseEvidence } from "@openagentsinc/agent-client-protocol-conformance";
import type {
  AcpProjectionEvent,
  AcpLifecycleOutcome,
  AcpRuntimeEvidence,
  AcpSessionUpdateRecord,
  AcpSessionSnapshot,
} from "@openagentsinc/agent-client-runtime-bridge";
import {
  AcpRuntimeProjector,
  bindAcpSession,
  createAcpRuntimeNativeEnvelope,
  createBoundedAcpNativeEvidenceStore,
} from "@openagentsinc/agent-client-runtime-bridge";

import {
  availableAcpProviderActions,
  buildAcpSupportBundle,
  type AcpProviderAction,
  type AcpProviderId,
  type AcpProviderProjection,
  type AcpSupportBundle,
} from "./acp-provider-contract.ts";
import type { AcpProviderLaneDriver } from "./provider-lane-acp.ts";

type Admitted = Extract<AcpPeerAdmissionDecision, { _tag: "PeerAdmitted" }>;
type PeerRuntime = GrokAcpPeerRuntime | CursorAcpPeerRuntime;

export type AcpProviderHostDependencies = Readonly<{
  cwd: () => Promise<string>;
  now?: () => Date;
  probeGrok?: (candidatePath?: string) => Promise<AcpExecutableProbe>;
  probeCursor?: (candidatePath?: string) => Promise<AcpExecutableProbe>;
  admitGrok?: (probe: AcpExecutableProbe) => Promise<Admitted>;
  admitCursor?: (probe: AcpExecutableProbe) => Promise<Admitted>;
  createGrok?: (
    cwd: string,
    admission: Admitted,
    onUpdate: (record: AcpSessionUpdateRecord) => void | Promise<void>,
  ) => Promise<GrokAcpPeerRuntime>;
  createCursor?: (
    cwd: string,
    admission: Admitted,
    onAuth: (state: "pending" | "cancelled") => void,
    onUpdate: (record: AcpSessionUpdateRecord) => void | Promise<void>,
  ) => Promise<CursorAcpPeerRuntime>;
  chooseExecutable?: (provider: AcpProviderId) => Promise<string | null>;
  loadAlternatePaths?: () => Promise<Partial<Record<AcpProviderId, string>>>;
  saveAlternatePath?: (provider: AcpProviderId, path: string) => Promise<void>;
}>;

type Slot = {
  projection: AcpProviderProjection;
  admission?: Admitted;
  runtime?: PeerRuntime;
  receiptRefs: string[];
  evidenceRefs: string[];
};

const initialProjection = (provider: AcpProviderId): AcpProviderProjection => ({
  provider,
  displayName: provider === "grok" ? "Grok CLI" : "Cursor Agent CLI",
  profileRef: provider === "grok" ? "grok-cli" : "cursor-agent",
  protocol: "Agent Client Protocol",
  install: "checking",
  executable: { source: "trusted-path", displayPath: null },
  version: null,
  profileState: "unprobed",
  auth: { state: "unknown", advertisedMethods: [], safeLogout: false },
  probe: { state: "not_run", code: null, observedAt: null },
  session: {
    state: "none",
    sessionRef: null,
    processRef: null,
    canNew: false,
    canList: false,
    canLoad: false,
    canResume: false,
    canCancel: false,
  },
  capabilities: { filesystem: false, terminal: false, permissions: false, questions: false },
  configuration: [],
  diagnosticCodes: [],
  conformanceRef: null,
});

const safeCode = (error: unknown): string => {
  const candidate =
    typeof error === "object" && error !== null && "kind" in error
      ? String((error as { kind?: unknown }).kind)
      : "probe_failed";
  return /^[A-Za-z0-9][A-Za-z0-9._:-]{0,119}$/.test(candidate) ? candidate : "probe_failed";
};

export const defaultGrokDesktopAuthOptions = (
  environment: Readonly<Record<string, string | undefined>> = process.env,
): Readonly<{
  environment: Readonly<{ HOME?: string }>;
  apiKeyConfigured: false;
}> => ({
  environment: Object.freeze({
    ...(typeof environment.HOME === "string" ? { HOME: environment.HOME } : {}),
  }),
  apiKeyConfigured: false,
});

const receiptRef = (provider: AcpProviderId, index: number): string =>
  `acp.${provider}.receipt.${index}`;

export const createAcpProviderHost = (dependencies: AcpProviderHostDependencies) => {
  const now = dependencies.now ?? (() => new Date());
  const releaseEvidence = (): Readonly<{
    diagnostics: ReadonlyArray<string>;
    peers: Readonly<Record<AcpProviderId, ReadonlyArray<AcpConformanceEvidenceRecord>>>;
  }> => {
    const compiled = checkedAcpReleaseEvidence({ now: now() });
    return compiled._tag === "ReleaseEvidenceReady"
      ? { diagnostics: [], peers: compiled.evidence }
      : {
          diagnostics: compiled.diagnostics,
          peers: { grok: [], cursor: [] },
        };
  };
  const slots: Record<AcpProviderId, Slot> = {
    grok: { projection: initialProjection("grok"), receiptRefs: [], evidenceRefs: [] },
    cursor: { projection: initialProjection("cursor"), receiptRefs: [], evidenceRefs: [] },
  };
  const alternatePaths: Partial<Record<AcpProviderId, string>> = {};
  const nativeEvidence = createBoundedAcpNativeEvidenceStore({
    maxEntries: 2_048,
    maxBytes: 16 * 1_048_576,
  });
  const activeTurns = new Map<
    AcpProviderId,
    Readonly<{
      turnRef: string;
      sessionRef: string;
      connectionRef: string;
      processGeneration: number;
      projector: AcpRuntimeProjector;
      emit: (event: AcpProjectionEvent) => void;
    }>
  >();
  const interruptibleTurns = new Map<
    string,
    Readonly<{
      provider: AcpProviderId;
      sessionRef: string;
    }>
  >();

  const projectRuntimeUpdate = async (
    provider: AcpProviderId,
    record: AcpSessionUpdateRecord,
  ): Promise<void> => {
    const active = activeTurns.get(provider);
    if (active === undefined || record.sessionId !== active.sessionRef) return;
    const update =
      typeof record.update === "object" && record.update !== null
        ? (record.update as Record<string, unknown>)
        : {};
    const envelope = createAcpRuntimeNativeEnvelope({
      profile: provider,
      protocolVersion: 1,
      connectionRef: active.connectionRef,
      processGeneration: active.processGeneration,
      method: "session/update",
      updateId: `${active.processGeneration}-${record.sequence}`,
      sessionId: record.sessionId,
      observedAt: now().toISOString(),
      discriminant:
        typeof update.sessionUpdate === "string" ? update.sessionUpdate : "unknown_session_update",
      validatedPayload: update,
      validationStatus: record.disposition === "applied" ? "validated" : "decode-failure",
    });
    if ("kind" in envelope) return;
    const projected = await active.projector.apply({ envelope, sequence: record.sequence });
    for (const event of projected.events) active.emit(event);
  };

  const projectRuntime = (provider: AcpProviderId): void => {
    const slot = slots[provider];
    const evidence: AcpRuntimeEvidence | undefined = slot.runtime?.evidence();
    const session: AcpSessionSnapshot | undefined = slot.runtime?.sessions().at(-1);
    if (evidence !== undefined) {
      slot.evidenceRefs = [
        `acp.${provider}.runtime.${evidence.runtimeGeneration}`,
        ...evidence.extensionMethods.map((_, index) => `acp.${provider}.extension.${index}`),
      ];
    }
    slot.projection = {
      ...slot.projection,
      auth:
        evidence === undefined
          ? slot.projection.auth
          : {
              state: "authenticated",
              advertisedMethods: evidence.authMethodIds.filter(
                (id): id is "cached_token" | "xai.api_key" | "cursor_login" =>
                  id === "cached_token" || id === "xai.api_key" || id === "cursor_login",
              ),
              safeLogout: evidence.capabilities.logout,
            },
      session: {
        state:
          session === undefined
            ? slot.projection.session.state
            : session.promptActive
              ? "running"
              : session.phase === "replay"
                ? "replaying"
                : session.phase === "closed"
                  ? "none"
                  : "running",
        sessionRef: session?.threadId ?? null,
        processRef: evidence?.connectionRef ?? null,
        canNew: evidence !== undefined,
        canList: evidence?.capabilities.list ?? false,
        canLoad: evidence?.capabilities.load ?? false,
        canResume: evidence?.capabilities.resume ?? false,
        canCancel: session?.promptActive ?? false,
      },
      capabilities: {
        filesystem: false,
        terminal: false,
        permissions: evidence !== undefined,
        questions:
          evidence?.extensionMethods.some(
            (method) => method.includes("ask") || method.includes("question"),
          ) ?? false,
      },
      configuration:
        session === undefined
          ? []
          : [
              ...(session.modes === undefined
                ? []
                : [
                    {
                      id: "mode",
                      label: "Mode",
                      value: session.modes.currentModeId,
                      provenance: "stable" as const,
                      state: "reconciled" as const,
                    },
                  ]),
              ...session.configOptions.flatMap((option, index) => {
                const candidate = option as unknown as {
                  id?: unknown;
                  name?: unknown;
                  currentValue?: unknown;
                };
                if (typeof candidate.id !== "string") return [];
                return [
                  {
                    id: candidate.id.slice(0, 120),
                    label:
                      typeof candidate.name === "string"
                        ? candidate.name.slice(0, 200)
                        : candidate.id.slice(0, 200),
                    value:
                      typeof candidate.currentValue === "string"
                        ? candidate.currentValue.slice(0, 200)
                        : null,
                    provenance: "stable" as const,
                    state: "reconciled" as const,
                  },
                ];
              }),
            ].slice(0, 64),
    };
  };

  const probe = async (provider: AcpProviderId): Promise<void> => {
    const slot = slots[provider];
    try {
      const checkedEvidence = releaseEvidence();
      const evidence = checkedEvidence.peers[provider];
      const candidatePath = alternatePaths[provider];
      const executable = await (provider === "grok"
        ? (
            dependencies.probeGrok ??
            ((_candidate) => probeGrokAcpExecutable(process.env, _candidate))
          )(candidatePath)
        : (
            dependencies.probeCursor ??
            ((_candidate) => probeCursorAcpExecutable(process.env, _candidate))
          )(candidatePath));
      const admission = await (provider === "grok"
        ? (
            dependencies.admitGrok ??
            ((value) => admitGrokAcpPeer({ probe: value, evidence, now: now() }))
          )(executable)
        : (
            dependencies.admitCursor ??
            ((value) => admitCursorAcpPeer({ probe: value, evidence, now: now() }))
          )(executable));
      slot.admission = admission;
      slot.projection = {
        ...slot.projection,
        install: "detected",
        executable: {
          source: candidatePath === undefined ? "trusted-path" : "validated-alternate",
          displayPath: executable.realPath.slice(0, 200),
        },
        version: admission.peerVersion,
        profileState: admission.supportState,
        probe: { state: "passed", code: null, observedAt: now().toISOString() },
        auth: { ...slot.projection.auth, state: "required" },
        diagnosticCodes: [
          ...admission.quarantinedCapabilities,
          ...(checkedEvidence.diagnostics.length > 0 ? ["release_evidence_unavailable"] : []),
        ].slice(0, 32),
        conformanceRef: admission.diagnostics.evidenceArtifactRefs.at(-1) ?? null,
      };
    } catch (error) {
      const code = safeCode(error);
      slot.admission = undefined;
      slot.projection = {
        ...initialProjection(provider),
        install: code === "missing_executable" ? "not_installed" : "checking",
        profileState: code === "missing_executable" ? "unprobed" : "incompatible",
        probe: { state: "failed", code, observedAt: now().toISOString() },
        diagnosticCodes: [code],
      };
    }
  };

  const ensureRuntime = async (provider: AcpProviderId): Promise<PeerRuntime | undefined> => {
    const slot = slots[provider];
    if (slot.runtime !== undefined) return slot.runtime;
    if (slot.admission === undefined) await probe(provider);
    if (slot.admission === undefined) return undefined;
    const cwd = await dependencies.cwd();
    const evidence = releaseEvidence().peers[provider];
    slot.projection = { ...slot.projection, auth: { ...slot.projection.auth, state: "pending" } };
    slot.runtime =
      provider === "grok"
        ? await (
            dependencies.createGrok ??
            ((root, admission, onUpdate) =>
              createGrokAcpPeerRuntime({
                cwd: root,
                admission,
                evidence,
                now: now(),
                onUpdate,
                // Desktop ACP intentionally uses Grok's existing local login. An ambient
                // shell API key is not an owner configuration decision and must not
                // override the advertised cached_token path.
                ...defaultGrokDesktopAuthOptions(),
              }))
          )(cwd, slot.admission, (record) => projectRuntimeUpdate(provider, record))
        : await (
            dependencies.createCursor ??
            ((root, admission, onAuth, onUpdate) =>
              createCursorAcpPeerRuntime({
                cwd: root,
                admission,
                evidence,
                now: now(),
                onUpdate,
                authorizeLogin: async () => {
                  onAuth("pending");
                  return "continue";
                },
              }))
          )(
            cwd,
            slot.admission,
            (state) => {
              slot.projection = { ...slot.projection, auth: { ...slot.projection.auth, state } };
            },
            (record) => projectRuntimeUpdate(provider, record),
          );
    const started = await slot.runtime.start();
    slot.receiptRefs = slot.runtime
      .receipts()
      .map((_, index) => receiptRef(provider, index))
      .slice(-128);
    if (!started.ok) {
      slot.projection = {
        ...slot.projection,
        auth: {
          ...slot.projection.auth,
          state:
            started.reason === "cancelled"
              ? "cancelled"
              : started.reason === "auth_required" || started.reason === "auth_lost"
                ? "required"
                : "failed",
        },
        diagnosticCodes: [started.reason],
      };
      return undefined;
    }
    projectRuntime(provider);
    return slot.runtime;
  };

  const action = async (provider: AcpProviderId, requested: AcpProviderAction) => {
    const slot = slots[provider];
    if (!availableAcpProviderActions(slot.projection).includes(requested)) return status();
    if (requested === "probe") await probe(provider);
    else if (requested === "select_alternate") {
      const candidate = await dependencies.chooseExecutable?.(provider);
      if (candidate !== undefined && candidate !== null) {
        alternatePaths[provider] = candidate;
        await probe(provider);
        if (slots[provider].projection.probe.state === "passed")
          await dependencies.saveAlternatePath?.(provider, candidate);
        else delete alternatePaths[provider];
      }
    } else if (requested === "authenticate" || requested === "reauthenticate")
      await ensureRuntime(provider);
    else if (requested === "logout") {
      const outcome = await slot.runtime?.logout();
      if (outcome?.ok)
        slot.projection = {
          ...slot.projection,
          auth: { ...slot.projection.auth, state: "required" },
        };
    } else if (requested === "new_session") {
      const runtime = await ensureRuntime(provider);
      if (runtime !== undefined) {
        slot.projection = {
          ...slot.projection,
          session: { ...slot.projection.session, state: "starting" },
        };
        const outcome = await runtime.newSession({
          cwd: await dependencies.cwd(),
          canonicalThreadSeed: randomUUID(),
        });
        if (!outcome.ok)
          slot.projection = {
            ...slot.projection,
            session: { ...slot.projection.session, state: "failed" },
            diagnosticCodes: [outcome.reason],
          };
        projectRuntime(provider);
      }
    } else if (requested === "cancel") {
      const session = slot.runtime?.sessions().at(-1);
      if (session !== undefined) {
        slot.projection = {
          ...slot.projection,
          session: { ...slot.projection.session, state: "cancelling" },
        };
        const outcome = await slot.runtime!.cancel(session.peerSessionId, "user");
        slot.projection = {
          ...slot.projection,
          session: {
            ...slot.projection.session,
            state: outcome.ok
              ? "none"
              : outcome.reason === "process_exit" || outcome.reason === "protocol_failure"
                ? "cancel_escalated"
                : "failed",
          },
          diagnosticCodes: outcome.ok ? [] : [outcome.reason],
        };
      }
    } else if (requested === "recover") {
      const session = slot.runtime?.sessions().at(-1);
      if (session !== undefined) {
        slot.projection = {
          ...slot.projection,
          session: { ...slot.projection.session, state: "recovering" },
        };
        const outcome: AcpLifecycleOutcome<AcpSessionSnapshot> = await slot.runtime!.recover({
          cwd: await dependencies.cwd(),
          canonicalThreadSeed: session.threadId,
          peerSessionId: session.peerSessionId,
        });
        slot.projection = {
          ...slot.projection,
          session: {
            ...slot.projection.session,
            state: outcome.ok
              ? "recovered"
              : outcome.reason === "missing_session" ||
                  outcome.reason === "restart_budget_exhausted"
                ? "non_recoverable"
                : "failed",
          },
          diagnosticCodes: outcome.ok ? [] : [outcome.reason],
        };
        projectRuntime(provider);
      }
    }
    if (slot.runtime !== undefined)
      slot.receiptRefs = slot.runtime
        .receipts()
        .map((_, index) => receiptRef(provider, index))
        .slice(-128);
    return status();
  };

  const status = () => ({
    state: "ok" as const,
    providers: [slots.grok.projection, slots.cursor.projection],
  });
  const initialize = async () => {
    Object.assign(alternatePaths, (await dependencies.loadAlternatePaths?.()) ?? {});
    await Promise.all([probe("grok"), probe("cursor")]);
    return status();
  };
  const supportBundle = (): AcpSupportBundle =>
    buildAcpSupportBundle({
      generatedAt: now().toISOString(),
      providers: (["grok", "cursor"] as const).map((provider) => ({
        projection: slots[provider].projection,
        receiptRefs: slots[provider].receiptRefs,
        evidenceRefs: slots[provider].evidenceRefs,
      })),
    });
  const driver = (provider: AcpProviderId): AcpProviderLaneDriver => ({
    runTurn: async (input) => {
      if (activeTurns.has(provider)) {
        return {
          ok: false,
          reason: "session_failed",
          detail: "This ACP provider already has an active turn.",
        };
      }
      const runtime = await ensureRuntime(provider);
      if (runtime === undefined) {
        return {
          ok: false,
          reason: "session_failed",
          detail: "Provider authentication or startup failed.",
        };
      }
      let session = runtime.sessions().find((candidate) => candidate.threadId === input.threadRef);
      if (session === undefined) {
        const created = await runtime.newSession({
          cwd: await dependencies.cwd(),
          canonicalThreadSeed: input.threadRef,
        });
        if (!created.ok) {
          return {
            ok: false,
            reason: created.reason === "timed_out" ? "timeout" : "session_failed",
            detail: created.safeDetail,
          };
        }
        session = created.value;
        projectRuntime(provider);
      }
      const evidence = runtime.evidence();
      if (evidence === undefined) {
        return {
          ok: false,
          reason: "session_failed",
          detail: "Provider runtime evidence is unavailable.",
        };
      }
      const binding = bindAcpSession({
        profile: provider,
        processGeneration: evidence.runtimeGeneration,
        connectionRef: evidence.connectionRef,
        peerSessionId: session.peerSessionId,
        canonicalThreadSeed: input.threadRef,
      });
      const projector = new AcpRuntimeProjector({
        binding,
        turnSeed: input.turnRef,
        store: nativeEvidence,
      });
      let text = "";
      const emit = (event: AcpProjectionEvent): void => {
        if (event.kind === "text.delta") text += event.text;
        input.emit(event);
      };
      activeTurns.set(provider, {
        turnRef: input.turnRef,
        sessionRef: session.peerSessionId,
        connectionRef: evidence.connectionRef,
        processGeneration: evidence.runtimeGeneration,
        projector,
        emit,
      });
      interruptibleTurns.set(input.turnRef, { provider, sessionRef: session.peerSessionId });
      emit(await projector.begin(now().toISOString()));
      try {
        const outcome = await runtime.prompt(session.peerSessionId, [
          { type: "text", text: input.message },
        ]);
        const stopReason = outcome.ok ? outcome.value.stopReason : outcome.reason;
        const settlement = createAcpRuntimeNativeEnvelope({
          profile: provider,
          protocolVersion: 1,
          connectionRef: evidence.connectionRef,
          processGeneration: evidence.runtimeGeneration,
          method: "session/prompt",
          requestId: input.turnRef,
          updateId: `prompt-result-${input.turnRef}`,
          sessionId: session.peerSessionId,
          observedAt: now().toISOString(),
          discriminant: `prompt_response/${stopReason}`,
          validatedPayload: { stopReason },
          validationStatus: "validated",
        });
        if (!("kind" in settlement)) {
          for (const event of await projector.settle(settlement, stopReason)) emit(event);
        }
        projectRuntime(provider);
        if (!outcome.ok) {
          return {
            ok: false,
            reason:
              outcome.reason === "cancelled"
                ? "interrupted"
                : outcome.reason === "timed_out"
                  ? "timeout"
                  : "session_failed",
            detail: outcome.safeDetail,
          };
        }
        return {
          ok: true,
          text,
          totalTokens: null,
          providerSessionRef: session.peerSessionId,
        };
      } finally {
        activeTurns.delete(provider);
        interruptibleTurns.delete(input.turnRef);
      }
    },
    interrupt: (turnRef) => {
      const active = interruptibleTurns.get(turnRef);
      const runtime = active === undefined ? undefined : slots[active.provider].runtime;
      if (active === undefined || runtime === undefined) return false;
      void runtime.cancel(active.sessionRef, "user");
      return true;
    },
  });
  const shutdown = async () => {
    await Promise.all(
      Object.values(slots).map((slot) => slot.runtime?.shutdown().catch(() => undefined)),
    );
  };
  return Object.freeze({ initialize, status, action, supportBundle, driver, shutdown });
};
