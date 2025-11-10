# SQLite Cosine Similarity for Embeddings

Summary: Core SQLite does not have native vector columns, but we can add fast cosine similarity search with either a loadable extension (sqlite-vec) or by using libSQL/Turso. For a local-first Apple Silicon app, sqlite-vec is the most portable choice; for a hosted fork with built-in ANN indexing, libSQL/Turso is excellent.

- sqlite-vec (recommended locally)
  - Adds vector columns, KNN, and distance metrics including cosine: `vec_distance_cosine(a, b)`.
  - Simple setup: load the extension, create a `VIRTUAL TABLE ... USING vec0(...)`, store FLOAT[N] embeddings, query by `MATCH` or compute distances explicitly.
  - Works across platforms; actively maintained (successor to sqlite-vss).

- libSQL/Turso (remote or embedded fork)
  - Native vector types (e.g., `F32_BLOB(384)`) and vector indexes.
  - Supports cosine metric via `libsql_vector_idx(..., 'metric=cosine')` and `vector_top_k`.
  - Great for hosted/team scenarios; not pure upstream SQLite.

Example (sqlite-vec):

```sql
-- Create a vector table (384-dim)
CREATE VIRTUAL TABLE files USING vec0(
  path TEXT,
  embedding FLOAT[384]
);

-- Insert
INSERT INTO files(path, embedding)
VALUES ('src/router.swift', '[0.12, 0.03, ...]');

-- Exact cosine search (lower distance = more similar)
SELECT path,
       vec_distance_cosine(embedding, vec_f32('[0.11, 0.02, ...]')) AS distance
FROM files
ORDER BY distance ASC
LIMIT 10;

-- KNN-style
SELECT path, distance
FROM files
WHERE embedding MATCH vec_f32('[0.11, 0.02, ...]')
ORDER BY distance
LIMIT 10;
```

Recommendation for OpenAgents:

- Phase 1 (current): Keep brute-force cosine in Swift using Accelerate over our `embeddings` table (Float32 BLOBs, L2-normalized). This is already implemented in `VectorStore.search()` and is fine up to ~100k vectors.
- Phase 2: Add an experimental sqlite-vec adapter behind a feature flag for ANN/KNN speedups on larger corpora. Keep schema compatibility by mirroring our `id`, `collection`, `metadata` fields alongside the vector index table.
- Phase 3: Optional libSQL/Turso path for cloud or shared workspaces.

Implementation notes:

- Our current schema uses `BLOB` to store `[Float]` as raw bytes and normalizes vectors, so cosine reduces to dot product via Accelerate (`vDSP_dotpr`).
- If/when we adopt sqlite-vec, we can sync from our `embeddings` table into a `vec0` table per `collection` and query by cosine distance.

