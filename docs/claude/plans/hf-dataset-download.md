# HuggingFace Dataset Download Service Plan

## Goal
Create a generic Effect service that downloads HuggingFace datasets (parquet format), starting with `open-thoughts/OpenThoughts-Agent-v1-SFT` containing ~15k agent trajectories.

## User Decisions
- **Storage:** Raw Parquet + ATIF conversion on-demand
- **Download:** Full download to disk (simple, reliable)
- **Location:** `.openagents/datasets/` with gitignore entry

## Architecture

### New Files
```
src/huggingface/
├── service.ts         # HFDatasetService - generic dataset download
├── schema.ts          # Schema types (HFDataset, HFFile, download config)
├── parquet.ts         # Parquet parsing utilities
├── openthoughts.ts    # OpenThoughts-specific adapter (SFT row → ATIF)
└── cli.ts             # CLI for manual downloads: `bun run hf:download`
```

### Service Interface

```typescript
// src/huggingface/schema.ts
export interface HFDatasetConfig {
  repo: string;                    // e.g. "open-thoughts/OpenThoughts-Agent-v1-SFT"
  revision?: string;               // branch/commit (default: "main")
  accessToken?: string;            // HF_TOKEN from env
  filePattern?: string;            // glob for files (default: "**/*.parquet")
}

export interface DownloadedDataset {
  repo: string;
  localPath: string;               // .openagents/datasets/<repo>/
  files: string[];                 // list of downloaded file paths
  totalBytes: number;
  downloadedAt: string;            // ISO timestamp
}
```

```typescript
// src/huggingface/service.ts
export class HFDatasetService extends Context.Tag("HFDatasetService")<
  HFDatasetService,
  {
    /** Download dataset files to local storage */
    download(config: HFDatasetConfig): Effect.Effect<DownloadedDataset, HFDatasetError>;

    /** Check if dataset is already downloaded */
    isDownloaded(repo: string): Effect.Effect<boolean, HFDatasetError>;

    /** Get local path for a dataset */
    getLocalPath(repo: string): Effect.Effect<string | null, HFDatasetError>;

    /** List downloaded datasets */
    listDownloaded(): Effect.Effect<DownloadedDataset[], HFDatasetError>;

    /** Delete a downloaded dataset */
    delete(repo: string): Effect.Effect<void, HFDatasetError>;
  }
>() {}
```

### OpenThoughts Adapter

```typescript
// src/huggingface/openthoughts.ts
export class OpenThoughtsService extends Context.Tag("OpenThoughtsService")<
  OpenThoughtsService,
  {
    /** Ensure dataset is downloaded, return local path */
    ensureDownloaded(): Effect.Effect<string, HFDatasetError>;

    /** Stream trajectories from parquet (lazy parsing) */
    streamTrajectories(): Effect.Effect<AsyncIterable<Trajectory>, HFDatasetError>;

    /** Get a specific trajectory by index or run_id */
    getTrajectory(idOrIndex: string | number): Effect.Effect<Trajectory | null, HFDatasetError>;

    /** Get total count of trajectories */
    count(): Effect.Effect<number, HFDatasetError>;
  }
>() {}
```

## Implementation Steps

### Step 1: Add Dependencies
```bash
bun add -E @huggingface/hub parquetjs-lite
```

### Step 2: Create Schema (`src/huggingface/schema.ts`)
- Define `HFDatasetConfig`, `DownloadedDataset`, `HFDatasetError`
- Define OpenThoughts SFT row type matching parquet schema:
  ```typescript
  interface SftRow {
    conversations: Array<{ content: string; role: "user" | "assistant" }>;
    task: string;
    episode: string;
    run_id: string;
    trial_name: string;
    agent: string;
    model: string;
    model_provider: string;
    date: string;
  }
  ```

### Step 3: Create HFDatasetService (`src/huggingface/service.ts`)
- Use `@huggingface/hub` for `listFiles` and `downloadFile`
- Store files at `.openagents/datasets/<repo-owner>/<repo-name>/`
- Track downloads in `.openagents/datasets/index.json`
- Follow existing Effect service patterns from `src/atif/service.ts`

### Step 4: Create Parquet Utilities (`src/huggingface/parquet.ts`)
- Wrap `parquetjs-lite` in Effect-friendly API
- Support streaming row iteration (memory efficient)
- Type-safe row decoding

### Step 5: Create OpenThoughts Adapter (`src/huggingface/openthoughts.ts`)
- Hardcode `open-thoughts/OpenThoughts-Agent-v1-SFT` repo
- Convert SFT rows to ATIF Trajectory format:
  ```typescript
  function sftRowToTrajectory(row: SftRow): Trajectory {
    return {
      schema_version: "ATIF-v1.4",
      session_id: row.run_id ?? `${row.task}__${row.episode}`,
      agent: {
        name: row.agent,
        version: "1.0",
        model_name: row.model,
      },
      steps: row.conversations.map((msg, idx) => ({
        step_id: idx + 1,
        timestamp: row.date,
        source: msg.role === "assistant" ? "agent" : "user",
        message: msg.content,
      })),
      extra: {
        source: "open-thoughts/OpenThoughts-Agent-v1-SFT",
        task: row.task,
        episode: row.episode,
        trial_name: row.trial_name,
        model_provider: row.model_provider,
      },
    };
  }
  ```

### Step 6: Create CLI (`src/huggingface/cli.ts`)
```bash
# Download OpenThoughts SFT dataset
bun run hf:download openthoughts-sft

# Generic dataset download
bun run hf:download --repo "open-thoughts/OpenThoughts-TB-dev"

# List downloaded datasets
bun run hf:list
```

### Step 7: Update .gitignore
Add to `.openagents/.gitignore`:
```
datasets/
```

### Step 8: Add package.json Scripts
```json
"hf:download": "bun src/huggingface/cli.ts download",
"hf:list": "bun src/huggingface/cli.ts list"
```

### Step 9: Write Tests
- Test download of small test dataset
- Test parquet parsing
- Test SFT → ATIF conversion

## Storage Structure

```
.openagents/
├── datasets/
│   ├── index.json                           # Track all downloads
│   └── open-thoughts/
│       └── OpenThoughts-Agent-v1-SFT/
│           └── data/
│               └── train-00000-of-00001.parquet  # ~110MB
└── .gitignore                               # Add: datasets/
```

## Critical Files to Modify
1. `package.json` - Add dependencies and scripts
2. `.openagents/.gitignore` - Add `datasets/`
3. Create all files in `src/huggingface/`

## Success Criteria
1. `bun run hf:download openthoughts-sft` downloads the ~110MB parquet file
2. Downloaded file is at `.openagents/datasets/open-thoughts/OpenThoughts-Agent-v1-SFT/data/train-00000-of-00001.parquet`
3. `OpenThoughtsService.count()` returns ~15,200
4. `OpenThoughtsService.streamTrajectories()` yields valid ATIF Trajectory objects
5. Effuse UI can ingest trajectories via the service

## Environment Variables
- `HF_TOKEN` - Optional HuggingFace access token (dataset is public, but rate limits apply)
