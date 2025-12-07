You want:

1. **Which datasets/files** to grab from OpenThoughts.
2. **How to download them** from Node/TypeScript via the Hugging Face JS hub lib.
3. A sketch of an **Effect service** that keeps pulling this stuff and turns it into ATIF trajectories for your viewer.

I’ll walk through each, then give you a concrete service skeleton.

---

## 1. What to download

### Primary trajectory dataset

The “agent trajectories” you care about live in:

* **`open-thoughts/OpenThoughts-Agent-v1-SFT`** ([Hugging Face][1])

This is a ~15.2k-row SFT dataset with columns:

* `conversations`: a list of `{ content: string, role: "user" | "assistant" }` — **the actual trajectory**.
* `task`, `episode`, `run_id`, `trial_name`
* `agent`, `model`, `model_provider`, `date`
* plus some task/env metadata (e.g. `terminus-2`).

Files tab shows it’s just a single Parquet shard: ([Hugging Face][2])

```text
data/train-00000-of-00001.parquet    ~110 MB
```

That’s the one you want to download and parse into ATIF trajectories.

### Related datasets (optional for now)

* **`open-thoughts/OpenThoughts-Agent-v1-RL`** – RL environment + reward info (less about full dialogue trajectories). ([Hugging Face][3])
* **`open-thoughts/OpenThoughts-TB-dev`** – 70-task TB dev benchmark (task descriptions / verifiers). ([Hugging Face][4])

For “show agent trajectories in our ATIF viewer”, **SFT is the main thing**; you can add RL/TB-dev later as extra metadata.

---

## 2. Downloading the Parquet with `@huggingface/hub` (TypeScript)

### Install

```bash
npm install @huggingface/hub parquetjs-lite # or apache-arrow / duckdb-wasm, etc.
```

### Basic TS snippet

```ts
import { listFiles, downloadFile, type RepoId } from "@huggingface/hub";
// e.g. parquetjs-lite – pick your parser
import { ParquetReader } from "parquetjs-lite";

const HF_TOKEN = process.env.HF_TOKEN!;

const SFT_REPO: RepoId = {
  type: "dataset",
  name: "open-thoughts/OpenThoughts-Agent-v1-SFT",
};

async function downloadOpenThoughtsSftShard() {
  // 1) Find parquet files under data/
  const files = await listFiles({
    repo: SFT_REPO,
    accessToken: HF_TOKEN,
  });

  const parquetFiles = files
    .map((f) => f.path)
    .filter((p) => p.startsWith("data/") && p.endsWith(".parquet"));

  if (parquetFiles.length === 0) {
    throw new Error("No parquet files found in SFT dataset");
  }

  // For now, just take the first shard
  const path = parquetFiles[0]; // "data/train-00000-of-00001.parquet"

  // 2) Download
  const blob = await downloadFile({
    repo: SFT_REPO,
    path,
    accessToken: HF_TOKEN,
  });

  const buf = Buffer.from(await blob.arrayBuffer());

  // 3) Parse parquet
  const reader = await ParquetReader.openBuffer(buf);
  const cursor = reader.getCursor();

  const rows: any[] = [];
  let row;
  while ((row = await cursor.next())) {
    rows.push(row);
  }
  await reader.close();

  return rows;
}
```

Each `row` will look (conceptually) like:

```ts
type SftRow = {
  conversations: { content: string; role: "user" | "assistant" }[];
  task: string;
  episode: string;
  run_id: string;
  trial_name: string;
  agent: string;
  model: string;
  model_provider: string;
  date: string; // ISO-ish
  // maybe more fields depending on their schema
};
```

You can confirm exact column names from the dataset viewer (we saw several above). ([Hugging Face][1])

---

## 3. Mapping rows → ATIF trajectories

Given your ATIF schema, you’d typically produce **one `Trajectory` per SFT row**.

Pseudo-code:

```ts
import type { Trajectory, Step } from "../atif/schema"; // your ATIF types

function sftRowToATIF(row: SftRow): Trajectory {
  const steps: Step[] = row.conversations.map((msg, idx) => ({
    step_id: idx + 1,
    timestamp: row.date, // or refine if they add per-message times later
    source:
      msg.role === "assistant"
        ? "agent"
        : "user", // ATIF StepSource
    message: {
      role: msg.role,
      content: msg.content,
    },
    // tool_calls / observations can be added if you later parse structured JSON from content
  }));

  return {
    schema_version: "1.4", // or your current ATIF version
    session_id: row.run_id ?? `${row.task}__${row.episode}`,
    agent: {
      id: row.agent,
      name: row.agent,
      model: row.model,
      provider: row.model_provider,
    },
    steps,
    final_metrics: undefined,
    metadata: {
      source: "open-thoughts/OpenThoughts-Agent-v1-SFT",
      task: row.task,
      episode: row.episode,
      trial_name: row.trial_name,
      date: row.date,
    },
  };
}
```

Then your viewer just reads these ATIF JSON files instead of raw SFT rows.

---

## 4. An Effect service for “sync N datasets from HF into ATIF”

Here’s a concrete **Effect-TS service** you can drop into your repo and refine.

### Interface

