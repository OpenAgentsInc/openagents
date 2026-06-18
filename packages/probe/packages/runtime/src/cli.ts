#!/usr/bin/env bun
import { existsSync } from "node:fs";
import { mkdir, readFile, readdir, stat, writeFile } from "node:fs/promises";
import { dirname, relative, resolve, sep } from "node:path";
import { createInterface, type Interface } from "node:readline/promises";
import { Cause, Effect, Exit, Schema as S } from "effect";
import {
  applyAnyWorkspaceFilePatch,
  editAnyWorkspaceFile,
  writeAnyWorkspaceFile as writeAnyWorkspaceFileWithBom,
} from "./file-mutation";
import {
  resolveProbeChatWorkspaceRoot,
  resolveProbeWorkspaceRoot,
  resolveWorkspacePath,
} from "./workspace";
import { marked } from "marked";
import {
  AppleFmBackendError,
  makeAppleFmClient,
  type AppleFmPlainTextCompletion,
  type AppleFmReadiness,
  type AppleFmToolStreamResult,
} from "./backends/apple-fm/client";
import {
  type AppleFmBlueprintToolProjection,
  projectProbeToolMenuToAppleFm,
} from "./backends/apple-fm/blueprint-tools";
import { makeAppleFmToolStreamProgramRunEvidence } from "./backends/apple-fm/program-run-evidence";
import { makeAppleFmToolCallbackSession } from "./backends/apple-fm/tools";
import { GeminiClientError, makeGeminiClient, type GeminiClient, type GeminiCompleteResult } from "./backends/gemini/client";
import { GEMINI_API_PROFILE_ID, GEMINI_DEFAULT_MODEL_ID } from "./backends/gemini/contract";
import {
  loadBlueprintSignatureRegistry,
  lookupBlueprintSignatures,
  planProbeToolMenu,
} from "./blueprint";
import {
  applyOpenAgentsAutopilotCoderStudiedRuntimeContextToToolMenuInput,
  loadOpenAgentsAutopilotCoderStudiedRuntimeContext,
  type OpenAgentsAutopilotCoderStudiedRuntimeContext,
} from "./benchmark/openagents-autopilot-coder-studied-runtime";
import {
  defineProbeLlmTool,
  makeProbeLlmMessage,
  makeProbeLlmRequest,
  probeLlmToolDefinitions,
  type ProbeLlmEvent,
  type ProbeLlmMessage,
  type ProbeLlmRequest,
  type ProbeLlmTools,
} from "./llm";
import { PROBE_APPLE_FM_BACKEND_CAPABILITY } from "./runner/identity";
import { makeOmegaAccountClient, type OmegaAccountClient } from "./omega/account-client";
import { sanitizeProbePublicProjection } from "./contracts/provider-account";
import { createProbeRenderer, createAssistantText, createCodeWithLineNumbers, detectFiletype, createDefaultSyntaxStyle, parseColor, TextRenderable, BoxRenderable, ScrollBoxRenderable } from "./opentui-renderer";
import { type ProbeRunnerIdentity } from "./runner/identity";
import {
  bestEffortRecordProbeTokenUsageEvent,
  makeAppleFmProbeTokenUsageEvent,
  makeGeminiProbeTokenUsageEvent,
  makeProbeTokenUsageTelemetryClientFromEnv,
  probeTokenUsageActorFromEnv,
  probeTokenUsagePrivacyFromEnv,
} from "./fleet/token-usage";

export interface ProbeCliResult {
  readonly exitCode: number;
  readonly stdout: string;
  readonly stderr: string;
}

export interface ProbeCliDeps {
  readonly accountClient?: OmegaAccountClient;
  readonly colors?: boolean;
  readonly env?: Readonly<Record<string, string | undefined>>;
  readonly fetch?: typeof fetch;
  readonly now?: Date;
}

export class ProbeCliError extends S.TaggedErrorClass<ProbeCliError>()("ProbeCliError", {
  message: S.String,
}) {}

export function runProbeCli(argv: ReadonlyArray<string>, deps: ProbeCliDeps = {}): Effect.Effect<ProbeCliResult, never> {
  return handleProbeCli(argv, deps).pipe(
    Effect.catch((error: ProbeCliError) =>
      Effect.succeed({
        exitCode: 1,
        stdout: "",
        stderr: `${error.message}\n`,
      }),
    ),
  );
}

function handleProbeCli(
  argv: ReadonlyArray<string>,
  deps: ProbeCliDeps,
): Effect.Effect<ProbeCliResult, ProbeCliError> {
  return Effect.gen(function* () {
    const [namespace, command, ...rest] = argv;
    const options = parseOptions(rest);

    if (namespace === "omega" && command === "link") {
      return yield* linkOmega(options, deps);
    }

    if (namespace === "auth" && command === "accounts") {
      return yield* listAccounts(options, deps);
    }

    if (namespace === "auth" && command === "add" && rest[0] === "chatgpt") {
      return yield* addChatGptAccount(parseOptions(rest.slice(1)), deps);
    }

    if (namespace === "backend" && command === "gemini" && rest[0] === "smoke") {
      return yield* geminiSmoke(parseOptions(rest.slice(1)), deps);
    }

    if (namespace === "backend" && command === "gemini" && rest[0] === "complete") {
      return yield* geminiComplete(parseOptions(rest.slice(1)), deps);
    }

    if (namespace === "chat") {
      return yield* geminiChatOnce(parseOptions(argv.slice(1)), deps);
    }

    if (namespace === "apple-fm" && command === "status") {
      return yield* appleFmStatus(options, deps);
    }

    if (namespace === "apple-fm" && command === "smoke") {
      return yield* appleFmSmoke(options, deps);
    }

    if (namespace === "apple-fm" && command === "tool-stream-demo") {
      return yield* appleFmToolStreamDemo(options, deps);
    }

    if (namespace === "studied-coder" && command === "context") {
      return yield* studiedCoderContext(options, deps);
    }

    return {
      exitCode: 1,
      stdout: usage(),
      stderr: "",
    };
  });
}

function studiedCoderContext(
  options: Record<string, string | true>,
  deps: ProbeCliDeps,
): Effect.Effect<ProbeCliResult, ProbeCliError> {
  return Effect.gen(function* () {
    const rootDir = resolve(stringOption(options, "root") ?? deps.env?.PROBE_STUDIED_CODER_ROOT ?? process.cwd());
    const editSitePath = stringOption(options, "edit-site");
    const commitHistoryLimit = numberOption(options, "commit-history-limit");
    const runtimeContext = yield* loadOpenAgentsAutopilotCoderStudiedRuntimeContext({
      commitHistoryLimit,
      editSitePath,
      rootDir,
    }).pipe(Effect.mapError((error) => new ProbeCliError({ message: `studied-coder context failed: ${String(error)}` })));

    const registryView = yield* loadBlueprintSignatureRegistry({ sourceKind: "staticFixture" }).pipe(
      Effect.mapError((error) => new ProbeCliError({ message: error.reason })),
    );
    const lookup = yield* lookupBlueprintSignatures({
      backendCapabilityRefs: [PROBE_APPLE_FM_BACKEND_CAPABILITY, "probe.blueprint.tool_menu"],
      lookupId: "blueprint_signature_lookup.openagents.studied_coder_context",
      registryView,
      request: {
        actorRef: "actor.openagents.autopilot_coder",
        allowedSurfaces: ["agent_api"],
        backendKind: "apple_fm_bridge",
        contextPackRef: runtimeContext.contextPackRef,
        programSignatureIds: ["program_signature.probe.tool_menu.project.v1"],
        riskCeiling: "medium",
      },
    }).pipe(Effect.mapError((error) => new ProbeCliError({ message: error.reason })));

    const menuInput = applyOpenAgentsAutopilotCoderStudiedRuntimeContextToToolMenuInput(
      {
        backendKind: "apple_fm_bridge",
        contextPackRefs: [lookup.contextPackRef ?? "context_pack.probe.assignment.base"],
        deniedToolRefs: [],
        lookup,
        menuId: "probe_tool_menu.openagents.studied_coder_context",
        sourceAuthorityRefs: ["source_authority.probe.assignment"],
        supportedToolRefs: ["tool.probe.read_file", "tool.probe.code_search", "tool.probe.record_evidence"],
      },
      runtimeContext,
    );
    const menu = yield* planProbeToolMenu(menuInput).pipe(
      Effect.mapError((error) => new ProbeCliError({ message: error.reason })),
    );

    return {
      exitCode: runtimeContext.correctnessGatePassed && runtimeContext.dogfoodLift.studiedBeatsBaseline ? 0 : 1,
      stdout: formatStudiedCoderContext(runtimeContext, menu.tools.length),
      stderr: "",
    };
  });
}

