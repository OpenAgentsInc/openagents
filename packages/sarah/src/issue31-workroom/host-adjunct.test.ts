import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { describe, expect, test } from "vite-plus/test";

import {
  ISSUE31_HOST_ADJUNCT_SCHEMA,
  MAX_ISSUE31_TIMESTAMP_MS,
  decodeIssue31HostAdjunct,
  isIssue31PublicRef,
} from "./host-adjunct.ts";

const fixture = (name: string): string =>
  readFileSync(new URL(`../../fixtures/issue31-workroom/${name}`, import.meta.url), "utf8");

const canonicalName = "openagents.omega.issue31.host.v1.canonical.json";
const negativePrivateName = "openagents.omega.issue31.host.v1.negative-private-field.json";
const negativeUnsafeName = "openagents.omega.issue31.host.v1.negative-unsafe-ref.json";
const negativeStateName = "openagents.omega.issue31.host.v1.negative-invalid-state.json";

describe(ISSUE31_HOST_ADJUNCT_SCHEMA, () => {
  test("decodes the byte-shared canonical Rust and TypeScript fixture", () => {
    const bytes = fixture(canonicalName);
    expect(createHash("sha256").update(bytes).digest("hex")).toBe(
      "c5ef757ef8787e7626cdad98ef50a83a763d90067c2b7bb4972783b032bb825d",
    );
    const decoded = decodeIssue31HostAdjunct(JSON.parse(bytes));
    expect(decoded.schema).toBe(ISSUE31_HOST_ADJUNCT_SCHEMA);
    expect(decoded.projections.map((projection) => projection.capability)).toEqual([
      "connection_identity",
      "full_auto_runs",
      "provider_accounts",
      "evidence_chain",
    ]);
    expect(decoded.projections.map((projection) => projection.commandState.kind)).toEqual([
      "idle",
      "pending",
      "refused",
      "terminal",
    ]);
  });

  test("rejects every byte-shared negative fixture", () => {
    expect(() => decodeIssue31HostAdjunct(JSON.parse(fixture(negativePrivateName)))).toThrow();
    expect(() => decodeIssue31HostAdjunct(JSON.parse(fixture(negativeUnsafeName)))).toThrow();
    expect(() => decodeIssue31HostAdjunct(JSON.parse(fixture(negativeStateName)))).toThrow(
      /projection state/,
    );
  });

  test("enforces capability, reference, role, and command laws", () => {
    const canonical = JSON.parse(fixture(canonicalName));
    expect(() =>
      decodeIssue31HostAdjunct({
        ...canonical,
        projections: canonical.projections.slice(0, 3),
      }),
    ).toThrow();
    expect(() =>
      decodeIssue31HostAdjunct({
        ...canonical,
        projections: canonical.projections.map((projection: unknown, index: number) =>
          index === 1
            ? { ...(projection as Record<string, unknown>), capability: "connection_identity" }
            : projection,
        ),
      }),
    ).toThrow(/four unique/);
    expect(() =>
      decodeIssue31HostAdjunct({
        ...canonical,
        projections: canonical.projections.map((projection: unknown, index: number) =>
          index === 1
            ? {
                ...(projection as Record<string, unknown>),
                role: { kind: "owner", status: "revoked", grantRef: "grant.omega.revoked" },
              }
            : projection,
        ),
      }),
    ).toThrow(/role state/);
    expect(() =>
      decodeIssue31HostAdjunct({
        ...canonical,
        projections: canonical.projections.map((projection: unknown, index: number) =>
          index === 1
            ? {
                ...(projection as Record<string, unknown>),
                commandState: {
                  kind: "pending",
                  intentRef: "intent.full-auto.pause.02",
                  actionRef: "action.full-auto.not-permitted",
                },
              }
            : projection,
        ),
      }),
    ).toThrow(/command state/);
  });

  test("rejects Nostr secrets and timestamps that cannot become mobile dates", () => {
    expect(isIssue31PublicRef("nsec1ownersecret")).toBe(false);
    expect(isIssue31PublicRef("ncryptsec1encryptedsecret")).toBe(false);
    expect(isIssue31PublicRef("authorization:owner_token")).toBe(false);
    expect(isIssue31PublicRef("credential.pem")).toBe(false);
    expect(isIssue31PublicRef("provider.key")).toBe(false);
    expect(isIssue31PublicRef("id_rsa_backup")).toBe(false);
    expect(isIssue31PublicRef(" host.omega.device-alpha")).toBe(false);
    expect(isIssue31PublicRef("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")).toBe(false);
    expect(isIssue31PublicRef("npub1publicidentity")).toBe(true);
    const canonical = JSON.parse(fixture(canonicalName));
    expect(() =>
      decodeIssue31HostAdjunct({
        ...canonical,
        generatedAtMs: MAX_ISSUE31_TIMESTAMP_MS + 1,
      }),
    ).toThrow();
  });
});
