# Inference Engineering Book Notes

**STATUS (2026-07-08): RETIRED FOR NOW — not current direction.**
OpenAgents is focused on Khala Code and business-facing work
(`docs/fable/MASTER_ROADMAP.md` rev 6). This program is retired
until an explicit owner decision revives it (earliest
reconsideration: after cashflow-positive). Preserved for history;
do not route new work, issues, or copy from this document.


Status: reading notes and Khala relevance scan, 2026-06-23.

Source read: `~/Downloads/Inference Engineering.pdf` by Philip Kiely,
published by Baseten Books. The PDF is 259 pages. Text was extracted locally
for review and a sample render was checked to confirm the PDF was readable.

Existing local corpus read before writing these notes:

- [`../README.md`](../README.md)
- [`../2026-06-19-inference-gateway-business.md`](../2026-06-19-inference-gateway-business.md)
- [`../2026-06-19-pricing-model.md`](../2026-06-19-pricing-model.md)
- [`../2026-06-19-pricing-vs-factory.md`](../2026-06-19-pricing-vs-factory.md)
- [`../2026-06-19-agent-cloud-revshare-everywhere.md`](../2026-06-19-agent-cloud-revshare-everywhere.md)
- [`../2026-06-19-fireworks-provider.md`](../2026-06-19-fireworks-provider.md)
- [`../2026-06-19-gateway-gemini-live-verification.md`](../2026-06-19-gateway-gemini-live-verification.md)
- [`../2026-06-19-decentralized-serving-shard-wan.md`](../2026-06-19-decentralized-serving-shard-wan.md)
- [`../2026-06-19-leyten-compute-shard-audit.md`](../2026-06-19-leyten-compute-shard-audit.md)
- [`../2026-06-19-cloud-primitives-fine-tuning-sandbox-scaffold-advance.md`](../2026-06-19-cloud-primitives-fine-tuning-sandbox-scaffold-advance.md)
- [`../2026-06-22-long-running-inference-response-strategies.md`](../2026-06-22-long-running-inference-response-strategies.md)
- [`../2026-06-22-verified-work-must-execute-the-artifact.md`](../2026-06-22-verified-work-must-execute-the-artifact.md)
- [`../2026-06-23-khala-head-to-head-m8-status.md`](../2026-06-23-khala-head-to-head-m8-status.md)
- [`../khala.md`](../khala.md)
- [`../khala-buildout-roadmap.md`](../khala-buildout-roadmap.md)
- [`../khala-head-to-head-demo.md`](../khala-head-to-head-demo.md)
- [`../khala-in-the-world.md`](../khala-in-the-world.md)
- [`../speculative-decoding-article.md`](../speculative-decoding-article.md)
- [`../fixtures/khala-head-to-head-dry-run.v1.json`](../fixtures/khala-head-to-head-dry-run.v1.json)
- [`../fixtures/khala-head-to-head-recorded-run.v1.json`](../fixtures/khala-head-to-head-recorded-run.v1.json)

## Bottom Line

The book is relevant to Khala. It reinforces that Khala should be treated as an
inference platform and control plane, not just as a model alias. The strongest
near-term takeaways are:

- define production latency, cost, cache, verification, and queue telemetry in
  Khala receipts and manifests;
- make prefix-cache and session-affinity behavior first-class in routing;
- keep interactive calls streamed, and complete the async batch path for long
  or detached work;
- benchmark provider and Pylon lanes with realistic Khala traffic before
  optimizing engines;
- use established inference engines for Pylon serving and reserve custom
  distributed-runtime work for the Psionic evidence boundary;
- gate every performance optimization that can change outputs, especially
  quantization, with executed Khala evals and acceptance receipts.

## Files

- [`book-reading-notes.md`](book-reading-notes.md) summarizes the book by
  chapter and maps the material to Khala.
- [`khala-investigation-notes.md`](khala-investigation-notes.md) is the
  prioritized list of things worth looking into next.
