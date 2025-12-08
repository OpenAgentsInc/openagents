# 0150 OpenRouter Inference System & Tracking

## Overview

Implemented a complete OpenRouter inference system with automatic tracking. This includes:
1. **OpenRouterInference Service** - High-level Effect service for sending inference requests to any OpenRouter model
2. **Auto Model Selection** - Support for OpenRouter's intelligent auto model selection (`openrouter/auto`)
3. **Inference Tracking** - Automatic SQLite-based storage of all requests and responses
4. **Cost Tracking** - Built-in cost analysis and statistics

This system enables cost tracking, model performance analysis, debugging, and learning from past interactions while providing a simple, type-safe API for inference requests.

## Architecture

The system is built in layers:

```
┌─────────────────────────────────────┐
│  OpenRouterInference (High-level)   │  ← Your code uses this
├─────────────────────────────────────┤
│  OpenRouterClient (HTTP layer)       │  ← Handles API calls
├─────────────────────────────────────┤
│  OpenRouterConfig (Configuration)    │  ← API keys, settings
├─────────────────────────────────────┤
│  InferenceStore (Tracking)           │  ← Automatic logging
└─────────────────────────────────────┘
```

### Component Overview

1. **OpenRouterInference** - Simple, high-level API for inference
2. **OpenRouterClient** - Low-level HTTP client with retry logic
3. **OpenRouterConfig** - Configuration management (API keys, timeouts, etc.)
4. **InferenceStore** - Database storage and querying
5. **Database Migration** - SQLite schema for tracking

## What Was Created

### 1. OpenRouterInference Service (`src/llm/openrouter-inference.ts`)

A high-level Effect service that provides a simple API for sending inference requests to OpenRouter. This is the main interface you'll use in your code.

**Key Features**:
- Simple `send()` method with clean parameters
- Automatic database tracking (transparent to user)
- Support for all OpenRouter models, including `openrouter/auto`
- Type-safe with Effect error handling
- Automatic extraction of model selection (for auto router)

**Interface**:
```typescript
interface IOpenRouterInference {
  send(
    model: string,
    messages: Array<{ role: "user" | "system" | "assistant"; content: string }>,
    options?: {
      temperature?: number;
      maxTokens?: number;
      tools?: Tool[];
      toolChoice?: "auto" | "required" | { type: "function"; function: { name: string } };
    }
  ): Effect.Effect<ChatResponse, Error, OpenRouterClient | InferenceStore>;
}
```

**Location**: `src/llm/openrouter-inference.ts`

### 2. OpenRouterClient Service (`src/llm/openrouter-http.ts`)

Low-level HTTP client that handles:
- API communication with OpenRouter
- Request/response transformation
- Retry logic with exponential backoff
- Error handling and mapping
- Timeout management
- Tool call format conversion

**Features**:
- Automatic retry on transient failures
- Configurable timeouts
- Support for tools and function calling
- Response transformation (preserves all metadata including `model` field)

**Location**: `src/llm/openrouter-http.ts`

### 3. OpenRouterConfig (`src/llm/openrouter-config.ts`)

Configuration management for OpenRouter:
- API key loading from environment (`.env.local` or `OPENROUTER_API_KEY`)
- Base URL configuration
- Timeout settings
- Logging configuration
- Referer and site name headers

**Environment Variables**:
- `OPENROUTER_API_KEY` (required) - Your OpenRouter API key
- `OPENROUTER_BASE_URL` (optional) - Default: `https://openrouter.ai/api/v1`
- `OPENROUTER_TIMEOUT_MS` (optional) - Default: `120000` (2 minutes)
- `OPENROUTER_REFERER` (optional) - HTTP-Referer header
- `OPENROUTER_SITE_NAME` (optional) - X-Title header
- `OPENROUTER_LOG_LEVEL` (optional) - `debug`, `info`, `warn`, `error`

**Location**: `src/llm/openrouter-config.ts`

### 4. Database Migration (`002_inferences.sql`)

