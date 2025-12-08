#!/usr/bin/env bun
/**
 * Demo script for OpenRouter inference service.
 *
 * Sends a simple "Introduce yourself" message to OpenRouter's auto model selector.
 *
 * Usage:
 *   bun run src/llm/openrouter-inference-demo.ts
 */

import { Effect, Console, Layer } from "effect";
import { OpenRouterInference, OpenRouterInferenceLive } from "./openrouter-inference.js";
import { openRouterLive } from "./openrouter-http.js";
import { InferenceStore, InferenceStoreLive } from "./inference-store.js";
import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import * as BunContext from "@effect/platform-bun/BunContext";
import * as DefaultServices from "effect/DefaultServices";
import { runMigrations } from "../storage/migrations.js";
import { Database } from "bun:sqlite";

const program = Effect.gen(function* () {
  // Ensure migrations are run
  yield* Console.log("🔧 Ensuring database migrations are applied...\n");
  const fs = yield* FileSystem.FileSystem;
  const path = yield* Path.Path;
  const dbPath = path.join(process.cwd(), ".openagents", "openagents.db");
  const migrationsDir = path.join(process.cwd(), ".openagents", "migrations");

  const dbExists = yield* fs.exists(dbPath);
  if (dbExists) {
    const db = new Database(dbPath);
    // Check if inferences table exists, if not run migrations
    const tableCheck = db
      .prepare(
        "SELECT name FROM sqlite_master WHERE type='table' AND name='inferences'",
      )
      .get();
    if (!tableCheck) {
      yield* runMigrations(db, migrationsDir).pipe(
        Effect.provide(
          Layer.mergeAll(
            Layer.syncContext(() => DefaultServices.liveServices),
            BunContext.layer,
          ),
        ),
        Effect.catchAll((error) =>
          Effect.gen(function* () {
            yield* Console.log(
              `⚠️  Migration warning: ${error.message}. Trying to create table directly...`,
            );
            // Try to apply just the inferences migration directly
            const migrationFile = path.join(
              migrationsDir,
              "002_inferences.sql",
            );
            const migrationSql = yield* fs.readFileString(migrationFile);
            try {
              db.exec(migrationSql);
              yield* Console.log("✓ Created inferences table directly");
            } catch (e) {
              yield* Console.log(
                `⚠️  Could not create table: ${String(e)}. Continuing anyway...`,
              );
            }
          }),
        ),
      );
    } else {
      yield* Console.log("✓ Inferences table already exists");
    }
    db.close();
  }

  const inference = yield* OpenRouterInference;
  const store = yield* InferenceStore;

  yield* Console.log("🤖 Sending inference request using free model option...\n");

  const response = yield* inference.send(
    "openrouter/auto", // Model parameter (will be overridden by free option)
    [
      {
        role: "user",
        content: "Introduce yourself",
      },
    ],
    {
      free: true, // Use default free model (arcee-ai/trinity-mini:free)
    },
  );

  // Log the entire response object to see all available fields
  yield* Console.log("📋 Full Response Object:");
  yield* Console.log(JSON.stringify(response, null, 2));
  yield* Console.log("\n");

  const choice = response.choices[0];
  const message = choice?.message;
  const content = message?.content ?? "(no content)";
  const model = response.model ?? "unknown";

  yield* Console.log(`📦 Model selected: ${model}\n`);
  yield* Console.log(`💬 Response:\n${content}\n`);

  if (response.usage) {
    yield* Console.log(
      `📊 Usage: ${response.usage.prompt_tokens ?? 0} prompt tokens, ${response.usage.completion_tokens ?? 0} completion tokens, ${response.usage.total_tokens ?? 0} total`,
    );
  }

  // Verify data was saved
  yield* Console.log("\n💾 Verifying inference was saved to database...\n");

  const recent = yield* store.listRecent(5);
  yield* Console.log(`📊 Found ${recent.length} recent inference(s):\n`);

  for (const record of recent) {
    yield* Console.log(
      `  ID: ${record.id} | Model: ${record.model} | Response Model: ${record.responseModel ?? "N/A"} | Created: ${record.createdAt}`,
    );
    if (record.costUsd !== null && record.costUsd > 0) {
      yield* Console.log(`    Cost: $${record.costUsd.toFixed(6)}`);
    } else {
      yield* Console.log(`    Cost: FREE`);
    }
  }

  // Get stats
  const stats = yield* store.getStats();
  yield* Console.log(`\n📈 Total inferences: ${stats.total}`);
  yield* Console.log(`💰 Total cost: $${stats.totalCost.toFixed(6)}`);
  if (Object.keys(stats.byModel).length > 0) {
    yield* Console.log("\n📊 By model:");
    for (const [model, data] of Object.entries(stats.byModel)) {
      yield* Console.log(
        `  ${model}: ${data.count} requests, $${data.cost.toFixed(6)}`,
      );
    }
  }

  return response;
});

const platformLayer = Layer.mergeAll(
  Layer.syncContext(() => DefaultServices.liveServices),
  BunContext.layer,
);

const inferenceLayer = OpenRouterInferenceLive.pipe(
  Layer.provideMerge(openRouterLive),
  Layer.provideMerge(InferenceStoreLive),
);

const main = program.pipe(
  Effect.provide(Layer.mergeAll(platformLayer, inferenceLayer)),
  Effect.tapErrorCause(Effect.logError),
  Effect.runPromise,
);

main
  .then(() => {
    console.log("\n✅ Demo completed successfully");
    process.exit(0);
  })
  .catch((error) => {
    console.error("\n❌ Demo failed:", error);
    process.exit(1);
  });
