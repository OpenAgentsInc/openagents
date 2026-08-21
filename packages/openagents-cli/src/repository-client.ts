import { Duration, Effect, Layer, Option, Redacted, Schedule, Schema } from "effect";
import * as Context from "effect/Context";

import {
  ApiErrorResponse,
  AuthenticatedUser,
  Repository,
  RepositoryImport,
  RepositoryImportAcceptedResponse,
  RepositoryImportStatusResponse,
  RepositoryListResponse,
  RepositoryResponse,
  repositoryFromAcceptedImport,
} from "./api-contract.js";
import { ApiTransport } from "./api-transport.js";
import {
  ApiError,
  ContractError,
  ImportFailed,
  ImportWaitTimeout,
  InputError,
  ProvisioningFailed,
  ProvisioningWaitTimeout,
  TransportError,
  type CliError,
} from "./errors.js";

export interface RepositoryTarget {
  readonly owner: string;
  readonly repo: string;
}

export interface AuthenticatedApi {
  readonly origin: string;
  readonly token: Redacted.Redacted<string>;
}

export interface CreateRepositoryInput extends AuthenticatedApi {
  readonly owner?: string;
  readonly name: string;
  readonly description?: string;
  readonly private: boolean;
  readonly defaultBranch?: string;
  readonly waitTimeoutMs?: number;
  readonly pollIntervalMs?: number;
  readonly idempotencyKey?: string;
}

export interface ImportRepositoryInput extends AuthenticatedApi {
  readonly owner?: string;
  readonly source: string;
  readonly name?: string;
  readonly private: boolean;
  readonly waitTimeoutMs: number;
  readonly pollIntervalMs?: number;
  readonly idempotencyKey?: string;
  readonly onProgress?: (progress: {
    readonly state: RepositoryImport["state"];
    readonly attemptCount: number;
  }) => Effect.Effect<void>;
}

export interface ImportRepositoryResult {
  readonly repository: Repository;
  readonly repositoryImport: RepositoryImport;
}

export interface ListRepositoriesInput extends AuthenticatedApi {
  readonly namespace?: string;
  readonly limit: number;
  readonly after?: string;
}

export interface ListRepositoriesResult {
  readonly repositories: ReadonlyArray<Repository>;
  readonly nextCursor: string | null;
}

export interface CloneRepositoryResult {
  readonly repository: Repository;
  readonly cloneUrl: string;
}

export interface RepositoryClientInterface {
  readonly create: (input: CreateRepositoryInput) => Effect.Effect<Repository, CliError>;
  readonly import: (
    input: ImportRepositoryInput,
  ) => Effect.Effect<ImportRepositoryResult, CliError>;
  readonly authenticatedUser: (
    input: AuthenticatedApi,
  ) => Effect.Effect<AuthenticatedUser, CliError>;
  readonly list: (input: ListRepositoriesInput) => Effect.Effect<ListRepositoriesResult, CliError>;
  readonly view: (
    input: AuthenticatedApi & RepositoryTarget,
  ) => Effect.Effect<Repository, CliError>;
  readonly cloneInfo: (
    input: AuthenticatedApi & RepositoryTarget,
  ) => Effect.Effect<CloneRepositoryResult, CliError>;
  readonly getImport: (
    input: AuthenticatedApi & { readonly importId: string },
  ) => Effect.Effect<RepositoryImport, CliError>;
}

export class RepositoryClient extends Context.Service<
  RepositoryClient,
  RepositoryClientInterface
>()("@openagentsinc/cli/RepositoryClient") {}

const namePattern = /^[a-z0-9](?:[a-z0-9_-]|\.(?=[a-z0-9])){0,63}$/;
const ownerPattern = /^[A-Za-z0-9](?:[A-Za-z0-9-]{0,38})$/;

export const validateRepositoryName = Effect.fn("RepositoryClient.validateName")(function* (
  name: string,
) {
  const normalized = name.trim().toLowerCase();
  if (!namePattern.test(normalized)) {
    return yield* new InputError({
      message: "Repository names must match [a-z0-9](?:[a-z0-9_-]|\\.(?=[a-z0-9])){0,63}.",
    });
  }
  return normalized;
});

export const validateOwner = Effect.fn("RepositoryClient.validateOwner")(function* (owner: string) {
  const normalized = owner.trim();
  if (!ownerPattern.test(normalized)) {
    return yield* new InputError({ message: `Invalid GitHub-backed namespace: ${owner}` });
  }
  return normalized;
});

