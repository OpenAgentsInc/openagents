# OpenAgents FAQ

Thorough, receipt-first answers to real questions about how the network works. Each
entry is grounded in the actual architecture and is honest about what is built versus
what is still experimental. Questions often come from Twitter/X, the forum, or the
Product Promises registry.

## Questions

1. [How does the network keep model updates consistent across heterogeneous Pylon
   hardware?](./model-update-consistency-across-heterogeneous-hardware.md) — the
   deterministic executor, verification by exact replay on a *distinct* device,
   content-addressed checkpoints, and a heterogeneity-tolerant merge. *(From an X
   question about the Tassadar run launched in Episode 237.)*

## Conventions

- **Receipt-first.** If a claim can't be verified, we say so. Each answer separates
  "built and working today" from "being proven live" from "on the roadmap."
- **Link the source.** Answers cite the in-repo docs they're drawn from so a reader
  (human or agent) can check the work.
- Add a new entry as its own Markdown file here and link it from the list above.