```ts
// src/huggingface/open-thoughts-service.ts
import { Context, Effect, Layer } from "effect";
import { listFiles, downloadFile, type RepoId } from "@huggingface/hub";
import { ParquetReader } from "parquetjs-lite";

import type { Trajectory } from "../atif/schema.js";

export interface OpenThoughtsConfig {
  accessToken: string;
  cacheDir?: string; // optional local cache root
}

export class OpenThoughtsService extends Context.Tag(
  "OpenThoughtsService"
)<OpenThoughtsService, {
  /** Download + parse OpenThoughts SFT trajectories as ATIF. */
  fetchSftTrajectories: Effect.Effect<Trajectory[], Error>;
  /** In the future: RL envs, TB-dev tasks, etc. */
  // fetchRlTasks: Effect.Effect<...>;
  // fetchTbDevTasks: Effect.Effect<...>;
}> {}
```

### Live implementation

```ts
// src/huggingface/open-thoughts-service.ts (continued)

const SFT_REPO: RepoId = {
  type: "dataset",
  name: "open-thoughts/OpenThoughts-Agent-v1-SFT",
};

export const OpenThoughtsServiceLive = (cfg: OpenThoughtsConfig) =>
  Layer.succeed(
    OpenThoughtsService,
    {
      fetchSftTrajectories: fetchSftTrajectories(cfg),
    }
  );

function fetchSftTrajectories(cfg: OpenThoughtsConfig) {
  return Effect.gen(function* () {
    const files = yield* Effect.tryPromise(() =>
      listFiles({
        repo: SFT_REPO,
        accessToken: cfg.accessToken,
      })
    );

    const parquetPath =
      files.find((f) => f.path.startsWith("data/") && f.path.endsWith(".parquet"))
        ?.path;

    if (!parquetPath) {
      return yield* Effect.fail(
        new Error("No parquet shard found in OpenThoughts-Agent-v1-SFT")
      );
    }

    const blob = yield* Effect.tryPromise(() =>
      downloadFile({
        repo: SFT_REPO,
        path: parquetPath,
        accessToken: cfg.accessToken,
      })
    );

    const buf = Buffer.from(await blob.arrayBuffer());

    const reader = yield* Effect.tryPromise(() =>
      ParquetReader.openBuffer(buf)
    );

    const cursor = reader.getCursor();

    const atifTrajectories: Trajectory[] = [];

    while (true) {
      const row = await cursor.next();
      if (!row) break;

      // Adapt this type as you refine schema
      const traj = sftRowToATIF(row as any);
      atifTrajectories.push(traj);
    }

    await reader.close();

    return atifTrajectories;
  });
}
```

Now in your app:

```ts
// e.g. src/atif/import-open-thoughts.ts
import { Effect } from "effect";
import { OpenThoughtsService, OpenThoughtsServiceLive } from "../huggingface/open-thoughts-service.js";

const program = Effect.gen(function* () {
  const svc = yield* OpenThoughtsService;
  const trajectories = yield* svc.fetchSftTrajectories;

  // e.g. write .atif.jsonl files for your viewer
  // or stream them to your existing StreamingWriter
});

Effect.runPromise(
  program.pipe(
    Effect.provide(
      OpenThoughtsServiceLive({
        accessToken: process.env.HF_TOKEN!,
      })
    )
  )
);
```

### Making it “repeatedly” sync

You can wrap `fetchSftTrajectories` in a scheduled job:

```ts
import { Schedule } from "effect";

const loop = Effect.repeat(
  OpenThoughtsService.pipe(
    Effect.flatMap((svc) => svc.fetchSftTrajectories),
    // compare with last revision, store only new ones, etc.
  ),
  Schedule.spaced("1 day") // or from config
);
```

Or just call it on demand when you want to refresh your ATIF corpus.

If you want to be fancy, you can:

* Use `@huggingface/hub`’s `listFiles` output to track **revision/oids** and skip download if unchanged.
* Persist a small “last synced revision” file next to your ATIF data.

---

## 5. Summary

* **Download from**: `open-thoughts/OpenThoughts-Agent-v1-SFT`, file `data/train-00000-of-00001.parquet`.
* **Use**: `@huggingface/hub` → `listFiles` + `downloadFile` to fetch Parquet from TypeScript.
* **Parse**: with `parquetjs-lite` (or Arrow/DuckDB), get rows with `conversations` and metadata.
* **Convert**: each row → ATIF `Trajectory` for your viewer.
* **Service**: wrap this in an Effect `OpenThoughtsService` with `fetchSftTrajectories`, and optionally schedule it to run periodically.

If you’d like, next step I can help you define the exact ATIF schema mapping (including tool calls parsed out of the `commands` JSON inside `content`) so the viewer can show both “chat bubbles” and “terminal actions” for each OpenThoughts trace.

[1]: https://huggingface.co/datasets/open-thoughts/OpenThoughts-Agent-v1-SFT "open-thoughts/OpenThoughts-Agent-v1-SFT · Datasets at Hugging Face"
[2]: https://huggingface.co/datasets/open-thoughts/OpenThoughts-Agent-v1-SFT/tree/main/data "open-thoughts/OpenThoughts-Agent-v1-SFT at main"
[3]: https://huggingface.co/open-thoughts/datasets "open-thoughts (OpenThoughts)"
[4]: https://huggingface.co/datasets/open-thoughts/OpenThoughts-TB-dev "open-thoughts/OpenThoughts-TB-dev · Datasets at Hugging Face"
