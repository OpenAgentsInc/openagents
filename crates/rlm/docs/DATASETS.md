# Datasets

**Archived:** The `bench-datasets` crate is not part of the current workspace. This document is
retained for historical reference.

The `bench-datasets` crate provides loaders for various benchmark datasets.

## Dataset Trait

All datasets implement:

```rust
#[async_trait]
pub trait Dataset: Send + Sync {
    type Task: TaskInstance;

    fn name(&self) -> &str;
    fn description(&self) -> &str;
    async fn load(&self) -> Result<Vec<Self::Task>>;
    fn expected_count(&self) -> Option<usize>;
    fn primary_metric(&self) -> &str;
}
```

## Configuration

All datasets accept a `DatasetConfig`:

```rust
let config = DatasetConfig::new("./data/sniah")
    .max_tasks(50)      // Limit tasks (for debugging)
    .seed(42)           // Random seed for sampling
    .shuffle();         // Shuffle task order
```

## S-NIAH (Single-Needle In A Haystack)

Tests finding a single piece of information in a large context.

### Usage

```rust
use bench_datasets::{SnihDataset, DatasetConfig, Dataset};

let dataset = SnihDataset::new(DatasetConfig::new("./data/sniah"));
let tasks = dataset.load().await?;

assert_eq!(dataset.name(), "s-niah");
assert_eq!(dataset.primary_metric(), "exact_match");
```

### Data Format

**JSONL format** (`sniah.jsonl`):
```json
{"id": "sniah-001", "query": "What is the secret code?", "context": "Long text with needle hidden...", "needle": "ABC123", "needle_position": 0.5}
{"id": "sniah-002", "query": "Find the password", "context": "...", "needle": "XYZ789"}
```

**JSON array format** (`sniah.json`):
```json
[
  {"id": "sniah-001", "query": "...", "context": "...", "needle": "..."},
  {"id": "sniah-002", "query": "...", "context": "...", "needle": "..."}
]
```

### Fields

| Field | Required | Description |
|-------|----------|-------------|
| `id` | Yes | Unique task identifier |
| `query` | Yes | The question to answer |
| `context` | Yes | The haystack text |
| `needle` | Yes | The answer to find |
| `needle_position` | No | Position in context (0.0-1.0) |
| `context_tokens` | No | Approximate token count |

### Synthetic Generation

For testing:

```rust
use bench_datasets::sniah::generate_synthetic_tasks;

let tasks = generate_synthetic_tasks(
    50,     // Number of tasks
    10000,  // Context length in characters
);
```

## BrowseComp-Plus

Web browsing comprehension with a document corpus.

### Usage

```rust
use bench_datasets::{BrowseCompDataset, DatasetConfig, Dataset};

let dataset = BrowseCompDataset::new(DatasetConfig::new("./data/browsecomp"));
let tasks = dataset.load().await?;

assert_eq!(dataset.name(), "browsecomp-plus");
assert_eq!(dataset.primary_metric(), "exact_match");
assert_eq!(dataset.expected_count(), Some(150));
```

### Data Format

Expects a directory with two files:

**`corpus.jsonl`** (or `corpus.json`):
```json
{"id": "doc-001", "title": "Page Title", "content": "Document text...", "url": "https://..."}
{"id": "doc-002", "title": "Another Page", "content": "More text..."}
```

**`tasks.jsonl`** (or `tasks.json`):
```json
{"id": "task-001", "question": "What is X?", "answer": "Y", "gold_doc_ids": ["doc-001", "doc-003"], "difficulty": "easy"}
{"id": "task-002", "question": "When did Z happen?", "answer": "2020", "gold_doc_ids": ["doc-002"]}
```

### Fields

**Corpus:**
| Field | Required | Description |
|-------|----------|-------------|
| `id` | Yes | Document identifier |
| `content` | Yes | Document text |
| `title` | No | Document title |
| `url` | No | Source URL |

**Tasks:**
| Field | Required | Description |
|-------|----------|-------------|
| `id` | Yes | Task identifier |
| `question` | Yes | The question |
| `answer` | Yes | Expected answer |
| `gold_doc_ids` | No | Document IDs containing answer |
| `difficulty` | No | Difficulty level |

### Synthetic Generation

```rust
use bench_datasets::browsecomp::generate_synthetic_tasks;

let tasks = generate_synthetic_tasks(50);
```

## OOLONG

Long-context understanding with two variants.

### TrecCoarse (Numeric Counting)

```rust
use bench_datasets::{OolongDataset, DatasetConfig, Dataset};

let dataset = OolongDataset::trec_coarse("./data/oolong");
let tasks = dataset.load().await?;

assert_eq!(dataset.name(), "oolong-trec");
assert_eq!(dataset.primary_metric(), "numeric_decay");
```

