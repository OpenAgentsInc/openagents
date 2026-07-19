# AssuranceSpec starter kit

Copy this directory's contents into a repository, replace the example
ProductSpec with the repository's accepted intent, and regenerate the proposal:

```sh
assurance-spec propose docs/product-specs/example.product-spec.md \
  --out assurance/example.assurance-spec.md --force
assurance-spec session begin assurance/example.assurance-spec.md --json
assurance-spec validate assurance/example.assurance-spec.md --json
assurance-spec ledgers assurance/example.assurance-spec.md --json
```

The committed `assurance/owned-runner.json` is consumed by
`assurance-spec owned-runner`. Structural validation blocks. Ledgers are
retained as information and never become a percentage or threshold gate.

OpenAgents intentionally does not include a `.github/workflows` file. This
repository forbids GitHub-hosted CI. Verification runs on OpenAgents-owned
infrastructure or locally through the same deterministic command. Downstream
repositories may integrate the command with infrastructure they control, but
the starter kit grants no hosted runner, credential, admission, or release
authority.

The npm package is published only after the separately authenticated owner
step. Until then, monorepo consumers use the workspace package and distribution
verification uses the exact local tarballs produced by `pack:public`.
