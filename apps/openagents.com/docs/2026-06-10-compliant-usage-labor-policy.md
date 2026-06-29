# Compliant-Usage Labor Policy

Policy ref: `provider.compliant_usage_labor.v1`

This policy governs OpenAgents labor-market jobs that run through Pylon,
NIP-90 labor job kinds, or adjacent accepted-work rails.

Contributors run jobs on their **own** provider accounts / API budgets under
their **own** provider terms.

OpenAgents pays for **accepted work output only**.

No provider credentials, sessions, or account access are ever transferred,
metered for resale, or brokered by OpenAgents or between participants.

Contributors are responsible for their own provider-terms compliance; the
runtime never exfiltrates provider auth material.

This is labor: selling work product, not capacity resale. Any feature request
that requires touching someone else's provider auth is out of policy and must
be declined.

Implementation notes:

- NIP-90 labor job helpers live in `nostr-effect` and are exposed through
  `@openagentsinc/nip90`.
- Labor requests carry input refs, acceptance criteria, expected artifact
  descriptors, bid/amount terms, and the policy ref.
- Labor results carry public-safe artifact refs and settlement amount evidence
  only. Raw provider payloads, credentials, session material, private logs,
  wallet material, invoices, preimages, and customer-private data stay out of
  public NIP-90 events and OpenAgents receipts.
