import { Effect, Option, Redacted } from "effect";
import { Argument, Command, Flag } from "effect/unstable/cli";

import type { Repository } from "./api-contract.js";
import { InputError } from "./errors.js";
import { CredentialStore } from "./credential-store.js";
import { type EndpointOverrides, Profile } from "./endpoint.js";
import { EnvironmentConfiguration } from "./environment.js";
import { GitRunner } from "./git-runner.js";
import { Output, type OutputMode } from "./output.js";
import { parseRepositoryTarget, RepositoryClient } from "./repository-client.js";
import { SecretInput } from "./secret-input.js";
import { resolveApiEndpoint, resolveApiSession } from "./session.js";

export const VERSION = "0.1.0";

const profileFlag = Flag.choice("profile", ["production", "staging", "local"]).pipe(
  Flag.withSchema(Profile),
  Flag.optional,
  Flag.withDescription("Select production, staging, or local API settings"),
);
const apiUrlFlag = Flag.string("api-url").pipe(
  Flag.optional,
  Flag.withDescription("Override the API origin"),
);
const jsonFlag = Flag.boolean("json").pipe(Flag.withDescription("Write machine-readable JSON"));

const rootCommand = Command.make("openagents").pipe(
  Command.withDescription("Manage OpenAgents repositories"),
  Command.withSharedFlags({ profile: profileFlag, apiUrl: apiUrlFlag, json: jsonFlag }),
);

const outputMode = (json: boolean): OutputMode => (json ? "json" : "human");

const endpointOverrides = (flags: {
  readonly profile: Option.Option<Profile>;
  readonly apiUrl: Option.Option<string>;
}): EndpointOverrides => ({ profile: flags.profile, apiUrl: flags.apiUrl });

const privateVisibility = Effect.fn("Cli.privateVisibility")(function* (
  isPublic: boolean,
  isPrivate: boolean,
) {
  if (isPublic && isPrivate) {
    return yield* new InputError({ message: "Use either --public or --private, not both." });
  }
  return !isPublic;
});

const repositoryHuman = (repository: Repository): ReadonlyArray<string> => [
  repository.full_name,
  `Visibility: ${repository.private ? "private" : "public"}`,
  `Default branch: ${repository.default_branch ?? "not set"}`,
  ...(repository.provisioning_state === undefined
    ? []
    : [`Provisioning: ${repository.provisioning_state}`]),
];

const authStatusCommand = Command.make("status", {}, () =>
  Effect.gen(function* () {
    const flags = yield* rootCommand;
    const endpoint = yield* resolveApiEndpoint(endpointOverrides(flags));
    const environment = yield* EnvironmentConfiguration;
    const output = yield* Output;
    if (Option.isSome(environment.token)) {
      return yield* output.write(
        {
          value: {
            origin: endpoint.origin,
            profile: endpoint.profile,
            authenticated: true,
            token_source: "environment",
            persistent_credentials: "unavailable",
          },
          human: [
            `API: ${endpoint.origin}`,
            "Authenticated with OPENAGENTS_TOKEN.",
            "Persistent credential storage is unavailable in this release.",
          ],
        },
        outputMode(flags.json),
      );
    }

    const credentials = yield* CredentialStore;
    const stored = yield* credentials
      .get(endpoint.origin)
      .pipe(
        Effect.catchTag("OpenAgentsCli.CredentialPersistenceUnavailable", () =>
          Effect.succeed(Option.none()),
        ),
      );
    yield* output.write(
      {
        value: {
          origin: endpoint.origin,
          profile: endpoint.profile,
          authenticated: Option.isSome(stored),
          token_source: Option.isSome(stored) ? "store" : null,
          persistent_credentials: "unavailable",
        },
        human: [
          `API: ${endpoint.origin}`,
          Option.isSome(stored) ? "Authenticated with a stored token." : "No token is available.",
          "Set OPENAGENTS_TOKEN to authenticate without persistence.",
        ],
      },
      outputMode(flags.json),
    );
  }),
).pipe(Command.withDescription("Show authentication status for the selected API"));

