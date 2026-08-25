/**
 * CLI command definitions for `openagents box`.
 */

import { Effect, Option } from "effect";
import { Argument, Command, Flag } from "effect/unstable/cli";

import { BoxClient, type BoxRecord, type BoxRunRecord, type BoxFanoutPlan } from "./box-client.js";
import { type EndpointOverrides, type Profile } from "./endpoint.js";
import { InputError } from "./errors.js";
import { Output, type OutputMode } from "./output.js";
import { resolveApiSession } from "./session.js";

interface SharedFlags {
  readonly profile: Option.Option<Profile>;
  readonly apiUrl: Option.Option<string>;
  readonly json: boolean;
  readonly noColor: boolean;
}

const endpointOverrides = (flags: {
  readonly profile: Option.Option<Profile>;
  readonly apiUrl: Option.Option<string>;
}): EndpointOverrides => ({ profile: flags.profile, apiUrl: flags.apiUrl });

const outputMode = (json: boolean): OutputMode => (json ? "json" : "human");

const conversationIdFlag = Flag.string("conversation").pipe(
  Flag.optional,
  Flag.withDescription("Conversation ID override"),
);

const boxIdArgument = Argument.string("box_id").pipe(
  Argument.withDescription("Box VM ID (e.g. bx_8bhkse3n)"),
);

const runIdArgument = Argument.string("run_id").pipe(
  Argument.withDescription("Box Run ID"),
);

const labelFlag = Flag.string("label").pipe(
  Flag.optional,
  Flag.withDescription("Optional label for the box"),
);

const timeoutFlag = Flag.integer("timeout").pipe(
  Flag.optional,
  Flag.withDescription("Timeout in seconds for command execution"),
);

const countFlag = Flag.integer("count").pipe(
  Flag.withDescription("Number of boxes to request"),
);

const budgetedFlag = Flag.boolean("budgeted").pipe(
  Flag.withDescription("Allow scaling up to budgeted limit (e.g. 10-15)"),
);

const labelsFlag = Flag.string("labels").pipe(
  Flag.optional,
  Flag.withDescription("Comma-separated list of labels for fanout boxes"),
);

const offsetFlag = Flag.integer("offset").pipe(
  Flag.optional,
  Flag.withDescription("Byte offset to start reading output from"),
);

const boxListHuman = (boxes: ReadonlyArray<BoxRecord>): ReadonlyArray<string> => {
  if (boxes.length === 0) return ["No boxes provisioned for this conversation."];
  const lines: string[] = ["BOX ID        STATE       SETUP     LABEL        CREATED"];
  for (const b of boxes) {
    const id = b.box_id.padEnd(13, " ");
    const state = b.state.padEnd(11, " ");
    const setup = b.setup_status.padEnd(9, " ");
    const label = (b.label ?? "-").padEnd(12, " ");
    const created = b.created_at;
    lines.push(`${id} ${state} ${setup} ${label} ${created}`);
  }
  return lines;
};

const boxViewHuman = (b: BoxRecord): ReadonlyArray<string> => [
  `Box ID:       ${b.box_id}`,
  `State:        ${b.state}`,
  `Setup Status: ${b.setup_status}`,
  `Label:        ${b.label ?? "-"}`,
  `Created:      ${b.created_at}`,
  ...(b.stopped_at ? [`Stopped:      ${b.stopped_at}`] : []),
];

const boxRunListHuman = (runs: ReadonlyArray<BoxRunRecord>): ReadonlyArray<string> => {
  if (runs.length === 0) return ["No runs recorded for this box."];
  const lines: string[] = ["RUN ID                               STATE      EXIT  COMMAND"];
  for (const r of runs) {
    const id = r.id.padEnd(36, " ");
    const state = r.state.padEnd(10, " ");
    const exit = (r.exit_status !== null && r.exit_status !== undefined ? String(r.exit_status) : "-").padEnd(5, " ");
    const cmd = r.command.length > 40 ? r.command.slice(0, 37) + "..." : r.command;
    lines.push(`${id} ${state} ${exit} ${cmd}`);
  }
  return lines;
};