Created a new SQLite table `inferences` that stores:
- **Request data**: Model ID, messages, and options (stored as JSON for flexibility)
- **Response data**: Complete response object with all metadata (stored as JSON)
- **Extracted fields**: Model, tokens, cost, etc. for fast querying without JSON parsing
- **Indexes**: Optimized for common query patterns (by model, timestamp, cost)
- **Full-text search**: FTS5 virtual table for searching response content

**Location**: `.openagents/migrations/002_inferences.sql`

### 2. InferenceStore Service (`src/llm/inference-store.ts`)

An Effect-based service that provides:
- `save()` - Automatically save inference with all request/response data
- `getById()` - Retrieve inference by ID
- `listByModel()` - List inferences filtered by model
- `listRecent()` - List recent inferences (with limit)
- `getStats()` - Aggregate statistics (total count, cost, breakdown by model)

**Key Features**:
- Non-blocking saves (errors don't fail inference requests)
- Automatic extraction of usage metrics and response content
- Type-safe Effect-based API
- Integrates with existing database infrastructure

### 3. Integration with OpenRouterInference

The `OpenRouterInference` service now automatically saves every inference to the database. No additional code needed - it just works!

**Location**: `src/llm/openrouter-inference.ts`

### 5. Demo Script

Complete demo script that:
- Runs migrations automatically
- Sends inference request to auto model
- Displays full response with metadata
- Verifies data was saved to database
- Shows statistics and recent inferences

**Location**: `src/llm/openrouter-inference-demo.ts`
**Run with**: `bun run demo:openrouter`

## Response Data Structure

### ChatResponse Type

```typescript
interface ChatResponse {
  id: string;                    // OpenRouter request ID
  model?: string;                 // Actual model used (important for auto router!)
  choices: Array<{
    message: {
      role: "assistant";
      content: string | null;
      tool_calls?: Array<{
        id: string;
        name: string;
        arguments: string;
      }>;
    };
  }>;
  usage?: {
    prompt_tokens?: number;
    completion_tokens?: number;
    total_tokens?: number;
    cost?: number;                // Cost in USD
    is_byok?: boolean;            // Bring-your-own-key
    prompt_tokens_details?: {
      cached_tokens?: number;
      audio_tokens?: number;
      video_tokens?: number;
    };
    cost_details?: {
      upstream_inference_cost?: number | null;
      upstream_inference_prompt_cost?: number;
      upstream_inference_completions_cost?: number;
    };
    completion_tokens_details?: {
      reasoning_tokens?: number;
      image_tokens?: number;
    };
  };
}
```

### Key Fields

- **`response.model`**: The actual model used (especially important when using `openrouter/auto`)
- **`response.usage.cost`**: Total cost in USD for the request
- **`response.usage.total_tokens`**: Total tokens used
- **`response.choices[0].message.content`**: The text response
- **`response.choices[0].message.tool_calls`**: Any function calls made

## Database Schema

### Table: `inferences`

```sql
CREATE TABLE inferences (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  model TEXT NOT NULL,                    -- Requested model (e.g., "openrouter/auto")
  request_id TEXT,                         -- OpenRouter response ID
  request_messages JSON NOT NULL,          -- Full request messages array
  request_options JSON,                    -- Request parameters (temperature, etc.)
  response_data JSON NOT NULL,             -- Complete response object
  response_id TEXT,                        -- Extracted response ID
  response_model TEXT,                     -- Actual model used (important for auto router)
  response_content TEXT,                   -- First message content (for search)
  prompt_tokens INTEGER,
  completion_tokens INTEGER,
  total_tokens INTEGER,
  cost_usd REAL,                           -- Cost in USD
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

### Indexes

- `idx_inferences_model` - Query by requested model
- `idx_inferences_response_model` - Query by actual model used
- `idx_inferences_request_id` - Lookup by OpenRouter request ID
- `idx_inferences_created_at` - Time-based queries
- `idx_inferences_cost` - Cost analysis queries
- `idx_inferences_model_created` - Composite for model + time queries

### Full-Text Search

FTS5 virtual table `inferences_fts` enables fast text search on response content with automatic sync via triggers.

## Setup

### 1. Environment Configuration

Create or update `.env.local` in your project root:

```bash
OPENROUTER_API_KEY=sk-or-v1-...
```

Or set it in your environment:
```bash
export OPENROUTER_API_KEY=sk-or-v1-...
```

### 2. Install Dependencies

The OpenRouter SDK is already included:
```json
{
  "dependencies": {
    "@openrouter/sdk": "0.1.27"
  }
}
```

### 3. Run Migrations

Migrations run automatically when the database is accessed, but you can also run them manually:

```bash
# The demo script handles this automatically
bun run demo:openrouter
```

## Usage Guide

### Basic Usage

The inference tracking is **automatic**. Just use the `OpenRouterInference` service:

```typescript
import { Effect, Layer, Console } from "effect";
import { OpenRouterInference, OpenRouterInferenceLive } from "./openrouter-inference.js";
import { openRouterLive } from "./openrouter-http.js";
import { InferenceStoreLive } from "./inference-store.js";
import * as BunContext from "@effect/platform-bun/BunContext";
import * as DefaultServices from "effect/DefaultServices";

const program = Effect.gen(function* () {
  const inference = yield* OpenRouterInference;

  // Send inference - automatically saved to database!
  const response = yield* inference.send("openrouter/auto", [
    { role: "user", content: "Hello! Introduce yourself." }
  ]);

  // Access response data
  const content = response.choices[0]?.message?.content ?? "";
  const model = response.model ?? "unknown"; // Actual model selected by auto router
  const usage = response.usage;

  yield* Console.log(`Model: ${model}`);
  yield* Console.log(`Response: ${content}`);
  if (usage) {
    yield* Console.log(`Tokens: ${usage.total_tokens} (cost: $${usage.cost ?? 0})`);
  }

  return response;
});

// Setup layers
const platformLayer = Layer.mergeAll(
  Layer.syncContext(() => DefaultServices.liveServices),
  BunContext.layer
);

const inferenceLayer = OpenRouterInferenceLive.pipe(
  Layer.provideMerge(openRouterLive),
  Layer.provideMerge(InferenceStoreLive)
);

// Run the program
Effect.runPromise(
  program.pipe(Effect.provide(Layer.mergeAll(platformLayer, inferenceLayer)))
);
```

### Using Specific Models

You can use any OpenRouter model ID:

```typescript
// Use a specific model
const response = yield* inference.send("mistralai/mistral-nemo", [
  { role: "user", content: "What is 2+2?" }
]);

// Use premium models
const premiumResponse = yield* inference.send("openai/gpt-5.1-codex-max", [
  { role: "user", content: "Write a function..." }
]);
```

### Using Free Model Option

The service provides a convenient `free` option that automatically uses the default free model (`arcee-ai/trinity-mini:free`):

```typescript
// Use free model option (automatically uses arcee-ai/trinity-mini:free)
const freeResponse = yield* inference.send(
  "openrouter/auto", // Model parameter (will be overridden)
  [
    { role: "user", content: "Hello!" }
  ],
  {
    free: true  // Uses DEFAULT_FREE_MODEL (arcee-ai/trinity-mini:free)
  }
);

// If you explicitly specify a free model, it will be respected
const explicitFree = yield* inference.send(
  "x-ai/grok-4.1-fast:free", // Explicit free model
  [{ role: "user", content: "Hello!" }],
  { free: true }  // This is redundant but harmless
);
```

**Default Free Model**: `arcee-ai/trinity-mini:free` (exported as `DEFAULT_FREE_MODEL` constant)

**Behavior**:
- If `free: true` and model doesn't contain `:free`, uses `DEFAULT_FREE_MODEL`
- If model already contains `:free`, respects the explicit model
- Free models have `cost: 0` in the response

### Using Auto Model Selection

The `openrouter/auto` model automatically selects the best model for your prompt:

```typescript
const response = yield* inference.send("openrouter/auto", [
  { role: "user", content: "Explain quantum computing" }
]);

// The response.model field tells you which model was actually used
console.log(`Auto router selected: ${response.model}`);
// Example output: "mistralai/mistral-nemo"
```

**Benefits of Auto Router**:
- Automatically chooses the best model for your prompt
- Optimizes for cost and performance
- Handles model availability automatically
- The `response.model` field shows which model was selected

### Advanced Options

```typescript
const response = yield* inference.send(
  "openrouter/auto",
  [
    { role: "system", content: "You are a helpful assistant." },
    { role: "user", content: "Write a haiku about coding" }
  ],
  {
    temperature: 0.7,        // Control randomness
    maxTokens: 500,          // Limit response length
    // tools: [...],         // Function calling (see below)
    // toolChoice: "auto"    // Tool usage strategy
  }
);
```

### Function Calling / Tools

The service supports OpenRouter's tool calling:

```typescript
import type { Tool } from "../tools/schema.js";

const tools: Tool[] = [
  {
    type: "function",
    function: {
      name: "get_weather",
      description: "Get the current weather for a location",
      parameters: {
        type: "object",
        properties: {
          location: { type: "string", description: "City name" }
        },
        required: ["location"]
      }
    }
  }
];

const response = yield* inference.send(
  "openrouter/auto",
  [
    { role: "user", content: "What's the weather in San Francisco?" }
  ],
  {
    tools,
    toolChoice: "auto"  // or "required" or { type: "function", function: { name: "get_weather" } }
  }
);

// Check for tool calls
const toolCalls = response.choices[0]?.message?.tool_calls ?? [];
for (const call of toolCalls) {
  console.log(`Tool: ${call.name}(${call.arguments})`);
}
```

### Error Handling

The service uses Effect's error handling:

```typescript
const program = Effect.gen(function* () {
  const inference = yield* OpenRouterInference;

  const response = yield* inference.send("openrouter/auto", [
    { role: "user", content: "Hello!" }
  ]).pipe(
    Effect.catchAll((error) =>
      Effect.gen(function* () {
        yield* Console.error(`Inference failed: ${error.message}`);
        // Handle error, maybe retry with different model
        return yield* inference.send("x-ai/grok-4.1-fast:free", [
          { role: "user", content: "Hello!" }
        ]);
      })
    )
  );

  return response;
});
```

### Querying Saved Inferences

```typescript
import { Effect } from "effect";
import { InferenceStore, InferenceStoreLive } from "./inference-store.js";

const program = Effect.gen(function* () {
  const store = yield* InferenceStore;

  // Get recent inferences
  const recent = yield* store.listRecent(10);
  console.log(`Found ${recent.length} recent inferences`);

  // Get inferences by model
  const byModel = yield* store.listByModel("openrouter/auto", 50);
  console.log(`Found ${byModel.length} auto router inferences`);

  // Get statistics
  const stats = yield* store.getStats();
  console.log(`Total: ${stats.total} inferences`);
  console.log(`Total cost: $${stats.totalCost.toFixed(6)}`);
  console.log("By model:", stats.byModel);

  // Get specific inference
  const inference = yield* store.getById(1);
  if (inference) {
    console.log("Request:", inference.requestMessages);
    console.log("Response:", inference.responseData);
  }
});

Effect.runPromise(program.pipe(Effect.provide(InferenceStoreLive)));
```

### Direct SQL Queries

You can also query the database directly using SQLite:

```bash
# List all inferences
sqlite3 .openagents/openagents.db "SELECT id, model, response_model, cost_usd, created_at FROM inferences ORDER BY created_at DESC LIMIT 10;"

# Total cost by model
sqlite3 .openagents/openagents.db "
  SELECT
    model,
    COUNT(*) as count,
    SUM(cost_usd) as total_cost,
    AVG(cost_usd) as avg_cost
  FROM inferences
  GROUP BY model
  ORDER BY total_cost DESC;
"

# Daily cost breakdown
sqlite3 .openagents/openagents.db "
  SELECT
    DATE(created_at) as date,
    COUNT(*) as requests,
    SUM(cost_usd) as daily_cost
  FROM inferences
  GROUP BY DATE(created_at)
  ORDER BY date DESC;
"

# Search response content
sqlite3 .openagents/openagents.db "
  SELECT id, model, response_content
  FROM inferences_fts
  WHERE response_content MATCH 'introduction'
  LIMIT 5;
"
```

### Accessing Full Request/Response Data

The `request_messages`, `request_options`, and `response_data` columns store complete JSON objects:

```typescript
const inference = yield* store.getById(1);
if (inference) {
  // Full request messages
  const messages = inference.requestMessages;

  // Request options (temperature, maxTokens, etc.)
  const options = inference.requestOptions;

  // Complete response with all metadata
  const response = inference.responseData;

  // Access nested fields
  const usage = response.usage;
  const costDetails = usage?.cost_details;
  const toolCalls = response.choices[0]?.message?.tool_calls;
}
```

## Use Cases

### 1. Cost Tracking

Track spending across different models and time periods:

```typescript
const stats = yield* store.getStats();
console.log(`Total spent: $${stats.totalCost.toFixed(6)}`);
for (const [model, data] of Object.entries(stats.byModel)) {
  console.log(`${model}: ${data.count} requests, $${data.cost.toFixed(6)}`);
}
```

### 2. Model Performance Analysis

Compare which models are being selected by the auto router:

```typescript
const autoInferences = yield* store.listByModel("openrouter/auto", 1000);
const modelCounts: Record<string, number> = {};
for (const inf of autoInferences) {
  const model = inf.responseModel || "unknown";
  modelCounts[model] = (modelCounts[model] || 0) + 1;
}
console.log("Auto router model selection:", modelCounts);
```

### 3. Debugging Failed Requests

Review past requests to debug issues:

```typescript
const recent = yield* store.listRecent(100);
for (const inf of recent) {
  if (inf.responseData.choices[0]?.message?.content === null) {
    console.log("Empty response:", {
      id: inf.id,
      model: inf.model,
      request: inf.requestMessages,
      response: inf.responseData
    });
  }
}
```

### 4. Learning from Past Interactions

Search for similar past interactions:

```sql
-- Find inferences with similar content
SELECT id, model, response_content
FROM inferences_fts
WHERE response_content MATCH 'error OR failed OR exception'
ORDER BY created_at DESC
LIMIT 20;
```

### 5. Cost Optimization

Identify expensive models or requests:

```sql
-- Most expensive requests
SELECT
  id,
  model,
  response_model,
  cost_usd,
  prompt_tokens,
  completion_tokens,
  created_at
FROM inferences
WHERE cost_usd IS NOT NULL
ORDER BY cost_usd DESC
LIMIT 10;
```

## Data Retention

- **No automatic deletion**: All inferences are preserved for historical analysis
- **Manual cleanup**: You can delete old records if needed:
  ```sql
  DELETE FROM inferences WHERE created_at < datetime('now', '-90 days');
  ```
- **Partitioning**: For very large datasets, consider partitioning by date

## Migration

The migration runs automatically when:
1. The demo script is executed (`bun run demo:openrouter`)
2. The database is accessed and the table doesn't exist

To manually run migrations:

```typescript
import { runMigrations } from "./storage/migrations.js";
import { Database } from "bun:sqlite";

const db = new Database(".openagents/openagents.db");
await Effect.runPromise(
  runMigrations(db, ".openagents/migrations")
    .pipe(Effect.provide(/* platform layer */))
);
db.close();
```

## Performance Considerations

- **Indexes**: All common query patterns are indexed
- **JSON storage**: Full data preserved, extracted fields for fast queries
- **FTS5**: Fast full-text search without table scans
- **Non-blocking saves**: Database writes don't slow down inference requests

## Error Handling

The save operation is **non-blocking** and **non-failing**:
- If database save fails, the inference request still succeeds
- Errors are logged as warnings, not thrown
- This ensures inference reliability even if database has issues

```typescript
// In openrouter-inference.ts
yield* store.save(model, request, response).pipe(
  Effect.tapError((error) =>
    Effect.sync(() =>
      console.warn(`Failed to save inference: ${error.message}`)
    )
  ),
  Effect.ignore  // Don't fail the request if save fails
);
```

## TypeScript Types

All types are exported from `src/llm/inference-store.ts`:

```typescript
import type { InferenceRecord } from "./inference-store.js";

// InferenceRecord includes:
interface InferenceRecord {
  id: number;
  model: string;
  requestId: string | null;
  requestMessages: Array<{ role: string; content: string }>;
  requestOptions: Record<string, unknown> | null;
  responseData: ChatResponse & { usage?: any };
  responseId: string | null;
  responseModel: string | null;
  responseContent: string | null;
  promptTokens: number | null;
  completionTokens: number | null;
  totalTokens: number | null;
  costUsd: number | null;
  createdAt: string;
}
```

## Testing

Run the demo to verify everything works:

```bash
bun run demo:openrouter
```

This will:
1. Run migrations (if needed)
2. Send an inference request
3. Save it to the database
4. Query and display the saved data
5. Show statistics

## Future Enhancements

Potential improvements:
- **Vector embeddings**: Store embeddings of requests/responses for semantic search
- **Caching layer**: Use past inferences to avoid duplicate requests
- **Analytics dashboard**: Visualize cost trends, model usage, etc.
- **Alerting**: Notify when costs exceed thresholds
- **Export**: Export data for external analysis (CSV, JSON, etc.)

## Service Architecture Details

### Layer Composition

The services are composed using Effect's Layer system:

```typescript
// Base platform layer (filesystem, path, etc.)
const platformLayer = Layer.mergeAll(
  Layer.syncContext(() => DefaultServices.liveServices),
  BunContext.layer
);

// OpenRouter configuration and client
const openRouterLayer = openRouterLive;  // Includes config + client

// Inference store (database)
const storeLayer = InferenceStoreLive;

// High-level inference service (depends on client + store)
const inferenceLayer = OpenRouterInferenceLive.pipe(
  Layer.provideMerge(openRouterLive),
  Layer.provideMerge(InferenceStoreLive)
);

// Complete layer for your program
const appLayer = Layer.mergeAll(platformLayer, inferenceLayer);
```

### Dependency Flow

```
OpenRouterInference
  ├─> OpenRouterClient (sends HTTP requests)
  │     └─> OpenRouterConfig (API key, settings)
  └─> InferenceStore (saves to database)
        └─> Database (SQLite)
```

### Configuration Loading

The system automatically loads configuration from:
1. Environment variables (`process.env` or `Bun.env`)
2. `.env.local` file (loaded via `dotenvLocalLayer`)
3. Default values for optional settings

The config layer is provided by `openRouterBaseLayer` which includes:
- Platform services (FileSystem, Path)
- Environment variable loading
- Configuration parsing

## Model Selection

### Available Models

OpenRouter supports hundreds of models. Common ones:

**Free Models**:
- `arcee-ai/trinity-mini:free` - **Default free model** (use `free: true` option)
- `x-ai/grok-4.1-fast:free` - Fast, free Grok model
- `amazon/nova-2-lite-v1:free` - Amazon Nova (free tier)

**Premium Models**:
- `openai/gpt-5.1-codex-max` - Latest OpenAI coding model
- `mistralai/mistral-nemo` - Mistral's efficient model
- `mistralai/mistral-large-2512` - Mistral's largest model
- `anthropic/claude-3.5-sonnet` - Claude Sonnet

**Auto Selection**:
- `openrouter/auto` - Automatically selects best model

### Finding Models

1. **OpenRouter Website**: https://openrouter.ai/models
2. **API Endpoint**: `GET https://openrouter.ai/api/v1/models`
3. **Local File**: `docs/local/openrouter_models.json` (cached model list)

The model list includes:
- Model IDs and names
- Pricing information
- Supported parameters
- Context lengths
- Capabilities (vision, tools, etc.)

## Best Practices

### 1. Use Auto Router for General Use

```typescript
// ✅ Good: Let OpenRouter choose the best model
const response = yield* inference.send("openrouter/auto", messages);

// ❌ Less optimal: Hard-coding a model unless you have a specific reason
const response = yield* inference.send("mistralai/mistral-nemo", messages);
```

### 2. Check Response Model

When using auto router, always check which model was selected:

```typescript
const response = yield* inference.send("openrouter/auto", messages);
console.log(`Used model: ${response.model}`);  // Important for debugging!
```

### 3. Handle Costs

Monitor costs using the tracking system:

```typescript
const stats = yield* store.getStats();
if (stats.totalCost > 10.0) {
  console.warn("Cost threshold exceeded!");
}
```

### 4. Use Appropriate Models for Tasks

- **Coding**: Use models with coding capabilities (`gpt-5.1-codex-max`, etc.)
- **General chat**: Auto router is fine
- **Free tier**: Use `:free` models for development/testing

### 5. Error Handling

Always handle errors gracefully:

```typescript
const response = yield* inference.send(model, messages).pipe(
  Effect.catchAll((error) => {
    // Log error, maybe fallback to free model
    console.error("Inference failed:", error);
    return inference.send("x-ai/grok-4.1-fast:free", messages);
  })
);
```

## Troubleshooting

### Common Issues

**1. Missing API Key**
```
Error: Missing OPENROUTER_API_KEY
```
**Solution**: Set `OPENROUTER_API_KEY` in `.env.local` or environment

**2. Model Not Found**
```
Error: Model 'invalid-model' not found
```
**Solution**: Check model ID at https://openrouter.ai/models

**3. Database Migration Fails**
```
Error: table _schema_version already exists
```
**Solution**: This is handled automatically. The system will create the inferences table directly if migrations fail.

**4. High Costs**
```
Cost: $0.50 for single request
```
**Solution**:
- Use `openrouter/auto` for cost optimization
- Use `:free` models for development
- Check model pricing before using premium models

### Debug Mode

Enable debug logging:

```bash
export OPENROUTER_LOG_LEVEL=debug
bun run demo:openrouter
```

This shows:
- Request details (model, messages count)
- Response metadata
- Retry attempts
- Database operations

## Related Files

### Core Services
- **Inference Service**: `src/llm/openrouter-inference.ts`
- **HTTP Client**: `src/llm/openrouter-http.ts`
- **Configuration**: `src/llm/openrouter-config.ts`
- **Tracking Store**: `src/llm/inference-store.ts`
- **Types**: `src/llm/openrouter-types.ts`

### Database
- **Migration**: `.openagents/migrations/002_inferences.sql`
- **Database**: `.openagents/openagents.db`

### Examples
- **Demo Script**: `src/llm/openrouter-inference-demo.ts`
- **CLI Tool**: `src/llm/openrouter-cli.ts` (alternative CLI interface)

### Documentation
- **Model Schema**: `docs/logs/20251208/0149-openrouter-models.md`
- **This Document**: `docs/logs/20251208/0150-inference-tracking-system.md`

## Additional Resources

- **OpenRouter Docs**: https://openrouter.ai/docs
- **Auto Router Guide**: https://openrouter.ai/docs/guides/routing/auto-model-selection
- **OpenRouter SDK**: https://github.com/openrouter/openrouter-sdk
- **Effect Documentation**: https://effect.website/

## Questions?

- Check the demo script (`src/llm/openrouter-inference-demo.ts`) for complete examples
- Review TypeScript types in `src/llm/openrouter-types.ts`
- Query the database directly for ad-hoc analysis
- All data is preserved in JSON format for full fidelity
- Enable debug logging for detailed operation visibility
