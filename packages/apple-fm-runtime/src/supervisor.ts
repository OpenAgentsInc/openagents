import type { AppleFmCompletionTurn, AppleFmProbe, AppleFmReadinessStatus } from "./client.js";
import type { AppleFmUsageTruth } from "./wire.js";

/**
 * `@openagentsinc/apple-fm-runtime` supervisor contract (AFS-02).
 *
 * The neutral, single-owned-session supervisor state machine with a generation
 * counter. It consumes an injected `AppleFmLauncher` boundary (the concrete
 * packaged launcher lives in the `./node` subpath) so the whole lifecycle is
 * testable with fakes and no real Apple Silicon device. Readiness is TRUE only
 * after a live `/health` probe reports ready. A crash resolves to a typed
 * failure; `dispose()` is idempotent and stops only a bridge THIS supervisor
 * launched (an adopted operator bridge is never stopped).
 *
 * It was extracted from `apps/openagents-desktop/src/apple-fm-host.ts` and made
 * UI-neutral: it emits an `AppleFmSupervisorStatus` projection with no Desktop
 * IPC schema id. Desktop keeps its Electron IPC contract and maps this neutral
 * projection into its renderer-visible status; Pylon can drive the same
 * supervisor through the same launcher boundary.
 */

/** One bounded read-only turn outcome from the launcher's client. */
export type AppleFmLauncherTurn = AppleFmCompletionTurn;

/** A launched or adopted sidecar session. `stop()` never stops an adopted bridge. */
export interface AppleFmLauncherSession {
  readonly mode: "launched" | "adopted";
  readonly probe: () => Promise<AppleFmProbe>;
  readonly complete: (prompt: string) => Promise<AppleFmLauncherTurn>;
  readonly stop: () => void;
}

export type AppleFmLaunchOutcome =
  | { readonly kind: "session"; readonly session: AppleFmLauncherSession }
  | { readonly kind: "helper_missing"; readonly blockerRef: string }
  | { readonly kind: "failed"; readonly blockerRef: string; readonly failureClass: string };

export interface AppleFmLauncher {
  /** macOS Apple Silicon gate. On any other platform the supervisor reports `not_supported`. */
  readonly supported: () => boolean;
  /** Adopt an existing healthy bridge, else verify + spawn + poll one. */
  readonly launch: (
    input: { readonly onCrash: (failureClass: string) => void },
  ) => Promise<AppleFmLaunchOutcome>;
}

export const appleFmSupervisorStates = [
  "not_supported",
  "candidate",
  "helper_missing",
  "launching",
  "adopted",
  "running",
  "ready",
  "unavailable",
  "failed",
  "stopped",
] as const;
export type AppleFmSupervisorState = (typeof appleFmSupervisorStates)[number];

export type AppleFmSupervisorMode = "local_launched" | "local_adopted" | "none";

/** The neutral, public-safe supervisor projection. No helper secrets cross it. */
export interface AppleFmSupervisorStatus {
  readonly supported: boolean;
  readonly state: AppleFmSupervisorState;
  readonly readiness: AppleFmReadinessStatus;
  readonly ready: boolean;
  readonly mode: AppleFmSupervisorMode;
  readonly model: string | null;
  readonly profileId: string | null;
  readonly usageTruth: AppleFmUsageTruth;
  readonly unavailableReason: string | null;
  readonly blockerRefs: ReadonlyArray<string>;
}

export interface AppleFmSupervisor {
  readonly status: () => AppleFmSupervisorStatus;
  /** Idempotent: start (or re-probe) the owned session and return the projection. */
  readonly ensureStarted: () => Promise<AppleFmSupervisorStatus>;
  /** Re-probe live readiness of the current session. */
  readonly refresh: () => Promise<AppleFmSupervisorStatus>;
  /** Run one bounded read-only turn; refuses unless live-ready. */
  readonly runTurn: (prompt: string) => Promise<AppleFmLauncherTurn>;
  /** Stop an owned session (never an adopted bridge) and return the projection. */
  readonly stop: () => AppleFmSupervisorStatus;
  readonly dispose: () => void;
}

const blockerForReason = (reason: string | undefined): ReadonlyArray<string> =>
  reason === undefined ? [] : [`blocker.apple_fm.${reason}`];

const refusedTurn = (failureClass: "unsupported_platform" | "not_ready"): AppleFmLauncherTurn => ({
  outcome: "failed",
  usageTruth: "unknown",
  failureClass,
});

/**
 * Create the neutral Apple FM supervisor over an injected launcher boundary.
 * Mirrors the Desktop host state machine exactly, minus the Desktop IPC schema.
 */
