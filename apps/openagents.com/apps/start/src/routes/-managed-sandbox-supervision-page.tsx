import type {
  ManagedSandboxSupervisionEnvelope,
  ManagedSandboxSupervisionOutcome,
  ManagedSandboxSupervisionProjection,
} from "@openagentsinc/managed-sandbox-contract";
import { useCallback, useEffect, useRef, useState } from "react";

import { useKhalaSyncSession } from "./-khala-sync-session";
import {
  fetchWebManagedSandboxEnvelope,
  makeWebManagedSandboxController,
  type WebManagedSandboxAction,
} from "./-managed-sandbox-web-client";

const buttonClass =
  "khala-focus min-h-11 border border-khala-border-strong bg-khala-surface-raised px-3 py-2 font-mono text-sm font-semibold text-khala-text disabled:cursor-not-allowed disabled:opacity-45";
const primaryButtonClass =
  "khala-focus min-h-11 border border-khala-energy-cyan/70 bg-khala-energy px-3 py-2 font-mono text-sm font-semibold text-black disabled:cursor-not-allowed disabled:opacity-45";

const titleCase = (value: string): string =>
  value.replaceAll("_", " ").replace(/^./u, (letter) => letter.toUpperCase());

const duration = (seconds: number): string => {
  if (seconds < 60) return `${seconds}s`;
  if (seconds < 3_600) return `${Math.floor(seconds / 60)}m`;
  return `${Math.floor(seconds / 3_600)}h ${Math.floor((seconds % 3_600) / 60)}m`;
};

const lifecycleClass = (projection: ManagedSandboxSupervisionProjection): string => {
  if (projection.attention.state === "recovery_required") {
    return "border-khala-danger/60 text-khala-danger";
  }
  if (projection.attention.state === "needs_action") {
    return "border-khala-warning/60 text-khala-warning";
  }
  if (projection.state.lifecycle === "ready" || projection.state.lifecycle === "idle") {
    return "border-khala-success/60 text-khala-success";
  }
  return "border-khala-energy-cyan/60 text-khala-energy-cyan";
};

const actorLabel = (actorRef: string): string => {
  switch (actorRef) {
    case "principal.sarah":
      return "Sarah";
    case "principal.desktop":
      return "Desktop";
    case "principal.mobile":
      return "Mobile";
    case "principal.web":
      return "Web";
    default:
      return "OpenAgents";
  }
};

const actionsFor = (
  projection: ManagedSandboxSupervisionProjection,
): ReadonlyArray<WebManagedSandboxAction> => {
  const actions: WebManagedSandboxAction[] = [];
  if (
    projection.runtime !== null &&
    (projection.runtime.status === "running" || projection.runtime.status === "interrupting")
  ) {
    actions.push("interrupt");
  }
  if (["ready", "idle", "running"].includes(projection.state.lifecycle)) actions.push("stop");
  if (projection.state.lifecycle === "stopped") actions.push("resume");
  if (projection.state.lifecycle !== "deleted" && projection.state.lifecycle !== "deleting") {
    actions.push("delete");
  }
  return actions;
};

const refGroups = (projection: ManagedSandboxSupervisionProjection) =>
  [
    ["Files", projection.outcomes.fileRefs],
    ["Changes", projection.outcomes.changeRefs],
    ["Artifacts", projection.outcomes.artifactRefs],
    ["Evidence", projection.outcomes.evidenceRefs],
    ["Receipts", projection.outcomes.receiptRefs],
  ] as const;

