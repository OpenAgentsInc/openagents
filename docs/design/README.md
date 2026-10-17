# Design knowledge base

Internal guidance for designing OpenAgents, starting with a full Bitcoin wallet
using Lightning and Spark. Reviewed 2026-10-17. These pages record design
recommendations, not a claim that every described feature ships today.

| Read | Use it for |
| --- | --- |
| [Design principles](principles.md) | Hierarchy, language, accessibility, and shared components |
| [Wallet experience](wallet.md) | First use, send, receive, recovery, and agent payments |
| [Source notes](sources.md) | WalletUI specifics, Bitcoin Design references, and adaptation limits |

## Start a design task

1. Read the relevant flow in [Wallet experience](wallet.md).
2. Read the [Spark wallet audit](../audits/2026-10-17-spark-wallet.md) for
   supported features, missing app flows, and unresolved owner decisions.
3. Sketch the normal path, waiting states, failures, and recovery together.
4. Review the result against [Design principles](principles.md).

The audit owns the dated implementation assessment. The
[Breez documentation](../breez/README.md) owns wallet architecture and payment
contracts. This folder owns experience guidance. When they differ, distinguish
an earlier proposal from a current implementation finding before designing UI.
In particular, the September wallet design's separate-seed position differs
from the October audit's identity work.

## Grow this collection

Add focused topic pages as work produces useful guidance. Record the source,
review date or revision, what OpenAgents adopts, and what needs adaptation.
Link implementation evidence instead of copying changing status tables. Mark
new proposals and unresolved decisions explicitly. Add each page to this index.
Do not store real recovery words, payment secrets, or customer screenshots here.

Related: [UI systems](../ui/README.md), [Rust Native](../coder/rust-native/README.md),
[app workspace guide](../../apps/openagents/OPENAGENTS.md).