const boxRunViewHuman = (r: BoxRunRecord): ReadonlyArray<string> => [
  `Run ID:       ${r.id}`,
  `Box ID:       ${r.box_id}`,
  `State:        ${r.state}`,
  `Command:      ${r.command}`,
  `Exit Status:  ${r.exit_status !== null && r.exit_status !== undefined ? String(r.exit_status) : "-"}`,
  `Timed Out:    ${r.timed_out ? "yes" : "no"}`,
  ...(r.failure_reason ? [`Failure:      ${r.failure_reason}`] : []),
  `Admitted:     ${r.admitted_at ?? "-"}`,
  `Dispatched:   ${r.dispatched_at ?? "-"}`,
  `Started:      ${r.started_at ?? "-"}`,
  `Finished:     ${r.finished_at ?? "-"}`,
];

const boxFanoutHuman = (plan: BoxFanoutPlan): ReadonlyArray<string> => [
  `Fanout Plan:  ${plan.id}`,
  `Requested:    ${String(plan.requested_count)} boxes (Budgeted: ${plan.budgeted ? "yes" : "no"})`,
  `Admitted:     ${String(plan.admitted.length)}`,
  ...plan.admitted.map((item) => `  [#${String(item.position)}] ${item.label} -> ${item.box_id ?? "allocating"} (${item.state})`),
  `Queued:       ${String(plan.queued.length)}`,
  ...plan.queued.map((item) => `  [#${String(item.position)}] ${item.label} (Reason: ${item.queue_reason ?? "waiting for capacity"})`),
];

