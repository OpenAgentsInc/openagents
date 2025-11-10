# Cosine Similarity Search with SQLite — Assessment and Plan

Last updated: 2025-11-10

## TL;DR

- We already support cosine similarity search today in the service layer, not inside SQLite: vectors are L2‑normalized and scored via Accelerate (vDSP) in Swift (VectorStore).
- Core SQLite has no vector KNN or cosine metric. To do cosine in SQL, use:
  - sqlite-vec (loadable extension; adds vector columns, KNN, cosine distance), or
  - libSQL/Turso (SQLite fork; native vector types + indexes; metric=cosine).
- Recommendation: keep current brute‑force Swift path for now; adopt sqlite‑vec (macOS) or libSQL/Turso when corpus size/latency requires ANN/SQL KNN (Phase 5+ in our plan).

## Current Implementation (What’s in the repo)

- Storage: `TinyvexDbLayer` persists embeddings as Float32 BLOBs in an `embeddings` table (id, collection, dims, model_id, metadata, timestamps).
- Service: `EmbeddingService` (macOS) coordinates the MLX provider and `VectorStore`.
- Search: `VectorStore` does cosine similarity as a dot product using vDSP (assumes L2‑normalized vectors) and returns top‑K. Brute‑force is sufficient for tens of thousands of vectors and avoids extension loading on iOS.

Implications:
- Consistent across iOS/macOS because the search is done in Swift.
- No SQL dependency for vector operations today.

## Doing Cosine in SQLite

SQLite itself doesn’t have vectors. Two practical paths add cosine KNN to SQL:

1) sqlite‑vec (recommended if you want a SQLite extension)
- Loadable C extension that adds vector columns and KNN search.
- Supports cosine distance (plus L2, L1, Hamming).

Example (384‑dim embeddings):
```sql
-- Create table with vector column
CREATE VIRTUAL TABLE files USING vec0(
  path TEXT,
  embedding FLOAT[384]
);

-- Insert JSON array (or BLOB)
INSERT INTO files(path, embedding)
VALUES ('src/router.swift', '[0.12, 0.03, ...]');

-- Exact cosine search (lower distance = closer)
SELECT path,
       vec_distance_cosine(embedding, vec_f32('[0.11, 0.02, ...]')) AS distance
FROM files
ORDER BY distance ASC
LIMIT 10;

-- KNN form (returns `distance` as well)
SELECT path, distance
FROM files
WHERE embedding MATCH vec_f32('[0.11, 0.02, ...]')
ORDER BY distance
LIMIT 10;
```

Notes:
- Designed to “run everywhere,” actively maintained; successor to sqlite‑vss.
- macOS can load extensions; iOS generally cannot load arbitrary SQLite extensions.

2) libSQL / Turso (SQLite fork)
- Native vector storage and indexes with metric selection, including cosine.

Example:
```sql
CREATE TABLE files (path TEXT, emb F32_BLOB(384));
CREATE INDEX files_vec_idx
  ON files(libsql_vector_idx(emb, 'metric=cosine'));

-- Top‑K by cosine
SELECT path
FROM vector_top_k('files_vec_idx', vector('[0.11, 0.02, ...]'), 10)
JOIN files ON files.rowid = id;
```

## When to Add SQL‑Level KNN

Add sqlite‑vec or libSQL/Turso when either of these holds:
- Corpus scales beyond ~100k vectors and brute‑force latency becomes a bottleneck.
- You need pure‑SQL KNN for integration simplicity or to leverage ANN indexes.

This matches our plan’s “Advanced Indexing (ANN)” milestone (Phase 5+). Until then, the Swift vDSP implementation is simpler, portable, and fast enough.

## Recommended Path for OpenAgents

- Near‑term (current):
  - Keep `VectorStore` brute‑force cosine (Swift/Accelerate) for semantic search.
  - SearchKit’s semantic leg should continue to call `EmbeddingService.semanticSearch()`.

- Mid‑term (Phase 5+):
  - macOS: load sqlite‑vec in Tinyvex and add a vector virtual table.
  - Backfill from `embeddings` table to vec virtual table; keep metadata in regular tables and join.
  - Update `VectorStore` to prefer SQL KNN if the vec extension is available; fallback to Swift otherwise.
  - Alternative: migrate macOS desktop to libSQL/Turso if native vector indexes are preferred.

- iOS considerations:
  - iOS cannot load arbitrary SQLite extensions; keeping vector search in Swift preserves portability. Our service runs on macOS, so desktop‑hosted vector KNN remains an option.

## Summary

- We support cosine search today via Swift (vDSP) with normalized vectors.
- SQLite KNN with cosine is available via sqlite‑vec or libSQL/Turso.
- Plan to introduce SQL‑level vector search when scale/latency demand it (Phase 5+), with desktop macOS hosting the extension or using libSQL/Turso.