function formatStudiedCoderContext(
  runtimeContext: OpenAgentsAutopilotCoderStudiedRuntimeContext,
  studiedToolCount: number,
): string {
  const lift = runtimeContext.dogfoodLift;
  return [
    "OpenAgents Autopilot-coder studied runtime context (internal dogfood only)",
    `repo: ${runtimeContext.repo}`,
    `commit: ${runtimeContext.commit}`,
    `editSite: ${runtimeContext.editSitePath}`,
    `contextPackRef: ${runtimeContext.contextPackRef}`,
    `planContextRef: ${runtimeContext.planContextRef}`,
    `runtimeContextRef: ${runtimeContext.runtimeContextRef}`,
    `correctnessGatePassed: ${runtimeContext.correctnessGatePassed}`,
    `studiedToolsInjected: ${studiedToolCount}`,
    "dogfood lift vs no-studied baseline:",
    `  studiedBeatsBaseline: ${lift.studiedBeatsBaseline}`,
    `  passRateLiftBps: ${lift.passRateLiftBps}`,
    `  rubricScoreLiftBps: ${lift.rubricScoreLiftBps}`,
    `  firstDivergenceStepLift: ${lift.firstDivergenceStepLift}`,
    `  wrongFileReadReduction: ${lift.wrongFileReadReduction}`,
    `  evalReportRef: ${lift.evalReportRef}`,
    `  scope: ${lift.scope}`,
    `  customerPublicClaimAllowed: ${lift.customerPublicClaimAllowed}`,
  ].join("\n") + "\n";
}

function geminiSmoke(
  options: Record<string, string | true>,
  deps: ProbeCliDeps,
): Effect.Effect<ProbeCliResult, ProbeCliError> {
  return geminiCompletionCommand({
    title: "Gemini smoke",
    options,
    deps,
    defaultPrompt: "Reply with: probe gemini smoke ok.",
  });
}

function geminiComplete(
  options: Record<string, string | true>,
  deps: ProbeCliDeps,
): Effect.Effect<ProbeCliResult, ProbeCliError> {
  return geminiCompletionCommand({
    title: "Gemini completion",
    options,
    deps,
    defaultPrompt: "Reply with a concise Probe Gemini completion.",
  });
}

function geminiChatOnce(
  options: Record<string, string | true>,
  deps: ProbeCliDeps,
): Effect.Effect<ProbeCliResult, ProbeCliError> {
  return Effect.gen(function* () {
    const prompt = stringOption(options, "prompt");

    if (prompt === undefined) {
      return {
        exitCode: 1,
        stdout: [
          "Probe Gemini chat is interactive.",
          "Run `probe chat` for the prompt, or `probe chat --prompt TEXT` for one turn.",
        ].join("\n") + "\n",
        stderr: "",
      };
    }

    const model = stringOption(options, "model") ?? GEMINI_DEFAULT_MODEL_ID;
    const client = yield* makeGeminiClient({
      profileId: stringOption(options, "profile") ?? deps.env?.PROBE_BACKEND_PROFILE ?? GEMINI_API_PROFILE_ID,
      explicitBaseUrl: stringOption(options, "base-url"),
      env: deps.env,
      fetch: deps.fetch,
      now: deps.now,
    }).pipe(Effect.mapError((error) => new ProbeCliError({ message: "reason" in error ? error.reason : String(error) })));
    const tools = makeGeminiChatTools(deps.env);
    const request = makeGeminiChatRequest({
      messages: [],
      model,
      prompt,
      maxTokens: numberOption(options, "max-tokens") ?? 65536,
      tools,
    });
    const result = yield* client.complete({ request, tools }).pipe(
      Effect.catch((error: GeminiClientError) => Effect.succeed(error)),
    );

    if (result instanceof GeminiClientError) {
      return {
        exitCode: 1,
        stdout: formatGeminiFailure("Probe Gemini chat", result, makeCliColors(options, deps)),
        stderr: "",
      };
    }

    yield* recordCliGeminiTokenUsage({
      command: "chat",
      deps,
      result,
    });

    return {
      exitCode: 0,
      stdout: formatGeminiChatTurn({
        apiKeySource: client.apiKey.source,
        colors: makeCliColors(options, deps),
        includeHeader: true,
        result,
      }),
      stderr: "",
    };
  });
}

function geminiCompletionCommand(input: {
  readonly title: string;
  readonly options: Record<string, string | true>;
  readonly deps: ProbeCliDeps;
  readonly defaultPrompt: string;
}): Effect.Effect<ProbeCliResult, ProbeCliError> {
  return Effect.gen(function* () {
    const model = stringOption(input.options, "model") ?? GEMINI_DEFAULT_MODEL_ID;
    const prompt = stringOption(input.options, "prompt") ?? input.defaultPrompt;
    const client = yield* makeGeminiClient({
      profileId: stringOption(input.options, "profile") ?? input.deps.env?.PROBE_BACKEND_PROFILE ?? GEMINI_API_PROFILE_ID,
      explicitBaseUrl: stringOption(input.options, "base-url"),
      env: input.deps.env,
      fetch: input.deps.fetch,
      now: input.deps.now,
    }).pipe(Effect.mapError((error) => new ProbeCliError({ message: "reason" in error ? error.reason : String(error) })));
    const result = yield* client.complete({
      request: makeProbeLlmRequest({
        model: { provider: "google", model },
        prompt,
        generation: { maxTokens: numberOption(input.options, "max-tokens") ?? 65536, temperature: 0 },
      }),
    }).pipe(Effect.catch((error: GeminiClientError) => Effect.succeed(error)));

    if (result instanceof GeminiClientError) {
      return {
        exitCode: 1,
        stdout: formatGeminiFailure(input.title, result, makeCliColors(input.options, input.deps)),
        stderr: "",
      };
    }

    yield* recordCliGeminiTokenUsage({
      command: input.title === "Gemini smoke" ? "backend.gemini.smoke" : "backend.gemini.complete",
      deps: input.deps,
      result,
    });

    return {
      exitCode: 0,
      stdout: formatGeminiCompletion(input.title, client.apiKey.source, result, makeCliColors(input.options, input.deps)),
      stderr: "",
    };
  });
}

function appleFmToolStreamDemo(
  options: Record<string, string | true>,
  deps: ProbeCliDeps,
): Effect.Effect<ProbeCliResult, ProbeCliError> {
  return Effect.gen(function* () {
    const client = yield* makeAppleFmClient({
      profileId: stringOption(options, "profile"),
      explicitBaseUrl: stringOption(options, "base-url"),
      env: deps.env,
      fetch: deps.fetch,
      now: deps.now,
    }).pipe(Effect.mapError((error) => new ProbeCliError({ message: error.reason })));
    yield* client.requireReady().pipe(
      Effect.mapError((error) => new ProbeCliError({ message: `${error.failureClass}: ${error.reason}` })),
    );
    const requestedPath = stringOption(options, "path") ?? "README.md";
    const prompt =
      stringOption(options, "prompt") ??
      `Use the read_file tool to read ${requestedPath}, then stream one concise sentence naming the file and its first heading.`;
    const registryView = yield* loadBlueprintSignatureRegistry({ sourceKind: "staticFixture" }).pipe(
      Effect.mapError((error) => new ProbeCliError({ message: error.reason })),
    );
    const lookup = yield* lookupBlueprintSignatures({
      backendCapabilityRefs: [PROBE_APPLE_FM_BACKEND_CAPABILITY, "probe.blueprint.tool_menu"],
      lookupId: "blueprint_signature_lookup.apple_fm.tool_stream_demo",
      registryView,
      request: {
        actorRef: "actor.probe.cli",
        allowedSurfaces: ["agent_api"],
        backendKind: "apple_fm_bridge",
        contextPackRef: `context_pack.probe.cli.${requestedPath}`,
        programSignatureIds: ["program_signature.probe.tool_menu.project.v1"],
        riskCeiling: "medium",
      },
    }).pipe(Effect.mapError((error) => new ProbeCliError({ message: error.reason })));
    const menu = yield* planProbeToolMenu({
      backendKind: "apple_fm_bridge",
      contextPackRefs: [lookup.contextPackRef ?? `context_pack.probe.cli.${requestedPath}`],
      deniedToolRefs: [],
      lookup,
      maxToolCount: 1,
      menuId: "probe_tool_menu.apple_fm.tool_stream_demo",
      sourceAuthorityRefs: [`source_authority.probe.workspace.${requestedPath}`],
      supportedToolRefs: ["tool.probe.read_file"],
    }).pipe(Effect.mapError((error) => new ProbeCliError({ message: error.reason })));
    const projectedMenu = yield* projectProbeToolMenuToAppleFm({
      enumHints: {
        "tool.probe.read_file": {
          path: [requestedPath],
        },
      },
      executors: {
        "tool.probe.read_file": (input) => readWorkspaceFile(input, requestedPath),
      },
      menu,
    }).pipe(Effect.mapError((error) => new ProbeCliError({ message: error.reason })));
    const toolSession = makeAppleFmToolCallbackSession({
      tools: projectedMenu.toolDefinitions,
      now: deps.now,
    });
    const result = yield* client.streamSessionWithTools({
      prompt,
      instructions: "Use available tools when the user asks to inspect a local file. Keep the final answer concise.",
      toolSession,
    }).pipe(Effect.mapError((error) => new ProbeCliError({ message: `${error.failureClass}: ${error.reason}` })));
    const programRunEvidence = yield* makeAppleFmToolStreamProgramRunEvidence({
      actorRef: "actor.probe.cli",
      menu,
      observedAt: (deps.now ?? new Date()).toISOString(),
      promptSummaryRef: `prompt_summary.probe.cli.${requestedPath}`,
      projection: projectedMenu.projection,
      result,
    }).pipe(Effect.mapError((error) => new ProbeCliError({ message: error.reason })));

    return {
      exitCode: 0,
      stdout: formatAppleFmToolStreamDemo(result, projectedMenu.projection, programRunEvidence),
      stderr: "",
    };
  });
}

