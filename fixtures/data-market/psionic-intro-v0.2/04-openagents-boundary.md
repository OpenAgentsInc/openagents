# Psionic And OpenAgents

OpenAgents uses Psionic as one downstream compute substrate, but the projects
do not own the same product layers.

From the Psionic side, the clean split is:

- Psionic owns execution substrate and evidence.
- OpenAgents owns desktop control, UI, and market workflows.
- Hosted or kernel authority owns canonical market and settlement objects.

This boundary matters for the Data Market because the thing being sold here is
not "the Psionic runtime itself." The bundle is introductory documentation
about Psionic.

An agent buying this dataset should understand:

- the bundle is educational material, not executable model artifacts
- the content was derived from the current Psionic repo on `2026-03-18`
- the bundled files are intentionally smaller and more introductory than the
  full Psionic source tree
- canonical authority still lives in the original Psionic docs, especially
  `README.md`, `docs/ARCHITECTURE.md`, and `docs/TRAIN_SYSTEM.md`

For OpenAgents `v0.2`, that makes this a good sample listing because it
exercises the current packaging, listing, grant, and buyer handoff path
without pretending the market already has a broad polished catalog or remote
blob-delivery product.
