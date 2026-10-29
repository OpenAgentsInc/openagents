# Building Psionic in Coax

This book builds a machine-learning library from small understandable pieces.
Every completed chapter runs the actual [Psionic library](../README.md), with
checks and independently specified results.

[Chapter 1](ch01-learning-a-number.md) starts with learning a single number.
You need basic programming and enough arithmetic to multiply and average two
numbers. It explains the small amount of calculus as it goes. For the
language, use [the Coax book](../../coax/book/README.md).

Commands run from the OpenAgents repository root. `psionic/check.sh` checks the
whole current book and library. Build once with `coax/build.sh` if you want to
run examples directly without the check command.

| Chapter | Status | Result |
| --- | --- | --- |
| [1. Learning a number](ch01-learning-a-number.md) | Runnable: CPU | Train `prediction = weight * x`, check every loss, predict a held-out value, and refuse malformed work. |

The [contents](SUMMARY.md) list completed chapters. The
[roadmap](../../docs/psionic/roadmap.md) lists future work. Planned language and
ML features are not runnable claims. Exact transcripts describe the current
CPU arithmetic/printing profile; per-value tolerances state the numerical
contract independently of formatting.
