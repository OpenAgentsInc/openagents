# Workbench session resolution

`Saved` wraps an existing NIP-TERM session record with its layout owner.
Terminal members belong to that owner; typed resource members retain their
own host, generation, workspace, resource ID, and revision. A session carries
no credential and opens or dispatches nothing.

`Saved::resolve` reads the current owner admission and resource state for each
member separately. Missing, expired, changed-disclosure, revoked, unsupported,
stale, and lost references remain visible. An owner or generation change never
substitutes another resource. A resource resolver has no execution method.
The owner's transport must still enforce its current grant on every operation.

`Override` binds one device identity to the saved session ID and revision.
Validation reuses the NIP-TERM layout rules; overriding never mutates the
host record. The TTY client consumes these records through `--session` and
`--layout`; native adapters can reuse the same portable resolution contract.
