import type { PortableTargetDescriptor } from "@openagentsinc/portable-session-contract";
import { describe, expect, test, vi } from "vite-plus/test";

import type { PortableCommandTargetInstallationPortResolver } from "./portable-command-broker-factory.js";
import {
  createPortableCommandInstallationPortResolver,
  PortableCommandInstallationResolverError,
} from "./portable-command-installation-resolver.js";

const descriptor = (
  targetClass: PortableTargetDescriptor["targetClass"],
): PortableTargetDescriptor => ({
  targetRef: `target.ide13.composite.${targetClass}`,
  targetClass,
  adapterRef: `adapter.ide13.composite.${targetClass}`,
  ownerRef: "owner.ide13.composite",
  compatibilityRef: "compatibility.ide13.composite",
  isolation: targetClass === "owner_local" ? "owner_host_process" : "dedicated_microvm",
  dataPosture: targetClass === "owner_local" ? "owner_device_only" : "openagents_managed_region",
  health: "ready",
});

const scope = (target: PortableTargetDescriptor) => ({
  commandExecutionClaimRef: "claim.ide13.composite",
  ownerRef: target.ownerRef,
  sessionRef: "session.ide13.composite",
  target,
  sourceAttachmentRef: "attachment.ide13.composite.source",
  sourceGeneration: 1,
  destinationAttachmentRef: "attachment.ide13.composite.destination",
  destinationGeneration: 2,
  grantBindings: [],
  capabilityTransfers: [],
});

const port = {
  install: vi.fn(async () => ({
    installationRef: "installation.ide13.composite",
    evidenceRef: "evidence.ide13.composite",
  })),
  wipe: vi.fn(async () => ({ wipeReceiptRef: "wipe.ide13.composite" })),
};

const resolverFor = (targetClass: PortableTargetDescriptor["targetClass"]) => {
  const resolve = vi.fn<PortableCommandTargetInstallationPortResolver["resolve"]>(
    async (input) => ({
      targetRef: input.target.targetRef,
      targetClass,
      adapterRef: input.target.adapterRef,
      port,
    }),
  );
  return { resolver: { resolve }, resolve };
};

describe("portable command installation-port resolver", () => {
  test.each(["owner_local", "openagents_managed"] as const)(
    "routes an admitted %s target to exactly one resolver",
    async (targetClass) => {
      const ownerLocal = resolverFor("owner_local");
      const openAgentsManaged = resolverFor("openagents_managed");
      const resolver = createPortableCommandInstallationPortResolver({
        ownerLocal: ownerLocal.resolver,
        openAgentsManaged: openAgentsManaged.resolver,
      });
      const target = descriptor(targetClass);

      await expect(resolver.resolve(scope(target))).resolves.toMatchObject({
        targetRef: target.targetRef,
        targetClass,
        adapterRef: target.adapterRef,
      });
      expect(ownerLocal.resolve).toHaveBeenCalledTimes(targetClass === "owner_local" ? 1 : 0);
      expect(openAgentsManaged.resolve).toHaveBeenCalledTimes(
        targetClass === "openagents_managed" ? 1 : 0,
      );
    },
  );

  test.each(["owner_managed", "managed_provider"] as const)(
    "keeps unsupported %s targets explicit without calling an adapter",
    async (targetClass) => {
      const ownerLocal = resolverFor("owner_local");
      const openAgentsManaged = resolverFor("openagents_managed");
      const resolver = createPortableCommandInstallationPortResolver({
        ownerLocal: ownerLocal.resolver,
        openAgentsManaged: openAgentsManaged.resolver,
      });

      await expect(resolver.resolve(scope(descriptor(targetClass)))).resolves.toBeNull();
      expect(ownerLocal.resolve).not.toHaveBeenCalled();
      expect(openAgentsManaged.resolve).not.toHaveBeenCalled();
    },
  );

  test("rejects a selected resolver that returns a mismatched target", async () => {
    const ownerLocal = resolverFor("openagents_managed");
    const openAgentsManaged = resolverFor("openagents_managed");
    const resolver = createPortableCommandInstallationPortResolver({
      ownerLocal: ownerLocal.resolver,
      openAgentsManaged: openAgentsManaged.resolver,
    });

    await expect(resolver.resolve(scope(descriptor("owner_local")))).rejects.toBeInstanceOf(
      PortableCommandInstallationResolverError,
    );
  });
});
