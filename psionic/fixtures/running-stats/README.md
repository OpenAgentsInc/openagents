# Independent observation-statistics reference

`psionic/tests/running_stats_oracle.py` computes exact two-pass moments with
Fraction and the fixed population-variance transform with 70-digit Decimal.
The reference was frozen before the incremental Coax implementation. The
oracle checks this JSON and its generated Coax vectors; `--write` creates
only absent files and refuses changed existing contents.

`example.out` is the exact debug/release public-example transcript. The
[contract](../../docs/robotics/running-statistics-rb03.md) and
[receipt](../../../docs/verification/robotics-running-stats-rb03-2026-11-15.md)
state numerical bounds, observed results and the incomplete collector work.
