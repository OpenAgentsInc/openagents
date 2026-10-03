# Source-read performance amendment

This follow-up changes deterministic source I/O after the three preparation runs.
It makes no model calls and changes no admission, scoring, ordering, catalog,
source-window, or packing policy. The 28 KiB candidate-state and 16 KiB pack limits
remain fixed.

The original implementation is preserved byte for byte as
[`context-at-round-one.py`](../../../../bench/jev-lifecycle/context-at-round-one.py),
SHA-256 `4e4bc6acf510bb852af38bc47e1bc20f66e3d3a2817e00ff1dcb4762a2d20acc`.
The amendment batches admitted source-file reads through `git cat-file
--batch-check` and `--batch`. It checks each immutable blob identity and the 2 MiB
per-file limit before fetching content, then validates response framing, UTF-8,
and the existing index size/hash binding. Package discovery and targeted reads
retain their original paths and semantics.

Before accepting this amendment, run focused malformed-response and bound tests,
then compare all context fields except elapsed time and both deterministic and
recorded-Jev packed outputs on the same three retained public inputs. Candidate
IDs, order, omissions, catalog, provenance, and payload hashes must match exactly.
Record five warm observations of each implementation on each task, alternating
implementation order. Retain all observations and their measurement boundary;
report the measured result even if preparation remains above one second.

No source or ranking policy may change to obtain a faster measurement. This
follow-up tests a deterministic implementation improvement and provides no new
estimate of Jev's benefit or native coding performance.