function appleFmSmoke(
  options: Record<string, string | true>,
  deps: ProbeCliDeps,
): Effect.Effect<ProbeCliResult, ProbeCliError> {
  return Effect.gen(function* () {
    const client = yield* makeAppleFmClient({
      profileId: stringOption(options, "profile"),
      explicitBaseUrl: stringOption(options, "base-url"),
      env: deps.env,
      fetch: deps.fetch,
      now: deps.now,
    }).pipe(Effect.mapError((error) => new ProbeCliError({ message: error.reason })));
    const prompt = stringOption(options, "prompt") ?? "Reply with: probe apple fm smoke ok.";
    const result = yield* client.smoke(prompt).pipe(
      Effect.catch((error: AppleFmBackendError) => Effect.succeed(error)),
    );

    if (result instanceof AppleFmBackendError) {
      return {
        exitCode: 1,
        stdout: formatAppleFmSmokeFailure(result),
        stderr: "",
      };
    }

    yield* recordCliAppleFmTokenUsage({
      command: "apple-fm.smoke",
      deps,
      result,
    });

    return {
      exitCode: 0,
      stdout: formatAppleFmSmokeCompletion(result),
      stderr: "",
    };
  });
}

function appleFmStatus(
  options: Record<string, string | true>,
  deps: ProbeCliDeps,
): Effect.Effect<ProbeCliResult, ProbeCliError> {
  return Effect.gen(function* () {
    const client = yield* makeAppleFmClient({
      profileId: stringOption(options, "profile"),
      explicitBaseUrl: stringOption(options, "base-url"),
      env: deps.env,
      fetch: deps.fetch,
      now: deps.now,
    }).pipe(Effect.mapError((error) => new ProbeCliError({ message: error.reason })));
    const readiness = yield* client.health();

    return {
      exitCode: readiness.ready ? 0 : 1,
      stdout: formatAppleFmStatus(readiness),
      stderr: "",
    };
  });
}

function linkOmega(options: Record<string, string | true>, deps: ProbeCliDeps): Effect.Effect<ProbeCliResult, ProbeCliError> {
  return Effect.gen(function* () {
    const now = deps.now ?? new Date();
    const statePath = resolveStatePath(options, deps.env);
    const runner: ProbeRunnerIdentity = {
      runnerId: stringOption(options, "runner-id") ?? "probe-local",
      kind: runnerKindOption(options, "kind") ?? "local",
      linkedSubject: stringOption(options, "subject") ?? "local-user",
      linkedAt: now.toISOString(),
      capabilities: ["probe.run", "omega.grant.resolve"],
    };

    const state = sanitizeProbePublicProjection({
      version: 1,
      omegaBaseUrl: omegaBaseUrl(options, deps.env),
      runner,
    });

    yield* Effect.tryPromise({
      try: () => mkdir(dirname(statePath), { recursive: true }),
      catch: (error) => new ProbeCliError({ message: `failed to create Probe state directory: ${String(error)}` }),
    });

    yield* Effect.tryPromise({
      try: () => writeFile(statePath, `${JSON.stringify(state, null, 2)}\n`, { mode: 0o600 }),
      catch: (error) => new ProbeCliError({ message: `failed to write Probe Omega link state: ${String(error)}` }),
    });

    return {
      exitCode: 0,
      stdout: `Linked Probe runner ${runner.runnerId} to ${state.omegaBaseUrl}\nState: ${statePath}\n`,
      stderr: "",
    };
  });
}

function listAccounts(options: Record<string, string | true>, deps: ProbeCliDeps): Effect.Effect<ProbeCliResult, ProbeCliError> {
  return Effect.gen(function* () {
    const client = deps.accountClient ?? makeOmegaAccountClient(clientOptions(options, deps.env));
    const response = yield* client.listProviderAccounts().pipe(
      Effect.mapError((error) => new ProbeCliError({ message: error.reason })),
    );

    if (response.accounts.length === 0) {
      return {
        exitCode: 0,
        stdout: "No Omega-connected ChatGPT accounts.\n",
        stderr: "",
      };
    }

    const lines = response.accounts.map((account) => {
      const label = account.accountLabel ?? account.providerAccountRef;
      const plan = account.planType === undefined ? "unknown-plan" : account.planType;
      return `${account.providerAccountRef}\t${label}\t${account.status}/${account.health}\t${plan}`;
    });

    return {
      exitCode: 0,
      stdout: `${lines.join("\n")}\n`,
      stderr: "",
    };
  });
}

function addChatGptAccount(
  options: Record<string, string | true>,
  deps: ProbeCliDeps,
): Effect.Effect<ProbeCliResult, ProbeCliError> {
  return Effect.gen(function* () {
    const client = deps.accountClient ?? makeOmegaAccountClient(clientOptions(options, deps.env));
    const started = yield* client.startChatGptDeviceLogin({ createNew: true }).pipe(
      Effect.mapError((error) => new ProbeCliError({ message: error.reason })),
    );
    const attempt = yield* client.readChatGptDeviceLogin(started.attemptId).pipe(
      Effect.mapError((error) => new ProbeCliError({ message: error.reason })),
    );

    return {
      exitCode: 0,
      stdout: [
        `Open ${started.verificationUrl}`,
        `Code ${started.userCode}`,
        `Attempt ${started.attemptId}: ${attempt.status}`,
        `Provider account ${attempt.providerAccountRef}`,
      ].join("\n") + "\n",
      stderr: "",
    };
  });
}

function parseOptions(args: ReadonlyArray<string>): Record<string, string | true> {
  const parsed: Record<string, string | true> = {};

  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];

    if (!arg.startsWith("--")) {
      continue;
    }

    const key = arg.slice(2);
    const next = args[index + 1];

    if (next === undefined || next.startsWith("--")) {
      parsed[key] = true;
      continue;
    }

    parsed[key] = next;
    index += 1;
  }

  return parsed;
}

function clientOptions(options: Record<string, string | true>, env: Readonly<Record<string, string | undefined>> = {}) {
  return {
    baseUrl: omegaBaseUrl(options, env),
    bearerToken: stringOption(options, "token") ?? env.PROBE_OMEGA_BEARER_TOKEN,
  };
}

function omegaBaseUrl(options: Record<string, string | true>, env: Readonly<Record<string, string | undefined>> = {}) {
  return stringOption(options, "base-url") ?? env.PROBE_OMEGA_BASE_URL ?? "https://openagents.com";
}

function resolveStatePath(
  options: Record<string, string | true>,
  env: Readonly<Record<string, string | undefined>> = {},
): string {
  return resolve(stringOption(options, "state") ?? env.PROBE_STATE_PATH ?? ".probe/omega-link.json");
}

function stringOption(options: Record<string, string | true>, key: string): string | undefined {
  const value = options[key];
  return typeof value === "string" ? value : undefined;
}

function numberOption(options: Record<string, string | true>, key: string): number | undefined {
  const value = stringOption(options, key);

  if (value === undefined) {
    return undefined;
  }

  const parsed = Number(value);

  return Number.isFinite(parsed) && parsed > 0 ? parsed : undefined;
}

function runnerKindOption(options: Record<string, string | true>, key: string): ProbeRunnerIdentity["kind"] | undefined {
  const value = stringOption(options, key);

  return value === "local" || value === "shc" || value === "pylon" || value === "sandbox" ? value : undefined;
}

function usage(): string {
  return [
    "Usage:",
    "  probe omega link [--base-url URL] [--runner-id ID] [--subject USER_OR_TEAM] [--kind local|shc|pylon|sandbox]",
    "  probe auth accounts [--base-url URL]",
    "  probe auth add chatgpt [--base-url URL]",
    "  probe chat [--profile gemini-api] [--model gemini-3.5-flash] [--prompt TEXT] [--color always|never] [--no-color] [--tui]",
    "  probe backend gemini smoke [--profile gemini-api] [--model gemini-3.5-flash] [--prompt TEXT]",
    "  probe backend gemini complete [--profile gemini-api] [--model gemini-3.5-flash] [--prompt TEXT]",
    "  probe apple-fm status [--base-url URL] [--profile apple-fm-local]",
    "  probe apple-fm smoke [--base-url URL] [--profile apple-fm-local] [--prompt TEXT]",
    "  probe apple-fm tool-stream-demo [--base-url URL] [--path FILE] [--prompt TEXT]",
    "  probe studied-coder context [--root DIR] [--edit-site FILE] [--commit-history-limit N]",
  ].join("\n") + "\n";
}

