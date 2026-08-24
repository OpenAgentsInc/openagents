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
import type { ChildGrant } from "./coder-child-gateway.js";
import { startChildGateway } from "./coder-child-gateway.js";
import { writeChildHarnessConfig } from "./coder-child-config.js";
import type { DelegationOutcome } from "./coder-delegate.js";
import {
  CHILD_MODELS,
  childLaneName,
  DelegateFleet,
  DevinHarness,
  describePrompt,
  firstAvailableChildModel,
  OpencodeHarness,
  resolveChildLane,
} from "./coder-delegate.js";
import { fleetPlainLines } from "./coder-fleet.js";
import { runCoderPlain } from "./coder-plain.js";
import type { CoderDelegation } from "./coder-session.js";
import type { ReplySource } from "./coder-session.js";
import { CoderSession, DummyReplySource } from "./coder-session.js";
import { CoderTaskRegistry } from "./coder-tasks.js";
import { runCoderUi } from "./coder-ui.js";
import { backendIds } from "./coder-backends.js";
import {
  discoverOllamaModel,
  isOllamaModelFlag,
  OllamaReplySource,
  parseOllamaModelFlag,
  resolveOllamaModel,
} from "./coder-ollama.js";
import {
  openThread,
  remintThread,
  ThreadUnavailable,
  type ThreadReplySource,
} from "./coder-thread.js";
import {
  assertResumable,
  fetchAllEvents,
  fetchThread,
  listThreads,
  pickLast,
  pickThread,
  replayEntries,
  replayWire,
  resumableThreads,
  type ThreadSummary,
} from "./coder-resume.js";
import { ThreadTranscriptWriter } from "./coder-transcript.js";
import { delegateTool, openagentsTool, shellTool, skillTool } from "./coder-tools.js";
import {
  describeLoad,
  isRefusal,
  loadPluginFromManifest,
  pluginIdentity,
  pluginTool,
  type LoadedPlugin,
} from "./coder-plugins.js";
import { spawnSync } from "node:child_process";
import { resolve as resolvePath } from "node:path";

import { rebuild, RELOAD_EXIT_CODE, sourceCheckout } from "./coder-reload.js";
import { loadSkillSelection, standingContext } from "./coder-skills.js";
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
  NetworkRefused,
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
import { IssueClient } from "./issue-client.js";
import { runGitCredentialHelper } from "./git-credential-helper.js";
import { Output, type OutputMode } from "./output.js";
import { ProjectClient } from "./project-client.js";
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
    "Request a scope for the new token; repeatable. Omit to take the server's default, which already reaches the chat API from `openagents coder`.",
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
    "Answer this prompt and exit instead of opening the interactive interface. " +
      "With --resume, this names the thread id to continue",
  ),
);
const coderResumeFlag = Flag.boolean("resume").pipe(
  Flag.withDescription(
    "Continue a thread of the account's instead of opening a new one. Bare --resume " +
      "shows a picker over this repository's recent threads; `--resume <id>` names one; " +
      "`--resume --last` continues the most recent without asking",
  ),
);
const coderLastFlag = Flag.boolean("last").pipe(
  Flag.withDescription("With --resume, continue the most recent thread without asking"),
);
const coderAllFlag = Flag.boolean("all").pipe(
  Flag.withDescription(
    "With --resume, list every thread on the account rather than this repository's",
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
]).pipe(
  Flag.optional,
  Flag.withDescription("Reasoning effort recorded on the thread as its admitted execution shape"),
);
// `--model` can name an `ollama:<model>` local model or a chat API backend.
// For a chat API backend a thread's grant still pins its own model and
// `POST /api/v3/threads` publishes no model parameter, so naming one cannot
// change which model answers. For `ollama:<model>` the local Ollama server is
// used directly and the named model is the one that runs.
const coderModelFlag = Flag.string("model").pipe(
  Flag.optional,
  Flag.withDescription(
    "A model name. Use `ollama:<model>` for a local Ollama server, or a chat API backend id",
  ),
);

/**
 * Turn a refused thread into an error the CLI already knows how to print.
 *
 * The server's code and sentence are carried through unchanged, so a caller
 * reading `--json` branches on `thread_quota_reached` and a person reading the
 * terminal is told the limit and how many threads the account is holding.
 */
const coderRefusal = (origin: string, cause: unknown, operation = "coder.thread.open") => {
  if (!(cause instanceof ThreadUnavailable)) {
    return new InputError({ message: `The thread could not be opened: ${String(cause)}` });
  }
  if (cause.code === "network_refused") {
    return new NetworkRefused({ origin, message: cause.message });
  }
  return new ApiError({
    operation,
    status: cause.status,
    code: cause.code,
    message: cause.message,
  });
};

/**
 * Delegation flags, shared by `coder` and `delegate`.
 *
 * None of them is required. Children run on the session's own thread grant
 * through a loopback gateway, which is what makes delegation work in a fresh
 * install with nothing configured: the earlier design demanded a child model
 * and a provider credential of the reader's own, so the fleet was off by
 * default and asking for one got a refusal. These flags are for the case where
 * somebody wants children on a different provider than the session.
 */
const childModelFlag = Flag.string("child-model").pipe(
  Flag.optional,
  Flag.withDescription(
    "Run children on this model instead of the session's own, as `provider/model`. " +
      "Defaults to OPENAGENTS_DELEGATE_MODEL, and to the session's thread grant when " +
      "neither is set",
  ),
);
const childCommandFlag = Flag.string("child-command").pipe(
  Flag.optional,
  Flag.withDescription(
    "The harness that runs a child. Defaults to OPENAGENTS_DELEGATE_COMMAND, or `opencode`",
  ),
);
const childConfigFlag = Flag.string("child-config").pipe(
  Flag.optional,
  Flag.withDescription(
    "A harness config file for children, passed as OPENCODE_CONFIG. This is how a " +
      "provider credential reaches a child without being stored by the CLI",
  ),
);
const childAskFlag = Flag.boolean("child-ask").pipe(
  Flag.withDescription(
    "Make children ask before using a tool. A delegated child has nobody to ask, so " +
      "this stops it at its first edit; it exists for a dry run over a directory you " +
      "do not want touched",
  ),
);
const concurrencyFlag = Flag.integer("concurrency").pipe(
  Flag.withDefault(4),
  Flag.withDescription("How many children may run at once. The rest queue"),
);

/**
 * The model delegated children run on.
 *
 * A child is a coding agent, and the model a session's own turns run on is
 * chosen for conversation, so children are pinned to Ox Alpha rather than
 * inheriting the parent's. It is a separate thread with its own budget, so a
 * fan-out cannot spend the authority the conversation is holding, and the
 * server issues it — no provider key reaches this process either way.
 */
const CHILD_THREAD_MODEL = "ox-alpha";

/** The children's thread, or the server's own words for why there is none. */
type ChildThread =
  | { readonly kind: "opened"; readonly thread: ThreadReplySource }
  | { readonly kind: "refused"; readonly reason: string };

/**
 * Open the thread children spend.
 *
 * A refusal is reported, never absorbed. Lending children the conversation's
 * own grant instead would run them on the conversation's model while the
 * interface said Ox Alpha, so a session that cannot open this thread delegates
 * to nothing and says which refusal stopped it.
 */
async function openChildThread(options: {
  readonly origin: string;
  readonly token: string;
  readonly objective: string;
}): Promise<ChildThread> {
  try {
    return {
      kind: "opened",
      thread: await openThread({
        origin: options.origin,
        token: options.token,
        objective: options.objective,
        model: process.env["OPENAGENTS_DELEGATE_THREAD_MODEL"] ?? CHILD_THREAD_MODEL,
      }),
    };
  } catch (cause) {
    const reason =
      cause instanceof ThreadUnavailable
        ? `${cause.message} (${cause.code})`
        : `The thread could not be opened: ${String(cause)}`;
    return { kind: "refused", reason };
  }
}

/** Delegation and whatever has to be torn down with it. */
interface DelegationSetup {
  readonly delegation: CoderDelegation;
  /** Stops the child gateway and removes the generated harness config. */
  close(): Promise<void>;
}

/**
 * Assemble delegation.
 *
 * Three ways a child gets a model, in order: a model named on the command line
 * or in the environment, with the reader's own harness config; the session's
 * thread grant, lent to children through a loopback gateway; or nothing, which
 * is only reached with no credential and no flag, and is what makes `/delegate`
 * say so instead of failing later.
 */
