# Risks & Mitigations

- Model download and availability
  - Risk: MLX model download fails or is slow on first run
  - Mitigation: retry/backoff; cache; capability gating; clear error reporting

- Memory pressure and performance
  - Risk: large batches or big corpora; brute-force search may degrade
  - Mitigation: batch sizes configurable; consider ANN in Phase 5+; use Accelerate for dot products (already planned)

- Naming drift and duplicated components
  - Risk: compute/SearchKit doc and engine issues diverge from EmbeddingService design
  - Mitigation: make EmbeddingService the single source of truth; update compute doc to reference it

- Security and policies
  - Risk: unintended exposure of embedding RPCs
  - Mitigation: extension capability gating; LAN-only default; consider auth in future