export const createAppleFmSupervisor = (launcher: AppleFmLauncher): AppleFmSupervisor => {
  const supported = launcher.supported();
  let state: AppleFmSupervisorState = supported ? "candidate" : "not_supported";
  let readiness: AppleFmReadinessStatus = supported ? "unreachable" : "unsupported";
  let ready = false;
  let mode: AppleFmSupervisorMode = "none";
  let model: string | null = null;
  let profileId: string | null = null;
  let usageTruth: AppleFmUsageTruth = "unknown";
  let unavailableReason: string | null = supported ? null : "unsupported_hardware";
  let blockerRefs: ReadonlyArray<string> = supported ? [] : ["blocker.apple_fm.unsupported_platform"];
  let session: AppleFmLauncherSession | null = null;
  let generation = 0;
  let starting: Promise<AppleFmSupervisorStatus> | null = null;
  let disposed = false;

  const status = (): AppleFmSupervisorStatus => ({
    supported,
    state,
    readiness,
    ready,
    mode,
    model,
    profileId,
    usageTruth,
    unavailableReason,
    blockerRefs: blockerRefs.slice(0, 8),
  });

  const applyProbe = (probe: AppleFmProbe): void => {
    readiness = probe.status;
    model = probe.model ?? model;
    profileId = probe.profileId ?? profileId;
    usageTruth = probe.usageTruth ?? usageTruth;
    if (probe.ready) {
      state = "ready";
      ready = true;
      unavailableReason = null;
      blockerRefs = [];
    } else {
      state = "unavailable";
      ready = false;
      unavailableReason = probe.unavailableReason ?? probe.status;
      blockerRefs = blockerForReason(probe.unavailableReason ?? probe.status);
    }
  };

  const refreshSession = async (owned: AppleFmLauncherSession, ownedGeneration: number): Promise<void> => {
    let probe: AppleFmProbe;
    try {
      probe = await owned.probe();
    } catch {
      probe = { status: "unreachable", ready: false, unavailableReason: "bridge_unreachable" };
    }
    if (generation !== ownedGeneration || session !== owned) return;
    applyProbe(probe);
  };

  const start = async (): Promise<AppleFmSupervisorStatus> => {
    if (disposed || !supported) return status();
    if (session !== null) {
      await refreshSession(session, generation);
      return status();
    }
    generation += 1;
    const ownedGeneration = generation;
    state = "launching";
    readiness = "unreachable";
    ready = false;
    mode = "none";
    unavailableReason = null;
    blockerRefs = [];
    const outcome = await launcher.launch({
      onCrash: (failureClass) => {
        if (generation !== ownedGeneration) return;
        session = null;
        state = "failed";
        readiness = "unreachable";
        ready = false;
        mode = "none";
        unavailableReason = "helper_crashed";
        blockerRefs = [`blocker.apple_fm.${failureClass}`];
      },
    });
    if (disposed || generation !== ownedGeneration) {
      if (outcome.kind === "session" && outcome.session.mode === "launched") outcome.session.stop();
      return status();
    }
    if (outcome.kind === "helper_missing") {
      state = "helper_missing";
      readiness = "unavailable";
      ready = false;
      unavailableReason = "helper_missing";
      blockerRefs = [outcome.blockerRef];
      return status();
    }
    if (outcome.kind === "failed") {
      state = "failed";
      readiness = "unreachable";
      ready = false;
      unavailableReason = outcome.failureClass;
      blockerRefs = [outcome.blockerRef];
      return status();
    }
    session = outcome.session;
    mode = outcome.session.mode === "adopted" ? "local_adopted" : "local_launched";
    state = outcome.session.mode === "adopted" ? "adopted" : "running";
    await refreshSession(outcome.session, ownedGeneration);
    return status();
  };

  const ensureStarted = (): Promise<AppleFmSupervisorStatus> => {
    if (disposed || !supported) return Promise.resolve(status());
    if (starting !== null) return starting;
    const pending = start().finally(() => {
      if (starting === pending) starting = null;
    });
    starting = pending;
    return pending;
  };

  return {
    status,
    ensureStarted,
    refresh: async () => {
      if (disposed || !supported || session === null) return status();
      await refreshSession(session, generation);
      return status();
    },
    runTurn: async (prompt): Promise<AppleFmLauncherTurn> => {
      if (disposed || !supported) return refusedTurn("unsupported_platform");
      if (session === null || state !== "ready" || !ready) return refusedTurn("not_ready");
      return session.complete(prompt);
    },
    stop: () => {
      if (!supported) return status();
      generation += 1;
      const owned = session;
      session = null;
      if (owned !== null && owned.mode === "launched") owned.stop();
      state = "stopped";
      readiness = "unreachable";
      ready = false;
      mode = "none";
      unavailableReason = null;
      blockerRefs = [];
      return status();
    },
    dispose: () => {
      if (disposed) return;
      disposed = true;
      generation += 1;
      const owned = session;
      session = null;
      if (owned !== null && owned.mode === "launched") owned.stop();
      state = "stopped";
      ready = false;
      mode = "none";
    },
  };
};