async function buildDelegation(options: {
  readonly model: string | undefined;
  readonly command: string | undefined;
  readonly configPath: string | undefined;
  readonly autoApprove: boolean;
  readonly concurrency: number;
  readonly cwd: string;
  /** The session's grant, when it has one. Children spend it by default. */
  readonly grant: ChildGrant | undefined;
  /** Mint a fresh grant, for when the one in hand has expired. */
  readonly refreshGrant?: (() => Promise<ChildGrant | undefined>) | undefined;
}): Promise<DelegationSetup | undefined> {
  const named = options.model ?? process.env["OPENAGENTS_DELEGATE_MODEL"];
  const command = options.command ?? process.env["OPENAGENTS_DELEGATE_COMMAND"];
  const namedConfig = options.configPath ?? process.env["OPENAGENTS_DELEGATE_CONFIG"];

  // One registry for the session, whatever a call chooses to run on, so the
  // children of two models still render as one fleet and stop together.
  const registry = new CoderTaskRegistry();

  // Labelled by the name that was asked for. A caller who names `ox-alpha` and
  // is answered `x-preview-f-free` cannot tell whether the request was honoured
  // or silently fell back, and one that was asked exactly this said so rather
  // than guess.
  const laneFor = (choice: string) => {
    const harness = /^devin(:.+)?$/.test(choice)
      ? new DevinHarness(choice.startsWith("devin:") ? { permissionMode: choice.slice(6) } : {})
      : new OpencodeHarness({
          model: choice,
          ...(command === undefined ? {} : { command }),
          ...(namedConfig === undefined ? {} : { configPath: namedConfig }),
          autoApprove: options.autoApprove,
        });

    return {
      fleet: new DelegateFleet(registry, harness, {
        maxConcurrent: Math.max(1, options.concurrency),
        cwd: options.cwd,
      }),
      label: `${harness.agent} (${childLaneName(harness.model)})`,
    };
  };

  // Cached, so a second call on the same model reuses its fleet rather than
  // starting a second one that competes with the first for the same cap.
  const lanes = new Map<string, { fleet: DelegateFleet; label: string }>();
  const fleetFor = (choice: string) => {
    const lane = resolveChildLane(choice);
    if (lane === undefined) return undefined;
    const existing = lanes.get(lane);
    if (existing !== undefined) return existing;
    const built = laneFor(lane);
    lanes.set(lane, built);
    return built;
  };

  // `--child-model devin` runs children on the Devin CLI instead. It brings its
  // own credentials and its own model, so it needs neither this session's grant
  // nor a gateway, and it is refused up front when it is not installed rather
  // than once per child.
  // An alias on the flag too, so `--child-model ox-alpha` means what it says
  // and the skill that documents it is not documenting a thing that fails.
  const askedFor = named === undefined ? undefined : resolveChildLane(named);

  if (askedFor !== undefined && /^devin(:(.+))?$/.test(askedFor)) {
    const mode = /^devin:(.+)$/.exec(askedFor)?.[1];
    const lane = laneFor(mode === undefined ? "devin" : `devin:${mode}`);
    return {
      delegation: { registry, ...lane, models: CHILD_MODELS, fleetFor },
      close: () => Promise.resolve(),
    };
  }

  let model: string;
  let configPath: string | undefined;
  let close: () => Promise<void>;

  // Free and grant-free first. A thread grant lives an hour, has to be minted,
  // and expires under a console that outlives it; the harness's own catalog
  // costs nothing and needs no credential from us. The grant stays as the
  // fallback for a machine whose harness lists none of them.
  const free =
    named === undefined || named.trim().length === 0
      ? await firstAvailableChildModel(command ?? "opencode")
      : undefined;

  if (free !== undefined) {
    const lane = fleetFor(free) ?? laneFor(free);
    return {
      delegation: { registry, ...lane, models: CHILD_MODELS, fleetFor },
      close: () => Promise.resolve(),
    };
  }

  if (named !== undefined && named.trim().length > 0) {
    // `--child-model ox-alpha` means the lane, not a literal model name the
    // harness has never heard of. Resolved here so the flag and the tool's
    // `model` parameter accept the same words, and an unrecognised name is
    // still passed through for the harness to refuse by name.
    model = askedFor ?? named;
    configPath = namedConfig;
    close = () => Promise.resolve();
  } else if (options.grant !== undefined) {
    const gateway = await startChildGateway(options.grant, options.refreshGrant);
    const harnessConfig = writeChildHarnessConfig({
      baseUrl: gateway.baseUrl,
      model: options.grant.model,
    });
    model = gateway.modelId;
    configPath = harnessConfig.path;
    close = async () => {
      await gateway.close();
      harnessConfig.remove();
    };
  } else {
    return undefined;
  }

  const harness = new OpencodeHarness({
    model,
    command,
    configPath,
    autoApprove: options.autoApprove,
  });
  const fleet = new DelegateFleet(registry, harness, {
    maxConcurrent: Math.max(1, options.concurrency),
    cwd: options.cwd,
  });
  return {
    delegation: {
      registry,
      fleet,
      label: `${harness.agent} (${model})`,
      models: CHILD_MODELS,
      fleetFor,
    },
    close,
  };
}

