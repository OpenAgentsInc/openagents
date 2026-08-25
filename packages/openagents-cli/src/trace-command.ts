/**
 * The `openagents trace` command family.
 *
 * This is the LOCAL half of the trace pipeline: list what session exports
 * exist on this machine, summarize one, and produce a redacted sibling copy.
 * The upload half needs a forge ingest route that does not exist yet, so
 * `trace upload` refuses with a typed error that names the missing route
 * instead of pretending.
 *
 * The family is defined through a factory taking the root command, so the
 * registration hunk in `cli.ts` stays a single import and a single list entry.
 */

import { existsSync } from "node:fs";
import { homedir } from "node:os";
import { isAbsolute, join, resolve } from "node:path";

import { Effect } from "effect";
import { Argument, Command, Flag } from "effect/unstable/cli";

import { InputError, TraceUploadUnsupported } from "./errors.js";
import { API_VERSION_PATH } from "./constants.js";
import { Output, type OutputMode } from "./output.js";
import {
  defaultDiscoveryBounds,
  defaultTraceStores,
  pathTraceStore,
  redactTraceFile,
  scanTraceStore,
  summarizeTraceFile,
  type TraceCandidate,
  type TraceStoreSpec,
  type TraceSummary,
} from "./trace-store.js";

/** The shared flags a trace handler reads back off the root command. */
interface SharedFlags {
  readonly json: boolean;
}

const outputMode = (json: boolean): OutputMode => (json ? "json" : "human");

/** The server half `trace upload` is waiting for. One place, one sentence. */
export const TRACE_INGEST_ROUTE_GAP =
  "openagents.com has no trace ingest route yet. Upload needs the server half first: " +
  `POST ${API_VERSION_PATH}/traces accepting an ATIF v1.7 document with owner_only default visibility. ` +
  "Until that route exists, this command refuses rather than pretending to upload.";

const listPathFlag = Flag.string("path").pipe(
  Flag.atLeast(0),
  Flag.withDescription(
    "Scan only this directory for trace documents; repeatable. Omit to scan the default stores.",
  ),
);
const listLimitFlag = Flag.integer("limit").pipe(
  Flag.withDefault(20),
  Flag.withDescription("Most files listed per store, newest first"),
);

const extraPathStores = (): ReadonlyArray<TraceStoreSpec> =>
  (process.env["OPENAGENTS_TRACE_PATHS"] ?? "")
    .split(":")
    .map((entry) => entry.trim())
    .filter((entry) => entry.length > 0)
    .map((entry) => pathTraceStore(resolve(entry)));

const candidateLine = (candidate: TraceCandidate): string =>
  `${candidate.kind}  ${candidate.modified_at}  ${candidate.bytes}B  ${candidate.path}`;

const summaryHuman = (summary: TraceSummary): ReadonlyArray<string> => {
  if (summary.format !== "atif") {
    return [
      `File: ${summary.path}`,
      `Format: ${summary.format === "jsonl" ? "line-delimited session log (not ATIF)" : "unknown"}`,
      `Size: ${summary.bytes} bytes`,
      ...(summary.lines === undefined ? [] : [`Lines: ${summary.lines}`]),
      "This slice summarizes ATIF documents only; foreign logs get metadata.",
    ];
  }
  const bySource = summary.steps_by_source ?? {};
  const tokens =
    summary.total_prompt_tokens === undefined && summary.total_completion_tokens === undefined
      ? "not recorded"
      : `${summary.total_prompt_tokens ?? 0} prompt, ${summary.total_completion_tokens ?? 0} completion`;
  return [
    `File: ${summary.path}`,
    `Schema: ${summary.schema_version ?? "(missing schema_version)"}`,
    ...(summary.session_id === undefined ? [] : [`Session: ${summary.session_id}`]),
    ...(summary.agent === undefined
      ? []
      : [`Agent: ${summary.agent.name ?? "unknown"} (${summary.agent.model ?? "unknown model"})`]),
    `Steps: ${summary.steps ?? 0} (${Object.entries(bySource)
      .map(([source, count]) => `${source} ${count}`)
      .join(", ")})`,
    `Models: ${(summary.models ?? []).join(", ") || "(none recorded)"}`,
    `Tool calls: ${summary.tool_calls ?? 0}`,
    `Tokens: ${tokens}`,
    ...(summary.first_timestamp === undefined || summary.last_timestamp === undefined
      ? []
      : [`Span: ${summary.first_timestamp} to ${summary.last_timestamp}`]),
  ];
};

/**
 * Resolve a `<path|id>` argument. A bare name that is not a file on disk is
 * tried as a file in the local export store, which is where `/export` writes.
 */
const resolveTraceArgument = Effect.fn("Trace.resolveTraceArgument")(function* (value: string) {
  const direct = isAbsolute(value) ? value : resolve(value);
  if (existsSync(direct)) return direct;
  const inExports = join(homedir(), ".openagents", "exports", value);
  if (!value.includes("/") && existsSync(inExports)) return inExports;
  return yield* new InputError({
    message: `No trace file exists at ${value}, and ~/.openagents/exports has no file by that name. Run openagents trace list to see what is discoverable.`,
  });
});

