Below is a pragmatic shortlist of **embedding models you can run locally on an Apple‑Silicon Mac** using either **Swift MLX** (via Apple’s MLX stack) or **Apple system frameworks**. I’ve focused on models that are well‑suited for **cosine‑similarity search over short natural‑language descriptions of codebase files** (e.g., “`src/router: URL parsing + auth guard`”).

---

## TL;DR — What to pick

* **Fast & tiny (English):** `all-MiniLM-L6-v2` (≈384‑d) or `bge-small-en-v1.5`. Good quality, tiny memory, ideal for 1–2 sentence file descriptions. ([Hugging Face][1])
* **Balanced quality (English) with adjustable dimensions:** `mxbai-embed-large-v1` (supports “Matryoshka” truncation to smaller dims like 512 to save space). ([Hugging Face][2])
* **Multilingual (many languages):** `snowflake-arctic-embed-l-v2.0` (MLX 4‑bit/8‑bit variants available). ([Hugging Face][3])
* **Small but modern (Google):** `embeddinggemma-300m-4bit` (task prompts include **code retrieval**). ([Hugging Face][4])
* **High‑accuracy, heavy:** `e5-mistral-7b-instruct` (4096‑d; slower/bigger than you likely need for short file blurbs). ([Hugging Face][5])

> If you just want something that works well and is light: start with **MiniLM** or **BGE‑small**. If you need multilingual or a bit more “bite,” try **Arctic‑Embed** or **MXBAI**.

---

## Option A — Run embeddings with **Swift MLX**

Apple’s **MLX** project has a Swift stack and example packages. The **MLX Swift Examples** repo exposes an **`MLXEmbedders`** library (Swift Package) for popular encoder / embedding models; add the package and pull an embedding model by its Hugging Face ID (many are already converted to MLX). ([GitHub][6])

### Known‑good MLX models (ready on Hugging Face in MLX format)

| Model                                                                    | Why use it                                                                                        | Notes / evidence                                                                                    |                                                                      |
| ------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------- |
| **`sentence-transformers/all-MiniLM-L6-v2`** (MLX 4‑bit build available) | Tiny, fast; 384‑dim vectors; great for short descriptions.                                        | The original model card states 384‑dim sentence vectors; MLX conversions exist. ([Hugging Face][1]) |                                                                      |
| **`BAAI/bge-small-en-v1.5`** (MLX 6‑bit build available)                 | Small/accurate English retrieval; robust baselines.                                               | MLX variant page; BGE documentation on retrieval usage. ([Hugging Face][7])                         |                                                                      |
| **`mlx-community/mxbai-embed-large-v1`**                                 | Strong general performance; supports **Matryoshka** (truncate to 768/512 dims to reduce storage). | MLX page shows usage + mentions MRL/truncation flow on base card. ([Hugging Face][8])               |                                                                      |
| **`mlx-community/snowflake-arctic-embed-l-v2.0`** (4‑bit & 8‑bit)        | Multilingual (≈70+ languages), high retrieval quality.                                            | MLX model pages; Snowflake’s write‑up ranks it strongly on MTEB retrieval. ([Hugging Face][3])      |                                                                      |
| **`mlx-community/embeddinggemma-300m-4bit`**                             | Small, modern; supports **task prefixes** including **`task: code retrieval                       | query:`**.                                                                                          | MLX page includes code‑retrieval prompt strings. ([Hugging Face][4]) |
| **`mlx-community/e5-mistral-7b-instruct-mlx`**                           | Very strong, but heavy; 4,096‑dim embeddings.                                                     | MLX page explicitly lists 4096‑dim. ([Hugging Face][5])                                             |                                                                      |
| **`mlx-community/Qwen3-Embedding-0.6B-4bit-DWQ`**                        | Small multilingual embedder variant in MLX.                                                       | MLX conversion page. ([Hugging Face][9])                                                            |                                                                      |