const coderCommand = Command.make(
  "coder",
  {
    prompt: coderPrompt,
    plain: coderPlainFlag,
    offline: coderOfflineFlag,
    resume: coderResumeFlag,
    last: coderLastFlag,
    all: coderAllFlag,
    reasoning: coderReasoningFlag,
    model: coderModelFlag,
    childModel: childModelFlag,
    childCommand: childCommandFlag,
    childConfig: childConfigFlag,
    childAsk: childAskFlag,
    concurrency: concurrencyFlag,
  },
  ({
    prompt,
    plain,
    offline,
    resume,
    last,
    all,
    reasoning,
    model,
    childModel,
    childCommand,
    childConfig,
    childAsk,
    concurrency,
  }) =>
    Effect.gen(function* () {
      const flags = yield* rootCommand;
      const terminal = yield* TerminalSession;
      const workspace = describeWorkspace();
      const endpoint = yield* resolveApiEndpoint(endpointOverrides(flags));

      // A `--model` value that starts with `ollama:` goes to the local Ollama
      // server and needs no account credential.
      //
      // With no `--model` at all, a machine already running Ollama answers from
      // it. That is the cheaper and more private default, and it is the one a
      // reader who installed Ollama meant: naming the model every time to get
      // the model they already chose is a flag that carries no decision. The
      // hosted backends stay one `--model` away, and `--offline` asks for
      // neither.
      // `--resume` continues a server thread, so the lanes that never touch
      // one are refused up front rather than silently ignored: a resumed
      // session that answered from the stand-in or a local model would show
      // the history of a thread it is not continuing.
      if ((last || all) && !resume) {
        return yield* new InputError({
          message: `--${last ? "last" : "all"} belongs to --resume.`,
        });
      }
      if (resume && offline) {
        return yield* new InputError({
          message: "--resume reads the thread from the server; it cannot combine with --offline.",
        });
      }
      if (resume && Option.isSome(model)) {
        return yield* new InputError({
          message:
            "--resume continues the thread on the model its grant pins; --model cannot change it.",
        });
      }

      const named = Option.getOrUndefined(model);
      const localModel =
        named === undefined && !offline && !resume
          ? yield* Effect.promise(() => discoverOllamaModel())
          : undefined;

      const wantsOllama = named === undefined ? localModel !== undefined : isOllamaModelFlag(named);
      const askedFor =
        named === undefined
          ? localModel
          : isOllamaModelFlag(named)
            ? parseOllamaModelFlag(named)
            : undefined;

      // A name is resolved against what is installed, so `ollama:qwen3.8`
      // reaches `qwen3.8:27b-mtp-q8_0`. An Ollama name carries its size and
      // quantisation after a colon, and a reader names the model they pulled;
      // sending that unresolved gets `model not found` from a server that has
      // it, which reads as the model being missing. The discovered default is
      // already a real name and needs no round trip.
      const resolved =
        wantsOllama && askedFor !== undefined && named !== undefined
          ? yield* Effect.promise(() => resolveOllamaModel(askedFor))
          : undefined;

      if (resolved !== undefined && resolved.model === undefined) {
        return yield* new InputError({
          message:
            resolved.installed.length === 0
              ? `No Ollama model matches ${askedFor}, and none are installed. Pull one with \`ollama pull\`.`
              : `No Ollama model matches ${askedFor}. Installed: ${resolved.installed.join(", ")}.`,
        });
      }

      const ollamaName = resolved?.model ?? askedFor;

      // Any other `--model` value still has to name a published backend. The
      // flag takes a string so an `ollama:` prefix can reach the local server,
      // which costs the enum `Flag.choice` used to enforce, so the check moves
      // here rather than disappearing.
      if (named !== undefined && !wantsOllama && !backendIds().includes(named)) {
        return yield* new InputError({
          message: `Unknown model ${named}. Use ollama:<model> for a local Ollama server, or one of: ${backendIds().join(", ")}.`,
        });
      }

      // The session opens a thread of its own and spends that thread's grant,
      // so the CLI still holds no provider key and nothing typed here reaches
      // the account's conversation. Without a credential it falls back to the
      // stand-in and says so rather than failing.
      // An Ollama session reads the credential too, though it opens no thread of
      // its own. The parent answers locally; children still spend a server
      // grant, so a local session delegates exactly as a thread session does.
      const stored = offline
        ? Option.none()
        : yield* findToken(endpoint.origin).pipe(
            Effect.catchTag("OpenAgentsCli.CredentialPersistenceUnavailable", () =>
              Effect.succeed(Option.none()),
            ),
          );

      // `--resume`: pick the thread, replay its transcript through the events
      // cursor, and re-mint its authority so the same thread continues on the
      // same grant lineage. The replay is read-only — nothing here posts an
      // event — and the picker is TTY-only: the non-interactive forms are
      // `--resume <id>` and `--resume --last`.
      const resumed = resume
        ? yield* Effect.tryPromise({
            try: async () => {
              if (Option.isNone(stored)) {
                throw new ThreadUnavailable(
                  "scope_missing",
                  "Resuming reads the account's threads. Run `openagents auth login` first.",
                );
              }
              const api = { origin: endpoint.origin, token: Redacted.value(stored.value.token) };
              const explicit = Option.getOrUndefined(prompt);

              let summary: ThreadSummary | undefined;
              if (explicit !== undefined) {
                summary = await fetchThread({ ...api, threadId: explicit });
              } else {
                const candidates = resumableThreads(
                  await listThreads(api),
                  workspace.repository,
                  all,
                );
                if (candidates.length === 0) {
                  throw new ThreadUnavailable(
                    "nothing_to_resume",
                    all
                      ? "This account holds no threads to resume."
                      : `No threads were opened from ${workspace.repository}. ` +
                          "Use --all to list every thread on the account.",
                  );
                }
                if (last) {
                  summary = pickLast(candidates);
                } else if (terminal.interactive && !plain && !flags.json) {
                  summary = await pickThread(candidates, {
                    stdin: process.stdin,
                    stdout: process.stdout,
                  });
                  // An empty answer cancels, and cancelling is not a failure.
                  if (summary === undefined) return undefined;
                } else {
                  throw new ThreadUnavailable(
                    "picker_needs_terminal",
                    "The picker needs a terminal. Use `--resume <id>` or `--resume --last`.",
                  );
                }
              }
              if (summary === undefined) return undefined;

              assertResumable(summary);
              const events = await fetchAllEvents({ ...api, threadId: summary.id });
              const source = await remintThread({ ...api, threadId: summary.id });
              // The replayed history reaches the model transcript and the
              // interface, never the transcript writer: the server already
              // holds these events, and a resume must not post them twice.
              source.preload(replayWire(events));
              return { source, entries: replayEntries(events) };
            },
            catch: (cause) => coderRefusal(endpoint.origin, cause, "coder.thread.resume"),
          })
        : undefined;

      if (resume && resumed === undefined) return;

      const thread =
        resumed !== undefined
          ? resumed.source
          : Option.isSome(stored) && !wantsOllama && !resume
            ? yield* Effect.tryPromise({
                try: () =>
                  openThread({
                    origin: endpoint.origin,
                    token: Redacted.value(stored.value.token),
                    objective: `openagents coder in ${workspace.repository} on ${workspace.branch}`,
                    reasoning: Option.getOrUndefined(reasoning),
                  }),
                // The server's own code and sentence, which is what turns a ninth
                // concurrent session from an obscure failure into an instruction
                // naming the ceiling and how many threads the account is holding.
                catch: (cause) => coderRefusal(endpoint.origin, cause),
              })
            : undefined;

      // A `--model ollama:<name>` session answers from the local Ollama server,
      // so it takes neither a thread nor the stand-in.
      const source: ReplySource =
        wantsOllama && ollamaName !== undefined
          ? new OllamaReplySource({
              model: ollamaName,
              ...(Option.isSome(reasoning) ? { reasoning: reasoning.value } : {}),
            })
          : (thread ?? new DummyReplySource());

      // Children get their own thread on their own model. The conversation
      // stays on the model it opened with, and a fan-out spends a budget the
      // reader's next question does not share.
      const childThread = Option.isSome(stored)
        ? yield* Effect.promise(() =>
            openChildThread({
              origin: endpoint.origin,
              token: Redacted.value(stored.value.token),
              objective: `delegated children of openagents coder in ${workspace.repository}`,
            }),
          )
        : undefined;

      const childGrant = childThread?.kind === "opened" ? childThread.thread.childGrant : undefined;

      const setup = yield* Effect.promise(() =>
        buildDelegation({
          model: Option.getOrUndefined(childModel),
          command: Option.getOrUndefined(childCommand),
          configPath: Option.getOrUndefined(childConfig),
          autoApprove: !childAsk,
          concurrency,
          cwd: process.cwd(),
          grant: childGrant,
          // A thread grant lives an hour; a console does not stop at one. When
          // the grant in hand has expired, another thread is opened and its
          // grant used, so a fan-out started in the afternoon does not fail on
          // a credential minted at breakfast.
          refreshGrant: async () => {
            if (Option.isNone(stored)) return undefined;
            const reopened = await openChildThread({
              origin: endpoint.origin,
              token: Redacted.value(stored.value.token),
              objective: `delegated children of openagents coder in ${workspace.repository}`,
            }).catch(() => undefined);
            return reopened?.kind === "opened" ? reopened.thread.childGrant : undefined;
          },
        }),
      );

      const skills = loadSkillSelection();
      const session = new CoderSession(
        source,
        workspace.repository,
        workspace.branch,
        setup?.delegation,
        standingContext(skills.active(), process.cwd()),
      );

      // The resumed thread's history goes on the session before anything new,
      // so both interfaces open showing the conversation being continued.
      if (resumed !== undefined) session.restore(resumed.entries);

      // The thread lane writes its transcript to the server as the turn loop
      // runs — `POST /api/v3/threads/{id}/events`, on the account token that
      // opened the thread. The server copy is the only durable copy; the
      // offline, Ollama, and stand-in lanes keep no record and attach nothing.
      // A failed post never reaches the turn loop: the writer queues, retries,
      // and says so once on the status line.
      const transcript =
        thread !== undefined && Option.isSome(stored)
          ? new ThreadTranscriptWriter({
              origin: endpoint.origin,
              threadId: thread.threadId,
              token: Redacted.value(stored.value.token),
              onTrouble: (message) => {
                session.notice(message);
              },
            })
          : undefined;
      if (transcript !== undefined) thread?.useTranscript(transcript);

      // The model is told what it can do rather than the reader being asked to
      // remember a slash command. A turn that needs three agents asks for them
      // mid-sentence, and `/delegate` stays as the way to launch a fan-out
      // without spending a turn to ask for one.
      // Skills do not depend on delegation: a session with no credential still
      // reads this repository's conventions, it just cannot hand work to a
      // child. A session with neither declares no tools at all.
      // Re-declared rather than declared once: switching a skill off in
      // `/skills` has to change what the next turn carries, and the tool
      // holding the catalog is the thing that changes.
      // Session-scoped WASM plugins, loaded with `/plugin load <manifest>`.
      // Experimental: the demo ahead of the plugin walking skeleton. A loaded
      // plugin materializes one tool for the rest of this session and nothing
      // outlives the process.
      const plugins: LoadedPlugin[] = [];
      const declareTools = () => {
        const active = skills.active();
        const tools = [
          shellTool(process.cwd()),
          ...(active.length === 0 ? [] : [skillTool(active)]),
          openagentsTool(),
          ...(setup === undefined ? [] : [delegateTool(setup.delegation)]),
          ...plugins.map((plugin) => pluginTool(plugin)),
        ];
        source.useTools?.(tools);
      };
      declareTools();

      const loadPlugin = (manifestPath: string): string => {
        const manifestFile = resolvePath(process.cwd(), manifestPath);
        const outcome = loadPluginFromManifest(manifestFile);
        const described = describeLoad(outcome);
        // The load is recorded both ways on purpose: the notice the interface
        // shows stays interface chatter, and the typed occurrence on the
        // session is what `/export` writes as a `source: "system"` step — a
        // capability-surface change with the full digest, ordered among the
        // turns. Refusals are recorded too; a trajectory that omits the load
        // that failed cannot explain the session that follows it.
        if (isRefusal(outcome)) {
          session.recordPluginEvent({
            message: described,
            event: "plugin_load_refused",
            code: outcome.code,
            plugin: { manifestPath: manifestFile },
          });
          return described;
        }
        // Reloading a name replaces it: a demo iterates on one plugin, and
        // two tools with one name would be a declaration the model cannot
        // tell apart.
        const at = plugins.findIndex((held) => held.manifest.name === outcome.manifest.name);
        if (at >= 0) plugins.splice(at, 1);
        plugins.push(outcome);
        declareTools();
        const identity = pluginIdentity(outcome);
        session.recordPluginEvent({
          message: described,
          event: "plugin_loaded",
          plugin: {
            name: identity.name,
            version: identity.version,
            artifactDigest: identity.artifactDigest,
            bytes: identity.bytes,
            abi: identity.abi,
            timeoutMs: identity.timeoutMs,
            capabilities: identity.capabilities,
            manifestPath: manifestFile,
            toolName: identity.toolName,
          },
        });
        return described;
      };

      // Delegation is off rather than quietly running children on the
      // conversation's model, so the refusal that turned it off is what the
      // reader sees.
      if (childThread?.kind === "refused") {
        session.notice(`This session cannot delegate: ${childThread.reason}`);
      }

      if (Option.isNone(stored) && !offline && !wantsOllama) {
        session.notice(
          "No stored credential, so replies come from the built-in stand-in. " +
            "Run `openagents auth login` to reach a real model.",
        );
      }

      if (wantsOllama && ollamaName === undefined) {
        session.notice(
          "`--model ollama:` is missing a model name. Use `ollama:<model>`, " +
            "for example `ollama:qwen3.8:27b-mtp-q8_0`.",
        );
      }

      // A grant pins the model the proxy will use, and the thread route takes
      // no model parameter, so a named backend cannot reach this turn. Saying
      // nothing would leave a reader with a flag that appeared to work.
      if (thread !== undefined && Option.isSome(model) && !wantsOllama) {
        session.notice(
          `This thread's grant pins ${thread.model}. \`--model\` names a chat API ` +
            "backend, which the inference proxy does not route to, so it had no effect.",
        );
      }

      // With --resume the positional argument named the thread, not a prompt.
      const oneShot = resume ? undefined : Option.getOrUndefined(prompt);
      const interactive = terminal.interactive && !plain && !flags.json && oneShot === undefined;

      const code = yield* Effect.promise(async () => {
        try {
          return interactive
            ? await runCoderUi(session, {
                stdin: process.stdin,
                stdout: process.stdout,
                skills,
                onSkillsChanged: declareTools,
                loadPlugin,
              })
            : await runCoderPlain(session, {
                stdin: process.stdin,
                stdout: process.stdout,
                prompt: oneShot,
                skills,
                loadPlugin,
              });
        } finally {
          // An account holds eight open threads at once. A terminal that closed
          // without giving its slot back would hold one until the authority
          // expired an hour later, which is the ninth session refused for a
          // session nobody is in. A process killed outright still leaves it to
          // the server's expiry reap.
          // Flush before revoking: revoking makes the thread terminal, and a
          // terminal thread refuses the events that are still queued.
          if (transcript !== undefined) await transcript.close();
          if (thread !== undefined) await thread.revoke();
          if (childThread?.kind === "opened") await childThread.thread.revoke();
          if (setup !== undefined) await setup.close();
        }
      });

      // The interface asks for a restart by exiting with a code of its own. It
      // is done here rather than there because the rebuild has to happen after
      // the screen is given back: a compiler writing over a live alt-screen is
      // a session the reader cannot read.
      if (code === RELOAD_EXIT_CODE) {
        const root = sourceCheckout();
        if (root !== undefined) {
          const built = rebuild(root);
          if (built.ok) {
            const restarted = spawnSync(process.execPath, process.argv.slice(1), {
              stdio: "inherit",
            });
            process.exitCode = restarted.status ?? 0;
            return;
          }
          // The compiler's own words, and the old session is not resumed: a
          // reload onto code that does not build would be a reload onto the
          // build before it, silently.
          process.stderr.write(`${built.output}\n\nReload failed: the build did not pass.\n`);
          process.exitCode = 1;
          return;
        }
      }

      if (code !== 0) {
        process.exitCode = code;
      }
    }),
).pipe(
  Command.withDescription(
    "Open a terminal coding session on a thread of its own, or continue one with --resume. Replies come from the thread's grant through the inference proxy, so nothing typed here reaches /chat; --offline answers from a built-in stand-in instead. The session can delegate: ask it to split work and it runs child coding agents on a thread of their own pinned to Ox Alpha, or launch a fan-out yourself with `/delegate [<n>x] <prompt>`, and the interface shows the fleet",
  ),
);

