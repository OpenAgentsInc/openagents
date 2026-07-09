# oa-workroomd Workroom Lifecycle

Status: Cloud MVP scaffold for `CND-017`

`oa-workroomd lifecycle` models the local lifecycle state machine for a managed
workroom. The MVP persists state and receipts in the workroom state directory so
restart sees the last active state.

```bash
oa-workroomd lifecycle status --json
oa-workroomd lifecycle create --json
oa-workroomd lifecycle start --json
oa-workroomd lifecycle pause --json
oa-workroomd lifecycle resume --json
oa-workroomd lifecycle expose --json
oa-workroomd lifecycle closeout --json
oa-workroomd lifecycle archive --json
oa-workroomd lifecycle destroy --json
```

The lifecycle files are:

```text
lifecycle-state.json
lifecycle-receipts.jsonl
```

The explicit states are:

- `not_created`
- `created`
- `running`
- `paused`
- `exposed`
- `closed_out`
- `archived`
- `destroyed`

Allowed transitions are intentionally narrow:

- `create`: `not_created` to `created`
- `start`: `created` or `paused` to `running`
- `pause`: `running` or `exposed` to `paused`
- `resume`: `paused` to `running`
- `expose`: `running` to `exposed`
- `closeout`: `running`, `paused`, or `exposed` to `closed_out`
- `archive`: `closed_out` to `archived`
- `destroy`: `closed_out` or `archived` to `destroyed`

Every accepted transition appends a `lifecycle_receipt` with action,
from-state, to-state, and a `sha256:` receipt digest. `destroyed` is terminal.

`closeout` and `destroy` consult the artifact closeout state. If a required
artifact policy exists, lifecycle closeout and destroy are blocked until
`closeout-manifest.json` has been submitted by `oa-workroomd closeout submit`.
