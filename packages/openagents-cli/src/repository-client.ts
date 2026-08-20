import { Duration, Effect, Layer, Option, Redacted, Schedule, Schema } from "effect";
import * as Context from "effect/Context";

import {
  ApiErrorResponse,
  Repository,
  RepositoryImport,
  RepositoryImportAcceptedResponse,
  RepositoryImportResponse,
  RepositoryListResponse,
  RepositoryResponse,
  unwrapAcceptedImport,
  unwrapRepository,
  unwrapRepositoryImport,
  unwrapRepositoryList,
} from "./api-contract.js";
import { ApiTransport } from "./api-transport.js";
import {
  ApiError,
  ContractError,
  ImportFailed,
  ImportWaitTimeout,
  InputError,
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
}

export interface ImportRepositoryInput extends AuthenticatedApi {
  readonly owner?: string;
  readonly source: string;
  readonly name?: string;
  readonly private: boolean;
  readonly waitTimeoutMs: number;
  readonly pollIntervalMs?: number;
}

export interface ImportRepositoryResult {
  readonly repository: Repository;
  readonly repositoryImport: RepositoryImport;
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
  readonly list: (input: AuthenticatedApi) => Effect.Effect<ReadonlyArray<Repository>, CliError>;
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

const namePattern = /^[A-Za-z0-9._-]+$/;
const ownerPattern = /^[A-Za-z0-9](?:[A-Za-z0-9-]{0,38})$/;

export const validateRepositoryName = Effect.fn("RepositoryClient.validateName")(function* (
  name: string,
) {
  const normalized = name.trim();
  if (normalized.length === 0 || normalized.length > 100 || !namePattern.test(normalized)) {
    return yield* new InputError({
      message:
        "Repository names must contain 1-100 letters, numbers, periods, underscores, or hyphens.",
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

const errorMessage = (body: unknown, status: number): { message: string; requestId?: string } => {
  const decoded = Schema.decodeUnknownOption(ApiErrorResponse)(body);
  if (Option.isNone(decoded)) return { message: `The API returned HTTP ${status}.` };
  const value = decoded.value;
  const message = value.message ?? value.error ?? `The API returned HTTP ${status}.`;
  return value.request_id === undefined ? { message } : { message, requestId: value.request_id };
};

class ImportPending extends Schema.TaggedErrorClass<ImportPending>()(
  "OpenAgentsCli.Internal.ImportPending",
  { importId: Schema.String },
) {}

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
        readonly acceptedStatuses: ReadonlyArray<number>;
      },
    ) {
      const response = yield* transport.request({
        origin: input.origin,
        method: input.method,
        path: input.path,
        token: input.token,
        ...(input.body === undefined ? {} : { body: input.body }),
      });
      if (!input.acceptedStatuses.includes(response.status)) {
        const details = errorMessage(response.body, response.status);
        return yield* new ApiError({
          operation,
          status: response.status,
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

    const create = Effect.fn("RepositoryClient.create")(function* (input: CreateRepositoryInput) {
      const name = yield* validateRepositoryName(input.name);
      const owner = input.owner === undefined ? undefined : yield* validateOwner(input.owner);
      const body = {
        name,
        private: input.private,
        ...(input.description === undefined ? {} : { description: input.description }),
      };
      const path =
        owner === undefined ? "/api/v3/user/repos" : `/api/v3/orgs/${encoded(owner)}/repos`;
      const value = yield* request("create repository", {
        ...input,
        method: "POST",
        path,
        body,
        acceptedStatuses: [201, 202],
      });
      return unwrapRepository(yield* decode("create repository", RepositoryResponse, value));
    });

    const list = Effect.fn("RepositoryClient.list")(function* (input: AuthenticatedApi) {
      const value = yield* request("list repositories", {
        ...input,
        method: "GET",
        path: "/api/v3/user/repos",
        acceptedStatuses: [200],
      });
      return unwrapRepositoryList(
        yield* decode("list repositories", RepositoryListResponse, value),
      );
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
      return unwrapRepository(yield* decode("view repository", RepositoryResponse, value));
    });

    const getImport = Effect.fn("RepositoryClient.getImport")(function* (
      input: AuthenticatedApi & { readonly importId: string },
    ) {
      const value = yield* request("read repository import", {
        ...input,
        method: "GET",
        path: `/api/v3/repository-imports/${encoded(input.importId)}`,
        acceptedStatuses: [200],
      });
      return unwrapRepositoryImport(
        yield* decode("read repository import", RepositoryImportResponse, value),
      );
    });

    const waitForImport = Effect.fn("RepositoryClient.waitForImport")(function* (
      input: AuthenticatedApi & {
        readonly importId: string;
        readonly timeoutMs: number;
        readonly pollIntervalMs: number;
      },
    ) {
      const poll: Effect.Effect<RepositoryImport, CliError | ImportPending> = getImport(input).pipe(
        Effect.flatMap(
          (repositoryImport): Effect.Effect<RepositoryImport, ImportFailed | ImportPending> => {
            if (repositoryImport.state === "completed") return Effect.succeed(repositoryImport);
            if (repositoryImport.state === "failed") {
              return Effect.fail(
                new ImportFailed({
                  importId: input.importId,
                  message:
                    repositoryImport.error_message ??
                    repositoryImport.error_code ??
                    `Repository import ${input.importId} failed.`,
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
      const body = {
        provider: "github",
        repository: `${source.owner}/${source.repo}`,
        private: input.private,
        ...(name === undefined ? {} : { name }),
      };
      const path =
        owner === undefined
          ? "/api/v3/user/repos/imports"
          : `/api/v3/orgs/${encoded(owner)}/repos/imports`;
      const value = yield* request("import repository", {
        ...input,
        method: "POST",
        path,
        body,
        acceptedStatuses: [201, 202],
      });
      const accepted = unwrapAcceptedImport(
        yield* decode("import repository", RepositoryImportAcceptedResponse, value),
      );
      if (accepted.repositoryImport.state === "completed" || input.waitTimeoutMs === 0) {
        return accepted;
      }
      const repositoryImport = yield* waitForImport({
        origin: input.origin,
        token: input.token,
        importId: String(accepted.repositoryImport.id),
        timeoutMs: input.waitTimeoutMs,
        pollIntervalMs: input.pollIntervalMs ?? 1_000,
      });
      return { ...accepted, repositoryImport };
    });

    const cloneInfo = Effect.fn("RepositoryClient.cloneInfo")(function* (
      input: AuthenticatedApi & RepositoryTarget,
    ) {
      const repository = yield* view(input);
      const cloneUrl =
        repository.clone_url ??
        `${input.origin}/git/${encoded(repository.owner.login)}/${encoded(repository.name)}.git`;
      return { repository, cloneUrl };
    });

    return RepositoryClient.of({
      create,
      import: importRepository,
      list,
      view,
      cloneInfo,
      getImport,
    });
  }),
);