const delegatePrompt = Argument.string("prompt").pipe(
  Argument.withDescription("The task every child performs"),
);
const delegateAgentsFlag = Flag.integer("agents").pipe(
  Flag.withDefault(1),
  Flag.withDescription("How many children run this prompt"),
);
const delegateDirFlag = Flag.string("dir").pipe(
  Flag.optional,
  Flag.withDescription("Where children work. Defaults to the current directory"),
);
const delegateDescriptionFlag = Flag.string("description").pipe(
  Flag.optional,
  Flag.withDescription("Three to five words naming the task. Defaults to the start of the prompt"),
);

/**
 * Run a fleet of children from the shell and report every one of them.
 *
 * This is the headless half of delegation, and it exists for the same reason
 * `--plain` does: a fan-out has to be runnable where there is no terminal to
 * draw into — a script, a CI job, another agent. It prints transitions as they
 * happen rather than a repainted table, because a log that overwrites itself is
 * unreadable once it is a file.
 */
const delegateCommand = Command.make(
  "delegate",
  {
    prompt: delegatePrompt,
    agents: delegateAgentsFlag,
    dir: delegateDirFlag,
    description: delegateDescriptionFlag,
    childModel: childModelFlag,
    childCommand: childCommandFlag,
    childConfig: childConfigFlag,
    childAsk: childAskFlag,
    concurrency: concurrencyFlag,
  },
  ({
    prompt,
    agents,
    dir,
    description,
    childModel,
    childCommand,
    childConfig,
    childAsk,
    concurrency,
  }) =>
    Effect.gen(function* () {
      const flags = yield* rootCommand;
      const cwd = Option.getOrUndefined(dir) ?? process.cwd();
      const endpoint = yield* resolveApiEndpoint(endpointOverrides(flags));
      const named = Option.getOrUndefined(childModel) ?? process.env["OPENAGENTS_DELEGATE_MODEL"];

      // Children spend a thread the same way the interactive session does, so
      // this command opens one rather than demanding a provider of its own. It
      // is skipped when a child model was named, because then the reader is
      // paying somebody else and a thread would be opened and never spent.
      const stored =
        named === undefined
          ? yield* findToken(endpoint.origin).pipe(
              Effect.catchTag("OpenAgentsCli.CredentialPersistenceUnavailable", () =>
                Effect.succeed(Option.none()),
              ),
            )
          : Option.none();

      // Every turn on this thread is a child's, so it is opened on the child
      // model directly rather than opening one thread to hold and another to
      // spend.
      const thread = Option.isSome(stored)
        ? yield* Effect.tryPromise({
            try: () =>
              openThread({
                origin: endpoint.origin,
                token: Redacted.value(stored.value.token),
                objective: `openagents delegate: ${describePrompt(prompt)}`,
                model: process.env["OPENAGENTS_DELEGATE_THREAD_MODEL"] ?? CHILD_THREAD_MODEL,
              }),
            catch: (cause) => coderRefusal(endpoint.origin, cause),
          })
        : undefined;

      const setup = yield* Effect.promise(() =>
        buildDelegation({
          model: Option.getOrUndefined(childModel),
          command: Option.getOrUndefined(childCommand),
          configPath: Option.getOrUndefined(childConfig),
          autoApprove: !childAsk,
          concurrency,
          cwd,
          grant: thread?.childGrant,
        }),
      );

      if (setup === undefined) {
        return yield* new InputError({
          message:
            "Nothing to run children on. Sign in with `openagents auth login` so " +
            "children can spend a thread, or pass --child-model provider/model to run " +
            "them on a provider of your own.",
        });
      }
      const delegation = setup.delegation;

      const count = Math.max(1, agents);
      const label = Option.getOrUndefined(description) ?? describePrompt(prompt);
      const registry = delegation.registry;

      const outcomes = yield* Effect.promise(async () => {
        // Transitions only. A child that is working reports through its task,
        // and reprinting every progress update would bury the four lines that
        // say what happened.
        const seen = new Map<string, string>();
        const unsubscribe = flags.json
          ? () => {}
          : registry.onChange(() => {
              for (const task of registry.list()) {
                if (seen.get(task.id) === task.status) continue;
                seen.set(task.id, task.status);
                process.stderr.write(`${task.id} ${task.status} · ${task.description}\n`);
              }
            });

        // Ctrl+C has to reach the children. Without this the command exits and
        // leaves every child agent running, spending, with nothing left
        // holding a handle on them. The handler is prepended and repeated on
        // exit because the runtime installs a signal handler of its own that
        // tears the process down, and whichever ends the process first must not
        // be the one that skips the children.
        const onSignal = () => {
          registry.stopAll();
        };
        process.prependListener("SIGINT", onSignal);
        process.prependListener("SIGTERM", onSignal);
        process.prependListener("exit", onSignal);

        try {
          return await Promise.all(
            Array.from({ length: count }, () =>
              delegation.fleet.submit({ description: label, prompt, cwd, background: false }),
            ),
          );
        } finally {
          process.off("SIGINT", onSignal);
          process.off("SIGTERM", onSignal);
          process.off("exit", onSignal);
          unsubscribe();
          await setup.close();
          // The thread's slot goes back even on the failure path: an account
          // holds eight, and a script that delegates in a loop would otherwise
          // be refused on its ninth run.
          if (thread !== undefined) await thread.revoke();
        }
      });

      if (flags.json) {
        return yield* Console.log(
          JSON.stringify(
            {
              agent: delegation.label,
              cwd,
              tasks: registry.list(),
              outcomes,
            },
            null,
            2,
          ),
        );
      }

      for (const line of fleetPlainLines(registry.list(), 100)) {
        yield* Console.log(line);
      }
      for (const outcome of outcomes) {
        yield* Console.log("");
        yield* Console.log(describeOutcome(outcome));
      }
      // A fleet where a child failed did not succeed, and a script that reads
      // only the exit code has to be told.
      if (outcomes.some((outcome) => outcome.status !== "completed")) {
        process.exitCode = 1;
      }
    }),
).pipe(
  Command.withDescription(
    "Run one prompt on many child coding agents at once and report each result. Children run on a thread of their own by default, so this needs no provider credential",
  ),
);

