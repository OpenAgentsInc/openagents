import { Clock, Console, Effect, Option, Redacted } from "effect";
import { Argument, Command, Flag } from "effect/unstable/cli";
import { hostname } from "node:os";

import { apiErrorDetails, type Repository } from "./api-contract.js";
import {
  API_BASE_PATH,
  PASSTHROUGH_METHODS,
  decodeRequestBody,
  parseRequestFields,
  parseRequestHeaders,
  resolveApiPath,
  resolveRequestMethod,
  successfulStatus,
} from "./api-passthrough.js";
import { ApiTransport } from "./api-transport.js";
import { BrowserLauncher } from "./browser-launcher.js";
import { runCoderPlain } from "./coder-plain.js";
import { CoderSession, DummyReplySource } from "./coder-session.js";
import { runCoderUi } from "./coder-ui.js";
import { OxAlphaReplySource } from "./coder-ox.js";
import { describeWorkspace } from "./coder-workspace.js";
import { ComputerClient } from "./computer-client.js";
import { ComputerUp } from "./computer-up.js";
import {
  ComputerConfiguration,
  type ComputerConfigurationValues,
  writeComputerConfiguration,
} from "./computer-config.js";
import { ComputerJournal, journalMaxBytes, journalReadTailBytes } from "./computer-journal.js";
import { ComputerProbe } from "./computer-probe.js";
import { formatAllowlist, resolveRoots, type Tier } from "./computer-policy.js";
import {
  ApiError,
  ComputerAlreadyPaired,
  ComputerMachineMismatch,
  ComputerMachineUnavailable,
  ComputerPairingInProgress,
  ComputerReconnectExhausted,
  InputError,
} from "./errors.js";
import { CredentialStore } from "./credential-store.js";
import {
  PendingDeviceAuthorizationStore,
  type PendingDeviceAuthorization,
} from "./device-authorization-store.js";
import { DeviceClient } from "./device-client.js";
import { type EndpointOverrides, Profile } from "./endpoint.js";
import { ForumClient } from "./forum-client.js";
import { GitRunner } from "./git-runner.js";
import { runGitCredentialHelper } from "./git-credential-helper.js";
import { Output, type OutputMode } from "./output.js";
import { parseRepositoryTarget, RepositoryClient } from "./repository-client.js";
import { RequestBodyInput } from "./request-body-input.js";
import { SecretInput } from "./secret-input.js";
import { findToken, resolveApiEndpoint, resolveApiSession } from "./session.js";
import { TerminalSession } from "./terminal-session.js";

// The version lives in `package.json`; see `version.ts`.
import { VERSION } from "./version.js";

// Re-exported because `main.ts` reads it from here, and imported above because
// this module uses it too. A bare `export … from` would do only the first, and
// every use in this file would be an unbound name.
export { VERSION };

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

const computerRootFlag = Flag.string("root").pipe(
  Flag.atLeast(0),
  Flag.withDescription("Inspect a declared directory; repeatable. Empty means no roots."),
);
const computerTierFlag = Flag.choice("tier", ["probe", "curated", "shell"] as const).pipe(
  Flag.optional,
  Flag.withDescription("Set the local execution ceiling for this Computer"),
);
const computerJournalLimitFlag = Flag.integer("limit").pipe(
  Flag.withDefault(20),
  Flag.withDescription("Maximum number of local journal entries to show"),
);

const computerConfigurationView = (
  config: ComputerConfigurationValues,
  roots: ReadonlyArray<string>,
) => ({
  tier: config.tier,
  roots: roots.length === 0 ? config.roots : resolveRoots(roots),
  pre_approved: config.preApproved,
  authority: "local_machine",
  paths: {
    config: config.paths.config,
    journal: config.paths.journal,
  },
});

const computerProbeCommand = Command.make("probe", { root: computerRootFlag }, ({ root }) =>
  Effect.gen(function* () {
    const flags = yield* rootCommand;
    const config = yield* ComputerConfiguration;
    const probe = yield* ComputerProbe;
    const roots = root.length === 0 ? config.roots : resolveRoots(root);
    const report = yield* probe.probe(roots);
    const output = yield* Output;
    yield* output.write(
      {
        value: report,
        human: [
          `Host: ${report.host.platform} ${report.host.release} ${report.host.architecture}`,
          `Hostname: ${report.host.hostname}`,
          `Roots: ${report.roots.join(", ") || "(none declared)"}`,
          `Coding agents present: ${report.codingAgents.filter((tool) => tool.present).length}/${report.codingAgents.length}`,
          `Toolchains present: ${report.toolchains.filter((tool) => tool.present).length}/${report.toolchains.length}`,
          `Worktrees inspected: ${report.worktrees.length}`,
        ],
      },
      outputMode(flags.json),
    );
  }),
).pipe(
  Command.withDescription(
    "Inspect this machine with fixed read-only probes. It needs no account or pairing; the local machine controls all access.",
  ),
);

