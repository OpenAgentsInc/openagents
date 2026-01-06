# Methods

The `rlm-methods` crate implements solution methods from the RLM paper.

## Method Trait

All methods implement:

```rust
#[async_trait]
pub trait Method: Send + Sync {
    fn name(&self) -> &str;
    async fn solve(&self, task: &dyn TaskInstance) -> Result<MethodResult>;
    async fn warmup(&mut self) -> Result<()> { Ok(()) }
    async fn reset(&mut self) -> Result<()> { Ok(()) }
}
```

## Base Method

Direct LLM call with full context. This is the simplest baseline.

### How It Works

1. Concatenate context and query into a prompt
2. Send to LLM
3. Return response as the answer

### Usage

```rust
use std::sync::Arc;
use rlm_methods::BaseMethod;
use lm_router::LmRouter;

let router = Arc::new(LmRouter::builder()
    .add_backend(backend)
    .build());

let method = BaseMethod::new(router, "model-name")
    .with_max_tokens(4096);
```

### Configuration

| Option | Default | Description |
|--------|---------|-------------|
| `max_tokens` | 4096 | Maximum response tokens |

### Prompt Template

```
You are a helpful assistant. Answer the following question based on the provided context.

Context:
{context}

Question: {query}

Please provide a direct and concise answer.
```

### Trajectory

Base method produces a simple trajectory:

```
Step 0: LlmCall { model: "...", prompt_tokens: X, completion_tokens: Y }
Step 1: Final { answer: "..." }
```

## Summary Agent Method

Iterative context summarization for handling very long contexts.

### How It Works

1. Chunk the context into manageable pieces
2. Summarize each chunk
3. Iteratively combine summaries until they fit in context
4. Use the final summary to answer the question

### Usage

```rust
use std::sync::Arc;
use rlm_methods::SummaryAgentMethod;
use lm_router::LmRouter;

let router = Arc::new(LmRouter::builder()
    .add_backend(backend)
    .build());

let method = SummaryAgentMethod::new(router, "model-name")
    .with_chunk_size(4000)           // Characters per chunk
    .with_max_tokens(2048)           // Max response tokens
    .with_target_summary_length(8000); // Target total length
```

### Configuration

| Option | Default | Description |
|--------|---------|-------------|
| `chunk_size` | 4000 | Characters per chunk |
| `max_tokens` | 2048 | Max response tokens |
| `target_summary_length` | 8000 | Target summary length |

### Algorithm

```
Input: context C, query Q

1. chunks = split(C, chunk_size)
2. summaries = []
3. for chunk in chunks:
      summaries.append(summarize(chunk))

4. while len(join(summaries)) > target_length:
      new_summaries = []
      for pair in pairs(summaries):
         new_summaries.append(combine(pair))
      summaries = new_summaries

5. final_summary = join(summaries)
6. answer = answer_question(final_summary, Q)
7. return answer
```

### Prompt Templates

**Chunk Summarization:**
```
Summarize the following text, preserving key facts and information that might be relevant for answering questions:

{text}

Summary:
```

**Combining Summaries:**
```
Combine the following summaries into a single coherent summary, preserving all key facts:

{summaries}

Combined Summary:
```

**Final Answer:**
```
Based on the following summarized context, answer the question.

Context Summary:
{summary}

Question: {query}

Answer:
```

### Trajectory

Summary agent produces a multi-step trajectory:

```
Step 0: SubQuery { prompt: "summarize chunk" }   # Chunk 1
Step 1: SubQuery { prompt: "summarize chunk" }   # Chunk 2
Step 2: SubQuery { prompt: "summarize chunk" }   # Chunk 3
Step 3: SubQuery { prompt: "combine summaries" } # Combine 1+2
Step 4: SubQuery { prompt: "combine summaries" } # Combine (1+2)+3
Step 5: LlmCall { model: "...", ... }            # Final answer
Step 6: Final { answer: "..." }
```

