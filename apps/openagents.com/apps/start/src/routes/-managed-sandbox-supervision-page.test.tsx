import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, test, vi } from "vite-plus/test";

import { decodeManagedSandboxSupervisionEnvelope } from "@openagentsinc/managed-sandbox-contract";

import { ManagedSandboxWebList } from "./-managed-sandbox-supervision-page";

const envelope = decodeManagedSandboxSupervisionEnvelope({
  observedAt: "2026-07-19T19:00:00.000Z",
  projections: [
    {
      schema: "openagents.managed_sandbox_supervision.v1",
      sandboxRef: "sandbox.web.ui",
      workUnitRef: "work.web.ui",
      attachmentRef: "attachment.web.ui",
      attachmentGeneration: 2,
      resourceGeneration: 5,
      version: 9,
      target: {
        targetRef: "target.gcp.sbx",
        provider: "google_cloud",
        region: "us-central1",
        isolation: "gce_vm",
        custody: "openagents_managed_region",
      },
      state: { lifecycle: "running", runtime: "running", acceptingWork: true },
      timing: {
        createdAt: "2026-07-19T18:00:00.000Z",
        updatedAt: "2026-07-19T18:59:00.000Z",
        leaseExpiresAt: "2026-07-19T20:00:00.000Z",
        elapsedSeconds: 3600,
        idleSeconds: 60,
        leaseState: "active",
      },
      budget: {
        class: "bounded",
        currency: "USD",
        maxCostMicros: 10_000,
        observedCostMicros: null,
        state: "unreported",
      },
      runtime: {
        turnRef: "turn.web.ui",
        status: "running",
        identity: {
          provider: "codex",
          modelRef: "model.gpt-5.6-sol",
          harnessRef: "harness.codex.v1",
        },
        actorRef: "principal.sarah",
        startedAt: "2026-07-19T18:01:00.000Z",
        settledAt: null,
        terminalReasonRef: null,
      },
      lastStructuralEvent: {
        eventRef: "event.web.ui.8",
        kind: "RuntimeToolCompleted",
        sequence: 8,
        observedAt: "2026-07-19T18:59:00.000Z",
      },
      attention: { state: "none", reasonRef: null },
      cleanup: { state: "not_started", receiptRef: null },
      outcomes: {
        fileRefs: ["file.safe.ui"],
        changeRefs: ["change.safe.ui"],
        artifactRefs: ["artifact.safe.ui"],
        evidenceRefs: ["evidence.safe.ui"],
        receiptRefs: ["receipt.safe.ui"],
      },
    },
  ],
});

describe("managed-sandbox authenticated web component", () => {
  test("shows exact supervision truth and typed controls without private capability", () => {
    const html = renderToStaticMarkup(
      <ManagedSandboxWebList
        deleteConfirmRef={null}
        envelope={envelope}
        onAction={vi.fn()}
        onDeleteConfirm={vi.fn()}
        onDeleteDismiss={vi.fn()}
        outcome={null}
        pendingRef={null}
      />,
    );
    for (const value of [
      "work.web.ui",
      "us-central1",
      "resource 5",
      "Sarah",
      "model.gpt-5.6-sol",
      "harness.codex.v1",
      "elapsed 1h 0m",
      "$0.01 cap",
      "RuntimeToolCompleted",
      "file.safe.ui",
      "change.safe.ui",
      "artifact.safe.ui",
      "Interrupt",
      "Stop",
      "Delete…",
    ])
      expect(html).toContain(value);
    expect(html).not.toMatch(
      /providerCredential|rawPath|runtimeOutput|privateKey|shell|pty|prompt/iu,
    );
  });

  test("uses a two-step destructive control and names the cleanup oracle", () => {
    const html = renderToStaticMarkup(
      <ManagedSandboxWebList
        deleteConfirmRef="sandbox.web.ui"
        envelope={envelope}
        onAction={vi.fn()}
        onDeleteConfirm={vi.fn()}
        onDeleteDismiss={vi.fn()}
        outcome={null}
        pendingRef={null}
      />,
    );
    expect(html).toContain("Confirm delete");
    expect(html).toContain("zero-residue cleanup receipt");
    expect(html).toContain("Cancel");
  });
});
