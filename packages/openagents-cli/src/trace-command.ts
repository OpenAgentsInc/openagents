/**
 * The `openagents trace` command family.
 *
 * `list`, `show`, and `redact` are the local half: what session exports exist
 * on this machine, what one holds, and a redacted sibling copy of it. `upload`
 * is the remote half, and it now sends the document to `POST /api/v1/traces`
 * rather than refusing -- that route exists.
 *
 * The family is defined through a factory taking the root command, so the
 * registration hunk in `cli.ts` stays a single import and a single list entry.
 */

import { existsSync, readFileSync, statSync } from "node:fs";
import { homedir } from "node:os";
import { isAbsolute, join, resolve } from "node:path";

import { Effect, Option } from "effect";
import { Argument, Command, Flag } from "effect/unstable/cli";

import { type EndpointOverrides, type Profile } from "./endpoint.js";
import { InputError } from "./errors.js";
import { Output, type OutputMode } from "./output.js";
import { resolveApiSession } from "./session.js";
import {
  DEFAULT_TRACE_VISIBILITY,
  MAXIMUM_TRACE_BYTES,
  TRACE_VISIBILITIES,
  TraceClient,
  isTraceVisibility,
} from "./trace-client.js";
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
  readonly profile: Option.Option<Profile>;
  readonly apiUrl: Option.Option<string>;
  readonly json: boolean;
}

const outputMode = (json: boolean): OutputMode => (json ? "json" : "human");

const endpointOverrides = (flags: SharedFlags): EndpointOverrides => ({
  profile: flags.profile,
  apiUrl: flags.apiUrl,
});

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

  const uploadVisibilityFlag = Flag.string("visibility").pipe(
    Flag.withDefault(DEFAULT_TRACE_VISIBILITY as string),
    Flag.withDescription(
      `Transparency rung to store the trace at: ${TRACE_VISIBILITIES.join(", ")}. ` +
        "dark is nothing public, pulse is metadata only, ledger is content and metadata, glass is full access.",
    ),
  );
  const uploadAssignmentFlag = Flag.string("assignment").pipe(
    Flag.optional,
    Flag.withDescription("Bind the trace to the forge attempt with this id"),
  );

  const traceUploadCommand = Command.make(
    "upload",
    {
      trace: traceArgument,
      visibility: uploadVisibilityFlag,
      assignment: uploadAssignmentFlag,
    },
    ({ assignment, trace, visibility }) =>
      Effect.gen(function* () {
        if (!isTraceVisibility(visibility)) {
          return yield* new InputError({
            message: `--visibility must be one of ${TRACE_VISIBILITIES.join(", ")}; got ${visibility}.`,
          });
        }
        const flags = yield* root;
        const output = yield* Output;
        const path = yield* resolveTraceArgument(trace);

        // Everything that can be checked against the file is checked before
        // anything leaves this machine, so a refusal names the file rather than
        // arriving as a status from a server the caller then has to interpret.
        const size = yield* Effect.try({
          try: () => statSync(path).size,
          catch: () => new InputError({ message: `The trace file at ${path} could not be read.` }),
        });
        if (size > MAXIMUM_TRACE_BYTES) {
          return yield* new InputError({
            message: `${path} is ${size} bytes; the ingest route accepts at most ${MAXIMUM_TRACE_BYTES}. Upload a redacted or trimmed copy instead.`,
          });
        }

        const document = yield* Effect.try({
          try: () => JSON.parse(readFileSync(path, "utf8")) as unknown,
          catch: () =>
            new InputError({
              message: `${path} is not JSON. The ingest route takes one ATIF document; a line-delimited session log has to be converted first.`,
            }),
        });
        if (document === null || typeof document !== "object" || Array.isArray(document)) {
          return yield* new InputError({
            message: `${path} is JSON but not an object, so it is not an ATIF document.`,
          });
        }
        // The server decides which schema versions it accepts. This only
        // catches a file that names none at all, which it can say more usefully
        // about the file than a 422 can.
        if (typeof (document as Record<string, unknown>)["schema_version"] !== "string") {
          return yield* new InputError({
            message: `${path} carries no schema_version, so it is not an ATIF document. openagents trace show ${path} reports what it is.`,
          });
        }

        const session = yield* resolveApiSession(endpointOverrides(flags));
        const traces = yield* TraceClient;
        const stored = yield* traces.upload({
          origin: session.endpoint.origin,
          token: session.token,
          document,
          visibility,
          ...(Option.isNone(assignment) ? {} : { assignmentId: assignment.value }),
        });

        yield* output.write(
          {
            value: { schema: "openagents.trace_upload.v1", input: path, ...stored },
            human: [
              // A 200 means the server already held this digest. Calling that an
              // upload would report a write that did not happen.
              stored.created
                ? `Uploaded ${path}`
                : `Already stored: the server holds this trace under the same digest.`,
              `Trace: ${stored.id}`,
              `Digest: ${stored.digest}`,
              `Stored: ${stored.byte_size} bytes at visibility ${stored.visibility}`,
              // No link. The response carries a url pointing at
              // GET /api/v1/traces/:id, and that route does not exist, so
              // printing it would hand the reader a 404 dressed as a receipt.
            ],
          },
          outputMode(flags.json),
        );
      }),
  ).pipe(
    Command.withDescription(
      `Upload one ATIF document to openagents.com. Stored at ${DEFAULT_TRACE_VISIBILITY} -- nothing public -- unless --visibility names a higher rung. Redact the trace first: what is uploaded is the file as it stands.`,
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