export function ManagedSandboxWebList({
  envelope,
  pendingRef,
  deleteConfirmRef,
  outcome,
  onAction,
  onDeleteConfirm,
  onDeleteDismiss,
}: Readonly<{
  envelope: ManagedSandboxSupervisionEnvelope;
  pendingRef: string | null;
  deleteConfirmRef: string | null;
  outcome: ManagedSandboxSupervisionOutcome | null;
  onAction: (
    projection: ManagedSandboxSupervisionProjection,
    action: WebManagedSandboxAction,
  ) => void;
  onDeleteConfirm: (projection: ManagedSandboxSupervisionProjection) => void;
  onDeleteDismiss: () => void;
}>) {
  if (envelope.projections.length === 0) {
    return (
      <section
        className="border border-khala-border bg-khala-surface p-6"
        data-managed-sandbox-empty=""
      >
        <h2 className="m-0 font-mono text-lg text-khala-text">No managed agents</h2>
        <p className="mt-2 mb-0 text-sm/6 text-khala-text-muted">
          This account has no managed sandbox lifecycle to supervise.
        </p>
      </section>
    );
  }
  return (
    <div className="grid gap-4" data-managed-sandbox-list="">
      {envelope.projections.map((projection) => {
        const pending = pendingRef === projection.sandboxRef;
        const runtime = projection.runtime;
        const latestOutcome =
          outcome?.projection?.sandboxRef === projection.sandboxRef ? outcome : null;
        return (
          <article
            aria-label={`Managed sandbox ${projection.sandboxRef}, ${projection.state.lifecycle}`}
            className="grid gap-4 border border-khala-border bg-khala-surface p-4 sm:p-5"
            data-managed-sandbox-ref={projection.sandboxRef}
            key={projection.sandboxRef}
          >
            <header className="flex flex-wrap items-start justify-between gap-3">
              <div className="min-w-0">
                <p className="m-0 font-mono text-xs text-khala-text-faint">
                  {projection.sandboxRef}
                </p>
                <h2 className="mt-1 mb-0 break-all font-mono text-lg font-semibold text-khala-text">
                  {projection.workUnitRef}
                </h2>
              </div>
              <span
                className={`border px-2 py-1 font-mono text-xs font-semibold ${lifecycleClass(projection)}`}
              >
                {titleCase(projection.state.lifecycle)}
              </span>
            </header>

            <dl className="grid grid-cols-1 gap-x-6 gap-y-3 text-sm sm:grid-cols-2 lg:grid-cols-3">
              <div>
                <dt className="text-khala-text-faint">Target</dt>
                <dd className="m-0 font-mono text-khala-text">
                  {projection.target.region} · {titleCase(projection.target.isolation)}
                </dd>
              </div>
              <div>
                <dt className="text-khala-text-faint">Generation</dt>
                <dd className="m-0 font-mono text-khala-text">
                  resource {projection.resourceGeneration} · attachment{" "}
                  {projection.attachmentGeneration}
                </dd>
              </div>
              <div>
                <dt className="text-khala-text-faint">Timing</dt>
                <dd className="m-0 font-mono text-khala-text">
                  elapsed {duration(projection.timing.elapsedSeconds)} · idle{" "}
                  {duration(projection.timing.idleSeconds)}
                </dd>
              </div>
              <div>
                <dt className="text-khala-text-faint">Lease</dt>
                <dd className="m-0 font-mono text-khala-text">
                  {titleCase(projection.timing.leaseState)} · {projection.timing.leaseExpiresAt}
                </dd>
              </div>
              <div>
                <dt className="text-khala-text-faint">Budget</dt>
                <dd className="m-0 font-mono text-khala-text">
                  ${(projection.budget.maxCostMicros / 1_000_000).toFixed(2)} cap ·{" "}
                  {projection.budget.state.replaceAll("_", " ")}
                </dd>
              </div>
              <div>
                <dt className="text-khala-text-faint">Cleanup</dt>
                <dd
                  className={`m-0 font-mono ${projection.cleanup.state === "recovery_required" ? "text-khala-danger" : "text-khala-text"}`}
                >
                  {projection.cleanup.state.replaceAll("_", " ")}
                  {projection.cleanup.receiptRef === null
                    ? ""
                    : ` · ${projection.cleanup.receiptRef}`}
                </dd>
              </div>
            </dl>

            {runtime === null ? null : (
              <p className="m-0 border-l-2 border-khala-energy-cyan/50 pl-3 font-mono text-sm/6 text-khala-text">
                {actorLabel(runtime.actorRef)} · {runtime.identity.provider} ·{" "}
                {runtime.identity.modelRef} · {runtime.identity.harnessRef} · {runtime.status}
              </p>
            )}
            {projection.lastStructuralEvent === null ? null : (
              <p className="m-0 font-mono text-xs text-khala-text-muted">
                Last structural event {projection.lastStructuralEvent.kind} #
                {projection.lastStructuralEvent.sequence} ·{" "}
                {projection.lastStructuralEvent.eventRef}
              </p>
            )}
            {refGroups(projection).map(([label, refs]) =>
              refs.length === 0 ? null : (
                <p className="m-0 break-all font-mono text-xs text-khala-text-muted" key={label}>
                  {label}: {refs.join(", ")}
                </p>
              ),
            )}
            {latestOutcome === null ? null : (
              <p
                className={`m-0 font-mono text-sm ${latestOutcome.state === "applied" ? "text-khala-success" : "text-khala-warning"}`}
                role="status"
              >
                {titleCase(latestOutcome.state)} ·{" "}
                {latestOutcome.reasonRef ?? latestOutcome.receiptRefs.join(", ")}
              </p>
            )}

            {deleteConfirmRef === projection.sandboxRef ? (
              <div className="grid gap-3 border-t border-khala-border pt-4">
                <p className="m-0 text-sm/6 text-khala-warning">
                  Delete requests teardown. Completion still requires a zero-residue cleanup
                  receipt.
                </p>
                <div className="flex flex-wrap gap-2">
                  <button
                    className={buttonClass}
                    disabled={pending}
                    onClick={() => onDeleteConfirm(projection)}
                    type="button"
                  >
                    {pending ? "Requesting delete…" : "Confirm delete"}
                  </button>
                  <button
                    className={buttonClass}
                    disabled={pending}
                    onClick={onDeleteDismiss}
                    type="button"
                  >
                    Cancel
                  </button>
                </div>
              </div>
            ) : (
              <div className="flex flex-wrap gap-2 border-t border-khala-border pt-4">
                {actionsFor(projection).map((action) => (
                  <button
                    className={action === "resume" ? primaryButtonClass : buttonClass}
                    disabled={pending}
                    key={action}
                    onClick={() => onAction(projection, action)}
                    type="button"
                  >
                    {pending
                      ? "Pending reconciliation…"
                      : action === "delete"
                        ? "Delete…"
                        : titleCase(action)}
                  </button>
                ))}
              </div>
            )}
          </article>
        );
      })}
    </div>
  );
}

