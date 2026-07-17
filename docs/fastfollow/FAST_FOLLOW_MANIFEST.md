# FastFollowManifest 0.1

`@openagentsinc/fast-follow-spec` compiles an authored FastFollowSpec and exact
inventory values into an immutable `FastFollowManifest`. The compiler is pure:
it receives bytes, identities, and inventory records and does not read files,
run Git, inspect clocks, fetch sources, or call a model.

The manifest binds:

- compiler version and content digest;
- exact authored document and canonical intent digests;
- target commit, tree, and digests for every declared authority file or tree;
- each source's exact Git commit/tree or release identity/artifact digest;
- selected-corpus byte digests, visibility, provenance, license state, and
  evidence confidence;
- the resolved directive graph and deterministic evidence-only work units; and
- a content digest over the canonical payload.

Public Git inventory reads selected bytes from `HEAD` object storage rather
than a mutable working tree. Directory authorities are represented by a stable
digest over the ordered tracked file list and exact blob bytes. Artifact
inventory requires both observed bytes and a release identity; an installed
application label alone is insufficient. Symlinks and repository escapes fail
closed.

## Authority and provenance

Every compiled work unit has `authority: "evidence_only"`. Source repository
instructions remain `untrusted_study_data`; the manifest grants no mutation,
network, credential, provider, spend, release, deployment, or SCM authority.

`license: "unknown"` and `license: "known_restricted"` permit study but set
`source_code_copying` to `denied_license_unknown_or_restricted`. Only explicitly
declared `known_permissive` provenance permits source-code copying, and target
authority still governs whether any adaptation is admitted.

Closed-product observations carry one of `verified_bytes`, `observed_artifact`,
or `inferred_bundle`. Confidence labels describe the evidence boundary and do
not promote an observation into a claim about unobserved behavior.

## Drift

`checkManifestFreshness` compares a manifest with newly inventoried values. It
returns `fresh` or `stale` with typed target-commit, target-tree,
target-authority, source-identity, and source-corpus reasons. It never mutates
or silently rebinds the old manifest; a caller must compile a new immutable
manifest after drift.
