import { describe, expect, test } from "vite-plus/test";

import {
  DesktopThreadVisibilityAuthorizationRequestSchemaLiteral,
  evaluateDesktopThreadVisibilityAudience,
} from "./thread-visibility-audience-authorization.ts";

const target = {
  audience: { kind: "owner_only" as const },
  administratorAccess: { kind: "none" as const },
};

const receipt = (overrides: Record<string, unknown> = {}) => ({
  schema: "openagents.thread_disclosure_receipt.v1" as const,
  receiptRef: "receipt.visibility.authorization.1",
  intentRef: "intent.visibility.authorization.1",
  idempotencyKey: "idempotency.visibility.authorization.1",
  threadRef: "thread.visibility.authorization.1",
  observedAt: "2026-07-17T19:03:00.000Z",
  kind: "thread.visibility.set" as const,
  result: {
    status: "visibility_applied" as const,
    visibilityVersion: 7,
    target,
  },
  ...overrides,
});

const request = (overrides: Record<string, unknown> = {}) => ({
  schema: DesktopThreadVisibilityAuthorizationRequestSchemaLiteral,
  actorRef: "actor.reader.1",
  ownerRef: "actor.owner.1",
  receipt: receipt(),
  authorities: [] as ReadonlyArray<unknown>,
  ...overrides,
});

const authorized = (basis: string) => ({
  status: "authorized",
  basis,
  receiptRef: "receipt.visibility.authorization.1",
  threadRef: "thread.visibility.authorization.1",
  visibilityVersion: 7,
});

describe("Desktop thread-visibility audience authorization", () => {
  test("authorizes the exact owner without inferring broader audience", () => {
    expect(
      evaluateDesktopThreadVisibilityAudience(request({ actorRef: "actor.owner.1" })),
    ).toEqual(authorized("owner"));
  });

  test("authorizes an internet-readable applied target without workspace facts", () => {
    expect(
      evaluateDesktopThreadVisibilityAudience(
        request({
          receipt: receipt({
            result: {
              status: "visibility_applied",
              visibilityVersion: 7,
              target: {
                audience: { kind: "internet_readable" },
                administratorAccess: { kind: "none" },
              },
            },
          }),
        }),
      ),
    ).toEqual(authorized("internet_readable"));
  });

  test("authorizes matching workspace membership and matching named group only", () => {
    const authorities = [
      { workspaceRef: "workspace.1", role: "member", groupRefs: ["group.1"] },
    ];
    expect(
      evaluateDesktopThreadVisibilityAudience(
        request({
          authorities,
          receipt: receipt({
            result: {
              status: "visibility_applied",
              visibilityVersion: 7,
              target: {
                audience: { kind: "workspace_members", workspaceRef: "workspace.1" },
                administratorAccess: { kind: "none" },
              },
            },
          }),
        }),
      ),
    ).toEqual(authorized("workspace_member"));
    expect(
      evaluateDesktopThreadVisibilityAudience(
        request({
          authorities,
          receipt: receipt({
            result: {
              status: "visibility_applied",
              visibilityVersion: 7,
              target: {
                audience: {
                  kind: "named_group",
                  workspaceRef: "workspace.1",
                  groupRef: "group.1",
                },
                administratorAccess: { kind: "none" },
              },
            },
          }),
        }),
      ),
    ).toEqual(authorized("named_group"));
  });

  test("authorizes only an explicit matching administrator fallback", () => {
    expect(
      evaluateDesktopThreadVisibilityAudience(
        request({
          authorities: [
            { workspaceRef: "workspace.1", role: "administrator", groupRefs: [] },
          ],
          receipt: receipt({
            result: {
              status: "visibility_applied",
              visibilityVersion: 7,
              target: {
                audience: { kind: "owner_only" },
                administratorAccess: { kind: "workspace_admins", workspaceRef: "workspace.1" },
              },
            },
          }),
        }),
      ),
    ).toEqual(authorized("workspace_administrator"));
  });

  test("denies missing, wrong-workspace, and wrong-group authority facts", () => {
    for (const value of [
      request(),
      request({
        authorities: [{ workspaceRef: "workspace.other", role: "member", groupRefs: [] }],
        receipt: receipt({
          result: {
            status: "visibility_applied",
            visibilityVersion: 7,
            target: {
              audience: { kind: "workspace_members", workspaceRef: "workspace.1" },
              administratorAccess: { kind: "none" },
            },
          },
        }),
      }),
      request({
        authorities: [{ workspaceRef: "workspace.1", role: "member", groupRefs: ["group.other"] }],
        receipt: receipt({
          result: {
            status: "visibility_applied",
            visibilityVersion: 7,
            target: {
              audience: { kind: "named_group", workspaceRef: "workspace.1", groupRef: "group.1" },
              administratorAccess: { kind: "none" },
            },
          },
        }),
      }),
    ]) {
      expect(evaluateDesktopThreadVisibilityAudience(value)).toEqual({
        status: "denied",
        reason: "no_matching_authority",
        receiptRef: "receipt.visibility.authorization.1",
        threadRef: "thread.visibility.authorization.1",
        visibilityVersion: 7,
      });
    }
  });

  test("rejects raw content, extra envelopes, export receipts, and malformed targets", () => {
    for (const value of [
      request({ transcript: "private thread" }),
      request({ receipt: { ...receipt(), content: "private thread" } }),
      request({
        receipt: receipt({
          kind: "thread.export.create",
          result: {
            status: "export_created",
            artifactRef: "artifact.1",
            artifactSha256: "a".repeat(64),
            format: "canonical_event_bundle",
            artifactAudience: { kind: "owner_only" },
          },
        }),
      }),
      request({
        receipt: receipt({
          result: {
            status: "visibility_applied",
            visibilityVersion: 7,
            target: {
              audience: { kind: "workspace_members", workspaceRef: "workspace.1", members: [] },
              administratorAccess: { kind: "none" },
            },
          },
        }),
      }),
    ]) {
      expect(evaluateDesktopThreadVisibilityAudience(value)).toEqual({
        status: "rejected",
        reason: "invalid_request",
      });
    }
  });

  test("rejects duplicate workspace facts, duplicate groups, and oversized authority", () => {
    for (const authorities of [
      [
        { workspaceRef: "workspace.1", role: "member", groupRefs: [] },
        { workspaceRef: "workspace.1", role: "administrator", groupRefs: [] },
      ],
      [{ workspaceRef: "workspace.1", role: "member", groupRefs: ["group.1", "group.1"] }],
      Array.from({ length: 33 }, (_, index) => ({
        workspaceRef: `workspace.${index}`,
        role: "member",
        groupRefs: [],
      })),
    ]) {
      expect(evaluateDesktopThreadVisibilityAudience(request({ authorities }))).toEqual({
        status: "rejected",
        reason: "invalid_request",
      });
    }
  });
});