export const makeBoxCommand = <R>(root: Effect.Effect<SharedFlags, never, R>) => {
  const boxListCommand = Command.make(
    "list",
    { conversation: conversationIdFlag },
    ({ conversation }) =>
      Effect.gen(function* () {
        const flags = yield* root;
        const session = yield* resolveApiSession(endpointOverrides(flags));
        const client = yield* BoxClient;
        const output = yield* Output;
        const boxes = yield* client.list({
          origin: session.endpoint.origin,
          token: session.token,
          ...(Option.isNone(conversation) ? {} : { conversationId: conversation.value }),
        });
        yield* output.write(
          {
            value: { boxes },
            human: boxListHuman(boxes),
          },
          outputMode(flags.json),
        );
      }),
  ).pipe(Command.withDescription("List active and recent Box VMs in a conversation"));

  const boxCreateCommand = Command.make(
    "create",
    { conversation: conversationIdFlag, label: labelFlag },
    ({ conversation, label }) =>
      Effect.gen(function* () {
        const flags = yield* root;
        const session = yield* resolveApiSession(endpointOverrides(flags));
        const client = yield* BoxClient;
        const output = yield* Output;
        const box = yield* client.create({
          origin: session.endpoint.origin,
          token: session.token,
          ...(Option.isNone(conversation) ? {} : { conversationId: conversation.value }),
          ...(Option.isNone(label) ? {} : { label: label.value }),
        });
        yield* output.write(
          {
            value: { box },
            human: [
              `Provisioned Box ${box.box_id} (state: ${box.state}, setup: ${box.setup_status}).`,
              ...(box.label ? [`Label: ${box.label}`] : []),
            ],
          },
          outputMode(flags.json),
        );
      }),
  ).pipe(Command.withDescription("Provision a new Box VM"));

  const boxViewCommand = Command.make(
    "view",
    { boxId: boxIdArgument, conversation: conversationIdFlag },
    ({ boxId, conversation }) =>
      Effect.gen(function* () {
        const flags = yield* root;
        const session = yield* resolveApiSession(endpointOverrides(flags));
        const client = yield* BoxClient;
        const output = yield* Output;
        const box = yield* client.view({
          origin: session.endpoint.origin,
          token: session.token,
          boxId,
          ...(Option.isNone(conversation) ? {} : { conversationId: conversation.value }),
        });
        yield* output.write(
          {
            value: { box },
            human: boxViewHuman(box),
          },
          outputMode(flags.json),
        );
      }),
  ).pipe(Command.withDescription("Inspect a Box VM's status and lifecycle"));

  const boxExecCommand = Command.make(
    "exec",
    {
      boxId: boxIdArgument,
      conversation: conversationIdFlag,
      timeout: timeoutFlag,
      command: Argument.string("command").pipe(
        Argument.withDescription("Command to execute"),
        Argument.variadic({ min: 1 }),
      ),
    },
    ({ boxId, command, conversation, timeout }) =>
      Effect.gen(function* () {
        const flags = yield* root;
        const session = yield* resolveApiSession(endpointOverrides(flags));
        const client = yield* BoxClient;
        const output = yield* Output;
        const cmdStr = command.join(" ");
        const result = yield* client.exec({
          origin: session.endpoint.origin,
          token: session.token,
          boxId,
          command: cmdStr,
          ...(Option.isNone(conversation) ? {} : { conversationId: conversation.value }),
          ...(Option.isNone(timeout) ? {} : { timeoutSeconds: timeout.value }),
        });
        yield* output.write(
          {
            value: { result },
            human: [
              ...(result.stdout ? [result.stdout.trimEnd()] : []),
              ...(result.stderr ? [`[STDERR] ${result.stderr.trimEnd()}`] : []),
              ...(result.timed_out ? ["[TIMED OUT]"] : []),
            ],
          },
          outputMode(flags.json),
        );
        if (result.exit_code !== 0) {
          process.exitCode = result.exit_code;
        }
      }),
  ).pipe(Command.withDescription("Execute a command synchronously on a Box VM"));

  const boxStopCommand = Command.make(
    "stop",
    { boxId: boxIdArgument, conversation: conversationIdFlag },
    ({ boxId, conversation }) =>
      Effect.gen(function* () {
        const flags = yield* root;
        const session = yield* resolveApiSession(endpointOverrides(flags));
        const client = yield* BoxClient;
        const output = yield* Output;
        const box = yield* client.stop({
          origin: session.endpoint.origin,
          token: session.token,
          boxId,
          ...(Option.isNone(conversation) ? {} : { conversationId: conversation.value }),
        });
        yield* output.write(
          {
            value: { box },
            human: [`Stopped Box ${box.box_id} (state: ${box.state}). Slot released.`],
          },
          outputMode(flags.json),
        );
      }),
  ).pipe(Command.withDescription("Stop and snapshot a Box VM to release capacity"));

  const boxRunCommand = Command.make(
    "run",
    {
      boxId: boxIdArgument,
      conversation: conversationIdFlag,
      command: Argument.string("command").pipe(
        Argument.withDescription("Command to execute as a background run"),
        Argument.variadic({ min: 1 }),
      ),
    },
    ({ boxId, command, conversation }) =>
      Effect.gen(function* () {
        const flags = yield* root;
        const session = yield* resolveApiSession(endpointOverrides(flags));
        const client = yield* BoxClient;
        const output = yield* Output;
        const cmdStr = command.join(" ");
        const run = yield* client.startRun({
          origin: session.endpoint.origin,
          token: session.token,
          boxId,
          command: cmdStr,
          ...(Option.isNone(conversation) ? {} : { conversationId: conversation.value }),
        });
        yield* output.write(
          {
            value: { run },
            human: [
              `Started background run ${run.id} on Box ${run.box_id}.`,
              `State: ${run.state}`,
              `Inspect with: openagents box runs view ${run.box_id} ${run.id}`,
            ],
          },
          outputMode(flags.json),
        );
      }),
  ).pipe(Command.withDescription("Start a durable background command run on a Box VM"));

  const boxRunsListCommand = Command.make(
    "list",
    { boxId: boxIdArgument, conversation: conversationIdFlag },
    ({ boxId, conversation }) =>
      Effect.gen(function* () {
        const flags = yield* root;
        const session = yield* resolveApiSession(endpointOverrides(flags));
        const client = yield* BoxClient;
        const output = yield* Output;
        const runs = yield* client.listRuns({
          origin: session.endpoint.origin,
          token: session.token,
          boxId,
          ...(Option.isNone(conversation) ? {} : { conversationId: conversation.value }),
        });
        yield* output.write(
          {
            value: { runs },
            human: boxRunListHuman(runs),
          },
          outputMode(flags.json),
        );
      }),
  ).pipe(Command.withDescription("List durable runs on a Box VM"));

  const boxRunsViewCommand = Command.make(
    "view",
    { boxId: boxIdArgument, runId: runIdArgument, conversation: conversationIdFlag },
    ({ boxId, conversation, runId }) =>
      Effect.gen(function* () {
        const flags = yield* root;
        const session = yield* resolveApiSession(endpointOverrides(flags));
        const client = yield* BoxClient;
        const output = yield* Output;
        const run = yield* client.viewRun({
          origin: session.endpoint.origin,
          token: session.token,
          boxId,
          runId,
          ...(Option.isNone(conversation) ? {} : { conversationId: conversation.value }),
        });
        yield* output.write(
          {
            value: { run },
            human: boxRunViewHuman(run),
          },
          outputMode(flags.json),
        );
      }),
  ).pipe(Command.withDescription("View details of a Box run"));

  const boxRunsOutputCommand = Command.make(
    "output",
    {
      boxId: boxIdArgument,
      runId: runIdArgument,
      conversation: conversationIdFlag,
      offset: offsetFlag,
    },
    ({ boxId, conversation, offset, runId }) =>
      Effect.gen(function* () {
        const flags = yield* root;
        const session = yield* resolveApiSession(endpointOverrides(flags));
        const client = yield* BoxClient;
        const output = yield* Output;
        const result = yield* client.runOutput({
          origin: session.endpoint.origin,
          token: session.token,
          boxId,
          runId,
          ...(Option.isNone(conversation) ? {} : { conversationId: conversation.value }),
          ...(Option.isNone(offset) ? {} : { offset: offset.value }),
        });
        yield* output.write(
          {
            value: result,
            human: [result.output],
          },
          outputMode(flags.json),
        );
      }),
  ).pipe(Command.withDescription("Read bounded output stream from a Box run"));

  const boxRunsCancelCommand = Command.make(
    "cancel",
    { boxId: boxIdArgument, runId: runIdArgument, conversation: conversationIdFlag },
    ({ boxId, conversation, runId }) =>
      Effect.gen(function* () {
        const flags = yield* root;
        const session = yield* resolveApiSession(endpointOverrides(flags));
        const client = yield* BoxClient;
        const output = yield* Output;
        const run = yield* client.cancelRun({
          origin: session.endpoint.origin,
          token: session.token,
          boxId,
          runId,
          ...(Option.isNone(conversation) ? {} : { conversationId: conversation.value }),
        });
        yield* output.write(
          {
            value: { run },
            human: [`Requested cancellation for run ${run.id} (state: ${run.state}).`],
          },
          outputMode(flags.json),
        );
      }),
  ).pipe(Command.withDescription("Cancel an active Box run"));

  const boxRunsGroupCommand = Command.make("runs").pipe(
    Command.withDescription("Manage durable runs on Box VMs"),
    Command.withSubcommands([
      boxRunsListCommand,
      boxRunsViewCommand,
      boxRunsOutputCommand,
      boxRunsCancelCommand,
    ]),
  );

  const boxFanoutCommand = Command.make(
    "fanout",
    {
      count: countFlag,
      labels: labelsFlag,
      budgeted: budgetedFlag,
      conversation: conversationIdFlag,
    },
    ({ budgeted, count, conversation, labels }) =>
      Effect.gen(function* () {
        if (count < 1) {
          return yield* new InputError({ message: "--count must be at least 1." });
        }
        const flags = yield* root;
        const session = yield* resolveApiSession(endpointOverrides(flags));
        const client = yield* BoxClient;
        const output = yield* Output;
        const parsedLabels = Option.isSome(labels)
          ? labels.value.split(",").map((s) => s.trim()).filter((s) => s.length > 0)
          : undefined;
        const plan = yield* client.fanout({
          origin: session.endpoint.origin,
          token: session.token,
          count,
          ...(parsedLabels ? { labels: parsedLabels } : {}),
          budgeted,
          ...(Option.isNone(conversation) ? {} : { conversationId: conversation.value }),
        });
        yield* output.write(
          {
            value: { plan },
            human: boxFanoutHuman(plan),
          },
          outputMode(flags.json),
        );
      }),
  ).pipe(Command.withDescription("Request multi-box fanout admission plan"));

  return Command.make("box").pipe(
    Command.withDescription("Manage conversation-owned Box cloud computer sandboxes"),
    Command.withSubcommands([
      boxListCommand,
      boxCreateCommand,
      boxViewCommand,
      boxExecCommand,
      boxStopCommand,
      boxRunCommand,
      boxRunsGroupCommand,
      boxFanoutCommand,
    ]),
  );
};