export const parseRepositoryTarget = Effect.fn("RepositoryClient.parseTarget")(function* (
  fullName: string,
) {
  const parts = fullName.trim().split("/");
  if (parts.length !== 2) {
    return yield* new InputError({ message: "Use the repository format OWNER/REPO." });
  }
  const ownerPart = parts[0];
  const repoPart = parts[1];
  if (ownerPart === undefined || repoPart === undefined) {
    return yield* new InputError({ message: "Use the repository format OWNER/REPO." });
  }
  const owner = yield* validateOwner(ownerPart);
  const repo = yield* validateRepositoryName(repoPart);
  return { owner, repo } satisfies RepositoryTarget;
});

const encoded = (value: string) => encodeURIComponent(value);

const errorMessage = (
  body: unknown,
  status: number,
): { message: string; code?: string; requestId?: string } => {
  const decoded = Schema.decodeUnknownOption(ApiErrorResponse)(body);
  if (Option.isNone(decoded)) return { message: `The API returned HTTP ${status}.` };
  const value = decoded.value;
  const message = value.message ?? value.error ?? `The API returned HTTP ${status}.`;
  return {
    message,
    ...(typeof value.code === "string" ? { code: value.code } : {}),
    ...(typeof value.request_id === "string" ? { requestId: value.request_id } : {}),
  };
};

class ImportPending extends Schema.TaggedErrorClass<ImportPending>()(
  "OpenAgentsCli.Internal.ImportPending",
  { importId: Schema.String },
) {}

class RepositoryPending extends Schema.TaggedErrorClass<RepositoryPending>()(
  "OpenAgentsCli.Internal.RepositoryPending",
  { repository: Schema.String },
) {}

const retryMutation = <A>(effect: Effect.Effect<A, CliError>) =>
  effect.pipe(
    Effect.retry({
      times: 2,
      while: (failure) => failure instanceof TransportError,
    }),
  );

