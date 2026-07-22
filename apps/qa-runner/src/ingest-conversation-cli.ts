// CLI: turn a local Claude / Codex / OpenAgents conversation into a public
// OpenAgents trace at /trace/{uuid} (issue: local-conversation -> public trace).
//
// Usage:
//   node --import tsx src/ingest-conversation-cli.ts <conversationId> [flags]
//
// Flags:
//   -s, --source <claude|codex|openagents|auto>  source kind (default: auto)
//   -v, --visibility <public|unlisted|owner_only> stored visibility (default: public)
//       --dry-run            build + redact + validate locally; do NOT upload
//       --out <file>         with --dry-run, write the redacted ATIF JSON here
//       --base-url <url>     ingest base (default: $OPENAGENTS_BASE_URL or openagents.com)
//       --token <oa_agent_…> agent bearer (default: $OPENAGENTS_AGENT_TOKEN)
//       --agent-name <name>  trajectory agent display name
//       --model <id>         fallback model id for sources that omit one
//       --home <dir>         override HOME for source lookup (testing)
//       --user-data <dir>    also probe this app userData dir for the openagents
//                            source (Full Auto host `threads.json` run threads)
//       --json               print the machine-readable result as JSON
//   -h, --help
//
// Pipeline: resolve id -> convert to ATIF-v1.7 -> DEEP-REDACT (the same
// TraceRedactor the ingest API trusts) -> local public-safety tripwire preflight
// -> POST /api/traces (agent bearer + Idempotency-Key). The stored trace is
// PUBLIC by default so /trace/{uuid} needs no auth. Evidence only: an ingested
// trace grants no accepted-work, payout, or public-claim authority.

import { readFileSync, writeFileSync } from "node:fs";
import { basename, resolve as resolvePath } from "node:path";

import { Effect } from "effect";

import {
  atifTraceTripwire,
  decodeAtifTrajectorySync,
  validateAtifTrajectory,
} from "@openagentsinc/atif/trace";

import { serializeTrajectory } from "./atif";
import { multiTranscriptToConversation } from "./multi-transcript-to-conversation";
import { convertOpenAgentsConversationToAtif } from "./openagents-conversation-to-atif";
import type { ConversationSourceKind } from "./conversation-source";
import { ConversationNotFoundError } from "./conversation-source";
import {
  buildTrajectoryFromConversationId,
  capTrajectorySteps,
  INGEST_MAX_STEPS,
} from "./ingest-conversation";
import {
  idempotencyKeyForTrajectory,
  type PublishTraceConfig,
  publishTrace,
  type TraceVisibility,
} from "./publish-trace";
import { redactValue } from "./redaction";

interface Args {
  readonly id: string | undefined;
  readonly source: ConversationSourceKind | "auto";
  readonly visibility: TraceVisibility;
  readonly dryRun: boolean;
  readonly out: string | undefined;
  readonly baseUrl: string | undefined;
  readonly token: string | undefined;
  readonly agentName: string | undefined;
  readonly model: string | undefined;
  readonly home: string | undefined;
  readonly userData: string | undefined;
  readonly file: string | undefined;
  readonly maxSteps: number;
  readonly json: boolean;
  readonly help: boolean;
}

const USAGE = `oa-trace-ingest — publish a local conversation as a public /trace/{uuid}

  node --import tsx src/ingest-conversation-cli.ts <conversationId> [flags]

  -s, --source <claude|codex|openagents|auto>   default: auto
  -v, --visibility <public|unlisted|owner_only> default: public
      --dry-run                 build + redact + validate; do NOT upload
      --out <file>              (dry-run) write the redacted ATIF JSON here
      --base-url <url>          default: $OPENAGENTS_BASE_URL or https://openagents.com
      --token <oa_agent_…>      default: $OPENAGENTS_AGENT_TOKEN
      --agent-name <name>       trajectory agent display name
      --model <id>              fallback model id
      --max-steps <n>           cap steps (default/hard-max 2000; keeps a prefix)
      --home <dir>              override HOME for source lookup (testing)
      --user-data <dir>         also probe this app userData dir for the
                                openagents source: a Full Auto isolated-host
                                'threads.json' run thread (or that dir's
                                KhalaDesktop/conversations.json)
      --file <path>             ingest a combined multi-harness transcript JSONL
                                (seven-lane/multi-harness live-smoke output)
      --json                    machine-readable JSON result
  -h, --help
`;