const computerPolicyCommand = Command.make("policy", { root: computerRootFlag }, ({ root }) =>
  Effect.gen(function* () {
    const flags = yield* rootCommand;
    const config = yield* ComputerConfiguration;
    const output = yield* Output;
    const view = computerConfigurationView(config, root);
    yield* output.write(
      {
        value: {
          schema: "openagents.computer_policy.v1",
          ...view,
          allowlist: formatAllowlist(),
          scope: "local inspection and policy",
          network: false,
        },
        human: [
          "Authority: this machine decides what may run.",
          `Effective tier: ${view.tier}`,
          `Declared roots: ${view.roots.join(", ") || "(none declared)"}`,
          "Empty roots mean that no working directory is reachable.",
          "Path rules follow this host's POSIX or Windows semantics.",
          "Curated allowlist:",
          ...formatAllowlist().map((entry) => `  ${entry}`),
          `Configuration: ${view.paths.config}`,
          "No account, pairing, or network is needed for this command.",
        ],
      },
      outputMode(flags.json),
    );
  }),
).pipe(
  Command.withDescription(
    "Show the local Computer tier, roots, and read-only allowlist. Path rules follow this host's POSIX or Windows semantics; the server cannot raise this machine's policy.",
  ),
);

const computerStatusCommand = Command.make("status", {}, () =>
  Effect.gen(function* () {
    const flags = yield* rootCommand;
    const config = yield* ComputerConfiguration;
    const endpoint = yield* resolveApiEndpoint(endpointOverrides(flags));
    const credentials = yield* CredentialStore;
    const pendingStore = yield* PendingDeviceAuthorizationStore;
    const stored = yield* credentials.get(endpoint.origin, "computer");
    const pending = yield* pendingStore.get(endpoint.origin);
    const computerPending =
      Option.isSome(pending) && pending.value.kind === "computer" ? pending.value : undefined;
    const remoteStatus = Option.isSome(stored)
      ? yield* (yield* ComputerClient).status(endpoint.origin, stored.value)
      : Option.none();
    const paired = Option.isSome(remoteStatus);
    const state = paired
      ? "paired"
      : Option.isSome(stored)
        ? "unpaired"
        : computerPending === undefined
          ? "local"
          : "pairing_pending";
    const output = yield* Output;
    const value = {
      schema: "openagents.computer_status.v1",
      state,
      paired,
      endpoint: endpoint.origin,
      tier: config.tier,
      roots: config.roots,
      machine: {
        platform: process.platform,
        architecture: process.arch,
        hostname: hostname(),
      },
      paths: {
        config: config.paths.config,
        journal: config.paths.journal,
      },
      journal_retention_bytes: journalMaxBytes,
      journal_read_tail_bytes: journalReadTailBytes,
      network: false,
      remote_state: paired ? "active" : "unpaired",
      ...(Option.isSome(remoteStatus)
        ? { machine_id: remoteStatus.value.machine_id }
        : computerPending?.machine_id === undefined
          ? {}
          : { machine_id: computerPending.machine_id }),
    };
    yield* output.write(
      {
        value,
        human: [
          `Computer state: ${state}`,
          `Pairing: ${
            paired
              ? "paired"
              : Option.isSome(stored)
                ? "no longer active; run computer logout"
                : computerPending === undefined
                  ? "not configured"
                  : "in progress"
          }`,
          `Endpoint: ${endpoint.origin}`,
          `Tier: ${config.tier}`,
          `Roots: ${config.roots.join(", ") || "(none declared)"}`,
          `Configuration: ${config.paths.config}`,
          `Journal: ${config.paths.journal}`,
          `Journal retention: last ${journalMaxBytes} bytes; reads inspect the last ${journalReadTailBytes} bytes`,
          "The machine, not the server, decides what runs here.",
          "Path rules follow this host's POSIX or Windows semantics.",
          ...(Option.isSome(stored) && !paired
            ? ["The server no longer accepts this machine token; run computer logout."]
            : []),
        ],
      },
      outputMode(flags.json),
    );
  }),
).pipe(
  Command.withDescription(
    "Show local Computer state, pairing state, and file locations without printing secrets.",
  ),
);

const computerUpCommand = Command.make("up", {}, () =>
  Effect.gen(function* () {
    const flags = yield* rootCommand;
    const endpoint = yield* resolveApiEndpoint(endpointOverrides(flags));
    const up = yield* ComputerUp;
    const reason = yield* up.serve(endpoint.origin, VERSION);
    if (reason.includes("machine_unavailable")) {
      return yield* new ComputerMachineUnavailable({
        message: "The Computer machine is unavailable; the connection stopped.",
      });
    }
    if (reason.includes("machine_mismatch")) {
      return yield* new ComputerMachineMismatch({
        message: "The Computer machine does not match the paired identity; the connection stopped.",
      });
    }
    if (reason.includes("retry_exhausted")) {
      return yield* new ComputerReconnectExhausted({
        message: `The Computer connection stopped after bounded retries (${reason}).`,
      });
    }
    const output = yield* Output;
    yield* output.write(
      {
        value: { schema: "openagents.computer_connection.v1", state: "closed", reason },
        human: [`Computer connection ended: ${reason}`],
      },
      outputMode(flags.json),
    );
  }),
).pipe(
  Command.withDescription(
    "Serve bounded Computer requests over an outbound connection. Transport loss and machine_reconnecting retry with bounded backoff; authorization refusals stop the command.",
  ),
);