> The **MLX Community** org on Hugging Face curates MLX‑converted weights for Apple Silicon; you can also **convert/quantize** your own embedding model to MLX with `mlx-lm convert`. ([Hugging Face][10])

#### How to wire into a Swift app (high level)

1. **Add MLX Swift Examples** as a dependency and link **`MLXEmbedders`** (see repo README). ([GitHub][6])
2. **Load an embedder** by its model ID (e.g., `mlx-community/bge-small-en-v1.5-6bit`) and call its encode function to get vectors. (The package’s docs/README show the pattern and the dependency setup.) ([GitHub][6])
3. **Normalize** each vector (L2) and store them (e.g., as `Float32`/`Float16`) alongside your file metadata.
4. At query time, **embed the query**, normalize, and do cosine similarity (dot product of unit vectors).

> If you need to bring an embedding model that doesn’t have MLX weights yet, the **`mlx-lm`** toolkit provides a one‑liner conversion/quantization path (`mlx_lm.convert --hf-path ... -q`). ([GitHub][11])

---

## Option B — Use **Apple system frameworks** (no MLX required)

As of iOS 18/macOS 15, Apple’s **Foundation Models** framework **does not ship an “Embeddings API”** for vectorization. Apple suggests pairing Foundation Models with a separate embedding solution; on device, that can be Apple’s **Natural Language** framework, which provides **word/sentence embeddings** and, on newer OSes, **contextual sentence embeddings**. ([Apple Developer][12])

* **`NLEmbedding.sentenceEmbedding(for:)`** — classic sentence embeddings (English, etc.). Good for fast, local semantics. ([Apple Developer][13])
* **`NLContextualEmbedding` (iOS 18+/macOS 15+)** — transformer‑based **contextual** sentence embeddings with broader language support; more modern than the classic static embeddings. ([Apple Developer][14])

> Apple’s own dev forums note that **RAG is possible on-device**, but **you must provide your own embeddings** (Natural Language or a Core ML/MLX model) and your own vector search. ([Apple Developer][12])

### Minimal Swift example (Apple **NaturalLanguage**, cosine over file snippets)

```swift
import NaturalLanguage
import Accelerate

// 1) Get an on-device sentence embedder (English shown; pick your language)
guard let embedder = NLEmbedding.sentenceEmbedding(for: .english) else {
    fatalError("No on-device sentence embedding available.")
}

// 2) Embed and L2-normalize (helper)
func embed(_ text: String) -> [Float] {
    let vectorDoubles = embedder.vector(for: text) ?? []
    let v = vectorDoubles.map { Float($0) }
    var norm: Float = 0
    vDSP_svesq(v, 1, &norm, vDSP_Length(v.count))
    norm = sqrtf(norm)
    return v.map { $0 / max(norm, 1e-12) }
}

// 3) Build your index (vectors + file paths/comments)
let corpus: [(path: String, desc: String)] = [
  ("src/router.swift", "Parses URLs and applies auth guard"),
  ("db/migrate.sql", "Schema migration scripts for v3"),
  // ...
]
let index = corpus.map { (path: $0.path, vec: embed($0.desc)) }

// 4) Query
let q = embed("auth check in HTTP routing")
let topK = index
  .map { (path: $0.path, score: vDSP_dotpr(q, 1, $0.vec, 1, vDSP_Length(q.count))) } // cosine == dot (already normalized)
  .sorted { $0.score > $1.score }
  .prefix(5)

print(topK)
```

API references: **`NLEmbedding.sentenceEmbedding(for:)`**, **`NLEmbedding.vector(for:)`**. For contextual embeddings on iOS 18+/macOS 15+, see **`NLContextualEmbedding`**. ([Apple Developer][13])

---

## Practical notes for codebase‑file descriptions

