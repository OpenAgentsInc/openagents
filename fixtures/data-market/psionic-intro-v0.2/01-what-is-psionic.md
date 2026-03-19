# What Psionic Is

Psionic is a Rust-native machine learning stack. Its role is to own the
machine-facing execution substrate rather than the app-facing product shell.

At a high level, Psionic is trying to replace the usual mixed Python/C++ stack
with a coherent Rust crate family that can truthfully describe:

- what model or artifact ran
- what backend ran it
- what runtime and environment were used
- what receipts or proof artifacts were produced
- what training, serving, cluster, and transport facts are attached to the run

The easiest beginner framing is:

- inference: run a model for text generation or embeddings
- training: run bounded or larger training-class workflows with explicit state,
  checkpoints, and lineage
- serving: expose those capabilities through stable request and response
  contracts
- provider truth: let downstream systems report what actually happened without
  pretending the UI or market layer is part of Psionic

Psionic is broader than OpenAgents, but OpenAgents uses Psionic as one compute
substrate.

Important non-goals:

- Psionic is not the desktop UI.
- Psionic is not wallet or payout logic.
- Psionic is not market settlement authority.
- Psionic is not a hidden Python control plane behind Rust wrappers.

If a new reader only remembers one thing, it should be this:

Psionic owns execution truth. OpenAgents owns the desktop product shell and the
market-facing workflows above it.