const authTokenStdinCommand = Command.make("token-stdin", {}, () =>
  Effect.gen(function* () {
    const flags = yield* rootCommand;
    const endpoint = yield* resolveApiEndpoint(endpointOverrides(flags));
    const input = yield* SecretInput;
    const credentials = yield* CredentialStore;
    const output = yield* Output;
    const token = yield* input.readToken();
    yield* credentials.set(endpoint.origin, Redacted.make(token));
    yield* output.write(
      {
        value: { origin: endpoint.origin, stored: true },
        human: [`Stored an OpenAgents token for ${endpoint.origin}.`],
      },
      outputMode(flags.json),
    );
  }),
).pipe(
  Command.withDescription("Read a token from standard input and store it for the selected API"),
);

const authLogoutCommand = Command.make("logout", {}, () =>
  Effect.gen(function* () {
    const flags = yield* rootCommand;
    const endpoint = yield* resolveApiEndpoint(endpointOverrides(flags));
    const credentials = yield* CredentialStore;
    const output = yield* Output;
    yield* credentials.remove(endpoint.origin);
    yield* output.write(
      {
        value: { origin: endpoint.origin, removed: true },
        human: [`Removed the stored OpenAgents token for ${endpoint.origin}.`],
      },
      outputMode(flags.json),
    );
  }),
).pipe(Command.withDescription("Remove the stored token for the selected API"));

const authCommand = Command.make("auth").pipe(
  Command.withDescription("Manage API authentication"),
  Command.withSubcommands([authTokenStdinCommand, authStatusCommand, authLogoutCommand]),
);

const createTarget = Argument.string("name").pipe(
  Argument.withDescription("Repository name or namespace/name"),
);
const descriptionFlag = Flag.string("description").pipe(
  Flag.optional,
  Flag.withDescription("Set the repository description"),
);
const publicFlag = Flag.boolean("public").pipe(Flag.withDescription("Create a public repository"));
const privateFlag = Flag.boolean("private").pipe(
  Flag.withDescription("Create a private repository (the default)"),
);

const repoCreateCommand = Command.make(
  "create",
  {
    target: createTarget,
    description: descriptionFlag,
    public: publicFlag,
    private: privateFlag,
  },
  ({ description, private: isPrivate, public: isPublic, target }) =>
    Effect.gen(function* () {
      const flags = yield* rootCommand;
      const session = yield* resolveApiSession(endpointOverrides(flags));
      const repositories = yield* RepositoryClient;
      const output = yield* Output;
      const visibility = yield* privateVisibility(isPublic, isPrivate);
      const parsed = target.includes("/") ? yield* parseRepositoryTarget(target) : undefined;
      const repository = yield* repositories.create({
        origin: session.endpoint.origin,
        token: session.token,
        name: parsed?.repo ?? target,
        private: visibility,
        ...(parsed === undefined ? {} : { owner: parsed.owner }),
        ...(Option.isNone(description) ? {} : { description: description.value }),
      });
      yield* output.write(
        { value: repository, human: ["Repository created.", ...repositoryHuman(repository)] },
        outputMode(flags.json),
      );
    }),
).pipe(Command.withDescription("Create an empty OpenAgents repository"));

const importSource = Argument.string("source").pipe(
  Argument.withDescription("GitHub repository in namespace/name format"),
);
const importNameFlag = Flag.string("name").pipe(
  Flag.optional,
  Flag.withDescription("Override the destination repository name"),
);
const importNamespaceFlag = Flag.string("namespace").pipe(
  Flag.optional,
  Flag.withDescription("Import into an eligible GitHub organization namespace"),
);
const waitTimeoutFlag = Flag.integer("wait-timeout").pipe(
  Flag.withDefault(300),
  Flag.withDescription("Seconds to wait for the durable import (0 does not wait)"),
);

