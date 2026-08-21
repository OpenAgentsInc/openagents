import { Console, Effect, Option, Redacted } from "effect";
import { Argument, Command, Flag } from "effect/unstable/cli";

import type { Repository } from "./api-contract.js";
import { BrowserLauncher } from "./browser-launcher.js";
import { InputError } from "./errors.js";
import { CredentialStore } from "./credential-store.js";
import { DeviceClient } from "./device-client.js";
import { type EndpointOverrides, Profile } from "./endpoint.js";
import { GitRunner } from "./git-runner.js";
import { runGitCredentialHelper } from "./git-credential-helper.js";
import { Output, type OutputMode } from "./output.js";
import { parseRepositoryTarget, RepositoryClient } from "./repository-client.js";
import { SecretInput } from "./secret-input.js";
import { findToken, resolveApiEndpoint, resolveApiSession } from "./session.js";
import { TerminalSession } from "./terminal-session.js";

export const VERSION = "0.1.1";

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
const noColorFlag = Flag.boolean("no-color").pipe(Flag.withDescription("Disable ANSI output"));

const rootCommand = Command.make("openagents").pipe(
  Command.withDescription("Manage OpenAgents repositories"),
  Command.withSharedFlags({
    profile: profileFlag,
    apiUrl: apiUrlFlag,
    json: jsonFlag,
    noColor: noColorFlag,
  }),
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
  `Provisioning: ${repository.lifecycle_state}`,
];