const parseArgs = (argv: ReadonlyArray<string>): Args => {
  let id: string | undefined;
  let source: ConversationSourceKind | "auto" = "auto";
  let visibility: TraceVisibility = "public";
  let dryRun = false;
  let out: string | undefined;
  let baseUrl: string | undefined;
  let token: string | undefined;
  let agentName: string | undefined;
  let model: string | undefined;
  let home: string | undefined;
  let userData: string | undefined;
  let file: string | undefined;
  let maxSteps = INGEST_MAX_STEPS;
  let json = false;
  let help = false;

  const next = (i: number): string => {
    const value = argv[i + 1];
    if (value === undefined) throw new Error(`missing value for ${argv[i]}`);
    return value;
  };

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    switch (arg) {
      case "-s":
      case "--source":
        source = next(i) as ConversationSourceKind | "auto";
        i += 1;
        break;
      case "-v":
      case "--visibility":
        visibility = next(i) as TraceVisibility;
        i += 1;
        break;
      case "--dry-run":
        dryRun = true;
        break;
      case "--out":
        out = next(i);
        i += 1;
        break;
      case "--base-url":
        baseUrl = next(i);
        i += 1;
        break;
      case "--token":
        token = next(i);
        i += 1;
        break;
      case "--agent-name":
        agentName = next(i);
        i += 1;
        break;
      case "--model":
        model = next(i);
        i += 1;
        break;
      case "--max-steps": {
        const parsed = Number.parseInt(next(i), 10);
        if (!Number.isFinite(parsed) || parsed <= 0) {
          throw new Error(`--max-steps must be a positive integer`);
        }
        maxSteps = Math.min(parsed, INGEST_MAX_STEPS);
        i += 1;
        break;
      }
      case "--home":
        home = next(i);
        i += 1;
        break;
      case "--user-data":
        userData = next(i);
        i += 1;
        break;
      case "--file":
        file = next(i);
        i += 1;
        break;
      case "--json":
        json = true;
        break;
      case "-h":
      case "--help":
        help = true;
        break;
      default:
        if (arg !== undefined && !arg.startsWith("-") && id === undefined) id = arg;
        else if (arg !== undefined) throw new Error(`unknown argument: ${arg}`);
    }
  }

  return {
    id,
    source,
    visibility,
    dryRun,
    out,
    baseUrl,
    token,
    agentName,
    model,
    home,
    userData,
    file,
    maxSteps,
    json,
    help,
  };
};

const resolveBaseUrl = (args: Args): string =>
  (
    args.baseUrl ??
    process.env.OPENAGENTS_BASE_URL ??
    process.env.PYLON_OPENAGENTS_BASE_URL ??
    "https://openagents.com"
  ).replace(/\/+$/, "");

const resolveToken = (args: Args): string | undefined =>
  (
    args.token ??
    process.env.OPENAGENTS_AGENT_TOKEN ??
    process.env.OPENAGENTS_AGENT_PENDING_TOKEN
  )?.trim() || undefined;