export const makeTraceCommand = <R>(root: Effect.Effect<SharedFlags, never, R>) => {
  const traceListCommand = Command.make(
    "list",
    { path: listPathFlag, limit: listLimitFlag },
    ({ limit, path }) =>
      Effect.gen(function* () {
        if (limit <= 0) {
          return yield* new InputError({ message: "--limit must be greater than zero." });
        }
        const flags = yield* root;
        const output = yield* Output;
        const stores =
          path.length > 0
            ? path.map((entry) => pathTraceStore(resolve(entry)))
            : [...defaultTraceStores(homedir()), ...extraPathStores()];
        const bounds = { ...defaultDiscoveryBounds, maxFilesPerStore: limit };
        const results = yield* Effect.sync(() =>
          stores.map((store) => scanTraceStore(store, bounds)),
        );
        const scans = results.map((result) => result.scan);
        const traces = results
          .flatMap((result) => result.candidates)
          .sort((a, b) => b.modified_at.localeCompare(a.modified_at));
        yield* output.write(
          {
            value: { schema: "openagents.trace_list.v1", stores: scans, traces },
            human: [
              ...scans.map(
                (scan) =>
                  `${scan.kind}: ${scan.root} ${
                    scan.present
                      ? `(${scan.matched} matched, ${scan.listed} listed${
                          scan.skipped_symlinks > 0
                            ? `, ${scan.skipped_symlinks} symlinks skipped`
                            : ""
                        }${scan.truncated ? ", scan truncated at its entry budget" : ""})`
                      : "(not present)"
                  }`,
              ),
              ...(traces.length === 0 ? ["No trace files found."] : traces.map(candidateLine)),
            ],
          },
          outputMode(flags.json),
        );
      }),
  ).pipe(
    Command.withDescription(
      "Discover local coding-agent session exports: the OpenAgents export store plus known Claude and Codex session directories. Read-only, bounded, and symlink-safe; foreign stores are listed as metadata only.",
    ),
  );

  const traceArgument = Argument.string("trace").pipe(
    Argument.withDescription("A trace file path, or a file name inside ~/.openagents/exports"),
  );

  const traceShowCommand = Command.make("show", { trace: traceArgument }, ({ trace }) =>
    Effect.gen(function* () {
      const flags = yield* root;
      const output = yield* Output;
      const path = yield* resolveTraceArgument(trace);
      const summary = yield* Effect.try({
        try: () => summarizeTraceFile(path),
        catch: () => new InputError({ message: `The trace file at ${path} could not be read.` }),
      });
      yield* output.write(
        {
          value: { schema: "openagents.trace_summary.v1", ...summary },
          human: summaryHuman(summary),
        },
        outputMode(flags.json),
      );
    }),
  ).pipe(
    Command.withDescription(
      "Summarize one trace: steps, sources, models, tool calls, and token totals, without printing any payload.",
    ),
  );

  const traceRedactCommand = Command.make("redact", { trace: traceArgument }, ({ trace }) =>
    Effect.gen(function* () {
      const flags = yield* root;
      const output = yield* Output;
      const path = yield* resolveTraceArgument(trace);
      if (path.endsWith(".redacted.json") || path.endsWith(".redacted.jsonl")) {
        return yield* new InputError({
          message: `${path} is already a redacted copy; redact the original instead.`,
        });
      }
      const result = yield* Effect.try({
        try: () => redactTraceFile(path, homedir()),
        catch: () =>
          new InputError({ message: `The trace file at ${path} could not be redacted.` }),
      });
      yield* output.write(
        {
          value: { schema: "openagents.trace_redaction.v1", ...result },
          human: [
            `Wrote ${result.output}`,
            result.total === 0
              ? "Nothing matched the redaction rules."
              : `Redacted ${result.total} match${result.total === 1 ? "" : "es"}: ${Object.entries(
                  result.counts,
                )
                  .map(([category, count]) => `${category} ${count}`)
                  .join(", ")}`,
            ...(result.valid_json === false
              ? ["Warning: the redacted copy no longer parses as JSON; review it before sharing."]
              : []),
          ],
        },
        outputMode(flags.json),
      );
    }),
  ).pipe(
    Command.withDescription(
      "Write a conservatively redacted sibling copy (.redacted.json): bearer and API tokens, JWTs, secret-named fields, environment-variable values, and home paths are removed. Only counts are reported; the matched text is never echoed.",
    ),
  );

  const uploadPublicFlag = Flag.boolean("public").pipe(
    Flag.withDescription("Ask for public visibility instead of the owner_only default"),
  );
  const uploadUnlistedFlag = Flag.boolean("unlisted").pipe(
    Flag.withDescription("Ask for unlisted visibility instead of the owner_only default"),
  );

  const traceUploadCommand = Command.make(
    "upload",
    { trace: traceArgument, public: uploadPublicFlag, unlisted: uploadUnlistedFlag },
    ({ public: isPublic, trace, unlisted }) =>
      Effect.gen(function* () {
        if (isPublic && unlisted) {
          return yield* new InputError({ message: "Use either --public or --unlisted, not both." });
        }
        // Validate the local half first so the refusal is about the real gap,
        // not about a path typo the reader would rather hear about now.
        yield* resolveTraceArgument(trace);
        return yield* new TraceUploadUnsupported({ message: TRACE_INGEST_ROUTE_GAP });
      }),
  ).pipe(
    Command.withDescription(
      "Upload a redacted trace to openagents.com with owner_only visibility by default. The forge ingest route does not exist yet, so this refuses and names the missing server half.",
    ),
  );

  return Command.make("trace").pipe(
    Command.withDescription(
      "Discover, inspect, and redact local coding-agent traces (ATIF). Discovery is read-only; nothing here rewrites or deletes a source session record.",
    ),
    Command.withSubcommands([
      traceListCommand,
      traceShowCommand,
      traceRedactCommand,
      traceUploadCommand,
    ]),
  );
};
