import {
  DEFAULT_DIAGNOSTIC_MAX_OUTPUT_TOKENS,
  DEFAULT_DIAGNOSTIC_REPEATS,
  DEFAULT_MODEL_ID,
  DEFAULT_OPENAGENTS_API_BASE,
  DEFAULT_RELEASE_API_BASE,
  DEFAULT_RELEASE_REPO,
  bootstrapInstalledPylon,
  ensureReleaseInstall,
  launchInstalledPylonWithUpdates,
  resolveBootstrapOutcome,
  resolvePlatformTarget,
  renderBootstrapSummary,
  runInstalledPylonCli,
} from "./index.js";
import {
  createTelemetryClient,
  detectPackageInvoker,
  installSourceForTelemetry,
  telemetryFailureContext,
} from "./telemetry.js";

function parseIntegerFlag(value, label) {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    throw new Error(`${label} must be a positive integer.`);
  }
  return parsed;
}

function createTerminalStyles(enableColor) {
  if (!enableColor) {
    return {
      bold: (value) => value,
      cyan: (value) => value,
      dim: (value) => value,
      green: (value) => value,
      red: (value) => value,
      yellow: (value) => value,
    };
  }

  const wrap = (open, close) => (value) => `${open}${value}${close}`;
  return {
    bold: wrap("\u001B[1m", "\u001B[22m"),
    cyan: wrap("\u001B[36m", "\u001B[39m"),
    dim: wrap("\u001B[2m", "\u001B[22m"),
    green: wrap("\u001B[32m", "\u001B[39m"),
    red: wrap("\u001B[31m", "\u001B[39m"),
    yellow: wrap("\u001B[33m", "\u001B[39m"),
  };
}

function createReporter({ enableColor = process.stdout.isTTY && !process.env.NO_COLOR } = {}) {
  const styles = createTerminalStyles(enableColor);
  return {
    status({ message, detail = null }) {
      const prefix = styles.cyan("›");
      const suffix = detail ? ` ${styles.dim(detail)}` : "";
      console.log(`${prefix} ${styles.bold(message)}${suffix}`);
    },
    success(message, detail = null) {
      const suffix = detail ? ` ${styles.dim(detail)}` : "";
      console.log(`${styles.green("✓")} ${styles.bold(message)}${suffix}`);
    },
    warning(message, detail = null) {
      const suffix = detail ? ` ${styles.dim(detail)}` : "";
      console.log(`${styles.yellow("!")} ${styles.bold(message)}${suffix}`);
    },
    failure(message, detail = null) {
      const suffix = detail ? ` ${styles.dim(detail)}` : "";
      console.error(`${styles.red("x")} ${styles.bold(message)}${suffix}`);
    },
  };
}

