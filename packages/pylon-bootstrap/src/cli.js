import {
  DEFAULT_DIAGNOSTIC_MAX_OUTPUT_TOKENS,
  DEFAULT_DIAGNOSTIC_REPEATS,
  DEFAULT_MODEL_ID,
  DEFAULT_RELEASE_API_BASE,
  DEFAULT_RELEASE_REPO,
  bootstrapInstalledPylon,
  ensureReleaseInstall,
  launchInstalledPylonTui,
  resolveBootstrapOutcome,
  resolvePlatformTarget,
  renderBootstrapSummary,
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

Description:
  Download the latest tagged standalone Pylon release asset for this machine,
  or a specific tagged Pylon version when --version is set. If no matching
  asset exists for the local platform, fetch the exact tagged source checkout
  and build it locally instead. Cache the binaries, run the first-run smoke
  path, and then open the Pylon terminal UI by default with live status
  updates. The launcher checks GitHub for newer tagged pylon-v... releases on
  each default run, but only caches the standalone binaries under the local
  bootstrap root; it does not replace your global npm or bun pylon command.

Options:
  --version <x.y.z>                    Resolve a specific Pylon release.
  --install-root <path>                Override the launcher cache/install root.
  --config-path <path>                 Override OPENAGENTS_PYLON_CONFIG_PATH.
  --pylon-home <path>                  Override OPENAGENTS_PYLON_HOME.
  --model <model-id>                   Model to diagnose, and optionally
                                       prefetch into the local GGUF cache.
                                       Default: ${DEFAULT_MODEL_ID}
  --download-curated-cache             Prefetch the optional Hugging Face GGUF
                                       cache before opening the TUI.
  --diagnostic-repeats <n>             Repeat count for pylon gemma diagnose.
                                       Default: ${DEFAULT_DIAGNOSTIC_REPEATS}
  --diagnostic-max-output-tokens <n>   Max output tokens for diagnostics.
                                       Default: ${DEFAULT_DIAGNOSTIC_MAX_OUTPUT_TOKENS}
  --skip-model-download                Keep the curated GGUF cache skipped.
  --skip-diagnostics                   Skip pylon gemma diagnose.
  --no-launch                          Do not open pylon-tui after bootstrap.
  --verbose                            Print extra network and recovery detail.
  --debug-network                      Alias for --verbose.
  --json                               Emit a machine-readable JSON summary.

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
    skipDiagnostics: false,
    noLaunch: false,
    verbose: false,
    json: false,
    help: false,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
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
      case "--verbose":
      case "--debug-network":
        options.verbose = true;
        break;
      case "--json":
        options.json = true;
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
    launchInstalledPylonTuiImpl = launchInstalledPylonTui,
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
      if (!options.noLaunch) {
        await launchInstalledPylonTuiImpl(
          {
            ...options,
            ...install,
            version: install.version,
          },
          {
            ...dependencies,
            onStatus: reporter?.status,
          },
        );
      } else {
        reporter?.warning(
          "Skipped Pylon terminal UI launch",
          "pass no flag to open pylon-tui by default",
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