* **Normalization & cosine:** Always L2‑normalize vectors once; then cosine similarity is just a dot product. (The examples in MLX cards often demonstrate dot products on normalized outputs.) ([Hugging Face][8])
* **Prompting style (when relevant):** Some embedding families expect a **query instruction** only on the *query* side (not documents) for best retrieval (e.g., BGE, MXBAI). Use the model card’s suggestion if you want peak quality; otherwise plain text usually works fine for short file blurbs. ([Hugging Face][15])
* **Dimensionality & storage:**

  * MiniLM/BGE‑small (~384‑d) → ~1.5 KB/vec in `float32` (~0.75 KB in `float16`). Great if you’re indexing tens/hundreds of thousands of files. ([Hugging Face][1])
  * MXBAI supports **Matryoshka** truncation (e.g., 1024 → 512 dims) with graceful quality trade‑offs. ([Hugging Face][2])
  * E5‑Mistral 7B uses 4096‑d vectors; much larger index. Prefer only if you really need the extra accuracy. ([Hugging Face][5])
* **Multilingual:** For repos with mixed non‑English comments/READMEs, **Arctic‑Embed** and **Qwen3‑Embedding** are good small/medium picks; Apple’s `NLContextualEmbedding` also broadened language coverage in iOS 18/macOS 15. ([Hugging Face][3])

---

## Example: MLX choices (quick links)

* **MiniLM (fast, tiny):** MLX 4‑bit build: `mlx-community/all-MiniLM-L6-v2-4bit`. Original model is 384‑d. ([Hugging Face][16])
* **BGE‑small (fast, accurate):** MLX 6‑bit: `mlx-community/bge-small-en-v1.5-6bit`. ([Hugging Face][7])
* **MXBAI (balanced, MRL):** `mlx-community/mxbai-embed-large-v1`. ([Hugging Face][8])
* **Arctic‑Embed (multilingual):** `mlx-community/snowflake-arctic-embed-l-v2.0-4bit`. ([Hugging Face][3])
* **EmbeddingGemma (small, code‑aware prompts):** `mlx-community/embeddinggemma-300m-4bit`. ([Hugging Face][4])
* **E5‑Mistral (heavy):** `mlx-community/e5-mistral-7b-instruct-mlx` (4096‑d). ([Hugging Face][5])

> To use these from Swift, add the **MLX Swift Examples** package and refer to the **`MLXEmbedders`** library; the repo README shows installation and the family of example libraries. ([GitHub][6])

---

## Does the **Foundation Models** (“aModels”) API provide embeddings?

* Apple’s **Foundation Models** framework (the public API to Apple’s on‑device LLM) **does not currently expose a dedicated embedding API**; Apple’s own guidance is to pair it with a separate embedding solution (e.g., **NaturalLanguage** embeddings or a Core ML/MLX embedder) and your own vector search. ([Apple Developer][12])
* You can still do **RAG** fully on‑device by: (1) generating document vectors via `NLEmbedding`/`NLContextualEmbedding` or MLX models, (2) running cosine‑similarity search locally, (3) passing retrieved text to the Foundation Model for the final response. ([Apple Developer][12])

---

## Implementation tips (cosine search over file descriptions)

* **Create short, specific descriptions** (1–2 lines) per file or module; embed **titles + 1‑sentence summary**.
* **Normalize once** and store vectors in `Float16` to cut index size in half with minimal recall loss for cosine search.
* **Accelerate**: use `vDSP` for dot products (see Swift sample above) or batch with `BNNS` if you need large in‑memory top‑K.
* **Indexing**: for macOS apps, a light option is an in‑process ANN library or an embedded DB with a vector extension; but for a few 10^4 files, a pure Swift in‑memory scan with `vDSP` is often already sub‑millisecond.

---

### References & docs

* **MLX Swift & Embedders:** MLX Swift Examples repo (mentions **`MLXEmbedders`** library and how to add it as a Swift Package). ([GitHub][6])
* **Converting models to MLX:** `mlx-lm convert` tool; MLX + Hugging Face integration notes. ([GitHub][11])
* **NL sentence embeddings:** `NLEmbedding.sentenceEmbedding(for:)`. ([Apple Developer][13])
* **NL contextual embeddings (iOS 18+/macOS 15+):** `NLContextualEmbedding`. ([Apple Developer][14])
* **Foundation Models don’t ship embeddings:** Apple dev forums thread highlighting the need to bring your own embedding model for RAG. ([Apple Developer][12])
* **Model cards (MLX builds):** MiniLM, BGE‑small, MXBAI, Arctic‑Embed, EmbeddingGemma, E5‑Mistral, Qwen3‑Embedding. ([Hugging Face][16])

