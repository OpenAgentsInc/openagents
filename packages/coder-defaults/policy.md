# Coder defaults adoption policy

A tool is a candidate for Coder's defaults when a published result on a
test set is Better, checks by at least three distinct trainers confirmed
it, and at least one Better result on a second test set externally
validates it. A trainer's linked keys count as one trainer, and a check by
the result's trainer or the test set's author doesn't count. A validating
test set counts only when someone other than the tool's author released
it, after the tool's release, on the same task distribution: reproduction
on the author's own tests proves reproducibility; this proves the result
wasn't fitted to them.

A candidate is adopted only when an OpenAgents operator decides to adopt
it. The decision is an `openagents.eval-admission.v1` document that cites
the confirmed reports and the validating result, and the next
`coder-defaults` release depends on the tool's release and cites the
decision. Once the defaults hold anything, the decision should also cite a
marginal report, current defaults plus the candidate against current
defaults alone, and a regression check across the whole set; the evidence
asked of a candidate rises with what a wrong decision by it could do.

Adoption earns XP under NIP-XP's `eval-adopt` rule. XP is never spent,
transferred, or converted, and adoption pays no money.
