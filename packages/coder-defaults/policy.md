# Coder defaults adoption policy

A tool is a candidate for Coder's defaults when a published result on a
test set is Better and checks by at least three distinct trainers
confirmed it. A trainer's linked keys count as one trainer, and a check by
the result's trainer or the test set's author doesn't count.

A candidate is adopted only when an OpenAgents operator decides to adopt
it. The decision is an `openagents.eval-admission.v1` document that cites
the confirmed reports, and the next `coder-defaults` release depends on
the tool's release and cites the decision.

Adoption earns XP under NIP-XP's `eval-adopt` rule. XP is never spent,
transferred, or converted, and adoption pays no money.
