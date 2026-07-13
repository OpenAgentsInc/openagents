# Authoring workflow

Generate the initial document with `assurance-spec propose`; do not copy a
nearby AssuranceSpec and repair its identity by hand. Preserve the generated
subject binding and criterion refs. Inventory entries are candidates, not
proof, and must not be promoted into obligations without reviewed reasoning.

For each criterion, separate distinct proof claims into stable obligation IDs.
Fill the complete proof design: domains, technique, environment refs, oracle,
falsifier, evidence requirements and proof rung, independence, dependencies,
and activation gate. A structurally valid obligation may still produce
`obligation_needs_design`; keep that diagnostic until the design is real.

Run both commands after every edit:

```sh
assurance-spec validate assurance/<name>.assurance-spec.md
assurance-spec coverage assurance/<name>.assurance-spec.md
```

Validation proves format integrity. Coverage reports design readiness. Neither
command admits the proposal or proves the subject implementation.