## Planned Methods

### CodeAct + BM25

ReAct-style agent with BM25 retrieval.

**Actions:**
- `search(query)`: BM25 retrieval over context
- `execute(code)`: Python code execution
- `final(answer)`: Return final answer

**Algorithm:**
```
while not done:
    action = llm.generate_action(context, history)
    if action == search(q):
        results = bm25_search(context, q)
        history.append(results)
    elif action == execute(code):
        output = python_exec(code)
        history.append(output)
    elif action == final(answer):
        return answer
```

### RLM Full

Recursive Language Model with `llm_query` capability.

**Key Feature:** The LLM can programmatically call itself:

```python
# Inside LLM-generated code
answer = llm_query("What is the capital of France?")
# answer = "Paris"
```

**Algorithm:**
```
while not done:
    code = llm.generate_code(context, query)
    if contains_llm_query(code):
        # Replace llm_query calls with actual LLM responses
        code = resolve_queries(code, llm)
    output = execute(code)
    if contains_final_answer(output):
        return extract_answer(output)
```

### RLM No Sub-calls (Ablation)

Same as RLM Full but with `llm_query` disabled.

This tests the contribution of recursive sub-calls vs. just having a REPL environment.

## Creating Custom Methods

```rust
use async_trait::async_trait;
use std::sync::Arc;
use bench_harness::{Method, MethodResult, TaskInstance, Trajectory, StepType};
use lm_router::LmRouter;

pub struct MyMethod {
    router: Arc<LmRouter>,
    model: String,
}

impl MyMethod {
    pub fn new(router: Arc<LmRouter>, model: impl Into<String>) -> Self {
        Self {
            router,
            model: model.into(),
        }
    }
}

#[async_trait]
impl Method for MyMethod {
    fn name(&self) -> &str {
        "my-method"
    }

    async fn solve(&self, task: &dyn TaskInstance) -> bench_harness::Result<MethodResult> {
        let mut trajectory = Trajectory::new(task.id(), self.name());

        // Your method logic here...

        let response = self.router
            .complete(&self.model, prompt, max_tokens)
            .await
            .map_err(|e| bench_harness::Error::MethodError(e.to_string()))?;

        trajectory.add_llm_call(&self.model, &response.usage, &response.text);

        let answer = extract_answer(&response.text);
        trajectory.add_final(&answer);

        Ok(MethodResult::new(answer, trajectory, response.usage))
    }
}
```

## Prompt Utilities

Helper functions for working with prompts:

```rust
use rlm_methods::prompts::{format_prompt, extract_final_answer};

// Format a prompt template
let prompt = format_prompt(
    "Context: {context}\n\nQuestion: {query}",
    "Some context",
    "What is X?",
);

// Extract answer from response
let response = "Let me think... FINAL_ANSWER: 42";
let answer = extract_final_answer(response);
assert_eq!(answer, Some("42".to_string()));
```

### Answer Extraction

`extract_final_answer` looks for these patterns:
- `FINAL_ANSWER: <answer>`
- `Answer: <answer>`

## Comparing Methods

```rust
use std::sync::Arc;
use bench_harness::{ExperimentRunner, ExperimentConfig, ExactMatchMetric};
use rlm_methods::{BaseMethod, SummaryAgentMethod};

let router = Arc::new(router);

let mut runner = ExperimentRunner::new(config, tasks);

// Add multiple methods
runner.add_method(Arc::new(BaseMethod::new(router.clone(), "model")));
runner.add_method(Arc::new(SummaryAgentMethod::new(router.clone(), "model")));

runner.add_metric(Box::new(ExactMatchMetric));

let results = runner.run().await?;

// Compare results
for (method, stats) in &results.per_method {
    println!("{}: {:.2}% (tokens: {})",
        method,
        stats.primary_score * 100.0,
        stats.mean_usage.total_tokens
    );
}
```
