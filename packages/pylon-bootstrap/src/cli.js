import {
  DEFAULT_DIAGNOSTIC_MAX_OUTPUT_TOKENS,
  DEFAULT_DIAGNOSTIC_REPEATS,
  DEFAULT_MODEL_ID,
  DEFAULT_RELEASE_API_BASE,
  DEFAULT_RELEASE_REPO,
  bootstrapInstalledPylon,
  ensureReleaseInstall,
  renderBootstrapSummary,
} from "./index.js";

function parseIntegerFlag(value, label) {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    throw new Error(`${label} must be a positive integer.`);
  }
  return parsed;
}

export function usage() {
  return `Usage:
  npx @openagentsinc/pylon [options]

Description:
  Download the latest tagged standalone Pylon release asset for this machine,
  or a specific tagged Pylon version when --version is set. Verify its
  checksum, cache the binaries locally, and run the first-run smoke path.

Options:
  --version <x.y.z>                    Resolve a specific Pylon release.
  --install-root <path>                Override the launcher cache/install root.
  --config-path <path>                 Override OPENAGENTS_PYLON_CONFIG_PATH.
  --pylon-home <path>                  Override OPENAGENTS_PYLON_HOME.
  --model <model-id>                   Model to download and diagnose.
                                       Default: ${DEFAULT_MODEL_ID}
  --diagnostic-repeats <n>             Repeat count for pylon gemma diagnose.
                                       Default: ${DEFAULT_DIAGNOSTIC_REPEATS}
  --diagnostic-max-output-tokens <n>   Max output tokens for diagnostics.
                                       Default: ${DEFAULT_DIAGNOSTIC_MAX_OUTPUT_TOKENS}
  --skip-model-download                Skip pylon gemma download.
  --skip-diagnostics                   Skip pylon gemma diagnose.
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
    skipModelDownload: false,
    skipDiagnostics: false,
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
  const options = parseArgs(argv);
  if (options.help) {
    console.log(usage());
    return null;
  }

  const install = await ensureReleaseInstall(options, dependencies);
  const summary = await bootstrapInstalledPylon(
    {
      ...options,
      ...install,
      version: install.version,
    },
    dependencies,
  );

  if (options.json) {
    console.log(JSON.stringify(summary, null, 2));
  } else {
    console.log(renderBootstrapSummary(summary));
  }
  return summary;
}
