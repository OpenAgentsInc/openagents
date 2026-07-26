import { createHash } from "node:crypto";

import {
  ForgeGitHubMirrorObservedState,
  type ForgeGitHubMirrorIntent,
} from "@openagentsinc/forge-protocol";
import { Context, Effect, Layer, Redacted, Schema } from "effect";

import { type ForgeGitRepositoryShape, ForgeGitRepository } from "./repository.js";

export type ForgeGitHubMirrorDestination = Readonly<{
  authorizationHeader?: Redacted.Redacted<string> | undefined;
  destinationUrl: string;
  sshKeyPath?: string | undefined;
}>;

export type ForgeGitHubMirrorDestinationResolver = (
  input: Readonly<{
    destinationGithubRepository: string;
    repositoryRef: string;
    tenantRef: string;
  }>,
) => Effect.Effect<ForgeGitHubMirrorDestination, ForgeGitHubMirrorRunnerError>;

export const makeGitHubHttpsMirrorDestinationResolver = (input: {
  readonly allowedRepositories: ReadonlySet<string>;
  readonly githubToken: Redacted.Redacted<string>;
}): ForgeGitHubMirrorDestinationResolver =>
  Effect.fn("ForgeGitHubMirrorDestination.resolve")(function* (request) {
    const repository = request.destinationGithubRepository;
    if (
      !/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/u.test(repository) ||
      !input.allowedRepositories.has(repository)
    ) {
      return yield* new ForgeGitHubMirrorRunnerError({
        operation: "ForgeGitHubMirrorDestination.resolve",
        reason: "forge_github_mirror_destination_not_allowed",
        retryable: false,
      });
    }
    return {
      authorizationHeader: Redacted.make(
        `Authorization: Bearer ${Redacted.value(input.githubToken)}`,
      ),
      destinationUrl: `https://github.com/${repository}.git`,
    };
  });

export const makeGitHubSshMirrorDestinationResolver = (input: {
  readonly allowedRepositories: ReadonlySet<string>;
  readonly sshKeyPath: string;
}): ForgeGitHubMirrorDestinationResolver =>
  Effect.fn("ForgeGitHubMirrorDestination.resolveSsh")(function* (request) {
    const repository = request.destinationGithubRepository;
    if (
      !/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/u.test(repository) ||
      !input.allowedRepositories.has(repository)
    ) {
      return yield* new ForgeGitHubMirrorRunnerError({
        operation: "ForgeGitHubMirrorDestination.resolveSsh",
        reason: "forge_github_mirror_destination_not_allowed",
        retryable: false,
      });
    }
    return {
      destinationUrl: `git@github.com:${repository}.git`,
      sshKeyPath: input.sshKeyPath,
    };
  });

export class ForgeGitHubMirrorRunnerError extends Schema.TaggedErrorClass<ForgeGitHubMirrorRunnerError>()(
  "ForgeGitHubMirrorRunnerError",
  {
    operation: Schema.String,
    reason: Schema.String,
    retryable: Schema.Boolean,
  },
) {}

export interface ForgeGitHubMirrorRunnerShape {
  readonly observe: (
    intent: ForgeGitHubMirrorIntent,
  ) => Effect.Effect<ForgeGitHubMirrorObservedState, ForgeGitHubMirrorRunnerError>;
  readonly project: (
    intent: ForgeGitHubMirrorIntent,
  ) => Effect.Effect<ForgeGitHubMirrorObservedState, ForgeGitHubMirrorRunnerError>;
}

export class ForgeGitHubMirrorRunner extends Context.Service<
  ForgeGitHubMirrorRunner,
  ForgeGitHubMirrorRunnerShape
>()("@openagentsinc/forge-git-service/GitHubMirrorRunner") {}

