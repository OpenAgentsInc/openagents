#!/usr/bin/env bun
import { Command, Options } from "@effect/cli";
import * as BunContext from "@effect/platform-bun/BunContext";
import * as BunRuntime from "@effect/platform-bun/BunRuntime";
import { Console, Effect } from "effect";
import { importBeadsIssues, BeadsImportError } from "../tasks/beads.js";

const beadsPathOption = Options.text("beads-path").pipe(Options.withDefault(".beads/issues.jsonl"));
const tasksPathOption = Options.text("tasks-path").pipe(Options.withDefault(".openagents/tasks.jsonl"));

const importCommand = Command.make(
  "beads-import",
  { beadsPath: beadsPathOption, tasksPath: tasksPathOption },
  ({ beadsPath, tasksPath }) =>
    Effect.gen(function* () {
      const result = yield* importBeadsIssues(beadsPath, tasksPath);
      yield* Console.log(`Imported ${result.count} issues to ${result.tasksPath}`);
    }),
);

const cli = Command.run(importCommand, {
  name: "openagents",
  version: "0.1.0",
});

cli(process.argv).pipe(
  Effect.catchAll((error) =>
    Console.error(
      error instanceof BeadsImportError || error instanceof Error ? error.message : String(error),
    ),
  ),
  Effect.provide(BunContext.layer),
  BunRuntime.runMain,
);
