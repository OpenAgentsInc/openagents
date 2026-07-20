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

export class PylonPortableDestinationHelperSupervisorError extends Schema.TaggedErrorClass<PylonPortableDestinationHelperSupervisorError>()(
  "PylonPortableDestinationHelperSupervisorError",
  {
    reason: Schema.Literals([
      "authentication_failed",
      "conflicting_replay",
      "helper_not_live",
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
): Promise<void> => {
  for (const handle of [...handles].reverse()) {
    await Promise.resolve(handle.dispose()).catch(() => undefined);
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
  }>,
): PylonPortableDestinationHelperSupervisor => {
  const now = options.now ?? (() => new Date());
  const adapters = new Map((options.adapters ?? []).map((adapter) => [adapter.kind, adapter]));
  if (
    typeof options.authenticator.authenticate !== "function" ||
    adapters.size !== (options.adapters ?? []).length ||
    [...adapters.keys()].some((kind) => !HELPER_KINDS.includes(kind))
  ) {
    throw failure("invalid_configuration", "configuration");
  }

  const instances = new Map<string, ActiveInstance>();
  const pending = new Map<string, PendingInstance>();

  const disposeActiveReservation = async (reservationRef: string): Promise<void> => {
    const instance = instances.get(reservationRef);
    if (instance === undefined) return;
    instances.delete(reservationRef);
    instance.removeCallerAbort();
    instance.controller.abort(failure("invalid_scope", reservationRef));
    await disposeHandles(instance.handles);
  };

  const disposeActiveSession = async (sessionRef: string): Promise<void> => {
    const reservations = [...instances.entries()]
      .filter(([, instance]) => instance.input.sessionRef === sessionRef)
      .map(([reservationRef]) => reservationRef);
    for (const reservationRef of reservations) await disposeActiveReservation(reservationRef);
  };

  const disposeReservation = async (reservationRef: string): Promise<void> => {
    if (!SAFE_REF.test(reservationRef)) throw failure("invalid_scope", reservationRef);
    const inFlight = pending.get(reservationRef);
    inFlight?.controller.abort(failure("invalid_scope", reservationRef));
    await disposeActiveReservation(reservationRef);
    await inFlight?.promise.catch(() => undefined);
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
    for (const reservationRef of reservations) await disposeReservation(reservationRef);
  };

  const disposeAll = async (): Promise<void> => {
    for (const reservationRef of new Set([...instances.keys(), ...pending.keys()])) {
      await disposeReservation(reservationRef);
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
              omissionRef: `omission.pylon.portable.${kind}.adapter_unavailable`,
              evidenceRefs: [],
            });
            continue;
          }
          if (signal.aborted)
            throw failure("helper_start_failed", input.destinationRunnerSessionReservationRef);
          const handle = await adapter.start({ ...input, authentication, signal });
          if (!(await exactHandle(handle))) {
            await Promise.resolve(handle.dispose()).catch(() => undefined);
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
        await disposeHandles(handles);
        if (error instanceof PylonPortableDestinationHelperSupervisorError) throw error;
        throw failure("helper_start_failed", input.destinationRunnerSessionReservationRef);
      }
      if (signal.aborted) {
        await disposeHandles(handles);
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
          void disposeReservation(input.destinationRunnerSessionReservationRef);
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

  return { activate, disposeReservation, disposeSession, disposeAll };
};