const repoImportCommand = Command.make(
  "import",
  {
    source: importSource,
    name: importNameFlag,
    namespace: importNamespaceFlag,
    public: publicFlag,
    private: privateFlag,
    waitTimeout: waitTimeoutFlag,
  },
  ({ name, namespace, private: isPrivate, public: isPublic, source, waitTimeout }) =>
    Effect.gen(function* () {
      if (waitTimeout < 0) {
        return yield* new InputError({ message: "--wait-timeout must be zero or greater." });
      }
      const flags = yield* rootCommand;
      const session = yield* resolveApiSession(endpointOverrides(flags));
      const repositories = yield* RepositoryClient;
      const output = yield* Output;
      const visibility = yield* privateVisibility(isPublic, isPrivate);
      const result = yield* repositories.import({
        origin: session.endpoint.origin,
        token: session.token,
        source,
        private: visibility,
        waitTimeoutMs: waitTimeout * 1_000,
        ...(Option.isNone(name) ? {} : { name: name.value }),
        ...(Option.isNone(namespace) ? {} : { owner: namespace.value }),
      });
      yield* output.write(
        {
          value: result,
          human: [
            `Imported ${source} into ${result.repository.full_name}.`,
            `Import state: ${result.repositoryImport.state}`,
            "This is a one-time import. Later GitHub changes do not sync.",
          ],
        },
        outputMode(flags.json),
      );
    }),
).pipe(Command.withDescription("Import a GitHub repository once"));

const repoListCommand = Command.make("list", {}, () =>
  Effect.gen(function* () {
    const flags = yield* rootCommand;
    const session = yield* resolveApiSession(endpointOverrides(flags));
    const repositories = yield* RepositoryClient;
    const output = yield* Output;
    const listed = yield* repositories.list({
      origin: session.endpoint.origin,
      token: session.token,
    });
    yield* output.write(
      {
        value: { repositories: listed },
        human:
          listed.length === 0
            ? ["No repositories found."]
            : listed.map((repository) => repository.full_name),
      },
      outputMode(flags.json),
    );
  }),
).pipe(Command.withDescription("List repositories available to you"));

const repositoryArgument = Argument.string("repository").pipe(
  Argument.withDescription("Repository in namespace/name format"),
);

const repoViewCommand = Command.make("view", { repository: repositoryArgument }, ({ repository }) =>
  Effect.gen(function* () {
    const flags = yield* rootCommand;
    const session = yield* resolveApiSession(endpointOverrides(flags));
    const target = yield* parseRepositoryTarget(repository);
    const repositories = yield* RepositoryClient;
    const output = yield* Output;
    const value = yield* repositories.view({
      origin: session.endpoint.origin,
      token: session.token,
      ...target,
    });
    yield* output.write({ value, human: repositoryHuman(value) }, outputMode(flags.json));
  }),
).pipe(Command.withDescription("Show one repository"));

const cloneDirectory = Argument.string("directory").pipe(Argument.optional);
const repoCloneCommand = Command.make(
  "clone",
  { repository: repositoryArgument, directory: cloneDirectory },
  ({ directory, repository }) =>
    Effect.gen(function* () {
      const flags = yield* rootCommand;
      const session = yield* resolveApiSession(endpointOverrides(flags));
      const target = yield* parseRepositoryTarget(repository);
      const repositories = yield* RepositoryClient;
      const git = yield* GitRunner;
      const output = yield* Output;
      const info = yield* repositories.cloneInfo({
        origin: session.endpoint.origin,
        token: session.token,
        ...target,
      });
      yield* git.clone({
        url: info.cloneUrl,
        ...(Option.isNone(directory) ? {} : { directory: directory.value }),
      });
      yield* output.write(
        {
          value: { repository: info.repository, clone_url: info.cloneUrl, cloned: true },
          human: [`Cloned ${info.repository.full_name}.`],
        },
        outputMode(flags.json),
      );
    }),
).pipe(Command.withDescription("Clone a repository with git"));

const repoCommand = Command.make("repo").pipe(
  Command.withDescription("Manage repositories"),
  Command.withSubcommands([
    repoCreateCommand,
    repoImportCommand,
    repoListCommand,
    repoViewCommand,
    repoCloneCommand,
  ]),
);

export const openagentsCommand = rootCommand.pipe(
  Command.withSubcommands([authCommand, repoCommand]),
);

export const runCliWith = Command.runWith(openagentsCommand, { version: VERSION });