const computerPairCommand = Command.make(
  "pair",
  { tier: computerTierFlag, root: computerRootFlag },
  ({ root, tier }) =>
    Effect.gen(function* () {
      const flags = yield* rootCommand;
      const endpoint = yield* resolveApiEndpoint(endpointOverrides(flags));
      const config = yield* ComputerConfiguration;
      const credentials = yield* CredentialStore;
      const pendingStore = yield* PendingDeviceAuthorizationStore;
      const output = yield* Output;
      const stored = yield* credentials.get(endpoint.origin, "computer");
      if (Option.isSome(stored)) {
        return yield* new ComputerAlreadyPaired({
          message: `This Computer is already paired with ${endpoint.origin}; run computer logout before pairing again.`,
        });
      }

      const pending = yield* pendingStore.get(endpoint.origin);
      if (Option.isSome(pending)) {
        const pendingValue = pending.value;
        if (pendingValue.kind !== "computer") {
          return yield* new ComputerPairingInProgress({
            message: `An OpenAgents authorization is already pending for ${endpoint.origin}; complete it before pairing this Computer.`,
          });
        }
        const now = yield* Clock.currentTimeMillis;
        if (pendingValue.expires_at_ms > now) {
          return yield* new ComputerPairingInProgress({
            message: `A Computer pairing is already pending for ${endpoint.origin}; finish it before starting another.`,
          });
        }
        yield* pendingStore.remove(endpoint.origin);
      }

      const selectedTier: Tier = Option.getOrElse(tier, () => config.tier);
      const roots = root.length === 0 ? config.roots : resolveRoots(root);
      yield* writeComputerConfiguration(
        { tier: selectedTier, roots, preApproved: config.preApproved },
        config.paths,
      );
      const started = yield* (yield* ComputerClient).start(endpoint.origin, {
        name: hostname(),
        tier: selectedTier,
        platform: `${process.platform}-${process.arch}`,
        agentVersion: VERSION,
        roots,
      });
      const expiresAtMs = Date.parse(started.expires_at);
      if (!Number.isFinite(expiresAtMs)) {
        return yield* new InputError({
          message: "The Computer pairing response did not contain a valid expiry.",
        });
      }
      const terminal = yield* TerminalSession;
      if (terminal.interactive && !flags.json) {
        const browser = yield* BrowserLauncher;
        if (!(yield* browser.open(started.verify_url))) {
          yield* Console.error("The browser did not open. Open the approval URL above.");
        }
      }
      const pendingAuthorization: PendingDeviceAuthorization = {
        origin: endpoint.origin,
        device_code: started.pairing_id,
        user_code: started.code,
        verification_uri: started.verify_url,
        verification_uri_complete: started.verify_url,
        expires_at_ms: expiresAtMs,
        interval: started.interval_seconds,
        kind: "computer",
        state: "pending",
      };
      yield* pendingStore.set(pendingAuthorization);
      yield* output.write(
        {
          value: {
            endpoint: endpoint.origin,
            pairing_pending: true,
            verification_url: started.verify_url,
            code: started.code,
            expires_at: started.expires_at,
            interval_seconds: started.interval_seconds,
            tier: selectedTier,
            roots,
          },
          human: [
            `Approve this Computer at ${started.verify_url}`,
            `Pairing code: ${started.code}`,
            "Waiting for approval...",
          ],
        },
        outputMode(flags.json),
      );

      const claim = yield* (yield* ComputerClient).wait(endpoint.origin, started);
      yield* credentials.set(endpoint.origin, Redacted.make(claim.token), "computer");
      yield* pendingStore.set({
        ...pendingAuthorization,
        state: "paired",
        machine_id: claim.machine_id,
      });
      yield* output.write(
        {
          value: {
            endpoint: endpoint.origin,
            paired: true,
            machine_id: claim.machine_id,
            name: claim.name,
            token_source: "computer_credential_store",
          },
          human: [
            `Computer paired with ${endpoint.origin}.`,
            "The machine token is in the OS credential store.",
          ],
        },
        outputMode(flags.json),
      );
    }),
).pipe(
  Command.withDescription(
    "Pair this Computer through browser approval and store its machine token",
  ),
);

const computerLogoutCommand = Command.make("logout", {}, () =>
  Effect.gen(function* () {
    const flags = yield* rootCommand;
    const endpoint = yield* resolveApiEndpoint(endpointOverrides(flags));
    const credentials = yield* CredentialStore;
    const pendingStore = yield* PendingDeviceAuthorizationStore;
    const output = yield* Output;
    const stored = yield* credentials.get(endpoint.origin, "computer");
    const pending = yield* pendingStore.get(endpoint.origin);
    if (Option.isSome(stored)) {
      yield* credentials.remove(endpoint.origin, "computer");
    }
    if (Option.isSome(pending) && pending.value.kind === "computer") {
      yield* pendingStore.remove(endpoint.origin);
    }
    yield* output.write(
      {
        value: {
          endpoint: endpoint.origin,
          removed: Option.isSome(stored),
          remote_state: "unverified",
        },
        human: [
          `Removed the local Computer pairing for ${endpoint.origin}.`,
          "No local machine token remains. Remote pairing state is not queried.",
        ],
      },
      outputMode(flags.json),
    );
  }),
).pipe(Command.withDescription("Remove this Computer's local machine token and pairing state"));

