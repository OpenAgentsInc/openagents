import { describe, expect, test } from "vite-plus/test";

import {
  classifyThreadDisclosureReplay,
  decodeThreadDisclosureIntent,
  decodeThreadDisclosureReceipt,
} from "./thread-disclosure.js";

const visibilityIntent = {
  schema: "openagents.thread_disclosure_intent.v1" as const,
  intentRef: "intent.visibility.1",
  idempotencyKey: "idempotency.visibility.1",
  threadRef: "thread.desktop.1",
  actorRef: "actor.owner.1",
  expectedVisibilityVersion: { state: "known" as const, value: 3 },
  createdAt: "2026-07-17T14:00:00.000Z",
  kind: "thread.visibility.set" as const,
  target: {
    audience: {
      kind: "named_group" as const,
      workspaceRef: "workspace.1",
      groupRef: "group.reviewers",
    },
    administratorAccess: { kind: "workspace_admins" as const, workspaceRef: "workspace.1" },
  },
};

const receiptBase = {
  schema: "openagents.thread_disclosure_receipt.v1" as const,
  receiptRef: "receipt.disclosure.1",
  intentRef: visibilityIntent.intentRef,
  idempotencyKey: visibilityIntent.idempotencyKey,
  threadRef: visibilityIntent.threadRef,
  observedAt: "2026-07-17T14:00:01.000Z",
};

describe("thread disclosure contracts", () => {
  test("decodes explicit visibility and administrator access without an unlisted state", () => {
    expect(decodeThreadDisclosureIntent(visibilityIntent)).toEqual(visibilityIntent);
    expect(() =>
      decodeThreadDisclosureIntent({
        ...visibilityIntent,
        target: { ...visibilityIntent.target, audience: { kind: "unlisted" } },
      }),
    ).toThrow();
  });

  test("decodes a ref-only export intent with an explicit artifact audience", () => {
    expect(
      decodeThreadDisclosureIntent({
        ...visibilityIntent,
        intentRef: "intent.export.1",
        idempotencyKey: "idempotency.export.1",
        kind: "thread.export.create",
        format: "canonical_event_bundle",
        artifactAudience: { kind: "owner_only" },
      }),
    ).toMatchObject({
      kind: "thread.export.create",
      format: "canonical_event_bundle",
      artifactAudience: { kind: "owner_only" },
    });
  });

  test("rejects raw thread material, malformed identity, timestamps, and cross-workspace policy", () => {
    expect(() =>
      decodeThreadDisclosureIntent({ ...visibilityIntent, transcript: "private" }),
    ).toThrow();
    expect(() =>
      decodeThreadDisclosureIntent({ ...visibilityIntent, threadRef: "../private" }),
    ).toThrow();
    expect(() =>
      decodeThreadDisclosureIntent({ ...visibilityIntent, createdAt: "not-a-time" }),
    ).toThrow();
    expect(() =>
      decodeThreadDisclosureIntent({
        ...visibilityIntent,
        target: {
          ...visibilityIntent.target,
          administratorAccess: { kind: "workspace_admins", workspaceRef: "workspace.other" },
        },
      }),
    ).toThrow("same workspace");
  });

  test("keeps pending, rejected, and failed receipt evidence distinct", () => {
    for (const result of [
      { status: "accepted_pending" },
      { status: "rejected", reasonRef: "reason.policy.denied" },
      { status: "failed", reasonRef: "reason.transport.failed" },
    ]) {
      expect(
        decodeThreadDisclosureReceipt({
          ...receiptBase,
          kind: "thread.visibility.set",
          result,
        }).result.status,
      ).toBe(result.status);
    }
  });

  test("binds an applied visibility receipt to exact policy evidence", () => {
    expect(
      decodeThreadDisclosureReceipt({
        ...receiptBase,
        kind: "thread.visibility.set",
        result: {
          status: "visibility_applied",
          visibilityVersion: 4,
          target: visibilityIntent.target,
        },
      }),
    ).toMatchObject({
      kind: "thread.visibility.set",
      result: { status: "visibility_applied", visibilityVersion: 4 },
    });
  });

  test("binds export creation to artifact ref, digest, format, and audience only", () => {
    const receipt = decodeThreadDisclosureReceipt({
      ...receiptBase,
      kind: "thread.export.create",
      result: {
        status: "export_created",
        artifactRef: "artifact.thread_export.1",
        artifactSha256: "a".repeat(64),
        format: "json",
        artifactAudience: { kind: "internet_readable" },
      },
    });
    expect(receipt.result).toMatchObject({
      status: "export_created",
      artifactRef: "artifact.thread_export.1",
      artifactAudience: { kind: "internet_readable" },
    });
    expect(JSON.stringify(receipt)).not.toContain("content");
    expect(() =>
      decodeThreadDisclosureReceipt({
        ...receiptBase,
        kind: "thread.export.create",
        result: {
          status: "export_created",
          artifactRef: "artifact.thread_export.1",
          artifactSha256: "not-a-digest",
          format: "json",
          artifactAudience: { kind: "owner_only" },
        },
      }),
    ).toThrow();
  });

  test("fails closed when an applied receipt contradicts its intent kind", () => {
    expect(() =>
      decodeThreadDisclosureReceipt({
        ...receiptBase,
        kind: "thread.export.create",
        result: {
          status: "visibility_applied",
          visibilityVersion: 4,
          target: visibilityIntent.target,
        },
      }),
    ).toThrow("cannot produce");
    expect(() =>
      decodeThreadDisclosureReceipt({
        ...receiptBase,
        kind: "thread.visibility.set",
        result: {
          status: "export_created",
          artifactRef: "artifact.thread_export.1",
          artifactSha256: "b".repeat(64),
          format: "markdown",
          artifactAudience: { kind: "owner_only" },
        },
      }),
    ).toThrow("cannot produce");
  });

  test("classifies new, exact retry, and conflicting identity reuse", () => {
    const decoded = decodeThreadDisclosureIntent(visibilityIntent);
    expect(classifyThreadDisclosureReplay(decoded, decoded)).toBe("exact_retry");
    expect(
      classifyThreadDisclosureReplay(
        decoded,
        decodeThreadDisclosureIntent({
          ...visibilityIntent,
          intentRef: "intent.visibility.2",
          idempotencyKey: "idempotency.visibility.2",
        }),
      ),
    ).toBe("new");
    expect(
      classifyThreadDisclosureReplay(
        decoded,
        decodeThreadDisclosureIntent({
          ...visibilityIntent,
          target: {
            audience: { kind: "internet_readable" },
            administratorAccess: { kind: "none" },
          },
        }),
      ),
    ).toBe("conflicting_reuse");
  });
});
