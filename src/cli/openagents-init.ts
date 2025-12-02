#!/usr/bin/env bun
import { Command, Options } from "@effect/cli";
import * as BunContext from "@effect/platform-bun/BunContext";
import * as BunRuntime from "@effect/platform-bun/BunRuntime";
import { Console, Effect, Option } from "effect";
import { initOpenAgentsProject, InitProjectError } from "../tasks/init.js";

const dirOption = Options.text("dir").pipe(Options.withDefault("."));
const projectIdOption = Options.text("project-id").pipe(Options.optional);
const allowExistingOption = Options.boolean("allow-existing").pipe(Options.withDefault(false));

const initCommand = Command.make(
  "init",
  {
    dir: dirOption,
    projectId: projectIdOption,
    allowExisting: allowExistingOption,
  },
  ({ dir, projectId, allowExisting }) =>
    Effect.gen(function* () {
      const maybeProjectId = Option.getOrUndefined(projectId);
      const initArgs: { rootDir: string; allowExisting: boolean; projectId?: string } = {
        rootDir: dir,
        allowExisting,
      };
      if (maybeProjectId !== undefined) {
        initArgs.projectId = maybeProjectId;
      }

      const result = yield* initOpenAgentsProject(initArgs);

      yield* Console.log("Initialized .openagents:");
      yield* Console.log(`  projectId: ${result.projectId}`);
      yield* Console.log(`  project.json: ${result.projectPath}`);
      yield* Console.log(`  tasks.jsonl: ${result.tasksPath}`);
    }),
);

const cli = Command.run(initCommand, {
  name: "openagents",
  version: "0.1.0",
});

cli(process.argv).pipe(
  Effect.catchAll((error) =>
    Console.error(
      error instanceof InitProjectError || error instanceof Error ? error.message : String(error),
    ),
  ),
  Effect.provide(BunContext.layer),
  BunRuntime.runMain,
);