function describeOutcome(outcome: DelegationOutcome): string {
  if (outcome.status === "completed") return `${outcome.taskId} completed:\n${outcome.result}`;
  if (outcome.status === "failed") return `${outcome.taskId} failed: ${outcome.error}`;
  if (outcome.status === "stopped") return `${outcome.taskId} stopped.`;
  return `refused (${outcome.code}): ${outcome.reason}`;
}

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

// The issue and project command groups. Both read the same repository
// inference the `repo` group uses, so a caller inside a checkout names an
// issue by its number alone.

const issueNumberArgument = Argument.string("number").pipe(
  Argument.withDescription("Issue number, with or without a leading #"),
);
const projectNumberArgument = Argument.string("number").pipe(
  Argument.withDescription("Project number, with or without a leading #"),
);
const projectItemArgument = Argument.string("item").pipe(
  Argument.withDescription("Project item id"),
);

const parseTrackerNumber = Effect.fn("Cli.parseTrackerNumber")(function* (
  label: string,
  value: string,
) {
  const trimmed = value.trim().replace(/^#/u, "");
  if (!/^\d+$/u.test(trimmed) || Number.parseInt(trimmed, 10) < 1) {
    return yield* new InputError({
      message: `${label} must be a positive number, such as 129 or #129.`,
    });
  }
  return Number.parseInt(trimmed, 10);
});

const parseTrackerNumbers = Effect.fn("Cli.parseTrackerNumbers")(function* (
  label: string,
  values: ReadonlyArray<string>,
) {
  const parsed: Array<number> = [];
  for (const value of values) parsed.push(yield* parseTrackerNumber(label, value));
  return parsed;
});

const bodyFlag = Flag.string("body").pipe(Flag.optional, Flag.withDescription("Body text"));
const bodyFileFlag = Flag.string("body-file").pipe(
  Flag.optional,
  Flag.withDescription("Read the body from a file, or from - for standard input"),
);

const resolveBodyText = Effect.fn("Cli.resolveBodyText")(function* (
  body: Option.Option<string>,
  bodyFile: Option.Option<string>,
) {
  if (Option.isSome(body) && Option.isSome(bodyFile)) {
    return yield* new InputError({ message: "Use either --body or --body-file, not both." });
  }
  if (Option.isSome(body)) return Option.some(body.value);
  if (Option.isNone(bodyFile)) return Option.none<string>();
  const bodyInput = yield* RequestBodyInput;
  return Option.some(yield* bodyInput.read(bodyFile.value));
});

const resolveTrackerTarget = (repo: Option.Option<string>, origin: string) =>
  resolveRepositoryArgument(Option.none<string>(), repo, origin);

const names = (value: unknown, key: string): ReadonlyArray<string> =>
  Array.isArray(value)
    ? value.map((entry) => (typeof entry === "string" ? entry : String(record(entry)[key] ?? "")))
    : [];

const orNone = (values: ReadonlyArray<string>): string =>
  values.length === 0 ? "none" : values.join(", ");

const issueReferences = (value: unknown): ReadonlyArray<string> =>
  Array.isArray(value) ? value.map((entry) => `#${String(record(entry)["number"] ?? "?")}`) : [];

const issueRow = (issue: Record<string, unknown>): string => {
  const extension = record(issue["openagents"]);
  const labels = names(issue["labels"], "name");
  return [
    `#${String(issue["number"] ?? "?")}`.padEnd(7),
    String(issue["state"] ?? "").padEnd(8),
    String(issue["title"] ?? ""),
    labels.length === 0 ? "" : `  (${labels.join(", ")})`,
    extension["blocked"] === true ? "  [blocked]" : "",
  ].join("");
};

const issueListHuman = (
  issues: ReadonlyArray<Record<string, unknown>>,
  pagination: Record<string, unknown>,
): ReadonlyArray<string> => {
  if (issues.length === 0) return ["No issues found."];
  const total = pagination["total"];
  return [
    ...issues.map(issueRow),
    "",
    typeof total === "number"
      ? `Showing ${issues.length} of ${total} issues.`
      : `Showing ${issues.length} issues.`,
  ];
};

const issueViewHuman = (value: unknown): ReadonlyArray<string> => {
  const issue = record(value);
  const extension = record(issue["openagents"]);
  const milestone = record(issue["milestone"]);
  const body = typeof issue["body"] === "string" ? issue["body"] : "";
  return [
    `#${String(issue["number"] ?? "?")}  ${String(issue["title"] ?? "")}`,
    `State:      ${String(issue["state"] ?? "")}`,
    `Author:     ${String(record(issue["user"])["login"] ?? "unknown")}`,
    `Labels:     ${orNone(names(issue["labels"], "name"))}`,
    `Assignees:  ${orNone(names(issue["assignees"], "login"))}`,
    `Milestone:  ${milestone["title"] === undefined ? "none" : String(milestone["title"])}`,
    `Progress:   ${String(extension["progress"] ?? "unknown")}`,
    `Blocked:    ${extension["blocked"] === true ? "yes" : "no"}`,
    `Blocked by: ${orNone(issueReferences(extension["blocked_by"]))}`,
    `Blocks:     ${orNone(issueReferences(extension["blocks"]))}`,
    "",
    body,
  ];
};

const commentThreadHuman = (value: unknown): ReadonlyArray<string> => {
  const comments = rows(value, "comments");
  if (comments.length === 0) return ["", "No comments."];
  return [
    "",
    `Comments (${comments.length}):`,
    ...comments.map(
      (comment) =>
        `- ${String(record(comment["user"])["login"] ?? "unknown")}: ${String(comment["body"] ?? "")}`,
    ),
  ];
};

const dependencyHuman = (value: unknown): ReadonlyArray<string> => {
  const graph = record(value);
  const edges = (key: string) =>
    Array.isArray(graph[key])
      ? graph[key].map(
          (entry) =>
            `  #${String(record(entry)["number"] ?? "?")} ${String(record(entry)["state"] ?? "")} ${String(record(entry)["title"] ?? "")}`,
        )
      : [];
  const blockedBy = edges("blocked_by");
  const blocks = edges("blocks");
  return [
    `Blocked: ${graph["blocked"] === true ? "yes" : "no"}`,
    "Blocked by:",
    ...(blockedBy.length === 0 ? ["  none"] : blockedBy),
    "Blocks:",
    ...(blocks.length === 0 ? ["  none"] : blocks),
  ];
};

const issueListStateFlag = Flag.choice("state", ["open", "closed", "all"] as const).pipe(
  Flag.withDefault("open" as const),
  Flag.withDescription("Filter by state"),
);
const issueListLabelFlag = Flag.string("label").pipe(
  Flag.optional,
  Flag.withDescription("Filter by one label name"),
);
const issueListAssigneeFlag = Flag.string("assignee").pipe(
  Flag.optional,
  Flag.withDescription("Filter by assignee login"),
);
const issueListMilestoneFlag = Flag.string("milestone").pipe(
  Flag.optional,
  Flag.withDescription("Filter by milestone number"),
);
const issueListSearchFlag = Flag.string("search").pipe(
  Flag.optional,
  Flag.withDescription("Match a substring of the title or body"),
);
const issueListBlockedFlag = Flag.choice("blocked", ["true", "false"] as const).pipe(
  Flag.optional,
  Flag.withDescription("Keep only blocked or only unblocked issues"),
);
const issueListLimitFlag = Flag.integer("limit").pipe(
  Flag.withDefault(25),
  Flag.withDescription("Read this many issues, paging past the server's 25 to a page"),
);

const issueListCommand = Command.make(
  "list",
  {
    repo: repositoryOverrideFlag,
    state: issueListStateFlag,
    label: issueListLabelFlag,
    assignee: issueListAssigneeFlag,
    milestone: issueListMilestoneFlag,
    search: issueListSearchFlag,
    blocked: issueListBlockedFlag,
    limit: issueListLimitFlag,
  },
  ({ assignee, blocked, label, limit, milestone, repo, search, state }) =>
    Effect.gen(function* () {
      const flags = yield* rootCommand;
      const session = yield* resolveApiSession(endpointOverrides(flags));
      const target = yield* resolveTrackerTarget(repo, session.endpoint.origin);
      const issues = yield* IssueClient;
      const output = yield* Output;
      const result = yield* issues.list({
        origin: session.endpoint.origin,
        token: session.token,
        ...target,
        limit,
        state,
        ...(Option.isNone(label) ? {} : { label: label.value }),
        ...(Option.isNone(assignee) ? {} : { assignee: assignee.value }),
        ...(Option.isNone(milestone) ? {} : { milestone: milestone.value }),
        ...(Option.isNone(search) ? {} : { search: search.value }),
        ...(Option.isNone(blocked) ? {} : { blocked: blocked.value === "true" }),
      });
      yield* output.write(
        {
          value: { pagination: result.pagination, issues: result.issues },
          human: issueListHuman(result.issues.map(record), result.pagination),
        },
        outputMode(flags.json),
      );
    }),
).pipe(Command.withDescription("List issues, paging until --limit is met"));

const issueCommentsFlag = Flag.boolean("comments").pipe(
  Flag.withDescription("Include the comment thread"),
);

const issueViewCommand = Command.make(
  "view",
  { number: issueNumberArgument, repo: repositoryOverrideFlag, comments: issueCommentsFlag },
  ({ comments, number, repo }) =>
    Effect.gen(function* () {
      const issueNumber = yield* parseTrackerNumber("An issue number", number);
      const flags = yield* rootCommand;
      const session = yield* resolveApiSession(endpointOverrides(flags));
      const target = yield* resolveTrackerTarget(repo, session.endpoint.origin);
      const issues = yield* IssueClient;
      const output = yield* Output;
      const scope = {
        origin: session.endpoint.origin,
        token: session.token,
        ...target,
        number: issueNumber,
      };
      const value = yield* issues.view(scope);
      if (!comments) {
        yield* output.write({ value, human: issueViewHuman(value) }, outputMode(flags.json));
        return;
      }
      const thread = yield* issues.comments(scope);
      yield* output.write(
        {
          value: { issue: value, comments: rows(thread, "comments") },
          human: [...issueViewHuman(value), ...commentThreadHuman(thread)],
        },
        outputMode(flags.json),
      );
    }),
).pipe(Command.withDescription("Show one issue, its prerequisites, and its body"));

const issueTitleFlag = Flag.string("title").pipe(Flag.withDescription("Issue title"));
const issueCreateLabelFlag = Flag.string("label").pipe(
  Flag.atLeast(0),
  Flag.withDescription("Apply an existing label; repeatable"),
);
const issueCreateAssigneeFlag = Flag.string("assignee").pipe(
  Flag.atLeast(0),
  Flag.withDescription("Assign a login; repeatable"),
);
const issueCreateMilestoneFlag = Flag.integer("milestone").pipe(
  Flag.optional,
  Flag.withDescription("Milestone number"),
);

const issueCreateCommand = Command.make(
  "create",
  {
    repo: repositoryOverrideFlag,
    title: issueTitleFlag,
    body: bodyFlag,
    bodyFile: bodyFileFlag,
    label: issueCreateLabelFlag,
    assignee: issueCreateAssigneeFlag,
    milestone: issueCreateMilestoneFlag,
  },
  ({ assignee, body, bodyFile, label, milestone, repo, title }) =>
    Effect.gen(function* () {
      if (title.trim() === "") {
        return yield* new InputError({ message: "Pass --title with the issue title." });
      }
      const text = yield* resolveBodyText(body, bodyFile);
      const flags = yield* rootCommand;
      const session = yield* resolveApiSession(endpointOverrides(flags));
      const target = yield* resolveTrackerTarget(repo, session.endpoint.origin);
      const issues = yield* IssueClient;
      const output = yield* Output;
      const value = yield* issues.create({
        origin: session.endpoint.origin,
        token: session.token,
        ...target,
        title,
        ...(Option.isNone(text) ? {} : { body: text.value }),
        ...(label.length === 0 ? {} : { labels: label }),
        ...(assignee.length === 0 ? {} : { assignees: assignee }),
        ...(Option.isNone(milestone) ? {} : { milestone: milestone.value }),
      });
      const created = record(value);
      yield* output.write(
        {
          value,
          human: [
            `Created #${String(created["number"] ?? "?")} ${String(created["title"] ?? "")}`,
            String(created["html_url"] ?? ""),
          ],
        },
        outputMode(flags.json),
      );
    }),
).pipe(Command.withDescription("Open an issue; --body-file - reads standard input"));

const issueStateCommentFlag = Flag.string("comment").pipe(
  Flag.optional,
  Flag.withDescription("Post this comment before the state change"),
);

const issueStateCommand = (
  name: "close" | "reopen",
  state: "closed" | "open",
  description: string,
) =>
  Command.make(
    name,
    { number: issueNumberArgument, repo: repositoryOverrideFlag, comment: issueStateCommentFlag },
    ({ comment, number, repo }) =>
      Effect.gen(function* () {
        const issueNumber = yield* parseTrackerNumber("An issue number", number);
        const flags = yield* rootCommand;
        const session = yield* resolveApiSession(endpointOverrides(flags));
        const target = yield* resolveTrackerTarget(repo, session.endpoint.origin);
        const issues = yield* IssueClient;
        const output = yield* Output;
        const scope = {
          origin: session.endpoint.origin,
          token: session.token,
          ...target,
          number: issueNumber,
        };
        // The comment is its own request. A state change that carried the
        // issue text would overwrite the body it was never given.
        const posted = Option.isNone(comment)
          ? undefined
          : yield* issues.comment({ ...scope, body: comment.value });
        const value = yield* issues.setState({ ...scope, state });
        yield* output.write(
          {
            value: posted === undefined ? value : { issue: value, comment: posted },
            human: [
              `${state === "closed" ? "Closed" : "Reopened"} #${issueNumber}.`,
              ...(posted === undefined ? [] : ["Comment posted."]),
            ],
          },
          outputMode(flags.json),
        );
      }),
  ).pipe(Command.withDescription(description));

const issueCloseCommand = issueStateCommand(
  "close",
  "closed",
  "Close an issue, optionally with a comment that says why",
);
const issueReopenCommand = issueStateCommand(
  "reopen",
  "open",
  "Reopen an issue, optionally with a comment that says why",
);

const issueCommentCommand = Command.make(
  "comment",
  {
    number: issueNumberArgument,
    repo: repositoryOverrideFlag,
    body: bodyFlag,
    bodyFile: bodyFileFlag,
  },
  ({ body, bodyFile, number, repo }) =>
    Effect.gen(function* () {
      const issueNumber = yield* parseTrackerNumber("An issue number", number);
      const text = yield* resolveBodyText(body, bodyFile);
      if (Option.isNone(text)) {
        return yield* new InputError({ message: "Pass --body or --body-file with the comment." });
      }
      const flags = yield* rootCommand;
      const session = yield* resolveApiSession(endpointOverrides(flags));
      const target = yield* resolveTrackerTarget(repo, session.endpoint.origin);
      const issues = yield* IssueClient;
      const output = yield* Output;
      const value = yield* issues.comment({
        origin: session.endpoint.origin,
        token: session.token,
        ...target,
        number: issueNumber,
        body: text.value,
      });
      yield* output.write(
        { value, human: [`Commented on #${issueNumber}.`] },
        outputMode(flags.json),
      );
    }),
).pipe(Command.withDescription("Comment on an issue"));

const labelAddFlag = Flag.string("add").pipe(
  Flag.atLeast(0),
  Flag.withDescription("Apply a label; repeatable"),
);
const labelRemoveFlag = Flag.string("remove").pipe(
  Flag.atLeast(0),
  Flag.withDescription("Remove a label; repeatable"),
);

const issueLabelCommand = Command.make(
  "label",
  {
    number: issueNumberArgument,
    repo: repositoryOverrideFlag,
    add: labelAddFlag,
    remove: labelRemoveFlag,
  },
  ({ add, number, remove, repo }) =>
    Effect.gen(function* () {
      const issueNumber = yield* parseTrackerNumber("An issue number", number);
      const flags = yield* rootCommand;
      const session = yield* resolveApiSession(endpointOverrides(flags));
      const target = yield* resolveTrackerTarget(repo, session.endpoint.origin);
      const issues = yield* IssueClient;
      const output = yield* Output;
      const scope = {
        origin: session.endpoint.origin,
        token: session.token,
        ...target,
        number: issueNumber,
      };
      let value =
        add.length === 0 && remove.length === 0
          ? yield* issues.labels(scope)
          : add.length === 0
            ? undefined
            : yield* issues.addLabels({ ...scope, labels: add });
      for (const name of remove) {
        value = yield* issues.removeLabel({ ...scope, label: name });
      }
      const applied = value ?? (yield* issues.labels(scope));
      yield* output.write(
        { value: applied, human: [`Labels: ${orNone(names(record(applied)["labels"], "name"))}`] },
        outputMode(flags.json),
      );
    }),
).pipe(Command.withDescription("Read, apply, or remove the labels on an issue"));

const assigneeArguments = Argument.string("login").pipe(
  Argument.withDescription("Account login"),
  Argument.variadic({ min: 1 }),
);

const issueAssignCommand = (name: "assign" | "unassign", description: string) =>
  Command.make(
    name,
    { number: issueNumberArgument, logins: assigneeArguments, repo: repositoryOverrideFlag },
    ({ logins, number, repo }) =>
      Effect.gen(function* () {
        const issueNumber = yield* parseTrackerNumber("An issue number", number);
        const flags = yield* rootCommand;
        const session = yield* resolveApiSession(endpointOverrides(flags));
        const target = yield* resolveTrackerTarget(repo, session.endpoint.origin);
        const issues = yield* IssueClient;
        const output = yield* Output;
        const scope = {
          origin: session.endpoint.origin,
          token: session.token,
          ...target,
          number: issueNumber,
          assignees: logins,
        };
        const value =
          name === "assign"
            ? yield* issues.addAssignees(scope)
            : yield* issues.removeAssignees(scope);
        yield* output.write(
          {
            value,
            human: [`Assignees: ${orNone(names(record(value)["assignees"], "login"))}`],
          },
          outputMode(flags.json),
        );
      }),
  ).pipe(Command.withDescription(description));

const issueAssignRunCommand = issueAssignCommand("assign", "Assign an issue to one or more logins");
const issueUnassignRunCommand = issueAssignCommand(
  "unassign",
  "Remove one or more logins from an issue",
);

const dependencyAddFlag = Flag.string("add").pipe(
  Flag.atLeast(0),
  Flag.withDescription("Record an issue this one waits on; repeatable"),
);
const dependencyRemoveFlag = Flag.string("remove").pipe(
  Flag.atLeast(0),
  Flag.withDescription("Drop a prerequisite edge; repeatable"),
);

const issueDepsCommand = Command.make(
  "deps",
  {
    number: issueNumberArgument,
    repo: repositoryOverrideFlag,
    add: dependencyAddFlag,
    remove: dependencyRemoveFlag,
  },
  ({ add, number, remove, repo }) =>
    Effect.gen(function* () {
      const issueNumber = yield* parseTrackerNumber("An issue number", number);
      const additions = yield* parseTrackerNumbers("A prerequisite", add);
      const removals = yield* parseTrackerNumbers("A prerequisite", remove);
      const flags = yield* rootCommand;
      const session = yield* resolveApiSession(endpointOverrides(flags));
      const target = yield* resolveTrackerTarget(repo, session.endpoint.origin);
      const issues = yield* IssueClient;
      const output = yield* Output;
      const scope = {
        origin: session.endpoint.origin,
        token: session.token,
        ...target,
        number: issueNumber,
      };
      let value =
        additions.length === 0
          ? undefined
          : yield* issues.addDependencies({ ...scope, blockedBy: additions });
      for (const blockedBy of removals) {
        value = yield* issues.removeDependency({ ...scope, blockedBy });
      }
      const graph = value ?? (yield* issues.dependencies(scope));
      yield* output.write({ value: graph, human: dependencyHuman(graph) }, outputMode(flags.json));
    }),
).pipe(Command.withDescription("Read, add, or remove the prerequisites of an issue"));

const issueCommand = Command.make("issue").pipe(
  Command.withDescription("Read and write issues"),
  Command.withSubcommands([
    issueListCommand,
    issueViewCommand,
    issueCreateCommand,
    issueCloseCommand,
    issueReopenCommand,
    issueCommentCommand,
    issueLabelCommand,
    issueAssignRunCommand,
    issueUnassignRunCommand,
    issueDepsCommand,
  ]),
);

const projectArchivedFlag = Flag.boolean("archived").pipe(
  Flag.withDescription("Include archived boards"),
);

const projectRow = (project: Record<string, unknown>): string =>
  [
    `#${String(project["number"] ?? "?")}`.padEnd(6),
    String(project["state"] ?? "").padEnd(8),
    String(project["title"] ?? ""),
    project["archived"] === true ? "  [archived]" : "",
  ].join("");

const projectListCommand = Command.make(
  "list",
  { repo: repositoryOverrideFlag, archived: projectArchivedFlag },
  ({ archived, repo }) =>
    Effect.gen(function* () {
      const flags = yield* rootCommand;
      const session = yield* resolveApiSession(endpointOverrides(flags));
      const target = yield* resolveTrackerTarget(repo, session.endpoint.origin);
      const projects = yield* ProjectClient;
      const output = yield* Output;
      const value = yield* projects.list({
        origin: session.endpoint.origin,
        token: session.token,
        ...target,
        archived,
      });
      const boards = rows(value, "projects");
      yield* output.write(
        { value, human: boards.length === 0 ? ["No projects found."] : boards.map(projectRow) },
        outputMode(flags.json),
      );
    }),
).pipe(Command.withDescription("List the projects of a repository"));

const projectViewCommand = Command.make(
  "view",
  { number: projectNumberArgument, repo: repositoryOverrideFlag },
  ({ number, repo }) =>
    Effect.gen(function* () {
      const projectNumber = yield* parseTrackerNumber("A project number", number);
      const flags = yield* rootCommand;
      const session = yield* resolveApiSession(endpointOverrides(flags));
      const target = yield* resolveTrackerTarget(repo, session.endpoint.origin);
      const projects = yield* ProjectClient;
      const output = yield* Output;
      const value = yield* projects.view({
        origin: session.endpoint.origin,
        token: session.token,
        ...target,
        number: projectNumber,
      });
      const project = record(value);
      yield* output.write(
        {
          value,
          human: [
            `#${String(project["number"] ?? "?")}  ${String(project["title"] ?? "")}`,
            `State:    ${String(project["state"] ?? "")}`,
            `Archived: ${project["archived"] === true ? "yes" : "no"}`,
            `Owner:    ${String(project["owner"] ?? "unknown")}`,
            "",
            typeof project["description"] === "string" ? project["description"] : "",
          ],
        },
        outputMode(flags.json),
      );
    }),
).pipe(Command.withDescription("Show one project"));

const projectTitleFlag = Flag.string("title").pipe(Flag.withDescription("Project title"));
const projectDescriptionFlag = Flag.string("description").pipe(
  Flag.optional,
  Flag.withDescription("Markdown project description"),
);

const projectCreateCommand = Command.make(
  "create",
  { repo: repositoryOverrideFlag, title: projectTitleFlag, description: projectDescriptionFlag },
  ({ description, repo, title }) =>
    Effect.gen(function* () {
      if (title.trim() === "") {
        return yield* new InputError({ message: "Pass --title with the project title." });
      }
      const flags = yield* rootCommand;
      const session = yield* resolveApiSession(endpointOverrides(flags));
      const target = yield* resolveTrackerTarget(repo, session.endpoint.origin);
      const projects = yield* ProjectClient;
      const output = yield* Output;
      const value = yield* projects.create({
        origin: session.endpoint.origin,
        token: session.token,
        ...target,
        title,
        ...(Option.isNone(description) ? {} : { description: description.value }),
      });
      const project = record(value);
      yield* output.write(
        {
          value,
          human: [
            `Created project #${String(project["number"] ?? "?")} ${String(project["title"] ?? "")}`,
          ],
        },
        outputMode(flags.json),
      );
    }),
).pipe(Command.withDescription("Create a project board"));

const projectItemRow = (item: Record<string, unknown>): string => {
  const issue = record(item["issue"]);
  const values = record(item["values"]);
  const pairs = Object.entries(values).map(([field, value]) => `${field}=${String(value)}`);
  return `${String(item["id"] ?? "?").padEnd(6)} #${String(issue["number"] ?? "?")}  ${pairs.join(" ")}`;
};

const projectItemsHuman = (value: unknown): ReadonlyArray<string> => {
  const items = rows(value, "items");
  return items.length === 0 ? ["No items on this board."] : items.map(projectItemRow);
};

const projectItemsCommand = Command.make(
  "items",
  { number: projectNumberArgument, repo: repositoryOverrideFlag },
  ({ number, repo }) =>
    Effect.gen(function* () {
      const projectNumber = yield* parseTrackerNumber("A project number", number);
      const flags = yield* rootCommand;
      const session = yield* resolveApiSession(endpointOverrides(flags));
      const target = yield* resolveTrackerTarget(repo, session.endpoint.origin);
      const projects = yield* ProjectClient;
      const output = yield* Output;
      const value = yield* projects.items({
        origin: session.endpoint.origin,
        token: session.token,
        ...target,
        number: projectNumber,
      });
      yield* output.write({ value, human: projectItemsHuman(value) }, outputMode(flags.json));
    }),
).pipe(Command.withDescription("List the items on a project board"));

const projectFieldsCommand = Command.make(
  "fields",
  { number: projectNumberArgument, repo: repositoryOverrideFlag },
  ({ number, repo }) =>
    Effect.gen(function* () {
      const projectNumber = yield* parseTrackerNumber("A project number", number);
      const flags = yield* rootCommand;
      const session = yield* resolveApiSession(endpointOverrides(flags));
      const target = yield* resolveTrackerTarget(repo, session.endpoint.origin);
      const projects = yield* ProjectClient;
      const output = yield* Output;
      const value = yield* projects.fields({
        origin: session.endpoint.origin,
        token: session.token,
        ...target,
        number: projectNumber,
      });
      const fields = rows(value, "fields");
      yield* output.write(
        {
          value,
          human:
            fields.length === 0
              ? ["No fields on this board."]
              : fields.map(
                  (field) =>
                    `${String(field["name"] ?? "")} (${String(field["data_type"] ?? "")}) ${orNone(names(record(field["options"])["values"], "name"))}`,
                ),
        },
        outputMode(flags.json),
      );
    }),
).pipe(Command.withDescription("List the fields of a project board"));

const projectIssueFlag = Flag.string("issue").pipe(
  Flag.withDescription("Issue number to place on the board"),
);

const projectItemAddCommand = Command.make(
  "item-add",
  { number: projectNumberArgument, repo: repositoryOverrideFlag, issue: projectIssueFlag },
  ({ issue, number, repo }) =>
    Effect.gen(function* () {
      const projectNumber = yield* parseTrackerNumber("A project number", number);
      const issueNumber = yield* parseTrackerNumber("An issue number", issue);
      const flags = yield* rootCommand;
      const session = yield* resolveApiSession(endpointOverrides(flags));
      const target = yield* resolveTrackerTarget(repo, session.endpoint.origin);
      const projects = yield* ProjectClient;
      const output = yield* Output;
      const value = yield* projects.addItem({
        origin: session.endpoint.origin,
        token: session.token,
        ...target,
        number: projectNumber,
        issueNumber,
      });
      yield* output.write({ value, human: projectItemsHuman(value) }, outputMode(flags.json));
    }),
).pipe(Command.withDescription("Put an issue on a project board"));

const projectValueFlag = Flag.keyValuePair("set").pipe(
  Flag.optional,
  Flag.withDescription("Set a field, as FIELD=VALUE; repeatable"),
);
const projectPositionFlag = Flag.integer("position").pipe(
  Flag.optional,
  Flag.withDescription("One-based rank within the destination column"),
);

const projectItemSetCommand = Command.make(
  "item-set",
  {
    number: projectNumberArgument,
    item: projectItemArgument,
    repo: repositoryOverrideFlag,
    set: projectValueFlag,
  },
  ({ item, number, repo, set }) =>
    Effect.gen(function* () {
      if (Option.isNone(set)) {
        return yield* new InputError({ message: "Pass --set FIELD=VALUE with the field to set." });
      }
      const projectNumber = yield* parseTrackerNumber("A project number", number);
      const flags = yield* rootCommand;
      const session = yield* resolveApiSession(endpointOverrides(flags));
      const target = yield* resolveTrackerTarget(repo, session.endpoint.origin);
      const projects = yield* ProjectClient;
      const output = yield* Output;
      const value = yield* projects.setItemValues({
        origin: session.endpoint.origin,
        token: session.token,
        ...target,
        number: projectNumber,
        itemId: item,
        values: set.value,
      });
      yield* output.write({ value, human: projectItemsHuman(value) }, outputMode(flags.json));
    }),
).pipe(Command.withDescription("Set stored field values on a project item"));

const projectItemMoveCommand = Command.make(
  "item-move",
  {
    number: projectNumberArgument,
    item: projectItemArgument,
    repo: repositoryOverrideFlag,
    set: projectValueFlag,
    position: projectPositionFlag,
  },
  ({ item, number, position, repo, set }) =>
    Effect.gen(function* () {
      if (Option.isNone(set) && Option.isNone(position)) {
        return yield* new InputError({
          message: "Pass --set FIELD=VALUE, --position, or both.",
        });
      }
      const projectNumber = yield* parseTrackerNumber("A project number", number);
      const flags = yield* rootCommand;
      const session = yield* resolveApiSession(endpointOverrides(flags));
      const target = yield* resolveTrackerTarget(repo, session.endpoint.origin);
      const projects = yield* ProjectClient;
      const output = yield* Output;
      const value = yield* projects.moveItem({
        origin: session.endpoint.origin,
        token: session.token,
        ...target,
        number: projectNumber,
        itemId: item,
        values: Option.isNone(set) ? {} : set.value,
        ...(Option.isNone(position) ? {} : { position: position.value }),
      });
      yield* output.write({ value, human: projectItemsHuman(value) }, outputMode(flags.json));
    }),
).pipe(Command.withDescription("Move a project item to another column or rank"));

const projectItemRemoveCommand = Command.make(
  "item-remove",
  { number: projectNumberArgument, item: projectItemArgument, repo: repositoryOverrideFlag },
  ({ item, number, repo }) =>
    Effect.gen(function* () {
      const projectNumber = yield* parseTrackerNumber("A project number", number);
      const flags = yield* rootCommand;
      const session = yield* resolveApiSession(endpointOverrides(flags));
      const target = yield* resolveTrackerTarget(repo, session.endpoint.origin);
      const projects = yield* ProjectClient;
      const output = yield* Output;
      yield* projects.removeItem({
        origin: session.endpoint.origin,
        token: session.token,
        ...target,
        number: projectNumber,
        itemId: item,
      });
      yield* output.write(
        { value: { item_id: item, removed: true }, human: [`Removed item ${item}.`] },
        outputMode(flags.json),
      );
    }),
).pipe(Command.withDescription("Take an item off a project board"));

const projectCommand = Command.make("project").pipe(
  Command.withDescription("Read and write project boards"),
  Command.withSubcommands([
    projectListCommand,
    projectViewCommand,
    projectCreateCommand,
    projectFieldsCommand,
    projectItemsCommand,
    projectItemAddCommand,
    projectItemSetCommand,
    projectItemMoveCommand,
    projectItemRemoveCommand,
  ]),
);

export const openagentsCommand = rootCommand.pipe(
  Command.withSubcommands([
    apiCommand,
    authCommand,
    coderCommand,
    delegateCommand,
    computerCommand,
    forumCommand,
    issueCommand,
    projectCommand,
    repoCommand,
  ]),
);

export const runCliWith = Command.runWith(openagentsCommand, { version: VERSION });