function formatAppleFmStatus(readiness: AppleFmReadiness): string {
  const health = readiness.health;
  const lines = [
    "Apple FM backend status",
    `profile: ${readiness.profile.id}`,
    `kind: ${readiness.profile.kind}`,
    `baseUrl: ${readiness.profile.baseUrl}`,
    `model: ${health?.modelId ?? health?.model ?? readiness.profile.model}`,
    `status: ${readiness.status}`,
  ];

  if (readiness.unavailableReason !== undefined) {
    lines.push(`unavailableReason: ${readiness.unavailableReason}`);
  }

  if (readiness.message !== undefined) {
    lines.push(`message: ${readiness.message}`);
  }

  if (health?.platform !== undefined) {
    lines.push(`platform: ${health.platform}`);
  }

  if (health?.version !== undefined) {
    lines.push(`version: ${health.version}`);
  }

  lines.push(`receipt: ${JSON.stringify(readiness.receipt)}`);

  return `${lines.join("\n")}\n`;
}

function formatAppleFmSmokeCompletion(completion: AppleFmPlainTextCompletion): string {
  return [
    "Apple FM smoke",
    `profile: ${completion.profile.id}`,
    `kind: ${completion.profile.kind}`,
    `model: ${completion.response.model ?? completion.profile.model}`,
    `probe: ${completion.text}`,
    `usage: ${formatUsage(completion.usage)}`,
    `receipt: ${JSON.stringify(completion.receipt)}`,
  ].join("\n") + "\n";
}

function formatAppleFmSmokeFailure(error: AppleFmBackendError): string {
  const lines = [
    "Apple FM smoke failed",
    `failureClass: ${error.failureClass}`,
    `message: ${error.reason}`,
  ];

  if (error.receipt !== undefined) {
    lines.push(`receipt: ${JSON.stringify(error.receipt)}`);
  }

  return `${lines.join("\n")}\n`;
}

function formatGeminiCompletion(
  title: string,
  apiKeySource: string,
  completion: GeminiCompleteResult,
  colors: ProbeCliColors = noCliColors,
): string {
  return [
    cliHeader(colors, title),
    cliField(colors, "profile", completion.profile.id),
    cliField(colors, "kind", completion.profile.kind),
    cliField(colors, "model", completion.finalRequest.model.model),
    cliField(colors, "apiKeySource", apiKeySource),
    cliField(colors, "apiKeyRedacted", "true"),
    cliLine(colors, "probe", completion.text, "assistant"),
    cliField(colors, "roundTrips", String(completion.roundTrips), "muted"),
    cliLine(colors, "usage", formatGeminiUsage(completion.receipt.usage), "usage"),
    cliField(colors, "receipt", JSON.stringify(completion.receipt), "muted"),
  ].join("\n") + "\n";
}

function formatGeminiChatTurn(input: {
  readonly apiKeySource: string;
  readonly colors?: ProbeCliColors;
  readonly includeHeader?: boolean;
  readonly result: GeminiCompleteResult;
}): string {
  const lines: string[] = [];
  const colors = input.colors ?? noCliColors;

  if (input.includeHeader === true) {
    lines.push(cliHeader(colors, "Probe Gemini chat"));
    lines.push(cliField(colors, "profile", input.result.profile.id));
    lines.push(cliField(colors, "kind", input.result.profile.kind));
    lines.push(cliField(colors, "model", input.result.finalRequest.model.model));
    lines.push(cliField(colors, "apiKeySource", input.apiKeySource));
    lines.push(cliField(colors, "apiKeyRedacted", "true"));
  }

  for (const event of input.result.events) {
    if (event.type === "tool-call") {
      lines.push(cliToolLine(colors, "tool_call", event.name, safeJson(event.input), "call"));
    }

    if (event.type === "tool-result") {
      lines.push(cliToolLine(colors, "tool_result", event.name, formatToolResultValue(event.result), "result"));
    }

    if (event.type === "tool-error") {
      lines.push(cliToolLine(colors, "tool_error", event.name, event.message, "error"));
    }
  }

  lines.push(cliLine(colors, "probe", input.result.text, "assistant"));
  lines.push(cliField(colors, "roundTrips", String(input.result.roundTrips), "muted"));
  lines.push(cliLine(colors, "usage", formatGeminiUsage(input.result.receipt.usage), "usage"));

  return `${lines.join("\n")}\n`;
}

function formatGeminiFailure(title: string, error: GeminiClientError, colors: ProbeCliColors = noCliColors): string {
  const lines = [
    cliHeader(colors, `${title} failed`, "error"),
    cliField(colors, "failureClass", error.failureClass, "error"),
    cliField(colors, "message", error.reason, "error"),
  ];

  if (error.receipt !== undefined) {
    lines.push(cliField(colors, "receipt", JSON.stringify(error.receipt), "muted"));
  }

  return `${lines.join("\n")}\n`;
}

function formatGeminiUsage(usage: GeminiCompleteResult["receipt"]["usage"]): string {
  if (usage === undefined) {
    return "unreported";
  }

  const parts: string[] = [];

  if (usage.inputTokens !== undefined) {
    parts.push(`input=${usage.inputTokens}`);
  }

  if (usage.outputTokens !== undefined) {
    parts.push(`output=${usage.outputTokens}`);
  }

  if (usage.totalTokens !== undefined) {
    parts.push(`total=${usage.totalTokens}`);
  }

  return parts.length === 0 ? "unreported" : parts.join(" ");
}

function recordCliGeminiTokenUsage(input: {
  readonly command: string;
  readonly deps: ProbeCliDeps;
  readonly result: GeminiCompleteResult;
}): Effect.Effect<void, never> {
  const env = input.deps.env ?? {};
  const client = makeProbeTokenUsageTelemetryClientFromEnv({
    env,
    fetch: input.deps.fetch,
  });

  return bestEffortRecordProbeTokenUsageEvent(
    client,
    makeGeminiProbeTokenUsageEvent({
      actor: probeTokenUsageActorFromEnv(env),
      agentSurface: "cli",
      command: input.command,
      privacy: probeTokenUsagePrivacyFromEnv(env),
      result: input.result,
      sourceRefs: {
        anonymizedSourceRef: `probe.cli.${input.command}.${input.result.profile.id}.${input.result.finalRequest.model.model}`,
      },
    }),
  );
}

function recordCliAppleFmTokenUsage(input: {
  readonly command: string;
  readonly deps: ProbeCliDeps;
  readonly result: AppleFmPlainTextCompletion;
}): Effect.Effect<void, never> {
  const env = input.deps.env ?? {};
  const client = makeProbeTokenUsageTelemetryClientFromEnv({
    env,
    fetch: input.deps.fetch,
  });

  return bestEffortRecordProbeTokenUsageEvent(
    client,
    makeAppleFmProbeTokenUsageEvent({
      actor: probeTokenUsageActorFromEnv(env),
      agentSurface: "cli",
      command: input.command,
      model: input.result.response.model ?? input.result.profile.model,
      observedAt: input.result.receipt.observedAt,
      privacy: probeTokenUsagePrivacyFromEnv(env),
      profile: input.result.profile,
      sourceRefs: {
        anonymizedSourceRef: `probe.cli.${input.command}.${input.result.profile.id}.${input.result.profile.model}`,
      },
      usage: input.result.usage,
    }),
  );
}

function formatUsage(usage: AppleFmPlainTextCompletion["usage"]): string {
  const parts = [`truth=${usage.truth}`];

  if (usage.promptTokens !== undefined) {
    parts.push(`prompt=${usage.promptTokens}`);
  }

  if (usage.completionTokens !== undefined) {
    parts.push(`completion=${usage.completionTokens}`);
  }

  if (usage.totalTokens !== undefined) {
    parts.push(`total=${usage.totalTokens}`);
  }

  return parts.join(" ");
}

type ProbeCliColorRole = "assistant" | "default" | "error" | "header" | "muted" | "prompt" | "tool" | "usage";

interface ProbeCliColors {
  readonly enabled: boolean;
}

const noCliColors: ProbeCliColors = { enabled: false };

const ansi = {
  reset: "\x1b[0m",
  bold: "\x1b[1m",
  dim: "\x1b[2m",
  cyan: "\x1b[36m",
  green: "\x1b[32m",
  yellow: "\x1b[33m",
  magenta: "\x1b[35m",
  blue: "\x1b[34m",
  red: "\x1b[31m",
  gray: "\x1b[90m",
  eraseLine: "\x1b[K",
} as const;

function renderMarkdown(text: string): string {
  const renderer = new (class extends marked.Renderer {
    strong(t: string): string { return `${ansi.bold}${t}${ansi.reset}`; }
    em(t: string): string { return `${ansi.dim}${t}${ansi.reset}`; }
    codespan(t: string): string { return `${ansi.green}${t}${ansi.reset}`; }
    del(t: string): string { return `${ansi.dim}${t}${ansi.reset}`; }
    link(href: string, _title: string | null, t: string): string {
      return `${t} ${ansi.blue}${href}${ansi.reset}`;
    }
    image(_href: string, _title: string | null, t: string): string {
      return t;
    }
    heading(t: string): string { return `${ansi.bold}${t}${ansi.reset}\n`; }
    paragraph(t: string): string { return `${t}\n`; }
    listitem(t: string): string { return `  • ${t}\n`; }
    blockquote(t: string): string { return `${ansi.yellow}> ${t}${ansi.reset}\n`; }
    codes(t: string, _language?: string): string {
      return `${ansi.gray}${t}${ansi.reset}\n`;
    }
    hr(): string { return `${ansi.gray}---${ansi.reset}\n`; }
    br(): string { return "\n"; }
    html(t: string): string { return t; }
    text(t: string): string { return t; }
  })();
  return marked.parse(text, { renderer });
}

