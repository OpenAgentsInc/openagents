import { Context, Effect, Layer, Redacted, Schema } from "effect";
import postgres, { type Sql } from "postgres";

import { ForgeGitConfiguration } from "./config.js";

export class ForgeGitDatabaseError extends Schema.TaggedErrorClass<ForgeGitDatabaseError>()(
  "ForgeGitDatabaseError",
  {
    operation: Schema.String,
    cause: Schema.Defect(),
  },
) {}

export interface ForgeGitDatabaseShape {
  readonly sql: Sql;
}

export class ForgeGitDatabase extends Context.Service<ForgeGitDatabase, ForgeGitDatabaseShape>()(
  "@openagentsinc/forge-git-service/Database",
) {}

export const layerDatabase = Layer.effect(
  ForgeGitDatabase,
  Effect.gen(function* () {
    const configuration = yield* ForgeGitConfiguration;
    const sql = yield* Effect.acquireRelease(
      Effect.sync(() =>
        postgres(Redacted.value(configuration.databaseUrl), {
          connect_timeout: 10,
          idle_timeout: 30,
          max: 10,
          prepare: false,
        }),
      ),
      (client) =>
        Effect.promise(async () => {
          await client.end({ timeout: 5 });
        }),
    );
    return ForgeGitDatabase.of({ sql });
  }),
);

export const databaseError = (operation: string) =>
  Effect.mapError(
    (cause: unknown) =>
      new ForgeGitDatabaseError({
        cause,
        operation,
      }),
  );