const authStatusCommand = Command.make("status", {}, () =>
  Effect.gen(function* () {
    const flags = yield* rootCommand;
    const endpoint = yield* resolveApiEndpoint(endpointOverrides(flags));
    const output = yield* Output;
    const token = yield* findToken(endpoint.origin).pipe(
      Effect.catchTag("OpenAgentsCli.CredentialPersistenceUnavailable", () =>
        Effect.succeed(Option.none()),
      ),
    );
    if (Option.isNone(token)) {
      return yield* output.write(
        {
          value: {
            origin: endpoint.origin,
            profile: endpoint.profile,
            authenticated: false,
            token_source: null,
            account: null,
            namespaces: [],
            token_expires_at: null,
            git_helper: { local: false, global: false },
          },
          human: [
            `API: ${endpoint.origin}`,
            "No token is available.",
            "Set OPENAGENTS_TOKEN or run openagents auth login.",
          ],
        },
        outputMode(flags.json),
      );
    }

    const repositories = yield* RepositoryClient;
    const git = yield* GitRunner;
    const user = yield* repositories.authenticatedUser({
      origin: endpoint.origin,
      token: token.value.token,
    });
    const gitHelper = yield* git.credentialHelperState(endpoint.origin);
    yield* output.write(
      {
        value: {
          origin: endpoint.origin,
          profile: endpoint.profile,
          authenticated: true,
          token_source: token.value.source,
          account: { id: user.id, login: user.login },
          namespaces: user.namespaces,
          token_expires_at: user.token_expires_at,
          git_helper: gitHelper,
        },
        human: [
          `API: ${endpoint.origin}`,
          `Authenticated as ${user.login} (${user.id}) with a ${token.value.source} token.`,
          `Eligible namespaces: ${user.namespaces.map((namespace) => namespace.login).join(", ")}.`,
          `Token expires: ${user.token_expires_at}.`,
          `Git helper: local ${gitHelper.local ? "configured" : "not configured"}; global ${gitHelper.global ? "configured" : "not configured"}.`,
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

const loginTokenStdinFlag = Flag.boolean("token-stdin").pipe(
  Flag.withDescription("Read and store a token from standard input instead of opening a browser"),
);
const authLoginCommand = Command.make(
  "login",
  { tokenStdin: loginTokenStdinFlag },
  ({ tokenStdin }) =>
    Effect.gen(function* () {
      const flags = yield* rootCommand;
      const endpoint = yield* resolveApiEndpoint(endpointOverrides(flags));
      const credentials = yield* CredentialStore;
      const output = yield* Output;
      const token = tokenStdin
        ? Redacted.make(yield* (yield* SecretInput).readToken())
        : yield* Effect.gen(function* () {
            const devices = yield* DeviceClient;
            const terminal = yield* TerminalSession;
            const authorization = yield* devices.start(endpoint.origin);
            yield* Console.error(
              `OpenAgents authorization URL: ${authorization.verification_uri_complete}`,
            );
            yield* Console.error(`OpenAgents authorization code: ${authorization.user_code}`);
            yield* Console.error("Waiting for approval...");
            if (terminal.interactive) {
              const browser = yield* BrowserLauncher;
              yield* browser.open(authorization.verification_uri_complete);
            }
            return yield* devices.wait(endpoint.origin, authorization);
          });
      yield* credentials.set(endpoint.origin, token);
      yield* output.write(
        {
          value: {
            origin: endpoint.origin,
            authenticated: true,
            token_source: tokenStdin ? "token_stdin" : "device_authorization",
          },
          human: [
            `Authenticated with ${endpoint.origin}.`,
            "The token is stored in your OS credential store.",
            "Run openagents auth setup-git --local to configure Git for this repository.",
          ],
        },
        outputMode(flags.json),
      );
    }),
).pipe(Command.withDescription("Authorize the CLI in your browser and store the resulting token"));

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

const gitCredentialOperation = Argument.choice("operation", ["get", "store", "erase"] as const);
const authGitCredentialCommand = Command.make(
  "git-credential",
  { operation: gitCredentialOperation },
  ({ operation }) =>
    Effect.gen(function* () {
      const flags = yield* rootCommand;
      const endpoint = yield* resolveApiEndpoint(endpointOverrides(flags));
      yield* runGitCredentialHelper(endpoint.origin, operation);
    }),
).pipe(Command.withDescription("Internal Git credential-helper protocol endpoint"));

const setupLocalFlag = Flag.boolean("local").pipe(
  Flag.withDescription("Configure the current Git repository"),
);
const setupGlobalFlag = Flag.boolean("global").pipe(
  Flag.withDescription("Configure your global Git settings"),
);
const setupYesFlag = Flag.boolean("yes").pipe(
  Flag.withDescription("Confirm a global Git credential-helper change"),
);
const authSetupGitCommand = Command.make(
  "setup-git",
  { local: setupLocalFlag, global: setupGlobalFlag, yes: setupYesFlag },
  ({ global, local, yes }) =>
    Effect.gen(function* () {
      if (local === global) {
        return yield* new InputError({ message: "Choose exactly one of --local or --global." });
      }
      if (global && !yes) {
        return yield* new InputError({ message: "Global setup requires --yes confirmation." });
      }
      if (global && !(yield* TerminalSession).interactive) {
        return yield* new InputError({
          message: "Global Git helper setup requires an interactive terminal.",
        });
      }
      const flags = yield* rootCommand;
      const endpoint = yield* resolveApiEndpoint(endpointOverrides(flags));
      const git = yield* GitRunner;
      const output = yield* Output;
      yield* git.configureCredentialHelper(endpoint.origin, local ? "local" : "global");
      yield* output.write(
        {
          value: { origin: endpoint.origin, scope: local ? "local" : "global", configured: true },
          human: [
            `Configured the ${local ? "local" : "global"} Git credential helper for ${endpoint.origin}.`,
          ],
        },
        outputMode(flags.json),
      );
    }),
).pipe(Command.withDescription("Configure Git to obtain OpenAgents credentials from this CLI"));

const authCommand = Command.make("auth").pipe(
  Command.withDescription("Manage API authentication"),
  Command.withSubcommands([
    authLoginCommand,
    authTokenStdinCommand,
    authStatusCommand,
    authLogoutCommand,
    authSetupGitCommand,
    authGitCredentialCommand,
  ]),
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
const defaultBranchFlag = Flag.string("default-branch").pipe(
  Flag.withDefault("main"),
  Flag.withDescription("Set the initial default branch"),
);
const waitTimeoutFlag = Flag.integer("wait-timeout").pipe(
  Flag.withDefault(300),
  Flag.withDescription("Seconds to wait for durable provisioning (0 does not wait)"),
);
const sourceDirectoryFlag = Flag.string("source").pipe(
  Flag.optional,
  Flag.withDescription("Attach the new repository to an existing Git worktree"),
);
const remoteNameFlag = Flag.string("remote").pipe(
  Flag.optional,
  Flag.withDescription("Name the Git remote attached with --source (defaults to origin)"),
);

const shellArgument = (value: string): string =>
  /^[A-Za-z0-9_./:@=-]+$/u.test(value) ? value : `'${value.replaceAll("'", `'"'"'`)}'`;

const repoCreateCommand = Command.make(
  "create",
  {
    target: createTarget,
    description: descriptionFlag,
    public: publicFlag,
    private: privateFlag,
    defaultBranch: defaultBranchFlag,
    waitTimeout: waitTimeoutFlag,
    source: sourceDirectoryFlag,
    remote: remoteNameFlag,
  },
  ({
    defaultBranch,
    description,
    private: isPrivate,
    public: isPublic,
    remote,
    source,
    target,
    waitTimeout,
  }) =>
    Effect.gen(function* () {
      if (waitTimeout < 0) {
        return yield* new InputError({ message: "--wait-timeout must be zero or greater." });
      }
      if (Option.isNone(source) && Option.isSome(remote)) {
        return yield* new InputError({ message: "Use --remote only with --source." });
      }
      const flags = yield* rootCommand;
      const session = yield* resolveApiSession(endpointOverrides(flags));
      const repositories = yield* RepositoryClient;
      const git = yield* GitRunner;
      const output = yield* Output;
      const sourceDirectory = Option.isSome(source) ? source.value : undefined;
      const remoteName = Option.isSome(remote) ? remote.value : "origin";
      const visibility = yield* privateVisibility(isPublic, isPrivate);
      const parsed = target.includes("/") ? yield* parseRepositoryTarget(target) : undefined;
      const repository = yield* repositories.create({
        origin: session.endpoint.origin,
        token: session.token,
        name: parsed?.repo ?? target,
        private: visibility,
        defaultBranch,
        waitTimeoutMs: waitTimeout * 1_000,
        ...(parsed === undefined ? {} : { owner: parsed.owner }),
        ...(Option.isNone(description) ? {} : { description: description.value }),
      });

      const attached =
        sourceDirectory !== undefined && repository.lifecycle_state === "ready"
          ? yield* repositories
              .cloneInfo({
                origin: session.endpoint.origin,
                token: session.token,
                owner: repository.owner.login,
                repo: repository.name,
              })
              .pipe(
                Effect.flatMap((info) =>
                  git.attachRemote({
                    origin: session.endpoint.origin,
                    url: info.cloneUrl,
                    directory: sourceDirectory,
                    remote: remoteName,
                  }),
                ),
              )
          : undefined;
      const nextPush =
        attached === undefined || sourceDirectory === undefined
          ? undefined
          : ["git", "-C", sourceDirectory, ...attached.nextPushArguments];
      yield* output.write(
        {
          value:
            attached === undefined
              ? repository
              : { repository, remote: attached.remote, next_push: nextPush },
          human: [
            "Repository created.",
            ...repositoryHuman(repository),
            ...(attached === undefined
              ? Option.isSome(source)
                ? ["The repository is still provisioning, so the CLI did not configure a remote."]
                : []
              : [
                  `Configured remote ${attached.remote} in ${sourceDirectory}.`,
                  `Next: ${nextPush?.map(shellArgument).join(" ")}`,
                ]),
          ],
        },
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
      const sourceTarget = yield* parseRepositoryTarget(source);
      const destination = Option.isSome(namespace) ? namespace.value : sourceTarget.owner;
      if (destination.toLowerCase() !== sourceTarget.owner.toLowerCase()) {
        return yield* new InputError({
          message: "--namespace must match the GitHub source owner in the first release.",
        });
      }
      const user = yield* repositories.authenticatedUser({
        origin: session.endpoint.origin,
        token: session.token,
      });
      const personal = destination.toLowerCase() === user.login.toLowerCase();
      if (
        !personal &&
        !user.namespaces.some(
          (candidate) =>
            candidate.type === "organization" &&
            candidate.login.toLowerCase() === destination.toLowerCase(),
        )
      ) {
        return yield* new InputError({
          message: `${destination} is not an eligible GitHub namespace for this account.`,
        });
      }
      const result = yield* repositories.import({
        origin: session.endpoint.origin,
        token: session.token,
        source: `${sourceTarget.owner}/${sourceTarget.repo}`,
        private: visibility,
        waitTimeoutMs: waitTimeout * 1_000,
        onProgress: ({ state, attemptCount }) =>
          Console.error(`Repository import state: ${state} (attempt ${attemptCount}).`),
        ...(Option.isNone(name) ? {} : { name: name.value }),
        ...(personal ? {} : { owner: destination }),
      });
      yield* output.write(
        {
          value: result,
          human: [
            `Imported ${sourceTarget.owner}/${sourceTarget.repo} into ${result.repository.full_name}.`,
            `Import state: ${result.repositoryImport.state}`,
            "This is a one-time import. Later GitHub changes do not sync.",
          ],
        },
        outputMode(flags.json),
      );
    }),
).pipe(Command.withDescription("Import a GitHub repository once"));

const listNamespaceFlag = Flag.string("namespace").pipe(
  Flag.optional,
  Flag.withDescription("Filter by a GitHub-backed namespace"),
);
const listLimitFlag = Flag.integer("limit").pipe(
  Flag.withDefault(30),
  Flag.withDescription("Return between 1 and 100 repositories"),
);
const listAfterFlag = Flag.string("after").pipe(
  Flag.optional,
  Flag.withDescription("Continue from an opaque repository cursor"),
);
const repoListCommand = Command.make(
  "list",
  { namespace: listNamespaceFlag, limit: listLimitFlag, after: listAfterFlag },
  ({ after, limit, namespace }) =>
    Effect.gen(function* () {
      const flags = yield* rootCommand;
      const session = yield* resolveApiSession(endpointOverrides(flags));
      const repositories = yield* RepositoryClient;
      const output = yield* Output;
      const listed = yield* repositories.list({
        origin: session.endpoint.origin,
        token: session.token,
        limit,
        ...(Option.isNone(namespace) ? {} : { namespace: namespace.value }),
        ...(Option.isNone(after) ? {} : { after: after.value }),
      });
      yield* output.write(
        {
          value: { repositories: listed.repositories, next_cursor: listed.nextCursor },
          human:
            listed.repositories.length === 0
              ? ["No repositories found."]
              : [
                  ...listed.repositories.map((repository) => repository.full_name),
                  ...(listed.nextCursor === null ? [] : [`Next cursor: ${listed.nextCursor}`]),
                ],
        },
        outputMode(flags.json),
      );
    }),
).pipe(Command.withDescription("List repositories available to you"));

const repositoryArgument = Argument.string("repository").pipe(
  Argument.withDescription("Repository in namespace/name format"),
);
const optionalRepositoryArgument = repositoryArgument.pipe(Argument.optional);
const repositoryOverrideFlag = Flag.string("repo").pipe(
  Flag.withAlias("R"),
  Flag.optional,
  Flag.withDescription("Select OWNER/REPO instead of inferring the origin remote"),
);

const resolveRepositoryArgument = Effect.fn("Cli.resolveRepositoryArgument")(function* (
  positional: Option.Option<string>,
  override: Option.Option<string>,
  origin: string,
) {
  if (Option.isSome(positional) && Option.isSome(override)) {
    return yield* new InputError({ message: "Pass a repository argument or --repo, not both." });
  }
  const git = yield* GitRunner;
  const selected = Option.isSome(override)
    ? override.value
    : Option.isSome(positional)
      ? positional.value
      : yield* git.inferRepository(origin);
  return yield* parseRepositoryTarget(selected);
});

const repoViewCommand = Command.make(
  "view",
  { repository: optionalRepositoryArgument, repo: repositoryOverrideFlag },
  ({ repo, repository }) =>
    Effect.gen(function* () {
      const flags = yield* rootCommand;
      const session = yield* resolveApiSession(endpointOverrides(flags));
      const target = yield* resolveRepositoryArgument(repository, repo, session.endpoint.origin);
      const repositories = yield* RepositoryClient;
      const output = yield* Output;
      const value = yield* repositories.view({
        origin: session.endpoint.origin,
        token: session.token,
        ...target,
      });
      yield* output.write({ value, human: repositoryHuman(value) }, outputMode(flags.json));
    }),
).pipe(Command.withDescription("Show one repository or infer it from the origin remote"));

const cloneDirectory = Argument.string("directory").pipe(Argument.optional);
const repoCloneCommand = Command.make(
  "clone",
  {
    repository: optionalRepositoryArgument,
    directory: cloneDirectory,
    repo: repositoryOverrideFlag,
  },
  ({ directory, repo, repository }) =>
    Effect.gen(function* () {
      const flags = yield* rootCommand;
      const session = yield* resolveApiSession(endpointOverrides(flags));
      const target = yield* resolveRepositoryArgument(repository, repo, session.endpoint.origin);
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