### Pairs (Set Aggregation)

```rust
let dataset = OolongDataset::pairs("./data/oolong");
let tasks = dataset.load().await?;

assert_eq!(dataset.name(), "oolong-pairs");
assert_eq!(dataset.primary_metric(), "f1");
```

### Data Format

**TrecCoarse** (`trec_coarse.jsonl`):
```json
{"id": "trec-001", "query": "How many documents mention X?", "context": "...", "answer": 42.0, "tolerance": 2.0}
{"id": "trec-002", "query": "Count documents about Y", "context": "...", "answer": 15.0}
```

**Pairs** (`pairs.jsonl`):
```json
{"id": "pairs-001", "query": "List all items in category X", "context": "...", "answers": ["item1", "item2", "item3"]}
{"id": "pairs-002", "query": "Find pairs with property Y", "context": "...", "answers": ["A-B", "C-D"]}
```

### Fields

**TrecCoarse:**
| Field | Required | Description |
|-------|----------|-------------|
| `id` | Yes | Task identifier |
| `query` | Yes | Counting question |
| `context` | Yes | Documents to count |
| `answer` | Yes | Numeric answer |
| `tolerance` | No | Acceptable margin |

**Pairs:**
| Field | Required | Description |
|-------|----------|-------------|
| `id` | Yes | Task identifier |
| `query` | Yes | Aggregation question |
| `context` | Yes | Source documents |
| `answers` | Yes | Expected answer set |

### Synthetic Generation

```rust
use bench_datasets::oolong::{generate_synthetic_trec_tasks, generate_synthetic_pairs_tasks};

let trec_tasks = generate_synthetic_trec_tasks(50);
let pairs_tasks = generate_synthetic_pairs_tasks(50);
```

## CodeQA

Multiple-choice code comprehension from LongBench v2.

### Usage

```rust
use bench_datasets::{CodeQADataset, DatasetConfig, Dataset};

let dataset = CodeQADataset::new(DatasetConfig::new("./data/codeqa"));
let tasks = dataset.load().await?;

assert_eq!(dataset.name(), "codeqa");
assert_eq!(dataset.primary_metric(), "multiple_choice_accuracy");
```

### Data Format

**JSONL format** (`codeqa.jsonl`):
```json
{"id": "codeqa-001", "question": "What does this function return?", "code": "def foo():\n    return 42", "choices": ["41", "42", "43", "None"], "answer": "B", "language": "python"}
{"id": "codeqa-002", "question": "What is the time complexity?", "code": "...", "choices": ["O(1)", "O(n)", "O(n^2)", "O(log n)"], "answer": "C"}
```

### Fields

| Field | Required | Description |
|-------|----------|-------------|
| `id` | Yes | Task identifier |
| `question` | Yes | The question |
| `code` | Yes | Code context |
| `choices` | Yes | Answer options (4 items) |
| `answer` | Yes | Correct answer (A/B/C/D) |
| `language` | No | Programming language |

### Synthetic Generation

```rust
use bench_datasets::codeqa::generate_synthetic_tasks;

let tasks = generate_synthetic_tasks(50);
```

## Directory Structure

Recommended data organization:

```
data/
├── sniah/
│   └── sniah.jsonl
├── browsecomp/
│   ├── corpus.jsonl
│   └── tasks.jsonl
├── oolong/
│   ├── trec_coarse.jsonl
│   └── pairs.jsonl
└── codeqa/
    └── codeqa.jsonl
```

## Creating Custom Datasets

Implement the `Dataset` trait:

```rust
use async_trait::async_trait;
use bench_datasets::{Dataset, DatasetConfig, Result};
use bench_harness::{SimpleTask, GroundTruth, TaskMetadata};

pub struct MyDataset {
    config: DatasetConfig,
}

#[async_trait]
impl Dataset for MyDataset {
    type Task = SimpleTask;

    fn name(&self) -> &str { "my-dataset" }

    fn description(&self) -> &str {
        "Description of my custom dataset"
    }

    fn primary_metric(&self) -> &str { "exact_match" }

    fn expected_count(&self) -> Option<usize> { Some(100) }

    async fn load(&self) -> Result<Vec<SimpleTask>> {
        // Load from self.config.data_path
        let mut tasks = vec![];

        // ... load logic ...

        // Optionally shuffle
        if self.config.shuffle {
            // shuffle tasks
        }

        // Optionally limit
        if let Some(max) = self.config.max_tasks {
            tasks.truncate(max);
        }

        Ok(tasks)
    }
}
```