export function usage() {
  return `Usage:
  npx @openagentsinc/pylon [options]
  bunx @openagentsinc/pylon [options]
  pylon [options]
  pylon [options] <pylon-command> [pylon-options]
  pylon [options] -- <pylon-command> [pylon-options]

Description:
  Download the latest tagged standalone Pylon release asset for this machine,
  or a specific tagged Pylon version when --version is set. If no matching
  asset exists for the local platform, fetch the exact tagged source checkout
  and build it locally instead. Cache the binaries, run the first-run smoke
  path, and then start the Pylon terminal UI by default. The terminal UI manages
  the earning worker and keeps live status visible. The launcher checks GitHub
  for newer tagged pylon-v... releases on each default run and periodically
  while the dashboard is open. Only releases initiated by AtlantisPleb are
  accepted. New standalone binaries are cached under the local bootstrap root;
  the global npm or bun pylon command is not replaced.

  When a Pylon command is provided, the launcher bootstraps the managed release
  and forwards that command to the installed pylon binary instead of opening
  pylon-tui. For example: pylon status --json.

Options:
  --version <x.y.z>                    Resolve a specific Pylon release.
  --install-root <path>                Override the launcher cache/install root.
  --config-path <path>                 Override OPENAGENTS_PYLON_CONFIG_PATH.
  --pylon-home <path>                  Override OPENAGENTS_PYLON_HOME.
  --model <model-id>                   Model to diagnose, and optionally
                                       prefetch into the local GGUF cache.
                                       Default: ${DEFAULT_MODEL_ID}
  --download-curated-cache             Prefetch the optional Hugging Face GGUF
                                       cache before launching pylon.
  --run-diagnostics                    Run optional pylon gemma diagnose.
  --diagnostic-repeats <n>             Repeat count when diagnostics are enabled.
                                       Default: ${DEFAULT_DIAGNOSTIC_REPEATS}
  --diagnostic-max-output-tokens <n>   Max output tokens when diagnostics are enabled.
                                       Default: ${DEFAULT_DIAGNOSTIC_MAX_OUTPUT_TOKENS}
  --skip-model-download                Keep the curated GGUF cache skipped.
  --skip-diagnostics                   Keep optional pylon gemma diagnose skipped.
  --no-launch                          Do not start pylon-tui after bootstrap.
  --no-updates                         Disable background GitHub release polling
                                       and dashboard restart while pylon runs.
  --verbose                            Print extra network and recovery detail.
  --debug-network                      Alias for --verbose.
  --json                               Emit a machine-readable JSON summary.
  --register-openagents                Register and heartbeat this Pylon with
                                       OpenAgents after local first-run smoke.
                                       Requires OPENAGENTS_AGENT_TOKEN or
                                       --openagents-agent-token.
  --openagents-api <url>               OpenAgents API base for registration.
                                       Default: ${DEFAULT_OPENAGENTS_API_BASE}
  --openagents-agent-token <token>     Agent bearer token for registration.
                                       Prefer the environment variable.
  --pylon-ref <ref>                    Explicit public-safe Pylon ref.
  --pylon-display-name <name>          Public-safe Pylon display name.
  --resource-mode <mode>               Public resource mode for registration.
                                       Default: background_20
  --capability-ref <ref>               Add a public-safe capability ref.
                                       May be repeated or comma-separated.
  --setup-mdk-wallet                   Initialize or reuse the local MDK agent
                                       wallet, create receive readiness, and
                                       report redacted wallet/payout refs.
  --mdk-wallet-home <path>             HOME override for isolated MDK wallet
                                       config during tests or operator smokes.
  --mdk-wallet-port <port>             MDK agent-wallet daemon port override.
  --mdk-receive-amount-sats <amount>   Tiny receive amount for readiness.
                                       Default: 1 satoshi of bitcoin.

Test and maintainer options:
  --repo <owner/name>                  Override the GitHub release repo.
  --api-base <url>                     Override the GitHub API base URL.
  -h, --help                           Show this help text.
`;
}