---

## What I’d do for your use case

1. Start with **`mlx-community/bge-small-en-v1.5-6bit`** or **`mlx-community/all-MiniLM-L6-v2-4bit`** (fast, tiny index). ([Hugging Face][7])
2. If you later need **more accuracy** without much latency hit, switch to **`mlx-community/mxbai-embed-large-v1`** at **512 dims** (Matryoshka). ([Hugging Face][2])
3. If you have **non‑English content**, try **`mlx-community/snowflake-arctic-embed-l-v2.0-4bit`**. ([Hugging Face][3])
4. If you want to avoid MLX entirely and stay inside Apple frameworks, use **`NLEmbedding`** or **`NLContextualEmbedding`** and keep cosine search with **Accelerate**. ([Apple Developer][13])

If you want, tell me roughly how many files you’ll index and your language mix—I'll size the embedding dims and memory footprint and sketch the top‑K search path for your target latency.

[1]: https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2 "sentence-transformers/all-MiniLM-L6-v2 · Hugging Face"
[2]: https://huggingface.co/mixedbread-ai/mxbai-embed-large-v1 "mixedbread-ai/mxbai-embed-large-v1 · Hugging Face"
[3]: https://huggingface.co/mlx-community/snowflake-arctic-embed-l-v2.0-4bit?utm_source=chatgpt.com "mlx-community/snowflake-arctic-embed-l-v2.0-4bit"
[4]: https://huggingface.co/mlx-community/embeddinggemma-300m-4bit "mlx-community/embeddinggemma-300m-4bit · Hugging Face"
[5]: https://huggingface.co/mlx-community/e5-mistral-7b-instruct-mlx "mlx-community/e5-mistral-7b-instruct-mlx · Hugging Face"
[6]: https://github.com/ml-explore/mlx-swift-examples "GitHub - ml-explore/mlx-swift-examples: Examples using MLX Swift"
[7]: https://huggingface.co/mlx-community/bge-small-en-v1.5-6bit "mlx-community/bge-small-en-v1.5-6bit · Hugging Face"
[8]: https://huggingface.co/mlx-community/mxbai-embed-large-v1 "mlx-community/mxbai-embed-large-v1 · Hugging Face"
[9]: https://huggingface.co/mlx-community/Qwen3-Embedding-0.6B-4bit-DWQ "mlx-community/Qwen3-Embedding-0.6B-4bit-DWQ · Hugging Face"
[10]: https://huggingface.co/mlx-community?utm_source=chatgpt.com "MLX Community"
[11]: https://github.com/ml-explore/mlx-lm?utm_source=chatgpt.com "ml-explore/mlx-lm: Run LLMs with MLX"
[12]: https://developer.apple.com/forums/forums/topics/machine-learning-and-ai?utm_source=chatgpt.com "Machine Learning & AI | Apple Developer Forums"
[13]: https://developer.apple.com/documentation/naturallanguage/nlembedding/sentenceembedding%28for%3A%29?utm_source=chatgpt.com "sentenceEmbedding(for:) | Apple Developer Documentation"
[14]: https://developer.apple.com/documentation/naturallanguage/nlcontextualembedding?utm_source=chatgpt.com "NLContextualEmbedding | Apple Developer Documentation"
[15]: https://huggingface.co/BAAI/bge-small-en-v1.5 "BAAI/bge-small-en-v1.5 · Hugging Face"
[16]: https://huggingface.co/mlx-community/all-MiniLM-L6-v2-4bit "mlx-community/all-MiniLM-L6-v2-4bit · Hugging Face"