export async function runIngestConversationCli(
  argv: ReadonlyArray<string>,
  log: (message: string) => void = (m) => console.log(m),
): Promise<number> {
  let args: Args;
  try {
    args = parseArgs(argv);
  } catch (error) {
    log(error instanceof Error ? error.message : String(error));
    log(USAGE);
    return 2;
  }

  if (args.help) {
    log(USAGE);
    return 0;
  }
  if (args.id === undefined && args.file === undefined) {
    log("error: a conversation id (or --file) is required.\n");
    log(USAGE);
    return 2;
  }

  // 1. Resolve + convert.
  let built: ReturnType<typeof buildTrajectoryFromConversationId>;
  try {
    if (args.file !== undefined) {
      const filePath = resolvePath(args.file);
      const conversation = multiTranscriptToConversation(readFileSync(filePath, "utf8"), {
        id: args.id ?? basename(filePath, ".jsonl"),
      });
      built = {
        resolved: {
          kind: "openagents",
          id: conversation.id ?? "multi",
          path: filePath,
          conversation,
        },
        trajectory: convertOpenAgentsConversationToAtif(conversation, {
          agentName: args.agentName ?? "OpenAgents multi-harness",
          defaultModelName: args.model ?? "openagents/multi-harness",
        }),
      };
    } else if (args.id !== undefined) {
      built = buildTrajectoryFromConversationId(args.id, {
        kind: args.source,
        ...(args.agentName === undefined ? {} : { agentName: args.agentName }),
        ...(args.model === undefined ? {} : { defaultModelName: args.model }),
        ...(args.home === undefined ? {} : { home: args.home }),
        ...(args.userData === undefined ? {} : { userData: args.userData }),
      });
    } else {
      // The initial guard proves id-or-file; file was handled above.
      log("error: a conversation id (or --file) is required.\n");
      return 1;
    }
  } catch (error) {
    if (error instanceof ConversationNotFoundError) {
      log(error.message);
      return 1;
    }
    log(`error building trajectory: ${error instanceof Error ? error.message : String(error)}`);
    return 1;
  }

  // Cap to the ingest step limit (keeps a valid prefix; notes the truncation).
  const resolved = built.resolved;
  const trajectory = capTrajectorySteps(built.trajectory, args.maxSteps);

  // 2. Redact (belt-and-suspenders over the converter output; the ingest API
  //    redacts again, and this same redactor is what it trusts).
  const { value: redacted, report } = redactValue(trajectory);

  // 3. Local preflight: structural validation + public-safety tripwire on the
  //    STRICT schema, so we fail fast with clear findings instead of a 422.
  let strict;
  try {
    strict = decodeAtifTrajectorySync(redacted);
  } catch (error) {
    log(
      `error: redacted trajectory failed strict ATIF decode: ${error instanceof Error ? error.message : String(error)}`,
    );
    return 1;
  }
  const structuralIssues = validateAtifTrajectory(strict);
  const tripwireFindings = atifTraceTripwire(strict);

  const idempotencyKey = idempotencyKeyForTrajectory(redacted);
  const sourceLabel =
    resolved.kind === "openagents"
      ? `openagents (${resolved.path})`
      : `${resolved.kind} (${resolved.path})`;

  if (structuralIssues.length > 0) {
    log(`error: trajectory failed structural validation:`);
    for (const issue of structuralIssues) log(`  - [${issue.code}] ${issue.message}`);
    return 1;
  }
  if (tripwireFindings.length > 0) {
    log(`error: trajectory still trips the public-safety tripwire after redaction:`);
    for (const finding of tripwireFindings) log(`  - [${finding.code}] ${finding.message}`);
    log(`Inspect with --dry-run --out <file> and remove the offending content upstream.`);
    return 1;
  }

  // 4a. Dry run: emit the redacted, validated trajectory (the exact wire body,
  //     minus keys the strict schema ignores) and stop.
  if (args.dryRun) {
    const serialized = serializeTrajectory(redacted);
    if (args.out !== undefined) {
      writeFileSync(args.out, serialized);
      log(`[dry-run] wrote redacted ATIF (${strict.steps.length} steps) to ${args.out}`);
    } else {
      log(serialized);
    }
    if (args.json) {
      log(
        JSON.stringify({
          dryRun: true,
          source: resolved.kind,
          path: resolved.path,
          steps: strict.steps.length,
          idempotencyKey,
          redaction: report,
        }),
      );
    } else {
      log(
        `[dry-run] source=${sourceLabel} steps=${strict.steps.length} ` +
          `redactions=${report.total} — not uploaded.`,
      );
    }
    return 0;
  }

  // 4b. Publish.
  const token = resolveToken(args);
  if (token === undefined) {
    log(
      "error: no agent token. Set OPENAGENTS_AGENT_TOKEN (or pass --token oa_agent_…). " +
        "Use --dry-run to build + validate without a token.",
    );
    return 1;
  }
  const base = resolveBaseUrl(args);
  const config: PublishTraceConfig = {
    url: `${base}/api/traces`,
    token,
    visibility: args.visibility,
  };

  const result = await Effect.runPromise(
    publishTrace({
      trajectory,
      config,
      idempotencyKey,
      shareBaseUrl: base,
      log: () => {},
    }),
  );

  if (!result.published) {
    log(`error: publish failed (${result.kind}): ${result.reason}`);
    return 1;
  }

  if (args.json) {
    log(
      JSON.stringify({
        published: true,
        uuid: result.uuid,
        url: result.url,
        visibility: result.visibility,
        replay: result.replay,
        source: resolved.kind,
        steps: strict.steps.length,
        redaction: result.redaction,
      }),
    );
  } else {
    log(
      `published ${result.visibility} trace from ${resolved.kind} (${strict.steps.length} steps` +
        `${result.replay ? ", idempotent replay" : ""}):`,
    );
    log(`  ${result.url}`);
  }
  return 0;
}

// Entrypoint when run directly (tsx / node).
const invokedDirectly =
  typeof process !== "undefined" &&
  Array.isArray(process.argv) &&
  /ingest-conversation-cli\.[cm]?tsx?$/.test(process.argv[1] ?? "");

if (invokedDirectly) {
  runIngestConversationCli(process.argv.slice(2))
    .then((code) => {
      process.exitCode = code;
    })
    .catch((error) => {
      console.error(error instanceof Error ? error.message : String(error));
      process.exitCode = 1;
    });
}
