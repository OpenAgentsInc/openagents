# Private fleet rollout guard

These explicit Coax tools make no cloud changes or model calls. The CLI only
loads authenticated `gcloud` JSON snapshots, observes fields and renders the
result. Image/spec/traffic policy is in `fleet-rollout-policy.coax`; readiness,
provider/checkpoint, namespace, URL, IAM and security admission is in the separate
`fleet-rollout-snapshot-policy.coax` authority root.

```sh
coax/web/tool.sh psionic/tools/fleet-rollout-guard.coax candidate SERVICE TARGET_REV OLD_REV IMMUTABLE_IMAGE BEFORE.json CANDIDATE.json REVISION.json IAM_BEFORE.json IAM_AFTER.json OLD_REVISION.json
coax/web/tool.sh psionic/tools/fleet-rollout-guard.coax promoted SERVICE TARGET_REV OLD_REV IMMUTABLE_IMAGE BEFORE.json PROMOTED.json REVISION.json IAM_BEFORE.json IAM_AFTER.json OLD_REVISION.json
coax/web/tool.sh psionic/tools/fleet-answer-check.coax ELEPHANT_AMBIENT_RESPONSE.json
```

`candidate` requires old production traffic to stay at 100%; `promoted` requires
100% on the exact candidate. Both require that candidate to be ready with the
exact immutable image. The complete ordered template spec and resolved revision
spec may change only their image leaf. They are compared separately because
Cloud Run adds a resolved container name absent from the requested service spec.
Template annotations are retained. Selected service security annotations are
`run.googleapis.com/ingress`, `run.googleapis.com/invoker-iam-disabled` and
`run.googleapis.com/default-url-disabled`; bindings/version and private IAM,
URLs, namespaces and the existing model/provider pins are also checked. This is
not a claim that every operational metadata field is identical.

Inputs must come from authenticated Cloud Run service/revision/IAM reads. These
local files are operator observations, not signed cloud attestations. Treat raw
snapshots as private; publish only reviewed metadata, hashes and synthetic model
responses, never tokens, cookies or request headers.

```sh
coax/bin/coax test psionic/tools/fleet-rollout-snapshot-policy.coax --library
coax/bin/coax mutate psionic/tools/fleet-rollout-snapshot-policy.coax --root-only --jobs 4
```

The separate root keeps new snapshot-policy mutations scoped; retain the earlier
passing image/spec/traffic policy result when its source has not changed. Live
candidate qualification and traffic promotion are operator actions outside these
tools. Preserve the old immutable revision for rollback and promote the same
candidate digest that was qualified.

The [fleet container recipe](../platform/fleet-container/default.nix) uses a
fixed Nixpkgs archive/NAR pin and Nix-built OpenSSL. Its scratch runtime carries
the complete transitive closure, absolute ELF interpreter, shell and CA bundle.
The exporter includes both pin and recipe in its explicit source ledger. Keep
generated contexts free of caller/provider credentials. Inspect post-fixup
linkage and closure size, qualify the immutable image under its final nonroot
user and through a private real TLS request, then use these snapshot gates
before promotion. The [Nix rollout record](../../docs/verification/fleet-nix-rollout-2026-10-30.md)
retains concrete receipts, intermediate/failed attempts and final public timing.