export function ManagedSandboxSupervisionPage() {
  const session = useKhalaSyncSession();
  const [envelope, setEnvelope] = useState<ManagedSandboxSupervisionEnvelope | null>(null);
  const [status, setStatus] = useState<"loading" | "ready" | "unavailable">("loading");
  const [pendingRef, setPendingRef] = useState<string | null>(null);
  const [deleteConfirmRef, setDeleteConfirmRef] = useState<string | null>(null);
  const [outcome, setOutcome] = useState<ManagedSandboxSupervisionOutcome | null>(null);
  const controllerRef = useRef<ReturnType<typeof makeWebManagedSandboxController> | null>(null);

  const refresh = useCallback(async () => {
    try {
      setEnvelope(await fetchWebManagedSandboxEnvelope());
      setStatus("ready");
    } catch {
      setStatus("unavailable");
    }
  }, []);

  useEffect(() => {
    if (session.status !== "signed_in" || typeof window === "undefined") return undefined;
    const controller = makeWebManagedSandboxController({ storage: window.localStorage });
    controllerRef.current = controller;
    let active = true;
    const reconcile = async () => {
      const outcomes = await controller.flush();
      if (!active) return;
      if (outcomes.length > 0) {
        setOutcome(outcomes.at(-1) ?? null);
        setPendingRef(null);
      }
      await refresh();
    };
    void reconcile();
    window.addEventListener("online", reconcile);
    const timer = window.setInterval(reconcile, 5_000);
    return () => {
      active = false;
      window.removeEventListener("online", reconcile);
      window.clearInterval(timer);
      controllerRef.current = null;
    };
  }, [refresh, session.status]);

  const request = useCallback(
    async (projection: ManagedSandboxSupervisionProjection, action: WebManagedSandboxAction) => {
      const controller = controllerRef.current;
      if (controller === null || pendingRef !== null) return;
      if (action === "delete") {
        setDeleteConfirmRef(projection.sandboxRef);
        return;
      }
      setPendingRef(projection.sandboxRef);
      setOutcome(null);
      const next = await controller.request(projection, action);
      if (next !== null) {
        setOutcome(next);
        setPendingRef(null);
        await refresh();
      }
    },
    [pendingRef, refresh],
  );

  const confirmDelete = useCallback(
    async (projection: ManagedSandboxSupervisionProjection) => {
      const controller = controllerRef.current;
      if (controller === null || pendingRef !== null) return;
      setPendingRef(projection.sandboxRef);
      setOutcome(null);
      const next = await controller.request(projection, "delete");
      if (next !== null) {
        setOutcome(next);
        setPendingRef(null);
        setDeleteConfirmRef(null);
        await refresh();
      }
    },
    [pendingRef, refresh],
  );

  return (
    <main
      className="min-h-screen bg-khala-void px-4 py-8 text-khala-text sm:px-6 lg:px-8"
      data-managed-sandbox-supervision=""
    >
      <div className="mx-auto grid w-full max-w-6xl gap-6">
        <header className="grid gap-2 border-b border-khala-border pb-6">
          <p className="m-0 font-mono text-xs font-semibold tracking-[0.18em] text-khala-energy-cyan uppercase">
            Authenticated controller
          </p>
          <h1 className="m-0 text-2xl font-semibold tracking-tight sm:text-3xl">Managed agents</h1>
          <p className="m-0 max-w-3xl text-sm/6 text-khala-text-muted">
            Lifecycle, runtime, budget, attention, outcomes, and cleanup truth from the same
            authority used by Desktop and Sarah. This browser hosts no runtime, cloud SDK,
            credential, filesystem, PTY, or shell.
          </p>
        </header>

        {session.status === "loading" ? (
          <p className="m-0 font-mono text-sm text-khala-text-muted">Checking owner session…</p>
        ) : session.status === "signed_out" ? (
          <section className="border border-khala-border bg-khala-surface p-6">
            <h2 className="m-0 text-lg font-semibold">Owner session required</h2>
            <p className="mt-2 mb-4 text-sm/6 text-khala-text-muted">
              Sign in through the Khala Sync owner session. The bearer token remains in an httpOnly
              cookie and never enters browser JavaScript.
            </p>
            <a className={primaryButtonClass} href="/khala/chat-sync">
              Open secure sign in
            </a>
          </section>
        ) : status === "unavailable" ? (
          <p
            className="m-0 border border-khala-warning/50 bg-khala-surface p-4 font-mono text-sm text-khala-warning"
            role="status"
          >
            Supervision is unavailable. Stored controls remain pending and will reconcile after
            reconnect.
          </p>
        ) : envelope === null ? (
          <p className="m-0 font-mono text-sm text-khala-text-muted">Loading managed agents…</p>
        ) : (
          <ManagedSandboxWebList
            deleteConfirmRef={deleteConfirmRef}
            envelope={envelope}
            onAction={(projection, action) => void request(projection, action)}
            onDeleteConfirm={(projection) => void confirmDelete(projection)}
            onDeleteDismiss={() => setDeleteConfirmRef(null)}
            outcome={outcome}
            pendingRef={pendingRef}
          />
        )}
      </div>
    </main>
  );
}