const observationRef = (
  intent: ForgeGitHubMirrorIntent,
  observation: Readonly<{
    destinationObjectId: string | null;
    divergence: string;
    observedAt: string;
  }>,
): string => {
  const digest = createHash("sha256")
    .update(
      [
        intent.intent_ref,
        observation.destinationObjectId ?? "missing",
        observation.divergence,
        observation.observedAt,
      ].join(":"),
    )
    .digest("hex");
  return `observation.forge.github-mirror.${digest.slice(0, 32)}`;
};

const assertOwnedIntent = (
  intent: ForgeGitHubMirrorIntent,
): Effect.Effect<void, ForgeGitHubMirrorRunnerError> =>
  intent.authority_mode === "openagents_git_authoritative"
    ? Effect.void
    : Effect.fail(
        new ForgeGitHubMirrorRunnerError({
          operation: "ForgeGitHubMirrorRunner.assertOwnedIntent",
          reason: "forge_github_mirror_not_owned_authority",
          retryable: false,
        }),
      );

const repositoryError = (operation: string, error: { code: string; status: number }) =>
  new ForgeGitHubMirrorRunnerError({
    operation,
    reason: error.code,
    retryable: error.status >= 500,
  });

export const makeForgeGitHubMirrorRunner = (
  repository: ForgeGitRepositoryShape,
  resolveDestination: ForgeGitHubMirrorDestinationResolver,
): ForgeGitHubMirrorRunnerShape => {
  const run = (
    operation: "observe" | "project",
    intent: ForgeGitHubMirrorIntent,
  ): Effect.Effect<ForgeGitHubMirrorObservedState, ForgeGitHubMirrorRunnerError> =>
    Effect.gen(function* () {
      yield* assertOwnedIntent(intent);
      const destination = yield* resolveDestination({
        destinationGithubRepository: intent.destination_github_repository,
        repositoryRef: intent.repository_ref,
        tenantRef: intent.tenant_ref,
      });
      const input = {
        ...(destination.authorizationHeader === undefined
          ? {}
          : { authorizationHeader: destination.authorizationHeader }),
        ...(destination.sshKeyPath === undefined ? {} : { sshKeyPath: destination.sshKeyPath }),
        destinationRef: intent.destination_github_ref,
        destinationUrl: destination.destinationUrl,
        expectedSourceObjectId: intent.source_object_id,
        repositoryRef: intent.repository_ref,
        sourceRef: intent.source_ref,
        tenantRef: intent.tenant_ref,
      };
      const observed = yield* repository[
        operation === "observe" ? "observeMirror" : "projectMirror"
      ](input).pipe(
        Effect.mapError((error) => repositoryError(`ForgeGitHubMirrorRunner.${operation}`, error)),
      );
      return ForgeGitHubMirrorObservedState.make({
        schema: "openagents.forge.github_mirror.observed_state.v0.1",
        authority_generation: intent.authority_generation,
        authority_mode: intent.authority_mode,
        destination_github_ref: intent.destination_github_ref,
        destination_github_repository: intent.destination_github_repository,
        destination_object_id: observed.destinationObjectId,
        divergence: observed.divergence,
        error_reason: null,
        intent_ref: intent.intent_ref,
        observation_ref: observationRef(intent, observed),
        observed_at: observed.observedAt,
        redacted: true,
        repository_ref: intent.repository_ref,
        source_object_id: observed.sourceObjectId,
        source_ref: intent.source_ref,
        source_refs: [...new Set([...intent.source_refs, intent.intent_ref])],
        tenant_ref: intent.tenant_ref,
      });
    });

  return ForgeGitHubMirrorRunner.of({
    observe: Effect.fn("ForgeGitHubMirrorRunner.observe")((intent) => run("observe", intent)),
    project: Effect.fn("ForgeGitHubMirrorRunner.project")((intent) => run("project", intent)),
  });
};

export const layerForgeGitHubMirrorRunner = (
  resolveDestination: ForgeGitHubMirrorDestinationResolver,
) =>
  Layer.effect(
    ForgeGitHubMirrorRunner,
    Effect.gen(function* () {
      const repository = yield* ForgeGitRepository;
      return makeForgeGitHubMirrorRunner(repository, resolveDestination);
    }),
  );
