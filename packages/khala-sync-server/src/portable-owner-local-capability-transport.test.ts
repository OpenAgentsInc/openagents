import { createHash } from "node:crypto";

import type { PortableCapabilityLease, SecretMaterial } from "@openagentsinc/portable-session-contract";
import { describe, expect, test } from "vite-plus/test";

import { makePylonOwnerLocalCapabilityTransportHandler } from "../../../apps/pylon/src/portable-owner-local-capability-transport.js";
import { OwnerLocalRemoteCapabilityInstallationPort } from "./portable-owner-local-capability-transport.js";

const authority = {
  commandExecutionClaimRef: "claim.ide13.capability",
  ownerRef: "owner.ide13.capability",
  pylonRef: "pylon.ide13.capability",
  sessionRef: "session.ide13.capability",
  attachmentRef: "attachment.ide13.capability",
  attachmentGeneration: 7,
  targetRef: "target.ide13.capability",
} as const;
const lease: PortableCapabilityLease = {
  leaseRef: "lease.ide13.capability", ownerRef: authority.ownerRef,
  sessionRef: authority.sessionRef, attachmentRef: authority.attachmentRef,
  attachmentGeneration: authority.attachmentGeneration, targetRef: authority.targetRef,
  capability: "provider", expiresAt: "2026-07-20T13:00:00.000Z", state: "issued",
};
const ref = (prefix: string, value: string) =>
  `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 32)}`;

describe("owner-local remote capability transport", () => {
  test("survives lost install and wipe ACKs with exact claim-bound idempotency", async () => {
    let installedDigest: string | undefined;
    let installCalls = 0;
    let wipeCalls = 0;
    const handler = makePylonOwnerLocalCapabilityTransportHandler({
      bearerToken: "private-pylon-bearer-ide13",
      authorize: async input => JSON.stringify(input) === JSON.stringify(authority),
      port: {
        install: async input => {
          installCalls += 1;
          const next = createHash("sha256").update(input.material).digest("hex");
          if (installedDigest !== undefined && installedDigest !== next) throw new Error("conflict");
          installedDigest = next;
          return { installationRef: ref("installation.capability", input.lease.leaseRef),
            evidenceRef: ref("evidence.capability-installed", input.lease.leaseRef) };
        },
        wipe: async input => {
          wipeCalls += 1;
          installedDigest = undefined;
          return { wipeReceiptRef: ref("receipt.capability-wiped", input.installationRef) };
        },
      },
    });
    let loseInstallAck = true;
    let loseWipeAck = true;
    const outboundBodies: Buffer[] = [];
    const fetch = async (input: string | URL | Request, init?: RequestInit) => {
      if (Buffer.isBuffer(init?.body)) outboundBodies.push(init.body);
      const response = await handler(new Request(input, init));
      const path = new URL(input instanceof Request ? input.url : input.toString()).pathname;
      if (path.endsWith("/install") && loseInstallAck) { loseInstallAck = false; throw new Error("lost ACK"); }
      if (path.endsWith("/wipe") && loseWipeAck) { loseWipeAck = false; throw new Error("lost ACK"); }
      return response;
    };
    const port = new OwnerLocalRemoteCapabilityInstallationPort({
      baseUrl: "https://pylon.example", bearerToken: "private-pylon-bearer-ide13",
      authority, fetch,
    });
    const material = new TextEncoder().encode("private fixture material") as SecretMaterial;
    const installInput = { lease, permissions: ["provider.turn.execute"], material };
    await expect(port.install(installInput)).rejects.toMatchObject({ reason: "unavailable" });
    const installed = await port.install(installInput);
    expect(installCalls).toBe(2);
    expect(outboundBodies).toHaveLength(2);
    expect(outboundBodies.every(body => body.every(byte => byte === 0))).toBe(true);
    expect(installed.installationRef).toMatch(/^installation\.capability\./u);
    const wipeInput = { leaseRef: lease.leaseRef, targetRef: lease.targetRef,
      attachmentRef: lease.attachmentRef, attachmentGeneration: lease.attachmentGeneration,
      installationRef: installed.installationRef };
    await expect(port.wipe(wipeInput)).rejects.toMatchObject({ reason: "unavailable" });
    await expect(port.wipe(wipeInput)).resolves.toMatchObject({ wipeReceiptRef: expect.any(String) });
    expect(wipeCalls).toBe(2);
  });

  test("refuses authority drift before reading secret bytes and returns no material", async () => {
    let installs = 0;
    const handler = makePylonOwnerLocalCapabilityTransportHandler({
      bearerToken: "private-pylon-bearer-ide13",
      authorize: async () => false,
      port: { install: async () => { installs += 1; throw new Error("must not run"); },
        wipe: async () => { throw new Error("must not run"); } },
    });
    const port = new OwnerLocalRemoteCapabilityInstallationPort({
      baseUrl: "https://pylon.example", bearerToken: "private-pylon-bearer-ide13",
      authority, fetch: (input, init) => handler(new Request(input, init)),
    });
    await expect(port.install({ lease, permissions: ["provider.turn.execute"],
      material: new Uint8Array([1, 2, 3]) as SecretMaterial })).rejects.toMatchObject({ reason: "refused" });
    expect(installs).toBe(0);
  });
});