export const repositoryClientLayer = Layer.effect(
  RepositoryClient,
  Effect.gen(function* () {
    const transport = yield* ApiTransport;

    const request = Effect.fn("RepositoryClient.request")(function* (
      operation: string,
      input: AuthenticatedApi & {
        readonly method: "GET" | "POST";
        readonly path: string;
        readonly body?: unknown;
        readonly headers?: Readonly<Record<string, string>>;
        readonly acceptedStatuses: ReadonlyArray<number>;
      },
    ) {
      const response = yield* transport.request({
        origin: input.origin,
        method: input.method,
        path: input.path,
        token: input.token,
        ...(input.headers === undefined ? {} : { headers: input.headers }),
        ...(input.body === undefined ? {} : { body: input.body }),
      });
      if (!input.acceptedStatuses.includes(response.status)) {
        const details = errorMessage(response.body, response.status);
        return yield* new ApiError({
          operation,
          status: response.status,
          ...(details.code === undefined ? {} : { code: details.code }),
          message: details.message,
          ...(response.requestId === undefined && details.requestId === undefined
            ? {}
            : { requestId: response.requestId ?? details.requestId }),
        });
      }
      return response.body;
    });

    const decode = <A, I, R>(
      operation: string,
      schema: Schema.Codec<A, I, R, never>,
      value: unknown,
    ) =>
      Schema.decodeUnknownEffect(schema)(value).pipe(
        Effect.mapError(
          (cause) =>
            new ContractError({
              operation,
              message: `The API response did not match the ${operation} contract.`,
              cause,
            }),
        ),
      );

    const waitForRepository = Effect.fn("RepositoryClient.waitForRepository")(function* (input: {
      readonly origin: string;
      readonly token: Redacted.Redacted<string>;
      readonly owner: string;
      readonly repo: string;
      readonly timeoutMs: number;
      readonly pollIntervalMs: number;
    }) {
      const fullName = `${input.owner}/${input.repo}`;
      const poll = request("read repository provisioning state", {
        ...input,
        method: "GET",
        path: `/api/v3/repos/${encoded(input.owner)}/${encoded(input.repo)}`,
        acceptedStatuses: [200],
      }).pipe(
        Effect.flatMap((value) => decode("read repository provisioning state", Repository, value)),
        Effect.flatMap(
          (repository): Effect.Effect<Repository, ProvisioningFailed | RepositoryPending> => {
            if (repository.lifecycle_state === "ready") return Effect.succeed(repository);
            if (repository.lifecycle_state === "failed") {
              return Effect.fail(
                new ProvisioningFailed({
                  repository: fullName,
                  message:
                    repository.provision_error_code ??
                    `Repository provisioning failed for ${fullName}.`,
                }),
              );
            }
            return Effect.fail(new RepositoryPending({ repository: fullName }));
          },
        ),
      );
      const result = yield* poll.pipe(
        Effect.retry({
          schedule: Schedule.spaced(Duration.millis(input.pollIntervalMs)),
          while: (failure) => failure instanceof RepositoryPending,
        }),
        Effect.timeoutOption(Duration.millis(input.timeoutMs)),
        Effect.catch((failure) =>
          failure instanceof RepositoryPending
            ? Effect.succeed(Option.none())
            : Effect.fail(failure),
        ),
      );
      if (Option.isNone(result)) {
        return yield* new ProvisioningWaitTimeout({
          repository: fullName,
          timeoutMs: input.timeoutMs,
          message: `Repository ${fullName} is still provisioning after ${input.timeoutMs} ms. Provisioning continues on the server.`,
        });
      }
      return result.value;
    });

    const create = Effect.fn("RepositoryClient.create")(function* (input: CreateRepositoryInput) {
      const name = yield* validateRepositoryName(input.name);
      const owner = input.owner === undefined ? undefined : yield* validateOwner(input.owner);
      const idempotencyKey = input.idempotencyKey ?? globalThis.crypto.randomUUID();
      const body = {
        name,
        private: input.private,
        ...(input.description === undefined ? {} : { description: input.description }),
        ...(input.defaultBranch === undefined ? {} : { default_branch: input.defaultBranch }),
      };
      const path =
        owner === undefined ? "/api/v3/user/repos" : `/api/v3/orgs/${encoded(owner)}/repos`;
      const value = yield* retryMutation(
        request("create repository", {
          ...input,
          method: "POST",
          path,
          body,
          headers: { "idempotency-key": idempotencyKey },
          acceptedStatuses: [201, 202],
        }),
      );
      const repository = yield* decode("create repository", RepositoryResponse, value);
      if (repository.lifecycle_state === "ready" || input.waitTimeoutMs === 0) return repository;
      return yield* waitForRepository({
        origin: input.origin,
        token: input.token,
        owner: repository.owner.login,
        repo: repository.name,
        timeoutMs: input.waitTimeoutMs ?? 300_000,
        pollIntervalMs: input.pollIntervalMs ?? 1_000,
      });
    });

    const authenticatedUser = Effect.fn("RepositoryClient.authenticatedUser")(function* (
      input: AuthenticatedApi,
    ) {
      const responseBody = yield* request("read authenticated user", {
        ...input,
        method: "GET",
        path: "/api/v3/user",
        acceptedStatuses: [200],
      });
      return yield* decode("read authenticated user", AuthenticatedUser, responseBody);
    });

    const list = Effect.fn("RepositoryClient.list")(function* (input: ListRepositoriesInput) {
      if (!Number.isInteger(input.limit) || input.limit < 1 || input.limit > 100) {
        return yield* new InputError({ message: "--limit must be between 1 and 100." });
      }
      const namespace =
        input.namespace === undefined ? undefined : yield* validateOwner(input.namespace);
      const parameters = new URLSearchParams({ per_page: String(input.limit) });
      if (input.after !== undefined) parameters.set("after", input.after);
      if (namespace !== undefined) parameters.set("namespace", namespace);
      const responseBody = yield* request("list repositories", {
        ...input,
        method: "GET",
        path: `/api/v3/user/repos?${parameters.toString()}`,
        acceptedStatuses: [200],
      });
      const pageResponse = yield* decode("list repositories", RepositoryListResponse, responseBody);
      return {
        repositories: pageResponse.repositories,
        nextCursor: pageResponse.next_cursor,
      };
    });

    const view = Effect.fn("RepositoryClient.view")(function* (
      input: AuthenticatedApi & RepositoryTarget,
    ) {
      const owner = yield* validateOwner(input.owner);
      const repo = yield* validateRepositoryName(input.repo);
      const value = yield* request("view repository", {
        ...input,
        method: "GET",
        path: `/api/v3/repos/${encoded(owner)}/${encoded(repo)}`,
        acceptedStatuses: [200],
      });
      return yield* decode("view repository", RepositoryResponse, value);
    });

    const getImportStatus = Effect.fn("RepositoryClient.getImportStatus")(function* (
      input: AuthenticatedApi & { readonly importId: string },
    ) {
      const value = yield* request("read repository import", {
        ...input,
        method: "GET",
        path: `/api/v3/repository-imports/${encoded(input.importId)}`,
        acceptedStatuses: [200],
      });
      return yield* decode("read repository import", RepositoryImportStatusResponse, value);
    });

    const getImport = Effect.fn("RepositoryClient.getImport")(function* (
      input: AuthenticatedApi & { readonly importId: string },
    ) {
      return (yield* getImportStatus(input)).import;
    });

    const waitForImport = Effect.fn("RepositoryClient.waitForImport")(function* (
      input: AuthenticatedApi & {
        readonly importId: string;
        readonly timeoutMs: number;
        readonly pollIntervalMs: number;
        readonly onProgress?: ImportRepositoryInput["onProgress"];
      },
    ) {
      let lastProgress: string | undefined;

      const reportProgress = (repositoryImport: RepositoryImport) => {
        const progress = `${repositoryImport.state}:${repositoryImport.attempt_count}`;

        if (input.onProgress === undefined || progress === lastProgress) return Effect.void;

        lastProgress = progress;
        return input.onProgress({
          state: repositoryImport.state,
          attemptCount: repositoryImport.attempt_count,
        });
      };

      const poll: Effect.Effect<RepositoryImportStatusResponse, CliError | ImportPending> =
        getImportStatus(input).pipe(
          Effect.tap((response) => reportProgress(response.import)),
          Effect.flatMap(
            (
              response,
            ): Effect.Effect<RepositoryImportStatusResponse, ImportFailed | ImportPending> => {
              const repositoryImport = response.import;
              if (repositoryImport.state === "completed") return Effect.succeed(response);
              if (repositoryImport.state === "failed") {
                return Effect.fail(
                  new ImportFailed({
                    importId: input.importId,
                    message:
                      repositoryImport.error_code ?? `Repository import ${input.importId} failed.`,
                  }),
                );
              }
              return Effect.fail(new ImportPending({ importId: input.importId }));
            },
          ),
        );
      const result = yield* poll.pipe(
        Effect.retry({
          schedule: Schedule.spaced(Duration.millis(input.pollIntervalMs)),
          while: (failure) => failure instanceof ImportPending,
        }),
        Effect.timeoutOption(Duration.millis(input.timeoutMs)),
        Effect.catch((failure) =>
          failure instanceof ImportPending ? Effect.succeed(Option.none()) : Effect.fail(failure),
        ),
      );
      if (Option.isNone(result)) {
        return yield* new ImportWaitTimeout({
          importId: input.importId,
          timeoutMs: input.timeoutMs,
          message: `Repository import ${input.importId} is still running after ${input.timeoutMs} ms. The import continues on the server.`,
        });
      }
      return result.value;
    });

    const importRepository = Effect.fn("RepositoryClient.import")(function* (
      input: ImportRepositoryInput,
    ) {
      const source = yield* parseRepositoryTarget(input.source);
      const owner = input.owner === undefined ? undefined : yield* validateOwner(input.owner);
      const name = input.name === undefined ? undefined : yield* validateRepositoryName(input.name);
      const idempotencyKey = input.idempotencyKey ?? globalThis.crypto.randomUUID();
      const body = {
        source: { provider: "github", repository: `${source.owner}/${source.repo}` },
        private: input.private,
        ...(name === undefined ? {} : { name }),
      };
      const path =
        owner === undefined
          ? "/api/v3/user/repos/imports"
          : `/api/v3/orgs/${encoded(owner)}/repos/imports`;
      const value = yield* retryMutation(
        request("import repository", {
          ...input,
          method: "POST",
          path,
          body,
          headers: { "idempotency-key": idempotencyKey },
          acceptedStatuses: [201, 202],
        }),
      );
      const accepted = yield* decode("import repository", RepositoryImportAcceptedResponse, value);
      const repository = repositoryFromAcceptedImport(accepted);
      if (accepted.import.state === "completed" || input.waitTimeoutMs === 0) {
        return { repository, repositoryImport: accepted.import };
      }
      const completed = yield* waitForImport({
        origin: input.origin,
        token: input.token,
        importId: accepted.import.id,
        timeoutMs: input.waitTimeoutMs,
        pollIntervalMs: input.pollIntervalMs ?? 1_000,
        ...(input.onProgress === undefined ? {} : { onProgress: input.onProgress }),
      });
      return { repository: completed.repository, repositoryImport: completed.import };
    });

    const cloneInfo = Effect.fn("RepositoryClient.cloneInfo")(function* (
      input: AuthenticatedApi & RepositoryTarget,
    ) {
      const repository = yield* view(input);
      const cloneUrl = repository.clone_url;
      const parsed = yield* Effect.try({
        try: () => new URL(cloneUrl),
        catch: (cause) =>
          new ContractError({
            operation: "validate clone URL",
            message: "The API returned an invalid clone URL.",
            cause,
          }),
      });
      const expectedPath = `/git/${encoded(repository.owner.login)}/${encoded(repository.name)}.git`;
      if (
        parsed.origin !== input.origin ||
        parsed.username !== "" ||
        parsed.password !== "" ||
        parsed.search !== "" ||
        parsed.hash !== "" ||
        parsed.pathname !== expectedPath
      ) {
        return yield* new ContractError({
          operation: "validate clone URL",
          message: "The API returned a clone URL outside the selected OpenAgents origin.",
          cause: new Error("clone URL authority mismatch"),
        });
      }
      return { repository, cloneUrl: parsed.toString() };
    });

    return RepositoryClient.of({
      create,
      import: importRepository,
      authenticatedUser,
      list,
      view,
      cloneInfo,
      getImport,
    });
  }),
);
