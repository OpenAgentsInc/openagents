import type { PortableOwnerLocalCapabilityMaterialAuthority } from "./portable-owner-local-capability-operation-routes";

type GrantState = "issued" | "used";

type ProviderGrant = Readonly<{
  grantRef: string;
  userId: string;
  provider: string;
  providerAccountRef: string;
  runnerSessionId: string | null;
  requestedAction: string | null;
  metadataJson: string | null;
  status: string;
  expiresAt: string;
}>;

type ResolvedProviderGrant = Readonly<{
  grantRef: string;
  ownerUserId: string;
  providerAccountRef: string;
  runnerSessionId?: string | undefined;
  requestedAction?: string | undefined;
  status: "used";
}>;

type GitHubGrant = Readonly<{
  grantRef: string;
  userId: string;
  connectionRef: string;
  secretRef: string;
  runnerSessionId: string | null;
  requestedAction: string | null;
  metadataJson: string | null;
  status: string;
  expiresAt: string;
}>;

type ResolvedGitHubGrant = Readonly<{
  grantRef: string;
  connectionRef: string;
  runnerSessionId?: string | undefined;
  requestedAction?: string | undefined;
  scopes: ReadonlyArray<string>;
}>;

type GitHubConnection = Readonly<{
  connectionRef: string;
  secretRef: string | null;
  scopes: ReadonlyArray<string>;
}>;

export type PortableOwnerLocalCapabilityMaterialAuthorityDependencies = Readonly<{
  recheckAuthority: () => Promise<Readonly<{ destinationRunnerSessionRef: string }>>;
  readProviderGrant: (grantRef: string) => Promise<ProviderGrant | undefined>;
  resolveProviderGrant: (input: Readonly<{
    actorAgentUserId: string;
    grantRef: string;
    providerAccountRef: string;
    runnerSessionRef: string;
  }>) => Promise<ResolvedProviderGrant | undefined>;
  readProviderMaterial: (
    ownerRef: string,
    providerAccountRef: string,
  ) => Promise<Uint8Array | undefined>;
  readGitHubGrant: (grantRef: string) => Promise<GitHubGrant | undefined>;
  resolveGitHubGrant: (input: Readonly<{
    grantRef: string;
    runnerSessionRef: string;
  }>) => Promise<ResolvedGitHubGrant | undefined>;
  readGitHubConnection: (ownerRef: string) => Promise<GitHubConnection | undefined>;
  readGitHubMaterial: (connectionRef: string) => Promise<Uint8Array | undefined>;
  githubScopesSatisfy: (scopes: ReadonlyArray<string>) => boolean;
  providerKind: string;
  now?: (() => Date) | undefined;
}>;

const EXPECTED_ACTION = "portable_session_resume";

const validState = (state: string): state is GrantState =>
  state === "issued" || state === "used";

const hasExactAncestry = (metadataJson: string | null, sourceGrantRef: string): boolean =>
  metadataJson?.includes(`\"reissuedFromGrantRef\":\"${sourceGrantRef}\"`) === true;

/**
 * Builds the in-process, one-shot material authority. It validates the exact
 * reissued destination grant and checks command/operation/Pylon authority both
 * before and after custody access. A post-read drift clears the private buffer.
 */
