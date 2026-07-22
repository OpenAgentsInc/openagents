import { createHash } from "node:crypto";
import { isAbsolute } from "node:path";

import type {
  IdePortableDestinationAuthentication,
  IdePortableDestinationHelperKind,
  IdePortableDestinationHelperReadiness,
} from "@openagentsinc/portable-session-contract";
import { Schema } from "effect";

const SAFE_REF = /^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$/u;
const HELPER_KINDS = ["pty", "lsp", "dap", "watcher", "native"] as const;
const HELPER_KIND_NAMES = new Set<string>(HELPER_KINDS);

export class PylonPortableDestinationHelperSupervisorError extends Schema.TaggedErrorClass<PylonPortableDestinationHelperSupervisorError>()(
  "PylonPortableDestinationHelperSupervisorError",
  {
    reason: Schema.Literals([
      "authentication_failed",
      "conflicting_replay",
      "helper_not_live",
      "helper_disposal_failed",
      "helper_start_failed",
      "invalid_configuration",
      "invalid_scope",
    ]),
    failureRef: Schema.String,
  },
) {}

export type PylonPortableDestinationHelperStartInput = Readonly<{
  destinationRunnerSessionReservationRef: string;
  sessionRef: string;
  destinationAttachmentRef: string;
  destinationGeneration: number;
  workspaceRef: string;
  workingDirectory: string;
  authorityEvidenceRef: string;
  authenticationPolicyRef: string;
  capabilityLeaseRefs: ReadonlyArray<string>;
  authentication: IdePortableDestinationAuthentication;
  signal: AbortSignal;
}>;

export type PylonPortableDestinationHelperLiveHandle = Readonly<{
  instanceRef: string;
  versionRef: string;
  evidenceRefs: ReadonlyArray<string>;
  isLive: () => boolean | Promise<boolean>;
  dispose: () => void | Promise<void>;
}>;

export type PylonPortableDestinationHelperAdapter = Readonly<{
  kind: IdePortableDestinationHelperKind;
  start: (
    input: PylonPortableDestinationHelperStartInput,
  ) => Promise<PylonPortableDestinationHelperLiveHandle>;
}>;

export type PylonPortableDestinationAuthenticator = Readonly<{
  authenticate: (
    input: Omit<PylonPortableDestinationHelperStartInput, "authentication" | "signal">,
  ) => Promise<IdePortableDestinationAuthentication>;
}>;

export type PylonPortableDestinationHelperActivationInput = Readonly<{
  destinationRunnerSessionReservationRef: string;
  sessionRef: string;
  destinationAttachmentRef: string;
  destinationGeneration: number;
  workspaceRef: string;
  workingDirectory: string;
  authorityEvidenceRef: string;
  authenticationPolicyRef: string;
  capabilityLeaseRefs: ReadonlyArray<string>;
  signal?: AbortSignal;
}>;

export type PylonPortableDestinationHelperActivation = Readonly<{
  authentication: IdePortableDestinationAuthentication;
  helpersObservedAt: string;
  helpers: ReadonlyArray<IdePortableDestinationHelperReadiness>;
  evidenceRefs: ReadonlyArray<string>;
}>;

export type PylonPortableDestinationHelperSupervisor = Readonly<{
  activate: (
    input: PylonPortableDestinationHelperActivationInput,
  ) => Promise<PylonPortableDestinationHelperActivation>;
  disposeReservation: (reservationRef: string) => Promise<void>;
  disposeSession: (sessionRef: string) => Promise<void>;
  disposeAll: () => Promise<void>;
  disposalFailures: () => ReadonlyArray<PylonPortableDestinationHelperSupervisorError>;
}>;

type ActiveInstance = {
  fingerprint: string;
  input: PylonPortableDestinationHelperActivationInput;
  result: PylonPortableDestinationHelperActivation;
  controller: AbortController;
  handles: ReadonlyArray<PylonPortableDestinationHelperLiveHandle>;
  removeCallerAbort: () => void;
};