export function parseArgs(argv) {
  const options = {
    version: null,
    repo: DEFAULT_RELEASE_REPO,
    apiBase: DEFAULT_RELEASE_API_BASE,
    installRoot: null,
    configPath: null,
    pylonHome: null,
    model: DEFAULT_MODEL_ID,
    diagnosticRepeats: DEFAULT_DIAGNOSTIC_REPEATS,
    diagnosticMaxOutputTokens: DEFAULT_DIAGNOSTIC_MAX_OUTPUT_TOKENS,
    skipModelDownload: true,
    skipDiagnostics: true,
    noLaunch: false,
    noUpdates: false,
    verbose: false,
    json: false,
    help: false,
    pylonArgs: [],
    openAgentsRegister: false,
    openAgentsApiBase: DEFAULT_OPENAGENTS_API_BASE,
    openAgentsAgentToken: null,
    openAgentsPylonRef: null,
    openAgentsPylonDisplayName: null,
    openAgentsResourceMode: "background_20",
    openAgentsCapabilityRefs: [],
    openAgentsSetupMdkWallet: false,
    openAgentsMdkWalletHome: null,
    openAgentsMdkWalletPort: null,
    openAgentsMdkReceiveAmountSats: 1,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--") {
      options.pylonArgs = argv.slice(index + 1);
      break;
    }
    if (!arg.startsWith("-")) {
      options.pylonArgs = argv.slice(index);
      break;
    }
    switch (arg) {
      case "--version":
        options.version = argv[++index];
        if (!options.version) {
          throw new Error("--version requires a value.");
        }
        break;
      case "--install-root":
        options.installRoot = argv[++index];
        if (!options.installRoot) {
          throw new Error("--install-root requires a value.");
        }
        break;
      case "--config-path":
        options.configPath = argv[++index];
        if (!options.configPath) {
          throw new Error("--config-path requires a value.");
        }
        break;
      case "--pylon-home":
        options.pylonHome = argv[++index];
        if (!options.pylonHome) {
          throw new Error("--pylon-home requires a value.");
        }
        break;
      case "--model":
        options.model = argv[++index];
        if (!options.model) {
          throw new Error("--model requires a value.");
        }
        break;
      case "--download-curated-cache":
        options.skipModelDownload = false;
        break;
      case "--run-diagnostics":
        options.skipDiagnostics = false;
        break;
      case "--diagnostic-repeats":
        options.diagnosticRepeats = parseIntegerFlag(
          argv[++index],
          "--diagnostic-repeats",
        );
        break;
      case "--diagnostic-max-output-tokens":
        options.diagnosticMaxOutputTokens = parseIntegerFlag(
          argv[++index],
          "--diagnostic-max-output-tokens",
        );
        break;
      case "--skip-model-download":
        options.skipModelDownload = true;
        break;
      case "--skip-diagnostics":
        options.skipDiagnostics = true;
        break;
      case "--no-launch":
        options.noLaunch = true;
        break;
      case "--no-updates":
        options.noUpdates = true;
        break;
      case "--verbose":
      case "--debug-network":
        options.verbose = true;
        break;
      case "--json":
        options.json = true;
        break;
      case "--register-openagents":
        options.openAgentsRegister = true;
        break;
      case "--openagents-api":
        options.openAgentsApiBase = argv[++index];
        if (!options.openAgentsApiBase) {
          throw new Error("--openagents-api requires a value.");
        }
        break;
      case "--openagents-agent-token":
        options.openAgentsAgentToken = argv[++index];
        if (!options.openAgentsAgentToken) {
          throw new Error("--openagents-agent-token requires a value.");
        }
        break;
      case "--pylon-ref":
        options.openAgentsPylonRef = argv[++index];
        if (!options.openAgentsPylonRef) {
          throw new Error("--pylon-ref requires a value.");
        }
        break;
      case "--pylon-display-name":
        options.openAgentsPylonDisplayName = argv[++index];
        if (!options.openAgentsPylonDisplayName) {
          throw new Error("--pylon-display-name requires a value.");
        }
        break;
      case "--resource-mode":
        options.openAgentsResourceMode = argv[++index];
        if (!options.openAgentsResourceMode) {
          throw new Error("--resource-mode requires a value.");
        }
        break;
      case "--capability-ref": {
        const value = argv[++index];
        if (!value) {
          throw new Error("--capability-ref requires a value.");
        }
        options.openAgentsCapabilityRefs.push(
          ...value
            .split(",")
            .map((entry) => entry.trim())
            .filter(Boolean),
        );
        break;
      }
      case "--setup-mdk-wallet":
        options.openAgentsSetupMdkWallet = true;
        break;
      case "--mdk-wallet-home":
        options.openAgentsMdkWalletHome = argv[++index];
        if (!options.openAgentsMdkWalletHome) {
          throw new Error("--mdk-wallet-home requires a value.");
        }
        break;
      case "--mdk-wallet-port":
        options.openAgentsMdkWalletPort = parseIntegerFlag(
          argv[++index],
          "--mdk-wallet-port",
        );
        break;
      case "--mdk-receive-amount-sats":
        options.openAgentsMdkReceiveAmountSats = parseIntegerFlag(
          argv[++index],
          "--mdk-receive-amount-sats",
        );
        break;
      case "--repo":
        options.repo = argv[++index];
        if (!options.repo) {
          throw new Error("--repo requires a value.");
        }
        break;
      case "--api-base":
        options.apiBase = argv[++index];
        if (!options.apiBase) {
          throw new Error("--api-base requires a value.");
        }
        break;
      case "-h":
      case "--help":
        options.help = true;
        break;
      default:
        throw new Error(`Unknown argument: ${arg}`);
    }
  }

  return options;
}

