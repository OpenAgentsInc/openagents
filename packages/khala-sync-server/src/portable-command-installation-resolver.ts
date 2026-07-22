import { createHash } from "node:crypto";

import { PortableRef } from "@openagentsinc/portable-session-contract";
import { Effect, Schema } from "effect";

import type {
  PortableCommandTargetInstallationPortResolution,
  PortableCommandTargetInstallationPortResolver,
} from "./portable-command-broker-factory.js";

const stableFailureRef = (code: string, scopeRef: string): string =>
  `failure.portable-command-installation-resolver.${createHash("sha256")
    .update(`${code}\u0000${scopeRef}`)
    .digest("hex")}`;

export class PortableCommandInstallationResolverError extends Schema.TaggedErrorClass<PortableCommandInstallationResolverError>()(
  "PortableCommandInstallationResolverError",
  {
    code: Schema.Literals(["invalid_configuration", "resolution_failed", "resolution_mismatch"]),
    failureRef: PortableRef,
  },
) {}

export type PortableCommandInstallationPortResolverSet = Readonly<{
  ownerLocal: PortableCommandTargetInstallationPortResolver;
  openAgentsManaged: PortableCommandTargetInstallationPortResolver;
}>;

/**
 * Routes only the two target classes that have concrete installation ports.
 * Owner-managed and audited-provider targets remain explicitly unsupported.
 */
export const createPortableCommandInstallationPortResolver = (
  resolvers: PortableCommandInstallationPortResolverSet,
): PortableCommandTargetInstallationPortResolver => {
  if (
    typeof resolvers.ownerLocal?.resolve !== "function" ||
    typeof resolvers.openAgentsManaged?.resolve !== "function"
  ) {
    throw new PortableCommandInstallationResolverError({
      code: "invalid_configuration",
      failureRef: stableFailureRef("invalid_configuration", "configuration"),
    });
  }

  const mismatch = (scopeRef: string) =>
    new PortableCommandInstallationResolverError({
      code: "resolution_mismatch",
      failureRef: stableFailureRef("resolution_mismatch", scopeRef),
    });

  const resolveEffect = Effect.fn("PortableCommandInstallationPortResolver.resolve")(
    (input: Parameters<PortableCommandTargetInstallationPortResolver["resolve"]>[0]) =>
      Effect.gen(function* () {
        const resolver =
          input.target.targetClass === "owner_local"
            ? resolvers.ownerLocal
            : input.target.targetClass === "openagents_managed"
              ? resolvers.openAgentsManaged
              : undefined;
        if (resolver === undefined) return null;
        const resolution = yield* Effect.tryPromise({
          try: () => resolver.resolve(input),
          catch: () =>
            new PortableCommandInstallationResolverError({
              code: "resolution_failed",
              failureRef: stableFailureRef("resolution_failed", input.target.targetRef),
            }),
        });
        if (resolution === null) return null;
        if (
          resolution.targetRef !== input.target.targetRef ||
          resolution.targetClass !== input.target.targetClass ||
          resolution.adapterRef !== input.target.adapterRef ||
          typeof resolution.port?.install !== "function" ||
          typeof resolution.port?.wipe !== "function"
        ) {
          return yield* mismatch(input.target.targetRef);
        }
        return resolution satisfies PortableCommandTargetInstallationPortResolution;
      }),
  );

  return { resolve: (input) => Effect.runPromise(resolveEffect(input)) };
};