const computerJournalCommand = Command.make(
  "journal",
  { limit: computerJournalLimitFlag },
  ({ limit }) =>
    Effect.gen(function* () {
      if (limit < 0) {
        return yield* new InputError({ message: "--limit must be zero or greater." });
      }
      const flags = yield* rootCommand;
      const journal = yield* ComputerJournal;
      const output = yield* Output;
      const entries = yield* journal.read(limit);
      yield* output.write(
        {
          value: {
            schema: "openagents.computer_journal.v1",
            entries,
          },
          human:
            entries.length === 0
              ? ["No local Computer requests are recorded."]
              : entries.map(
                  (entry) =>
                    `${entry.at} ${entry.decision}/${entry.outcome} ${entry.requestId} ${entry.argv.join(" ")}`,
                ),
        },
        outputMode(flags.json),
      );
    }),
).pipe(
  Command.withDescription(
    "Show the local record of Computer requests and decisions, including refusals; the journal is never sent to the server.",
  ),
);

const computerCommand = Command.make("computer").pipe(
  Command.withDescription(
    "Inspect local Computer policy and discovery. No account or pairing is needed; pairing, channel, and delegation are separate commands.",
  ),
  Command.withSubcommands([
    computerProbeCommand,
    computerPolicyCommand,
    computerStatusCommand,
    computerUpCommand,
    computerPairCommand,
    computerLogoutCommand,
    computerJournalCommand,
  ]),
);

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