export async function main(argv = process.argv.slice(2), dependencies = {}) {
  const {
    ensureReleaseInstallImpl = ensureReleaseInstall,
    bootstrapInstalledPylonImpl = bootstrapInstalledPylon,
    launchInstalledPylonImpl = launchInstalledPylonWithUpdates,
    runInstalledPylonCliImpl = runInstalledPylonCli,
    createTelemetryClientImpl = createTelemetryClient,
  } = dependencies;
  const options = parseArgs(argv);
  if (options.help) {
    console.log(usage());
    return null;
  }

  const reporter = options.json ? null : createReporter();
  const startedAt = Date.now();
  const target = (() => {
    try {
      return resolvePlatformTarget(options.platform, options.arch);
    } catch {
      return {
        os: process.platform,
        arch: process.arch,
      };
    }
  })();
  const telemetryClient =
    dependencies.telemetryClient ??
    createTelemetryClientImpl({
      fetchImpl: dependencies.fetchImpl ?? globalThis.fetch,
    });
  const sharedTelemetry = {
    requested_version: options.version ?? "latest",
    os: target.os,
    arch: target.arch,
    platform_key: `${target.os}-${target.arch}`,
    npm_or_bun_invoker: detectPackageInvoker(),
  };

  telemetryClient?.emit?.("installer_started", sharedTelemetry);

  let install = null;

  try {
    install = await ensureReleaseInstallImpl(options, {
      ...dependencies,
      onStatus: reporter?.status,
      telemetryClient,
    });
    const summary = await bootstrapInstalledPylonImpl(
      {
        ...options,
        ...install,
        version: install.version,
      },
      {
        ...dependencies,
        onStatus: reporter?.status,
        telemetryClient,
      },
    );

    telemetryClient?.emit?.("installer_finished", {
      ...sharedTelemetry,
      release_tag: summary.tagName,
      release_commit: install.sourceCommit ?? null,
      duration_ms: Date.now() - startedAt,
      result: "success",
      install_source: installSourceForTelemetry(
        summary.installMethod ?? install.installMethod,
        Boolean(summary.cached),
      ),
    });
    await telemetryClient?.flush?.();

    if (options.json) {
      console.log(JSON.stringify(summary, null, 2));
    } else {
      const outcome = resolveBootstrapOutcome(summary);
      if (outcome.level === "success") {
        reporter?.success(`Pylon ${outcome.verdict}`, outcome.detail);
      } else {
        reporter?.warning(`Pylon ${outcome.verdict}`, outcome.detail);
      }
      console.log(renderBootstrapSummary(summary));
      if (options.pylonArgs.length > 0) {
        await runInstalledPylonCliImpl(
          {
            ...options,
            ...install,
            version: install.version,
          },
          options.pylonArgs,
          {
            ...dependencies,
            onStatus: reporter?.status,
            telemetryClient,
          },
        );
      } else if (!options.noLaunch) {
        await launchInstalledPylonImpl(
          {
            ...options,
            ...install,
            version: install.version,
            pinnedVersion: Boolean(options.version),
          },
          {
            ...dependencies,
            onStatus: reporter?.status,
            telemetryClient,
          },
        );
      } else {
        reporter?.warning(
          "Skipped Pylon terminal UI launch",
          "pass no flag to open pylon-tui and start the earning worker",
        );
      }
    }
    return summary;
  } catch (error) {
    telemetryClient?.emit?.("installer_finished", {
      ...sharedTelemetry,
      release_tag: install?.tagName ?? null,
      release_commit: install?.sourceCommit ?? null,
      duration_ms: Date.now() - startedAt,
      result: "failed",
      install_source: install
        ? installSourceForTelemetry(install.installMethod, Boolean(install.cached))
        : null,
      ...telemetryFailureContext(error, "launcher"),
    });
    await telemetryClient?.flush?.();
    throw error;
  }
}
