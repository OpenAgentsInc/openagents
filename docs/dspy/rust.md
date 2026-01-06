# DSPy in Rust

## OpenAgents Integration

DSRs (dspy-rs) is integrated into OpenAgents via the `rlm` crate's `dspy` feature.

```bash
cargo build -p rlm --features dspy
```

See:
- [RLM DSPy Documentation](../../crates/rlm/docs/DSPY.md) - Usage guide
- [DSPy + RLM Concepts](./rlm.md) - Conceptual background

---

## Ecosystem Overview

Today, **there's basically one real "DSPy-in-Rust" implementation**, and then a handful of small projects that *use* it.

## 1) DSRs (crate: `dspy-rs`) — the main Rust DSPy rewrite

**DSRs** (often described as “DSPy Rust”) is a **ground-up rewrite (not a line-by-line port)** of DSPy in Rust, published as the `dspy-rs` crate. ([GitHub][1])

What it covers (high level):

* **Signatures**: declarative input/output schemas via macros/attribute macros (e.g. `#[Signature]`, `sign!`, `example!`). ([GitHub][1])
* **Predictors / Modules**: a `Predict`-style building block and a `Module` trait for composing pipelines. ([dsrs.herumbshandilya.com][2])
* **Optimizers (teleprompter-style)**: it explicitly exposes optimizer modules including:

  * **COPRO** ([Docs.rs][3])
  * **MIPROv2** (there’s a full worked Rust example, `08-optimize-mipro.rs`) ([Docs.rs][3])
  * **GEPA** (reflective optimizer w/ feedback + Pareto selection) ([Docs.rs][3])
* **Evaluation + data loading** patterns (e.g. an `Evaluator` trait; examples show loading HuggingFace datasets). ([Docs.rs][4])
* **LM connectivity**: docs say it supports LMs via the `async-openai` ecosystem (and shows OpenAI + Ollama-style configuration). ([dsrs.herumbshandilya.com][2])

Current “project temperature” / maturity signals:

* Docs.rs shows it’s moving quickly (multiple releases in late 2025; e.g. `0.7.3` dated **2025-11-14**). ([Docs.rs][5])
* Docs.rs also reports **~51.6% documented** (so: real, but still evolving). ([Docs.rs][3])

## 2) Ecosystem: small Rust repos that *use* DSRs

There are example / proof-of-concept repos built on top of DSRs (not separate DSPy reimplementations). For example, **`AnthonyRonning/dspy-rs-search-example`** is a “classifier-first agent” architecture demo explicitly using “DSPy Rust” (DSRs as a submodule). ([GitHub][6])

## 3) What *doesn’t* really exist (yet)

I did not find a second, independent, widely-used Rust port at the same level as DSRs—most references converge on **DSRs / `dspy-rs`** as *the* Rust option right now. ([Reddit][7])

If you tell me which DSPy features you care about most (e.g., BootstrapFewShot-style optimizers, RAG primitives, assertions, tool use, tracing), I can map “Python DSPy → DSRs equivalents” and call out likely gaps you’d need to implement in Rust.

[1]: https://github.com/krypticmouse/DSRs "GitHub - krypticmouse/DSRs: Performance centered DSPy rewrite to(not port) Rust"
[2]: https://dsrs.herumbshandilya.com/docs/getting-started/quickstart "Quickstart - DSRs"
[3]: https://docs.rs/dspy-rs/latest/dspy_rs/optimizer/index.html "dspy_rs::optimizer - Rust"
[4]: https://docs.rs/dspy-rs/latest/src/08_optimize_mipro/08-optimize-mipro.rs.html "08-optimize-mipro.rs - source"
[5]: https://docs.rs/crate/dspy-rs/latest "dspy-rs 0.7.3 - Docs.rs"
[6]: https://github.com/AnthonyRonning/dspy-rs-search-example "GitHub - AnthonyRonning/dspy-rs-search-example"
[7]: https://www.reddit.com/r/rust/comments/1p55z99/dspy_in_rust/ "DSPy in Rust : r/rust"