export const makePortableOwnerLocalCapabilityMaterialAuthority = (
  authority: PortableOwnerLocalCapabilityMaterialAuthority,
  dependencies: PortableOwnerLocalCapabilityMaterialAuthorityDependencies,
): (() => Promise<Uint8Array>) => async () => {
  const firstAuthority = await dependencies.recheckAuthority();
  const runnerSessionRef = firstAuthority.destinationRunnerSessionRef;
  const now = (dependencies.now ?? (() => new Date()))();
  let material: Uint8Array | undefined;
  let grantExpiresAt: string | undefined;

  try {
    if (authority.capability === "provider") {
      const existing = await dependencies.readProviderGrant(authority.destinationGrantRef);
      if (
        existing === undefined ||
        existing.grantRef !== authority.destinationGrantRef ||
        existing.userId !== authority.ownerRef ||
        existing.provider !== dependencies.providerKind ||
        !validState(existing.status) ||
        existing.runnerSessionId !== runnerSessionRef ||
        existing.requestedAction !== EXPECTED_ACTION ||
        !hasExactAncestry(existing.metadataJson, authority.sourceGrantRef) ||
        Date.parse(existing.expiresAt) <= now.getTime()
      ) {
        throw new Error("provider destination grant scope is invalid");
      }
      grantExpiresAt = existing.expiresAt;
      if (existing.status === "issued") {
        const resolved = await dependencies.resolveProviderGrant({
          actorAgentUserId: authority.actorAgentUserId,
          grantRef: authority.destinationGrantRef,
          providerAccountRef: existing.providerAccountRef,
          runnerSessionRef,
        });
        if (
          resolved === undefined ||
          resolved.grantRef !== authority.destinationGrantRef ||
          resolved.ownerUserId !== authority.ownerRef ||
          resolved.providerAccountRef !== existing.providerAccountRef ||
          resolved.runnerSessionId !== runnerSessionRef ||
          resolved.requestedAction !== EXPECTED_ACTION ||
          resolved.status !== "used"
        ) {
          throw new Error("provider destination grant resolution changed scope");
        }
      }
      const bytes = await dependencies.readProviderMaterial(
        authority.ownerRef,
        existing.providerAccountRef,
      );
      if (bytes === undefined || bytes.byteLength === 0) {
        throw new Error("provider destination grant material is unavailable");
      }
      material = bytes;
    } else if (authority.capability === "scm_read" || authority.capability === "scm_write") {
      const existing = await dependencies.readGitHubGrant(authority.destinationGrantRef);
      if (
        existing === undefined ||
        existing.grantRef !== authority.destinationGrantRef ||
        existing.userId !== authority.ownerRef ||
        !validState(existing.status) ||
        existing.runnerSessionId !== runnerSessionRef ||
        existing.requestedAction !== EXPECTED_ACTION ||
        !hasExactAncestry(existing.metadataJson, authority.sourceGrantRef) ||
        Date.parse(existing.expiresAt) <= now.getTime()
      ) {
        throw new Error("GitHub destination grant scope is invalid");
      }
      grantExpiresAt = existing.expiresAt;
      if (existing.status === "issued") {
        const resolved = await dependencies.resolveGitHubGrant({
          grantRef: authority.destinationGrantRef,
          runnerSessionRef,
        });
        if (
          resolved === undefined ||
          resolved.grantRef !== authority.destinationGrantRef ||
          resolved.runnerSessionId !== runnerSessionRef ||
          resolved.requestedAction !== EXPECTED_ACTION ||
          resolved.connectionRef !== existing.connectionRef ||
          !dependencies.githubScopesSatisfy(resolved.scopes)
        ) {
          throw new Error("GitHub destination grant resolution changed scope");
        }
      }
      const connection = await dependencies.readGitHubConnection(authority.ownerRef);
      if (
        connection === undefined ||
        connection.connectionRef !== existing.connectionRef ||
        connection.secretRef !== existing.secretRef ||
        !dependencies.githubScopesSatisfy(connection.scopes)
      ) {
        throw new Error("GitHub destination connection scope is invalid");
      }
      const bytes = await dependencies.readGitHubMaterial(existing.connectionRef);
      if (bytes === undefined || bytes.byteLength === 0) {
        throw new Error("GitHub destination grant material is unavailable");
      }
      material = bytes;
    } else {
      throw new Error("capability kind has no admitted material authority");
    }

    const finalAuthority = await dependencies.recheckAuthority();
    if (
      finalAuthority.destinationRunnerSessionRef !== runnerSessionRef ||
      grantExpiresAt === undefined ||
      Date.parse(grantExpiresAt) <= (dependencies.now ?? (() => new Date()))().getTime()
    ) {
      throw new Error("destination runner session authority changed");
    }
    return material;
  } catch (error) {
    material?.fill(0);
    throw error;
  }
};
