# Coder review

This package assembles, redacts, validates, and records evidence-backed reviews of Coder benchmark cycles.

Run a review from the repository root:

```sh
pnpm run coder:review -- <arguments>
```

The native OpenAgents CLI owns interactive and non-interactive Coder sessions. This package remains separate because it is an internal benchmark-analysis pipeline, not a user-facing CLI namespace.