function formatInlineMarkdown(text: string): string {
  const bold = (s: string) => `${ansi.bold}${s}${ansi.reset}`;
  const dim = (s: string) => `${ansi.dim}${s}${ansi.reset}`;

  const lines = text.split("\n").map((line) => {
    let l = line;
    l = l.replace(/^#{1,6}\s+/, "");
    l = l.replace(/^>\s?/, `${ansi.yellow}> ${ansi.reset}`);
    l = l.replace(/^[-*+]\s+/, `${ansi.cyan}• `);
    l = l.replace(/\*\*(.+?)\*\*/g, (_, s) => bold(s));
    l = l.replace(/__(.+?)__/g, (_, s) => bold(s));
    l = l.replace(/`([^`]+)`/g, (_, s) => `${ansi.green}${s}${ansi.reset}`);
    l = l.replace(/\[([^\]]+)\]\(([^)]+)\)/g, (_, t, u) => `${t} ${ansi.blue}${u}${ansi.reset}`);
    l = l.replace(/\*(.+?)\*/g, (_, s) => dim(s));
    l = l.replace(/_(.+?)_/g, (_, s) => dim(s));
    return l;
  });

  return lines.join("\n");
}

function makeCliColors(
  options: Record<string, string | true>,
  deps: ProbeCliDeps,
  defaultEnabled = false,
): ProbeCliColors {
  return {
    enabled: shouldUseCliColors(options, deps.env, deps.colors ?? defaultEnabled),
  };
}

function shouldUseCliColors(
  options: Record<string, string | true>,
  env: Readonly<Record<string, string | undefined>> = {},
  defaultEnabled = false,
): boolean {
  const option = stringOption(options, "color");

  if (option === "always") {
    return true;
  }

  if (option === "never" || options["no-color"] === true || env.PROBE_NO_COLOR !== undefined || env.NO_COLOR !== undefined) {
    return false;
  }

  if (env.PROBE_COLOR === "always" || (env.FORCE_COLOR !== undefined && env.FORCE_COLOR !== "0")) {
    return true;
  }

  if (env.PROBE_COLOR === "never" || env.TERM === "dumb") {
    return false;
  }

  return defaultEnabled;
}

function cliHeader(colors: ProbeCliColors, value: string, role: ProbeCliColorRole = "header"): string {
  return cliColor(colors, role, value);
}

function cliField(
  colors: ProbeCliColors,
  label: string,
  value: string,
  role: ProbeCliColorRole = "default",
): string {
  return `${cliLabel(colors, label, role)} ${cliColor(colors, role === "error" ? "error" : "muted", value)}`;
}

function cliLine(
  colors: ProbeCliColors,
  label: string,
  value: string,
  role: ProbeCliColorRole = "default",
): string {
  return `${cliLabel(colors, label, role)} ${value}`;
}

function cliToolLine(
  colors: ProbeCliColors,
  label: string,
  name: string,
  value: string,
  kind: "call" | "error" | "result",
): string {
  const role = kind === "error" ? "error" : "tool";

  return `${cliLabel(colors, label, role)} ${cliColor(colors, "tool", name)} ${cliColor(colors, kind === "call" ? "muted" : role, value)}`;
}

function cliLabel(colors: ProbeCliColors, label: string, role: ProbeCliColorRole): string {
  return cliColor(colors, role, `${label}:`);
}

function cliColor(colors: ProbeCliColors, role: ProbeCliColorRole, value: string): string {
  if (!colors.enabled) {
    return value;
  }

  const code = role === "assistant"
    ? `${ansi.bold}${ansi.green}`
    : role === "error"
      ? `${ansi.bold}${ansi.red}`
      : role === "header"
        ? `${ansi.bold}${ansi.cyan}`
        : role === "muted"
          ? ansi.gray
          : role === "prompt"
            ? `${ansi.bold}${ansi.cyan}`
            : role === "tool"
              ? ansi.magenta
              : role === "usage"
                ? ansi.yellow
                : ansi.cyan;

  return `${code}${value}${ansi.reset}`;
}

function formatAppleFmToolStreamDemo(
  result: AppleFmToolStreamResult,
  projection?: AppleFmBlueprintToolProjection,
  programRunEvidence?: { readonly programRunRef: string; readonly inputSnapshotHash: string },
): string {
  const lines = [
    "Apple FM tool stream demo",
    `bridgeSessionId: ${result.bridgeSessionId}`,
    `events: ${result.events.map((event) => event.kind).join(" -> ")}`,
  ];

  if (projection !== undefined) {
    lines.push(`blueprintLookupId: ${projection.lookupId}`);
    lines.push(`blueprintMenuId: ${projection.menuId}`);
    lines.push(`blueprintRegistryVersionRef: ${projection.registryVersionRef}`);
    lines.push(`blueprintProgramSignatures: ${projection.programSignatureIds.join(",")}`);
    lines.push(`blueprintTools: ${projection.toolRefs.map((tool) => `${tool.toolRef}:${tool.toolName}`).join(",")}`);
  }

  if (programRunEvidence !== undefined) {
    lines.push(`programRunRef: ${programRunEvidence.programRunRef}`);
    lines.push(`programRunInputSnapshotHash: ${programRunEvidence.inputSnapshotHash}`);
  }

  for (const event of result.events) {
    if (event.kind === "assistant_snapshot" && event.content !== undefined) {
      lines.push(`snapshot: ${event.content}`);
    }
  }

  for (const entry of result.toolTranscript) {
    lines.push(`tool: ${entry.toolName} ${entry.status} ${JSON.stringify(entry.input)}`);
  }

  lines.push(`final: ${result.completion.text}`);
  lines.push(`usage: ${formatUsage(result.completion.usage)}`);
  lines.push(`receipt: ${JSON.stringify(result.completion.receipt)}`);

  return `${lines.join("\n")}\n`;
}

function readWorkspaceFile(
  input: Readonly<Record<string, unknown>>,
  allowedPath: string,
): Effect.Effect<{ readonly path: string; readonly content?: string; readonly error?: string }, never> {
  return Effect.gen(function* () {
    const path = typeof input.path === "string" ? input.path : allowedPath;
    const workspace = resolveProbeWorkspaceRoot();
    const absolutePath = resolve(workspace, path);
    const relativePath = relative(workspace, absolutePath);

    if (
      path !== allowedPath ||
      relativePath.startsWith("..") ||
      relativePath === "" ||
      relativePath.split(sep).includes("..")
    ) {
      return {
        path,
        error: "path is outside the Blueprint-selected file scope",
      };
    }

    const content = yield* Effect.tryPromise({
      try: () => readFile(absolutePath, "utf8"),
      catch: (error) => error,
    }).pipe(
      Effect.catch((error) =>
        Effect.succeed(`failed to read ${path}: ${String(error)}`),
      ),
    );

    return {
      path,
      content: typeof content === "string" ? content.slice(0, 4000) : String(content),
    };
  });
}

function makeGeminiChatTools(env: Readonly<Record<string, string | undefined>> = {}): ProbeLlmTools {
  return {
    read_file: defineProbeLlmTool({
      name: "read_file",
      description: "Read a UTF-8 text file under the OpenAgents workspace. Use this when the user asks about local code or reference repos.",
      inputSchema: {
        type: "object",
        properties: {
          path: {
            type: "string",
            description: "A relative file path under the workspace.",
          },
        },
        required: ["path"],
      },
      execute: (input) => readAnyWorkspaceFile(input, env),
    }),
    write_file: defineProbeLlmTool({
      name: "write_file",
      description: "Write a UTF-8 text file under the OpenAgents workspace. Creates parent directories if needed. Preserves UTF-8 BOM if present. Use this to create new files or overwrite existing ones.",
      inputSchema: {
        type: "object",
        properties: {
          path: {
            type: "string",
            description: "A relative file path under the workspace.",
          },
          content: {
            type: "string",
            description: "The full file content to write.",
          },
        },
        required: ["path", "content"],
      },
      execute: (input) => writeAnyWorkspaceFileWithBom(input, env),
    }),
    edit_file: defineProbeLlmTool({
      name: "edit_file",
      description:
        "Replace exact text in one file under the workspace. Handles BOM and line endings automatically. Use this instead of write_file when you want to change part of a file.",
      inputSchema: {
        type: "object",
        properties: {
          path: {
            type: "string",
            description: "A relative file path under the workspace.",
          },
          oldString: {
            type: "string",
            description: "The exact text to replace. Must match the file content exactly, including whitespace and indentation.",
          },
          newString: {
            type: "string",
            description: "The replacement text. Must differ from oldString.",
          },
          replaceAll: {
            type: "boolean",
            description: "Replace all exact occurrences of oldString. Set to true only when you are certain there are multiple identical blocks to change.",
          },
        },
        required: ["path", "oldString", "newString"],
      },
      execute: (input) => editAnyWorkspaceFile(input, env),
    }),
    apply_patch: defineProbeLlmTool({
      name: "apply_patch",
      description: "Apply a structured patch with add, update, and delete operations across multiple files. Each operation starts with +ADD <path>, +UPDATE <path>, or +DELETE <path>. For +UPDATE, separate old and new content with ---.",
      inputSchema: {
        type: "object",
        properties: {
          patchText: {
            type: "string",
            description: "The full patch text describing add, update, and delete operations.",
          },
        },
        required: ["patchText"],
      },
      execute: (input) => applyAnyWorkspaceFilePatch(input, env),
    }),
    list_files: defineProbeLlmTool({
      name: "list_files",
      description: "List files under a directory in the OpenAgents workspace.",
      inputSchema: {
        type: "object",
        properties: {
          path: {
            type: "string",
            description: "A relative directory path under the workspace.",
          },
          limit: {
            type: "number",
            description: "Maximum files to return.",
          },
        },
      },
      execute: (input) => listWorkspaceFiles(input, env),
    }),
    search_code: defineProbeLlmTool({
      name: "search_code",
      description: "Search text in files under the OpenAgents workspace using ripgrep.",
      inputSchema: {
        type: "object",
        properties: {
          query: {
            type: "string",
            description: "The literal text or regex pattern to search for.",
          },
          path: {
            type: "string",
            description: "Optional relative directory or file path to search under.",
          },
          limit: {
            type: "number",
            description: "Maximum matching lines to return.",
          },
        },
        required: ["query"],
      },
      execute: (input) => searchWorkspaceCode(input, env),
    }),
    current_time: defineProbeLlmTool({
      name: "current_time",
      description: "Return the current local timestamp.",
      inputSchema: {
        type: "object",
        properties: {},
      },
      execute: () => Effect.succeed({ now: new Date().toISOString() }),
    }),
  };
}

function makeGeminiChatRequest(input: {
  readonly messages: ReadonlyArray<ProbeLlmMessage>;
  readonly model: string;
  readonly prompt: string;
  readonly maxTokens: number;
  readonly tools: ProbeLlmTools;
}): ProbeLlmRequest {
  return makeProbeLlmRequest({
    model: { provider: "google", model: input.model },
    system:
      "You are Probe, a concise coding agent. You can inspect the local OpenAgents workspace, including sibling repos and reference repos such as projects/repos/opencode, through tools. " +
      "Use list_files, search_code, and read_file when the user asks about local code. Do not refuse local workspace inspection just because the path is outside the Probe package. " +
      "When you use a tool, continue to a direct final answer after the tool result.",
    messages: [...input.messages, makeProbeLlmMessage("user", input.prompt)],
    tools: probeLlmToolDefinitions(input.tools),
    toolChoice: { type: "auto" },
    generation: { maxTokens: input.maxTokens, temperature: 0.2 },
  });
}

function readAnyWorkspaceFile(
  input: Readonly<Record<string, unknown>>,
  env: Readonly<Record<string, string | undefined>> = {},
): Effect.Effect<{ readonly path: string; readonly content?: string; readonly error?: string }, never> {
  return Effect.gen(function* () {
    const path = typeof input.path === "string" ? input.path : "";
    const workspace = resolveProbeChatWorkspaceRoot(env);
    const resolved = resolveWorkspacePath(workspace, path);

    if (resolved === undefined) {
      return {
        path,
        error: "path is outside the OpenAgents workspace file scope",
      };
    }

    const content = yield* Effect.tryPromise({
      try: () => readFile(resolved.absolutePath, "utf8"),
      catch: (error) => error,
    }).pipe(
      Effect.catch((error) =>
        Effect.succeed(`failed to read ${path}: ${String(error)}`),
      ),
    );

    return {
      path,
      content: typeof content === "string" ? content.slice(0, 100000) : String(content),
    };
  });
}

function writeAnyWorkspaceFile(
  input: Readonly<Record<string, unknown>>,
  env: Readonly<Record<string, string | undefined>> = {},
): Effect.Effect<{ readonly path: string; readonly content?: string; readonly error?: string }, never> {
  return Effect.gen(function* () {
    const path = typeof input.path === "string" ? input.path : "";
    const content = typeof input.content === "string" ? input.content : "";
    const workspace = resolveProbeChatWorkspaceRoot(env);
    const resolved = resolveWorkspacePath(workspace, path);

    if (resolved === undefined) {
      return { path, error: "path is outside the workspace file scope" };
    }

    if (!content) {
      return { path, error: "content is required" };
    }

    yield* Effect.tryPromise({
      try: () => mkdir(dirname(resolved.absolutePath), { recursive: true }),
      catch: (error) => error,
    }).pipe(
      Effect.catch((error) =>
        Effect.succeed(void 0),
      ),
    );

    const written = yield* Effect.tryPromise({
      try: () => writeFile(resolved.absolutePath, content, "utf8").then(() => true),
      catch: (error) => error,
    }).pipe(
      Effect.catch((error) =>
        Effect.succeed(`failed to write ${path}: ${String(error)}`),
      ),
    );

    if (written === true) {
      return { path, content: `written to ${resolved.relativePath}` };
    }

    return { path, error: written };
  });
}

function listWorkspaceFiles(
  input: Readonly<Record<string, unknown>>,
  env: Readonly<Record<string, string | undefined>> = {},
): Effect.Effect<{
  readonly path: string;
  readonly directories?: ReadonlyArray<string>;
  readonly files?: ReadonlyArray<string>;
  readonly truncated?: boolean;
  readonly error?: string;
}, never> {
  return Effect.gen(function* () {
    const path = typeof input.path === "string" ? input.path : ".";
    const limit = boundedToolLimit(input.limit, 80);
    const workspace = resolveProbeChatWorkspaceRoot(env);
    const resolved = resolveWorkspacePath(workspace, path);

    if (resolved === undefined) {
      return { path, error: "path is outside the OpenAgents workspace file scope" };
    }

    const listing = yield* collectWorkspaceEntries(resolved.absolutePath, resolved.relativePath, limit).pipe(
      Effect.catch((error) => Effect.succeed({ directories: [], files: [`failed to list ${path}: ${String(error)}`], truncated: false })),
    );

    return {
      path,
      ...listing,
    };
  });
}

function searchWorkspaceCode(
  input: Readonly<Record<string, unknown>>,
  env: Readonly<Record<string, string | undefined>> = {},
): Effect.Effect<{ readonly query: string; readonly path: string; readonly matches?: ReadonlyArray<string>; readonly truncated?: boolean; readonly error?: string }, never> {
  return Effect.gen(function* () {
    const query = typeof input.query === "string" ? input.query : "";
    const path = typeof input.path === "string" ? input.path : ".";
    const limit = boundedToolLimit(input.limit, 80);
    const workspace = resolveProbeChatWorkspaceRoot(env);
    const resolved = resolveWorkspacePath(workspace, path);

    if (query.length === 0) {
      return { query, path, error: "query is required" };
    }

    if (resolved === undefined) {
      return { query, path, error: "path is outside the OpenAgents workspace file scope" };
    }

    const output = yield* Effect.tryPromise({
      try: async () => {
        const child = Bun.spawn(["rg", "--line-number", "--no-heading", "--color", "never", query, resolved.relativePath], {
          cwd: workspace,
          stderr: "pipe",
          stdout: "pipe",
        });
        const text = await new Response(child.stdout).text();
        const errorText = await new Response(child.stderr).text();
        const exitCode = await child.exited;

        if (exitCode > 1) {
          return `ripgrep failed: ${errorText.trim()}`;
        }

        return text;
      },
      catch: (error) => `failed to search ${path}: ${String(error)}`,
    });
    const allMatches = output.split("\n").filter((line) => line.length > 0);
    const matches = allMatches.slice(0, limit);

    return {
      query,
      path,
      matches,
      truncated: allMatches.length > matches.length,
    };
  });
}

function collectWorkspaceEntries(
  absolutePath: string,
  relativePath: string,
  limit: number,
): Effect.Effect<{
  readonly directories: ReadonlyArray<string>;
  readonly files: ReadonlyArray<string>;
  readonly truncated: boolean;
}, unknown> {
  return Effect.tryPromise(async () => {
    const rootStat = await stat(absolutePath);

    if (rootStat.isFile()) {
      return { directories: [], files: [relativePath], truncated: false };
    }

    const files: string[] = [];
    const directories: string[] = [];
    const entries = await readdir(absolutePath, { withFileTypes: true });
    const visibleEntries = entries.filter((entry) => !shouldSkipWorkspaceEntry(entry.name));

    for (const entry of visibleEntries) {
      if (directories.length + files.length >= limit) {
        break;
      }

      const entryRelativePath = relativePath === "." ? entry.name : `${relativePath}/${entry.name}`;

      if (entry.isDirectory()) {
        directories.push(entryRelativePath);
        continue;
      }

      if (entry.isFile()) {
        files.push(entryRelativePath);
      }
    }

    return {
      directories,
      files,
      truncated: visibleEntries.length > directories.length + files.length,
    };
  });
}

function shouldSkipWorkspaceEntry(name: string): boolean {
  return name === ".git" || name === "node_modules" || name === ".next" || name === "dist" || name === "build";
}

function boundedToolLimit(value: unknown, fallback: number): number {
  return typeof value === "number" && Number.isFinite(value) && value > 0 ? Math.min(Math.floor(value), 500) : fallback;
}

function makeGeminiInteractiveTurnStream(colors: ProbeCliColors): {
  readonly onEvent: (event: ProbeLlmEvent) => void;
  readonly finish: (result?: GeminiCompleteResult) => void;
} {
  let textOpen = false;
  let sawText = false;
  let lastToolCallLine = "";

  const closeText = () => {
    if (textOpen) {
      process.stdout.write("\n");
      textOpen = false;
    }
  };

  return {
    onEvent: (event) => {
      if (event.type === "text-delta") {
        if (lastToolCallLine) {
          process.stdout.write("\n");
          lastToolCallLine = "";
        }
        if (!textOpen) {
          process.stdout.write(`${cliLabel(colors, "probe", "assistant")} `);
          textOpen = true;
        }

        sawText = true;
        process.stdout.write(formatInlineMarkdown(event.text));
        return;
      }

      if (event.type === "tool-call") {
        closeText();
        if (lastToolCallLine) {
          process.stdout.write("\n");
        }
        lastToolCallLine = cliToolLine(colors, "tool_call", event.name, safeJson(event.input), "call");
        process.stdout.write(lastToolCallLine);
        return;
      }

      if (event.type === "tool-result") {
        closeText();
        const resultLine = cliToolLine(colors, "tool_result", event.name, formatToolResultValue(event.result), "result");
        if (lastToolCallLine) {
          process.stdout.write(`\r${ansi.eraseLine}${lastToolCallLine} ${ansi.gray}→${ansi.reset} ${resultLine}\n`);
          lastToolCallLine = "";
        } else {
          process.stdout.write(`${resultLine}\n`);
        }
        return;
      }

      if (event.type === "tool-error") {
        closeText();
        const errorLine = cliToolLine(colors, "tool_error", event.name, event.message, "error");
        if (lastToolCallLine) {
          process.stdout.write(`\r${ansi.eraseLine}${lastToolCallLine} ${ansi.gray}→${ansi.reset} ${errorLine}\n`);
          lastToolCallLine = "";
        } else {
          process.stdout.write(`${errorLine}\n`);
        }
      }
    },
    finish: (result) => {
      if (lastToolCallLine) {
        process.stdout.write("\n");
        lastToolCallLine = "";
      }
      closeText();

      if (result === undefined) {
        return;
      }

      if (!sawText && result.text.length > 0) {
        process.stdout.write(`${cliLine(colors, "probe", renderMarkdown(result.text).trimEnd(), "assistant")}\n`);
      }

    },
  };
}

function formatToolResultValue(value: { readonly type: string; readonly value: unknown }): string {
  if (value.type === "error") {
    return String(value.value);
  }

  if (isReadFileToolResult(value.value)) {
    const r = value.value;
    return `${r.path}  (${r.content.length} chars)`;
  }

  if (isListFilesToolResult(value.value)) {
    const r = value.value;
    const parts: Array<string> = [];
    if (r.directories.length > 0) parts.push(`${r.directories.length} dirs`);
    if (r.files.length > 0) parts.push(`${r.files.length} files`);
    if (r.truncated) parts.push("truncated");
    return `${r.path}  ${parts.length > 0 ? parts.join(", ") : "empty"}`;
  }

  if (isSearchCodeToolResult(value.value)) {
    const r = value.value;
    const label = `${r.matches.length} match${r.matches.length === 1 ? "" : "es"}`;
    return `${r.query}  in  ${r.path}  (${label}${r.truncated ? ", truncated" : ""})`;
  }

  return safeJson(value.value);
}

function isReadFileToolResult(value: unknown): value is { readonly path: string; readonly content: string } {
  return (
    typeof value === "object" &&
    value !== null &&
    "path" in value &&
    typeof value.path === "string" &&
    "content" in value &&
    typeof value.content === "string"
  );
}

function isListFilesToolResult(value: unknown): value is {
  readonly path: string;
  readonly directories: ReadonlyArray<string>;
  readonly files: ReadonlyArray<string>;
  readonly truncated?: boolean;
} {
  return (
    typeof value === "object" &&
    value !== null &&
    "path" in value &&
    typeof value.path === "string" &&
    "directories" in value &&
    Array.isArray(value.directories) &&
    "files" in value &&
    Array.isArray(value.files)
  );
}

function isSearchCodeToolResult(value: unknown): value is {
  readonly query: string;
  readonly path: string;
  readonly matches: ReadonlyArray<string>;
  readonly truncated?: boolean;
} {
  return (
    typeof value === "object" &&
    value !== null &&
    "query" in value &&
    typeof value.query === "string" &&
    "path" in value &&
    typeof value.path === "string" &&
    "matches" in value &&
    Array.isArray(value.matches)
  );
}

function safeJson(value: unknown): string {
  try {
    return JSON.stringify(value) ?? String(value);
  } catch {
    return String(value);
  }
}

if (import.meta.main) {
  const argv = Bun.argv.slice(2);

  if (argv[0] === "chat" && stringOption(parseOptions(argv.slice(1)), "prompt") === undefined) {
    const chatArgs = argv.slice(1);
    const chatOptions = parseOptions(chatArgs);
    const useTui = chatOptions.tui === true && process.stdout.isTTY;
    process.exit(await runGeminiInteractiveChat(chatArgs, { colors: process.stdout.isTTY, env: Bun.env }, useTui));
  }

  const result = await Effect.runPromise(runProbeCli(argv, { colors: process.stdout.isTTY, env: Bun.env }));

  if (result.stdout.length > 0) {
    process.stdout.write(result.stdout);
  }

  if (result.stderr.length > 0) {
    process.stderr.write(result.stderr);
  }

  process.exit(result.exitCode);
}

async function runGeminiInteractiveChat(args: ReadonlyArray<string>, deps: ProbeCliDeps, useTui = false): Promise<number> {
  const options = parseOptions(args);
  const model = stringOption(options, "model") ?? GEMINI_DEFAULT_MODEL_ID;
  const maxTokens = numberOption(options, "max-tokens") ?? 65536;
  const tools = makeGeminiChatTools(deps.env);
  const colors = makeCliColors(options, deps);
  const clientResult = await Effect.runPromise(
    makeGeminiClient({
      profileId: stringOption(options, "profile") ?? deps.env?.PROBE_BACKEND_PROFILE ?? GEMINI_API_PROFILE_ID,
      explicitBaseUrl: stringOption(options, "base-url"),
      env: deps.env,
      fetch: deps.fetch,
      now: deps.now,
    }).pipe(Effect.catch((error) => Effect.succeed(error))),
  );

  if (clientResult instanceof GeminiClientError || "_tag" in clientResult) {
    const message = "reason" in clientResult ? clientResult.reason : String(clientResult);
    process.stderr.write(`${cliColor(colors, "error", message)}\n`);
    return 1;
  }

  process.stdout.write(
    `${cliField(colors, "profile", clientResult.profile.id)}  ${cliField(colors, "kind", clientResult.profile.kind)}  ${cliField(colors, "model", model)}  ${cliField(colors, "tools", "read_file,write_file,list_files,search_code,current_time", "tool")}\n`,
  );

  if (useTui) {
    return runGeminiTuiChat({ clientResult, colors, model, maxTokens, tools, deps, options });
  }

  const rl = createInterface({ input: process.stdin, output: process.stdout });
  let messages: ReadonlyArray<ProbeLlmMessage> = [];

  for (;;) {
    const rawPrompt = await readMultiLineInput(rl, cliColor(colors, "prompt", "> "));
    const prompt = rawPrompt.trim();

    if (prompt.length === 0) {
      continue;
    }

    if (prompt === "/exit" || prompt === "/quit") {
      rl.close();
      return 0;
    }

    const request = makeGeminiChatRequest({ messages, model, prompt, maxTokens, tools });
    const stream = makeGeminiInteractiveTurnStream(colors);
    const fiber = Effect.runFork(
      clientResult.complete({ request, tools, onEvent: stream.onEvent }).pipe(
        Effect.catch((error: GeminiClientError) => Effect.succeed(error)),
      ),
    );

    // Enable raw mode during streaming so we can detect Escape keypresses
    const wasRaw = process.stdin.isRaw;
    if (process.stdin.isTTY && typeof process.stdin.setRawMode === "function") {
      process.stdin.setRawMode(true);
    }

    let escapeCount = 0;
    let escapeTimer: ReturnType<typeof setTimeout> | undefined;

    const onKeyData = (chunk: Buffer) => {
      for (let i = 0; i < chunk.length; i++) {
        if (chunk[i] === 0x1b) {
          escapeCount++;
          if (escapeCount === 1) {
            process.stdout.write(`\n${cliColor(colors, "muted", "again to interrupt")}\n`);
            escapeTimer = setTimeout(() => {
              escapeCount = 0;
            }, 5000);
          } else if (escapeCount >= 2) {
            clearTimeout(escapeTimer);
            escapeTimer = undefined;
            fiber.interruptUnsafe();
          }
        } else if (escapeCount === 1) {
          escapeCount = 0;
          clearTimeout(escapeTimer);
          escapeTimer = undefined;
        }
      }
    };

    process.stdin.on("data", onKeyData);

    const result = await new Promise<GeminiCompleteResult | GeminiClientError | undefined>((resolve) => {
      fiber.addObserver((exit: Exit.Exit<GeminiCompleteResult | GeminiClientError, never>) => {
        if (Exit.isSuccess(exit)) {
          resolve(exit.value);
        } else if (exit.cause.reasons.some(Cause.isInterruptReason)) {
          resolve(undefined);
        } else {
          const fail = exit.cause.reasons.find(Cause.isFailReason);
          resolve(fail?.error ?? undefined);
        }
      });
    });

    process.stdin.removeListener("data", onKeyData);
    clearTimeout(escapeTimer);
    if (process.stdin.isTTY && typeof process.stdin.setRawMode === "function") {
      process.stdin.setRawMode(wasRaw ?? false);
    }

    if (result === undefined) {
      process.stdout.write(`${cliColor(colors, "muted", "interrupted")}\n`);
      continue;
    }

    if (result instanceof GeminiClientError) {
      stream.finish();
      process.stdout.write(formatGeminiFailure("Probe Gemini chat", result, colors));
      continue;
    }

    stream.finish(result);
    messages = [...result.finalRequest.messages, makeProbeLlmMessage("assistant", result.text)];
  }
}

async function runGeminiTuiChat(input: {
  readonly clientResult: GeminiClient;
  readonly colors: ProbeCliColors;
  readonly model: string;
  readonly maxTokens: number;
  readonly tools: ProbeLlmTools;
  readonly deps: ProbeCliDeps;
  readonly options: Record<string, string | true>;
}): Promise<number> {
  const { clientResult, model, maxTokens, tools } = input;
  const renderer = await createProbeRenderer();
  const session = new ScrollBoxRenderable(renderer, {
    scrollY: true,
    flexGrow: 1,
    width: "100%",
  });
  renderer.root.add(session);
  renderer.start();

  try {
    let messages: ReadonlyArray<ProbeLlmMessage> = [];

    for (;;) {
      renderer.suspend();
      const rl = createInterface({ input: process.stdin, output: process.stdout });
      const rawPrompt = await readMultiLineInput(rl, "> ");
      rl.close();
      renderer.resume();
      const prompt = rawPrompt.trim();

      if (prompt.length === 0) {
        continue;
      }

      if (prompt === "/exit" || prompt === "/quit") {
        return 0;
      }

      const assistantMd = createAssistantText(renderer);
      session.add(assistantMd);

      const request = makeGeminiChatRequest({ messages, model, prompt, maxTokens, tools });
      const onEvent = (event: ProbeLlmEvent) => {
        if (event.type === "text-delta") {
          assistantMd.content = (assistantMd.content ?? "") + event.text;
        }

        if (event.type === "reasoning-delta") {
          const label = new TextRenderable(renderer, {
            content: event.text,
            fg: parseColor("#8B949E"),
          });
          session.add(label);
        }

        if (event.type === "tool-call") {
          const label = new TextRenderable(renderer, {
            content: `[${event.name}]: ${safeJson(event.input)}`,
            fg: parseColor("#79C0FF"),
          });
          session.add(label);
        }

        if (event.type === "tool-result") {
          renderToolResultInSession(session, renderer, event.name, event.result);
        }

        if (event.type === "tool-error") {
          const errorLabel = new BoxRenderable(renderer, {
            border: true,
            borderType: "single",
            borderFg: parseColor("#FF7B72"),
            width: "100%",
          });
          const errorText = new TextRenderable(renderer, {
            content: `[${event.name}]: ${event.message}`,
            fg: parseColor("#FF7B72"),
          });
          errorLabel.add(errorText);
          session.add(errorLabel);
        }
      };
      const fiber = Effect.runFork(
        clientResult.complete({ request, tools, onEvent }).pipe(
          Effect.catch((error: GeminiClientError) => Effect.succeed(error)),
        ),
      );

      const result = await new Promise<GeminiCompleteResult | GeminiClientError | undefined>((resolve) => {
        fiber.addObserver((exit: Exit.Exit<GeminiCompleteResult | GeminiClientError, never>) => {
          if (Exit.isSuccess(exit)) {
            resolve(exit.value);
          } else if (exit.cause.reasons.some(Cause.isInterruptReason)) {
            resolve(undefined);
          } else {
            const fail = exit.cause.reasons.find(Cause.isFailReason);
            resolve(fail?.error ?? undefined);
          }
        });
      });

      if (result === undefined) {
        const label = new TextRenderable(renderer, {
          content: "[interrupted]",
          fg: parseColor("#8B949E"),
        });
        session.add(label);
        continue;
      }

      if (result instanceof GeminiClientError) {
        const errorLabel = new BoxRenderable(renderer, {
          border: true,
          borderType: "single",
          borderFg: parseColor("#FF7B72"),
          width: "100%",
        });
        const errorText = new TextRenderable(renderer, {
          content: `[error] ${result.reason}`,
          fg: parseColor("#FF7B72"),
        });
        errorLabel.add(errorText);
        session.add(errorLabel);
        continue;
      }

      messages = [...result.finalRequest.messages, makeProbeLlmMessage("assistant", result.text)];
    }
  } finally {
    renderer.destroy();
  }
}

function renderToolResultInSession(
  session: ScrollBoxRenderable,
  renderer: CliRenderer,
  name: string,
  result: { readonly type: string; readonly value: unknown },
): void {
  if (result.type === "error") {
    const errorText = new TextRenderable(renderer, {
      content: `[${name}]: ${String(result.value)}`,
      fg: parseColor("#FF7B72"),
    });
    session.add(errorText);
    return;
  }

  if (isReadFileToolResult(result.value)) {
    const r = result.value;
    const filetype = detectFiletype(r.path) ?? "plaintext";
    const header = new TextRenderable(renderer, {
      content: `[${name}]: ${r.path}  (${r.content.length} chars)`,
      fg: parseColor("#8B949E"),
    });
    session.add(header);
    const code = createCodeWithLineNumbers(renderer, r.content, filetype);
    session.add(code);
    return;
  }

  if (isListFilesToolResult(result.value)) {
    const r = result.value;
    const parts: Array<string> = [];
    if (r.directories.length > 0) parts.push(`${r.directories.length} dirs`);
    if (r.files.length > 0) parts.push(`${r.files.length} files`);
    if (r.truncated) parts.push("truncated");
    const summary = `${r.path}  ${parts.length > 0 ? parts.join(", ") : "empty"}`;
    const header = new TextRenderable(renderer, {
      content: `[${name}]: ${summary}`,
      fg: parseColor("#8B949E"),
    });
    session.add(header);

    const listing = [
      ...r.directories.map((d) => `  ${d}/`),
      ...r.files.map((f) => `  ${f}`),
    ].join("\n");
    if (listing.length > 0) {
      const code = createCodeWithLineNumbers(renderer, listing, "plaintext");
      session.add(code);
    }
    return;
  }

  if (isSearchCodeToolResult(result.value)) {
    const r = result.value;
    const label = `${r.matches.length} match${r.matches.length === 1 ? "" : "es"}`;
    const header = new TextRenderable(renderer, {
      content: `[${name}]: ${r.query}  in  ${r.path}  (${label}${r.truncated ? ", truncated" : ""})`,
      fg: parseColor("#8B949E"),
    });
    session.add(header);
    const matches = r.matches.join("\n");
    if (matches.length > 0) {
      const filetype = detectFiletype(r.path) ?? "plaintext";
      const code = createCodeWithLineNumbers(renderer, matches, filetype);
      session.add(code);
    }
    return;
  }

  const jsonText = safeJson(result.value);
  const header = new TextRenderable(renderer, {
    content: `[${name}]`,
    fg: parseColor("#8B949E"),
  });
  session.add(header);
  if (jsonText.length > 0 && jsonText !== "null" && jsonText !== "undefined") {
    const code = createCodeWithLineNumbers(renderer, jsonText, "plaintext");
    session.add(code);
  }
}

function readMultiLineInput(rl: Interface, prompt: string, debounceMs = 150): Promise<string> {
  const lines: string[] = [];
  process.stdout.write(prompt);

  return new Promise<string>((resolve) => {
    let timer: ReturnType<typeof setTimeout> | undefined;

    const onLine = (line: string) => {
      lines.push(line);
      clearTimeout(timer);
      timer = setTimeout(() => {
        rl.off("line", onLine);
        resolve(lines.join("\n"));
      }, debounceMs);
    };

    rl.on("line", onLine);
  });
}