type PendingInstance = Readonly<{
  fingerprint: string;
  input: PylonPortableDestinationHelperActivationInput;
  controller: AbortController;
  promise: Promise<PylonPortableDestinationHelperActivation>;
}>;

const stableRef = (prefix: string, value: string): string =>
  `${prefix}.${createHash("sha256").update(value).digest("hex")}`;

const canonical = (value: unknown): string => {
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`;
  if (value !== null && typeof value === "object") {
    return `{${Object.entries(value)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, item]) => `${JSON.stringify(key)}:${canonical(item)}`)
      .join(",")}}`;
  }
  return JSON.stringify(value);
};

const failure = (
  reason: PylonPortableDestinationHelperSupervisorError["reason"],
  scopeRef: string,
): PylonPortableDestinationHelperSupervisorError =>
  new PylonPortableDestinationHelperSupervisorError({
    reason,
    failureRef: stableRef(`failure.pylon.portable-destination-helper.${reason}`, scopeRef),
  });

const exactInput = (input: PylonPortableDestinationHelperActivationInput): boolean =>
  [
    input.destinationRunnerSessionReservationRef,
    input.sessionRef,
    input.destinationAttachmentRef,
    input.workspaceRef,
    input.authorityEvidenceRef,
    input.authenticationPolicyRef,
    ...input.capabilityLeaseRefs,
  ].every((ref) => SAFE_REF.test(ref)) &&
  Number.isSafeInteger(input.destinationGeneration) &&
  input.destinationGeneration > 0 &&
  isAbsolute(input.workingDirectory) &&
  new Set(input.capabilityLeaseRefs).size === input.capabilityLeaseRefs.length;

const exactAuthentication = (
  authentication: IdePortableDestinationAuthentication,
  input: PylonPortableDestinationHelperActivationInput,
  now: Date,
): boolean => {
  const observedAt = new Date(authentication.observedAt);
  const expiresAt = authentication.expiresAt === null ? null : new Date(authentication.expiresAt);
  return (
    authentication.state === "reauthenticated" &&
    authentication.policyRef === input.authenticationPolicyRef &&
    authentication.evidenceRef === input.authorityEvidenceRef &&
    Number.isFinite(observedAt.valueOf()) &&
    observedAt <= now &&
    (expiresAt === null || (Number.isFinite(expiresAt.valueOf()) && expiresAt > now))
  );
};

const exactHandle = async (handle: PylonPortableDestinationHelperLiveHandle): Promise<boolean> =>
  SAFE_REF.test(handle.instanceRef) &&
  SAFE_REF.test(handle.versionRef) &&
  handle.evidenceRefs.length > 0 &&
  handle.evidenceRefs.length <= 32 &&
  new Set(handle.evidenceRefs).size === handle.evidenceRefs.length &&
  handle.evidenceRefs.every((ref) => SAFE_REF.test(ref)) &&
  (await handle.isLive());

const disposeHandles = async (
  handles: ReadonlyArray<PylonPortableDestinationHelperLiveHandle>,
  scopeRef: string,
): Promise<void> => {
  const failedInstanceRefs: string[] = [];
  for (const handle of [...handles].reverse()) {
    try {
      await Promise.resolve(handle.dispose());
    } catch {
      failedInstanceRefs.push(handle.instanceRef);
    }
  }
  if (failedInstanceRefs.length > 0) {
    throw failure(
      "helper_disposal_failed",
      canonical({ failedInstanceRefs: failedInstanceRefs.sort(), scopeRef }),
    );
  }
};

export const makeEvidenceBoundPortableDestinationAuthenticator = (
  now: () => Date = () => new Date(),
): PylonPortableDestinationAuthenticator => ({
  authenticate: async (input) => ({
    state: "reauthenticated",
    policyRef: input.authenticationPolicyRef,
    evidenceRef: input.authorityEvidenceRef,
    observedAt: now().toISOString(),
    expiresAt: null,
  }),
});

export const makePylonPortableDestinationHelperSupervisor = (
  options: Readonly<{
    authenticator: PylonPortableDestinationAuthenticator;
    adapters?: ReadonlyArray<PylonPortableDestinationHelperAdapter>;
    now?: () => Date;
    unsupportedOmissionRefs?: Partial<Record<IdePortableDestinationHelperKind, string>>;
  }>,
): PylonPortableDestinationHelperSupervisor => {
  const now = options.now ?? (() => new Date());
  const adapters = new Map((options.adapters ?? []).map((adapter) => [adapter.kind, adapter]));
  if (
    typeof options.authenticator.authenticate !== "function" ||
    adapters.size !== (options.adapters ?? []).length ||
    [...adapters.keys()].some((kind) => !HELPER_KINDS.includes(kind)) ||
    Object.entries(options.unsupportedOmissionRefs ?? {}).some(
      ([kind, omissionRef]) =>
        !HELPER_KIND_NAMES.has(kind) ||
        typeof omissionRef !== "string" ||
        !SAFE_REF.test(omissionRef),
    )
  ) {
    throw failure("invalid_configuration", "configuration");
  }

  const instances = new Map<string, ActiveInstance>();
  const pending = new Map<string, PendingInstance>();
  const recordedDisposalFailures: PylonPortableDestinationHelperSupervisorError[] = [];

  const recordDisposalFailure = (
    error: PylonPortableDestinationHelperSupervisorError,
  ): void => {
    if (recordedDisposalFailures.some((prior) => prior.failureRef === error.failureRef)) return;
    recordedDisposalFailures.push(error);
  };

  const disposeExactHandles = async (
    handles: ReadonlyArray<PylonPortableDestinationHelperLiveHandle>,
    scopeRef: string,
  ): Promise<void> => {
    try {
      await disposeHandles(handles, scopeRef);
    } catch (error) {
      const typedError = error instanceof PylonPortableDestinationHelperSupervisorError
        ? error
        : failure("helper_disposal_failed", scopeRef);
      recordDisposalFailure(typedError);
      throw typedError;
    }
  };

  const disposeActiveReservation = async (reservationRef: string): Promise<void> => {
    const instance = instances.get(reservationRef);
    if (instance === undefined) return;
    instances.delete(reservationRef);
    instance.removeCallerAbort();
    instance.controller.abort(failure("invalid_scope", reservationRef));
    try {
      await disposeExactHandles(instance.handles, reservationRef);
    } catch (error) {
      if (!instances.has(reservationRef)) instances.set(reservationRef, instance);
      throw error;
    }
  };

  const disposeActiveSession = async (sessionRef: string): Promise<void> => {
    const reservations = [...instances.entries()]
      .filter(([, instance]) => instance.input.sessionRef === sessionRef)
      .map(([reservationRef]) => reservationRef);
    const failures: PylonPortableDestinationHelperSupervisorError[] = [];
    for (const reservationRef of reservations) {
      try {
        await disposeActiveReservation(reservationRef);
      } catch (error) {
        if (error instanceof PylonPortableDestinationHelperSupervisorError) failures.push(error);
        else failures.push(failure("helper_disposal_failed", reservationRef));
      }
    }
    if (failures.length > 0) {
      const combined = failure(
        "helper_disposal_failed",
        canonical({ failureRefs: failures.map((item) => item.failureRef).sort(), sessionRef }),
      );
      recordDisposalFailure(combined);
      throw combined;
    }
  };

  const disposeReservation = async (reservationRef: string): Promise<void> => {
    if (!SAFE_REF.test(reservationRef)) throw failure("invalid_scope", reservationRef);
    const inFlight = pending.get(reservationRef);
    inFlight?.controller.abort(failure("invalid_scope", reservationRef));
    await disposeActiveReservation(reservationRef);
    await inFlight?.promise.catch((error: unknown) => {
      if (
        error instanceof PylonPortableDestinationHelperSupervisorError &&
        error.reason === "helper_disposal_failed"
      ) {
        throw error;
      }
    });
    await disposeActiveReservation(reservationRef);
  };

  const disposeSession = async (sessionRef: string): Promise<void> => {
    if (!SAFE_REF.test(sessionRef)) throw failure("invalid_scope", sessionRef);
    const reservations = [
      ...new Set([
        ...[...instances.entries()]
          .filter(([, instance]) => instance.input.sessionRef === sessionRef)
          .map(([reservationRef]) => reservationRef),
        ...[...pending.entries()]
          .filter(([, instance]) => instance.input.sessionRef === sessionRef)
          .map(([reservationRef]) => reservationRef),
      ]),
    ];
    const failures: PylonPortableDestinationHelperSupervisorError[] = [];
    for (const reservationRef of reservations) {
      try {
        await disposeReservation(reservationRef);
      } catch (error) {
        if (error instanceof PylonPortableDestinationHelperSupervisorError) failures.push(error);
        else failures.push(failure("helper_disposal_failed", reservationRef));
      }
    }
    if (failures.length > 0) {
      const combined = failure(
        "helper_disposal_failed",
        canonical({ failureRefs: failures.map((item) => item.failureRef).sort(), sessionRef }),
      );
      recordDisposalFailure(combined);
      throw combined;
    }
  };

  const disposeAll = async (): Promise<void> => {
    const failures: PylonPortableDestinationHelperSupervisorError[] = [];
    for (const reservationRef of new Set([...instances.keys(), ...pending.keys()])) {
      try {
        await disposeReservation(reservationRef);
      } catch (error) {
        if (error instanceof PylonPortableDestinationHelperSupervisorError) failures.push(error);
        else failures.push(failure("helper_disposal_failed", reservationRef));
      }
    }
    if (failures.length > 0) {
      const combined = failure(
        "helper_disposal_failed",
        canonical({ failureRefs: failures.map((item) => item.failureRef).sort(), scopeRef: "all" }),
      );
      recordDisposalFailure(combined);
      throw combined;
    }
  };

  const activate = async (
    input: PylonPortableDestinationHelperActivationInput,
  ): Promise<PylonPortableDestinationHelperActivation> => {
    if (!exactInput(input) || input.signal?.aborted === true) {
      throw failure("invalid_scope", input.destinationRunnerSessionReservationRef);
    }
    const fingerprint = stableRef(
      "fingerprint.pylon.portable-destination-helper",
      canonical({ ...input, signal: undefined }),
    );
    const prior = instances.get(input.destinationRunnerSessionReservationRef);
    if (prior !== undefined) {
      if (prior.fingerprint !== fingerprint) {
        throw failure("conflicting_replay", input.destinationRunnerSessionReservationRef);
      }
      const live = await Promise.all(prior.handles.map((handle) => exactHandle(handle)));
      if (live.some((value) => !value)) {
        await disposeReservation(input.destinationRunnerSessionReservationRef);
        throw failure("helper_not_live", input.destinationRunnerSessionReservationRef);
      }
      return prior.result;
    }
    const inFlight = pending.get(input.destinationRunnerSessionReservationRef);
    if (inFlight !== undefined) {
      if (inFlight.fingerprint !== fingerprint) {
        throw failure("conflicting_replay", input.destinationRunnerSessionReservationRef);
      }
      return inFlight.promise;
    }
    const sessionInFlight = [...pending.values()].find(
      (instance) => instance.input.sessionRef === input.sessionRef,
    );
    if (sessionInFlight !== undefined) {
      throw failure("conflicting_replay", input.destinationRunnerSessionReservationRef);
    }

    const controller = new AbortController();
    const signal =
      input.signal === undefined
        ? controller.signal
        : AbortSignal.any([input.signal, controller.signal]);
    const run = async (): Promise<PylonPortableDestinationHelperActivation> => {
      const authentication = await options.authenticator.authenticate({
        destinationRunnerSessionReservationRef: input.destinationRunnerSessionReservationRef,
        sessionRef: input.sessionRef,
        destinationAttachmentRef: input.destinationAttachmentRef,
        destinationGeneration: input.destinationGeneration,
        workspaceRef: input.workspaceRef,
        workingDirectory: input.workingDirectory,
        authorityEvidenceRef: input.authorityEvidenceRef,
        authenticationPolicyRef: input.authenticationPolicyRef,
        capabilityLeaseRefs: input.capabilityLeaseRefs,
      });
      const authenticatedAt = now();
      if (!exactAuthentication(authentication, input, authenticatedAt)) {
        throw failure("authentication_failed", input.destinationRunnerSessionReservationRef);
      }
      if (signal.aborted) {
        throw failure("helper_start_failed", input.destinationRunnerSessionReservationRef);
      }
      await disposeActiveSession(input.sessionRef);

      const handles: PylonPortableDestinationHelperLiveHandle[] = [];
      const readiness: IdePortableDestinationHelperReadiness[] = [];
      try {
        for (const kind of HELPER_KINDS) {
          const adapter = adapters.get(kind);
          if (adapter === undefined) {
            readiness.push({
              kind,
              readiness: "unsupported",
              instanceRef: null,
              versionRef: null,
              omissionRef:
                options.unsupportedOmissionRefs?.[kind] ??
                `omission.pylon.portable.${kind}.adapter_unavailable`,
              evidenceRefs: [],
            });
            continue;
          }
          if (signal.aborted)
            throw failure("helper_start_failed", input.destinationRunnerSessionReservationRef);
          const handle = await adapter.start({ ...input, authentication, signal });
          if (!(await exactHandle(handle))) {
            await disposeExactHandles([handle], input.destinationRunnerSessionReservationRef);
            throw failure("helper_not_live", input.destinationRunnerSessionReservationRef);
          }
          handles.push(handle);
          readiness.push({
            kind,
            readiness: "ready",
            instanceRef: handle.instanceRef,
            versionRef: handle.versionRef,
            omissionRef: null,
            evidenceRefs: handle.evidenceRefs,
          });
        }
      } catch (error) {
        controller.abort(error);
        await disposeExactHandles(handles, input.destinationRunnerSessionReservationRef);
        if (error instanceof PylonPortableDestinationHelperSupervisorError) throw error;
        throw failure("helper_start_failed", input.destinationRunnerSessionReservationRef);
      }
      if (signal.aborted) {
        await disposeExactHandles(handles, input.destinationRunnerSessionReservationRef);
        throw failure("helper_start_failed", input.destinationRunnerSessionReservationRef);
      }

      const helpersObservedAt = now().toISOString();
      const receiptRef = stableRef("receipt.pylon.portable.destination-helpers", fingerprint);
      const result = {
        authentication,
        helpersObservedAt,
        helpers: readiness,
        evidenceRefs: [
          ...new Set([
            authentication.evidenceRef,
            receiptRef,
            ...readiness.flatMap((helper) => helper.evidenceRefs),
          ]),
        ],
      } satisfies PylonPortableDestinationHelperActivation;
      const removeCallerAbort = (() => {
        if (input.signal === undefined) return () => undefined;
        const onAbort = () => {
          void disposeReservation(input.destinationRunnerSessionReservationRef).catch((error) => {
            if (error instanceof PylonPortableDestinationHelperSupervisorError) {
              recordDisposalFailure(error);
            }
          });
        };
        input.signal.addEventListener("abort", onAbort, { once: true });
        return () => input.signal?.removeEventListener("abort", onAbort);
      })();
      instances.set(input.destinationRunnerSessionReservationRef, {
        fingerprint,
        input,
        result,
        controller,
        handles,
        removeCallerAbort,
      });
      if (signal.aborted) {
        await disposeActiveReservation(input.destinationRunnerSessionReservationRef);
        throw failure("helper_start_failed", input.destinationRunnerSessionReservationRef);
      }
      return result;
    };

    const promise = run().finally(() => {
      if (pending.get(input.destinationRunnerSessionReservationRef)?.promise === promise) {
        pending.delete(input.destinationRunnerSessionReservationRef);
      }
    });
    pending.set(input.destinationRunnerSessionReservationRef, {
      fingerprint,
      input,
      controller,
      promise,
    });
    return promise;
  };

  const disposalFailures = () => [...recordedDisposalFailures];

  return { activate, disposeReservation, disposeSession, disposeAll, disposalFailures };
};