const importVisibility = Effect.fn("Cli.importVisibility")(function* (
  isPublic: boolean,
  isPrivate: boolean,
) {
  if (isPublic && isPrivate) {
    return yield* new InputError({ message: "Use either --public or --private, not both." });
  }
  if (isPublic) return false;
  if (isPrivate) return true;
  return undefined;
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
const loginHeadlessFlag = Flag.boolean("headless").pipe(
  Flag.withDescription("Return an authorization URL and code without waiting for approval"),
);
const loginScopeFlag = Flag.string("scope").pipe(
  Flag.atLeast(0),
  Flag.withDescription(
    "Request a scope for the new token; repeatable. Omit to take the server's default. Use chat:account to reach the chat API from `openagents coder`.",
  ),
);
const loginResumeFlag = Flag.boolean("resume").pipe(
  Flag.withDescription("Complete the pending device authorization for the selected API"),
);

const resumeCommandFor = (endpoint: {
  readonly origin: string;
  readonly profile: string;
}): string =>
  endpoint.profile === "custom"
    ? `openagents --api-url ${endpoint.origin} auth login --resume`
    : `openagents --profile ${endpoint.profile} auth login --resume`;

const authLoginCommand = Command.make(
  "login",
  {
    tokenStdin: loginTokenStdinFlag,
    headless: loginHeadlessFlag,
    resume: loginResumeFlag,
    scope: loginScopeFlag,
  },
  ({ headless, resume, scope, tokenStdin }) =>
    Effect.gen(function* () {
      if ((tokenStdin && (headless || resume)) || (headless && resume)) {
        return yield* new InputError({
          message: "Use only one of --token-stdin, --headless, or --resume.",
        });
      }
      const flags = yield* rootCommand;
      const endpoint = yield* resolveApiEndpoint(endpointOverrides(flags));
      const credentials = yield* CredentialStore;
      const output = yield* Output;

      if (tokenStdin) {
        const token = Redacted.make(yield* (yield* SecretInput).readToken());
        yield* credentials.set(endpoint.origin, token);
        yield* output.write(
          {
            value: {
              origin: endpoint.origin,
              authenticated: true,
              token_source: "token_stdin",
            },
            human: [
              `Authenticated with ${endpoint.origin}.`,
              "The token is stored in your OS credential store.",
              "Run openagents auth setup-git --local to configure Git for this repository.",
            ],
          },
          outputMode(flags.json),
        );
        return;
      }

      const pendingStore = yield* PendingDeviceAuthorizationStore;
      const existingPending = yield* pendingStore.get(endpoint.origin);
      if (!resume && Option.isSome(existingPending) && existingPending.value.kind === "computer") {
        return yield* new InputError({
          message: `A Computer pairing is already pending for ${endpoint.origin}; complete it before starting API authorization.`,
        });
      }
      if (!resume) {
        const devices = yield* DeviceClient;
        const terminal = yield* TerminalSession;
        const authorization = yield* devices.start(endpoint.origin, scope);
        const returnForApproval = headless || !terminal.interactive || flags.json;
        if (returnForApproval) {
          const now = yield* Clock.currentTimeMillis;
          const resumeCommand = resumeCommandFor(endpoint);
          yield* pendingStore.set({
            origin: endpoint.origin,
            device_code: authorization.device_code,
            user_code: authorization.user_code,
            verification_uri: authorization.verification_uri,
            verification_uri_complete: authorization.verification_uri_complete,
            expires_at_ms: now + authorization.expires_in * 1_000,
            interval: authorization.interval,
          });
          yield* output.write(
            {
              value: {
                origin: endpoint.origin,
                authenticated: false,
                authorization_pending: true,
                verification_url: authorization.verification_uri_complete,
                user_code: authorization.user_code,
                expires_in: authorization.expires_in,
                resume_command: resumeCommand,
              },
              human: [
                "OpenAgents authorization is ready.",
                `Open this URL: ${authorization.verification_uri_complete}`,
                `Authorization code: ${authorization.user_code}`,
                `After you approve the request, run: ${resumeCommand}`,
              ],
            },
            outputMode(flags.json),
          );
          return;
        }

        yield* Console.error(
          `OpenAgents authorization URL: ${authorization.verification_uri_complete}`,
        );
        yield* Console.error(`OpenAgents authorization code: ${authorization.user_code}`);
        const browser = yield* BrowserLauncher;
        if (!(yield* browser.open(authorization.verification_uri_complete))) {
          yield* Console.error("The browser did not open. Open the authorization URL above.");
        }
        yield* Console.error("Waiting for approval...");
        const token = yield* devices.wait(endpoint.origin, authorization);
        yield* credentials.set(endpoint.origin, token);
        yield* output.write(
          {
            value: {
              origin: endpoint.origin,
              authenticated: true,
              token_source: "device_authorization",
            },
            human: [
              `Authenticated with ${endpoint.origin}.`,
              "The token is stored in your OS credential store.",
              "Run openagents auth setup-git --local to configure Git for this repository.",
            ],
          },
          outputMode(flags.json),
        );
        return;
      }

      const pending = yield* pendingStore.get(endpoint.origin);
      if (Option.isNone(pending)) {
        return yield* new InputError({
          message: `No pending authorization exists for ${endpoint.origin}. Run openagents auth login first.`,
        });
      }
      if (pending.value.kind === "computer") {
        return yield* new InputError({
          message: `The pending authorization for ${endpoint.origin} is a Computer pairing; use openagents computer status.`,
        });
      }
      const now = yield* Clock.currentTimeMillis;
      const expiresIn = Math.ceil((pending.value.expires_at_ms - now) / 1_000);
      if (expiresIn <= 0) {
        yield* pendingStore.remove(endpoint.origin);
        return yield* new InputError({
          message: "The pending authorization expired. Run openagents auth login again.",
        });
      }
      const token = yield* (yield* DeviceClient).wait(endpoint.origin, {
        device_code: pending.value.device_code,
        user_code: pending.value.user_code,
        verification_uri: pending.value.verification_uri,
        verification_uri_complete: pending.value.verification_uri_complete,
        expires_in: expiresIn,
        interval: pending.value.interval,
      });
      yield* credentials.set(endpoint.origin, token);
      yield* pendingStore.remove(endpoint.origin);
      yield* output.write(
        {
          value: {
            origin: endpoint.origin,
            authenticated: true,
            token_source: "device_authorization",
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
  Flag.withDescription("Create a private repository"),
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
        onProgress: ({ state, elapsedMs }) =>
          Console.error(
            `Repository provisioning: ${state} (${Math.floor(elapsedMs / 1_000)}s elapsed).`,
          ),
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
      const visibility = yield* importVisibility(isPublic, isPrivate);
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
        waitTimeoutMs: waitTimeout * 1_000,
        onProgress: ({ state, attemptCount, elapsedMs }) =>
          Console.error(
            `Repository import: ${state} (shallow snapshot, attempt ${attemptCount}, ${Math.floor(elapsedMs / 1_000)}s elapsed).`,
          ),
        ...(visibility === undefined ? {} : { private: visibility }),
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

const deleteYesFlag = Flag.boolean("yes").pipe(
  Flag.withDescription("Confirm permanent repository deletion"),
);
const repoDeleteCommand = Command.make(
  "delete",
  { repository: optionalRepositoryArgument, repo: repositoryOverrideFlag, yes: deleteYesFlag },
  ({ repo, repository, yes }) =>
    Effect.gen(function* () {
      if (!yes) {
        return yield* new InputError({
          message: "Repository deletion requires --yes confirmation.",
        });
      }
      const flags = yield* rootCommand;
      const session = yield* resolveApiSession(endpointOverrides(flags));
      const target = yield* resolveRepositoryArgument(repository, repo, session.endpoint.origin);
      const repositories = yield* RepositoryClient;
      const output = yield* Output;
      yield* repositories.remove({
        origin: session.endpoint.origin,
        token: session.token,
        ...target,
      });
      const fullName = `${target.owner}/${target.repo}`;
      yield* output.write(
        {
          value: { full_name: fullName, deleted: true },
          human: [`Deleted ${fullName}.`],
        },
        outputMode(flags.json),
      );
    }),
).pipe(Command.withDescription("Permanently delete a repository you own"));

const repoCommand = Command.make("repo").pipe(
  Command.withDescription("Manage repositories"),
  Command.withSubcommands([
    repoCreateCommand,
    repoImportCommand,
    repoListCommand,
    repoViewCommand,
    repoCloneCommand,
    repoDeleteCommand,
  ]),
);

const apiPathArgument = Argument.string("path").pipe(
  Argument.withDescription(
    `API path. A path without a leading slash resolves under ${API_BASE_PATH}, so repos/OWNER/REPO/issues and ${API_BASE_PATH}repos/OWNER/REPO/issues name the same route`,
  ),
);
const apiMethodFlag = Flag.choice("method", PASSTHROUGH_METHODS).pipe(
  Flag.withAlias("X"),
  Flag.optional,
  Flag.withDescription("Set the HTTP method (defaults to GET, or POST when a body is supplied)"),
);
const apiFieldFlag = Flag.string("field").pipe(
  Flag.withAlias("f"),
  Flag.atLeast(0),
  Flag.withDescription(
    "Add a body field as key=value, repeatable; values are sent as JSON strings",
  ),
);
const apiHeaderFlag = Flag.string("header").pipe(
  Flag.withAlias("H"),
  Flag.atLeast(0),
  Flag.withDescription("Add a request header as 'Name: value', repeatable"),
);
const apiInputFlag = Flag.string("input").pipe(
  Flag.optional,
  Flag.withDescription("Read the whole JSON body from a file, or from - for standard input"),
);

const prettyJson = (value: unknown): string => JSON.stringify(value, null, 2) ?? "null";

const apiCommand = Command.make(
  "api",
  {
    path: apiPathArgument,
    method: apiMethodFlag,
    field: apiFieldFlag,
    header: apiHeaderFlag,
    input: apiInputFlag,
  },
  ({ field, header, input, method, path }) =>
    Effect.gen(function* () {
      if (field.length > 0 && Option.isSome(input)) {
        return yield* new InputError({
          message: "Use either --field or --input, not both.",
        });
      }
      const flags = yield* rootCommand;
      const headers = yield* parseRequestHeaders(header);
      const fields = yield* parseRequestFields(field);
      const session = yield* resolveApiSession(endpointOverrides(flags));
      const requestPath = yield* resolveApiPath(session.endpoint.origin, path);
      const body = Option.isSome(input)
        ? yield* decodeRequestBody(
            yield* (yield* RequestBodyInput).read(input.value),
            input.value === "-" ? "Standard input" : `The file ${input.value}`,
          )
        : field.length > 0
          ? fields
          : undefined;
      const requestMethod = resolveRequestMethod(method, body !== undefined);

      const transport = yield* ApiTransport;
      const response = yield* transport.request({
        origin: session.endpoint.origin,
        method: requestMethod,
        path: requestPath,
        token: session.token,
        ...(Object.keys(headers).length === 0 ? {} : { headers }),
        ...(body === undefined ? {} : { body }),
      });

      const requestId = response.requestId ?? apiErrorDetails(response.body).requestId;
      if (!successfulStatus(response.status)) {
        if (response.body !== null) yield* Console.error(prettyJson(response.body));
        if (requestId !== undefined) yield* Console.error(`Request id: ${requestId}`);
        const details = apiErrorDetails(response.body);
        const summary = `The API returned HTTP ${response.status} for ${requestMethod} ${requestPath}.`;
        return yield* new ApiError({
          operation: "api",
          status: response.status,
          ...(details.code === undefined ? {} : { code: details.code }),
          message: details.message === undefined ? summary : `${summary} ${details.message}`,
          ...(requestId === undefined ? {} : { requestId }),
        });
      }

      const output = yield* Output;
      yield* output.write(
        { value: response.body, human: [prettyJson(response.body)] },
        outputMode(flags.json),
      );
    }),
).pipe(
  Command.withDescription(
    `Send an authenticated request to any OpenAgents API route and write the response body as JSON. A path without a leading slash resolves under ${API_BASE_PATH}; an absolute path must start with /api/ and must stay on the selected API origin. Use --field for string body fields and --input for any other JSON. The two are mutually exclusive`,
  ),
);

const coderPrompt = Argument.string("prompt").pipe(
  Argument.optional,
  Argument.withDescription(
    "Answer this prompt and exit instead of opening the interactive interface",
  ),
);
const coderPlainFlag = Flag.boolean("plain").pipe(
  Flag.withDescription("Use line-oriented output with no cursor control, even on a terminal"),
);
const coderOfflineFlag = Flag.boolean("offline").pipe(
  Flag.withDescription("Answer from the built-in stand-in instead of the chat API"),
);
const coderReasoningFlag = Flag.choice("reasoning", [
  "minimal",
  "low",
  "medium",
  "high",
  "max",
]).pipe(Flag.optional, Flag.withDescription("Reasoning effort the server passes to the provider"));

const coderCommand = Command.make(
  "coder",
  {
    prompt: coderPrompt,
    plain: coderPlainFlag,
    offline: coderOfflineFlag,
    reasoning: coderReasoningFlag,
  },
  ({ prompt, plain, offline, reasoning }) =>
    Effect.gen(function* () {
      const flags = yield* rootCommand;
      const terminal = yield* TerminalSession;
      const workspace = describeWorkspace();
      const endpoint = yield* resolveApiEndpoint(endpointOverrides(flags));

      // Replies come from the account chat API, which runs Ox Alpha through
      // OpenRouter on the server, so the CLI never holds a provider key and a
      // session costs exactly what the server metered. Without a credential it
      // falls back to the stand-in and says so rather than failing.
      const stored = offline
        ? Option.none()
        : yield* findToken(endpoint.origin).pipe(
            Effect.catchTag("OpenAgentsCli.CredentialPersistenceUnavailable", () =>
              Effect.succeed(Option.none()),
            ),
          );

      const source = Option.isSome(stored)
        ? new OxAlphaReplySource({
            origin: endpoint.origin,
            token: Redacted.value(stored.value.token),
            reasoning: Option.getOrUndefined(reasoning),
          })
        : new DummyReplySource();

      const session = new CoderSession(source, workspace.repository, workspace.branch);

      if (Option.isNone(stored) && !offline) {
        session.notice(
          "No stored credential, so replies come from the built-in stand-in. " +
            "Run `openagents auth login --scope chat:account` to reach Ox Alpha.",
        );
      }

      const oneShot = Option.getOrUndefined(prompt);
      const interactive = terminal.interactive && !plain && !flags.json && oneShot === undefined;

      const code = yield* Effect.promise(() =>
        interactive
          ? runCoderUi(session, { stdin: process.stdin, stdout: process.stdout })
          : runCoderPlain(session, {
              stdin: process.stdin,
              stdout: process.stdout,
              prompt: oneShot,
            }),
      );

      if (code !== 0) {
        process.exitCode = code;
      }
    }),
).pipe(
  Command.withDescription(
    "Open a terminal coding session. Replies come from Ox Alpha through the account chat API; --offline answers from a built-in stand-in instead",
  ),
);

// The forum commands and their client were published in the CLI but their
// source was never committed. Both are reconstructed from the compiled
// artifacts of that build; see `forum-client.ts` and issue #153.

const forumBoardFlag = Flag.string("board").pipe(
  Flag.optional,
  Flag.withDescription("The board slug, such as general"),
);
const forumPageFlag = Flag.string("page").pipe(
  Flag.optional,
  Flag.withDescription("One-based page number"),
);

/** A page number, or undefined when the flag is absent or not a page. */
const parsePage = (page: Option.Option<string>): number | undefined => {
  if (Option.isNone(page)) return undefined;
  const parsed = Number.parseInt(page.value, 10);
  return Number.isFinite(parsed) && parsed >= 1 ? parsed : undefined;
};

const record = (value: unknown): Record<string, unknown> =>
  value !== null && typeof value === "object" ? (value as Record<string, unknown>) : {};
const rows = (value: unknown, key: string): ReadonlyArray<Record<string, unknown>> => {
  const list = record(value)[key];
  return Array.isArray(list) ? list.map(record) : [];
};

const forumBoardsCommand = Command.make("boards", {}, () =>
  Effect.gen(function* () {
    const flags = yield* rootCommand;
    const session = yield* resolveApiSession(endpointOverrides(flags));
    const forums = yield* ForumClient;
    const output = yield* Output;
    const value = yield* forums.boards({ origin: session.endpoint.origin, token: session.token });
    const boards = rows(value, "boards");
    const human =
      boards.length === 0
        ? ["No boards found."]
        : boards.map(
            (board) =>
              `${String(board["slug"])} — ${String(board["title"])} (${String(board["topic_count"])} topics)`,
          );
    yield* output.write({ value, human }, outputMode(flags.json));
  }),
).pipe(Command.withDescription("List forum boards"));

const forumTopicsCommand = Command.make(
  "topics",
  { board: forumBoardFlag, page: forumPageFlag },
  ({ board, page }) =>
    Effect.gen(function* () {
      if (Option.isNone(board)) {
        return yield* new InputError({ message: "Pass --board with the board slug." });
      }
      const flags = yield* rootCommand;
      const session = yield* resolveApiSession(endpointOverrides(flags));
      const forums = yield* ForumClient;
      const output = yield* Output;
      const pageNum = parsePage(page);
      const value = yield* forums.topics({
        origin: session.endpoint.origin,
        token: session.token,
        board: board.value,
        ...(pageNum === undefined ? {} : { page: pageNum }),
      });
      const topics = rows(value, "topics");
      const human =
        topics.length === 0
          ? ["No topics found."]
          : topics.map(
              (topic) =>
                `${String(topic["id"]).slice(0, 8)} — ${String(topic["title"])} (${String(topic["posts_count"])} posts)`,
            );
      yield* output.write({ value, human }, outputMode(flags.json));
    }),
).pipe(Command.withDescription("List the topics in one forum board"));

const topicIdArgument = Argument.string("id").pipe(
  Argument.withDescription("Topic id (the prefix of a topic URL works too)"),
);

const forumTopicCommand = Command.make(
  "topic",
  { id: topicIdArgument, page: forumPageFlag },
  ({ id, page }) =>
    Effect.gen(function* () {
      const flags = yield* rootCommand;
      const session = yield* resolveApiSession(endpointOverrides(flags));
      const forums = yield* ForumClient;
      const output = yield* Output;
      const pageNum = parsePage(page);
      const value = yield* forums.topic({
        origin: session.endpoint.origin,
        token: session.token,
        id,
        ...(pageNum === undefined ? {} : { page: pageNum }),
      });
      const topic = record(record(value)["topic"]);
      const human = [
        String(topic["title"] ?? ""),
        ...rows(value, "posts").map((post) => {
          const author = record(post["author"]);
          const name = author["display_name"] === undefined ? "?" : String(author["display_name"]);
          return `#${String(post["post_number"])} ${name}: ${String(post["body_text"] ?? "").slice(0, 120)}`;
        }),
      ];
      yield* output.write({ value, human }, outputMode(flags.json));
    }),
).pipe(Command.withDescription("Read one forum topic and its posts"));

const forumTitleFlag = Flag.string("title").pipe(Flag.withDescription("Topic title"));
const forumBodyFlag = Flag.string("body").pipe(
  Flag.optional,
  Flag.withDescription("Post body text"),
);

const forumPostCommand = Command.make(
  "post",
  { title: forumTitleFlag, body: forumBodyFlag, board: forumBoardFlag },
  ({ title, body, board }) =>
    Effect.gen(function* () {
      if (title.trim() === "") {
        return yield* new InputError({ message: "Pass --title for the new topic." });
      }
      if (Option.isNone(body)) {
        return yield* new InputError({ message: "Pass --body with the first post text." });
      }
      const flags = yield* rootCommand;
      const session = yield* resolveApiSession(endpointOverrides(flags));
      const forums = yield* ForumClient;
      const output = yield* Output;
      const value = yield* forums.createTopic({
        origin: session.endpoint.origin,
        token: session.token,
        board: Option.getOrElse(board, () => "general"),
        title,
        bodyText: body.value,
      });
      const created = record(record(value)["topic"]);
      yield* output.write(
        { value, human: [`Created topic ${String(created["url"] ?? "")}`] },
        outputMode(flags.json),
      );
    }),
).pipe(Command.withDescription("Create a forum topic (--board defaults to general)"));

const topicArgument = Argument.string("topic").pipe(Argument.withDescription("Topic id or URL"));

const forumReplyCommand = Command.make(
  "reply",
  { topic: topicArgument, body: forumBodyFlag },
  ({ topic, body }) =>
    Effect.gen(function* () {
      // A pasted topic URL carries the id; take it rather than making the
      // reader extract it.
      const match = /([\da-f]{8}-[\da-f]{4}-[\da-f]{4}-[\da-f]{4}-[\da-f]{12})/i.exec(topic);
      const topicId = match === null ? topic : (match[1] ?? topic);
      if (Option.isNone(body)) {
        return yield* new InputError({ message: "Pass --body with the reply text." });
      }
      const flags = yield* rootCommand;
      const session = yield* resolveApiSession(endpointOverrides(flags));
      const forums = yield* ForumClient;
      const output = yield* Output;
      const value = yield* forums.reply({
        origin: session.endpoint.origin,
        token: session.token,
        topicId,
        bodyText: body.value,
      });
      yield* output.write({ value, human: ["Reply posted."] }, outputMode(flags.json));
    }),
).pipe(Command.withDescription("Reply to a forum topic"));

const actorRefArgument = Argument.string("actor_ref").pipe(
  Argument.withDescription("Legacy identity, such as agent:user_ed8297d8-…"),
);

const forumClaimCommand = Command.make("claim", { actorRef: actorRefArgument }, ({ actorRef }) =>
  Effect.gen(function* () {
    const flags = yield* rootCommand;
    const session = yield* resolveApiSession(endpointOverrides(flags));
    const forums = yield* ForumClient;
    const output = yield* Output;
    const value = yield* forums.claim({
      origin: session.endpoint.origin,
      token: session.token,
      actorRef,
    });
    yield* output.write({ value, human: ["Claim submitted for review."] }, outputMode(flags.json));
  }),
).pipe(Command.withDescription("Claim a legacy forum identity for your account"));

const forumClaimsCommand = Command.make("claims", {}, () =>
  Effect.gen(function* () {
    const flags = yield* rootCommand;
    const session = yield* resolveApiSession(endpointOverrides(flags));
    const forums = yield* ForumClient;
    const output = yield* Output;
    const value = yield* forums.claims({ origin: session.endpoint.origin, token: session.token });
    const claims = rows(value, "claims");
    const human =
      claims.length === 0
        ? ["No claims yet."]
        : claims.map((claim) => `${String(claim["actor_ref"])} — ${String(claim["status"])}`);
    yield* output.write({ value, human }, outputMode(flags.json));
  }),
).pipe(Command.withDescription("List your legacy identity claims"));

const forumCommand = Command.make("forum").pipe(
  Command.withDescription("Read and write the OpenAgents forum"),
  Command.withSubcommands([
    forumBoardsCommand,
    forumTopicsCommand,
    forumTopicCommand,
    forumPostCommand,
    forumReplyCommand,
    forumClaimCommand,
    forumClaimsCommand,
  ]),
);

export const openagentsCommand = rootCommand.pipe(
  Command.withSubcommands([
    apiCommand,
    authCommand,
    coderCommand,
    computerCommand,
    forumCommand,
    repoCommand,
  ]),
);

export const runCliWith = Command.runWith(openagentsCommand, { version: VERSION });
