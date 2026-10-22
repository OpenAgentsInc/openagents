# Native Kiln host

`kiln-host` runs Kiln's shared Rust agent loop without a browser. Foreman's
app chat investigates and claims one issue, then submits a frozen task. Kiln
uses free Pylon inference, reads Git objects at the pinned base, edits its
private file snapshot, and tests through the existing sandboxed Kiln runner.
It publishes the exact checked NIP-34 patch and uses the existing GitHub
bridge to open one draft PR. It never merges or deploys.

`publish-serve` imports the five active Foreman objectives as real NIP-WORK
initiatives through `work-host`. It retains stable native identities,
conditional revisions, original commands, and WORK history. It publishes
encrypted kind-3188 packets to the explicitly configured worker every half
hour. Public tags contain no objective names. The worker checks the admitted
authority, signed scope and history, original commands, artifact bytes, and
active state before intake and each turn. It rejects conflicting history.
The packets are transport bundles for WORK records, not a new Nostr kind.

```sh
kiln-host init --state STATE --config CONFIG.json
kiln-host publish-serve --state AUTHORITY_STATE --key FOREMAN_KEY --recipient WORKER_PUBKEY
kiln-host serve --state STATE
kiln-host profile --state STATE
kiln-host objectives --state STATE --objective improve-kiln
kiln-host submit --state STATE --input TASK.json
kiln-host status --state STATE --id TASK_ID
kiln-host stop --state STATE --id TASK_ID
```

`Config` and `Task` in `src/worker.rs` define the closed input shapes. Admit
the worker key on the runner's bounded admission list. The host's own Git
clone and private GitHub token file are separate from the owner's checkout.
The model receives neither the token nor a shell. Set
`OPENAGENTS_FOREMAN_KILN_STATE` on the website to select this delivery route;
an invalid configured path refuses dispatch instead of falling back.

Each task pins the exact published objective revision, source commit, crate,
issue, and brief. It permits at most 48 model turns, 96 tool calls, and a
two-hour intake window. One active or unresolved attempt blocks another;
the host accepts at most five tasks per UTC day and retains up to 512.
Changing a file invalidates its passing check. A restart restores the loop,
files, and pending commands. The runner records admission before execution and retains its signed result
in an encrypted Nostr artifact. Recovery queries that result and may redeliver
the exact request to the same runner, which never executes an admitted request
twice. An unresolved admission blocks new work; it is never retried with a new
identity. A lost PR reply is
recovered by its exact patch branch.

Only configured packages are executable. The runner supports root-workspace
packages and the fixed nested Psionic workspace, with separate Cargo targets
and compiler state. A job cannot span both workspaces. GPU runs still need
separately qualified device profiles. WORK
publication supplies objective data, not execution authority: the admitted
local host configuration and explicit app assignment authorize each bounded
task. This adapter does not claim native AUTO or WORK effectful execution.


## Other repository workspaces

`kiln-workspace` runs the same Kiln agent for explicitly scoped text files,
including the browser editor. Its separate configuration pins a Git base,
repository path list, buyer key file, model, serving profile, and Pylon public
key. It reads regular Git blobs and keeps edits in a private snapshot; it
cannot run a shell or change the checkout. Existing Cargo-only WORK admission
and runner checks remain unchanged.

Use `init --config FILE --state FILE`, then `advance --state FILE` for each
model turn. `status` reports progress; `export` returns the exact base,
original files, proposed files, and their digest. When the agent requests
`check`, the coordinator materializes and reviews that export in its worktree,
runs the issue's focused checks, and supplies `feedback --input FILE` with
`digest`, `code`, and `output`. A passing result counts only for that exact
snapshot. Further edits clear it. The agent can finish only after a matching
passing check; the coordinator still reviews the change and opens the draft PR.
The adapter permits 96 model turns, 256 tool calls, and six hours per workspace.

Use `stop --state FILE [--reason TEXT]` to stop a workspace after its current
advance finishes. Its saved work and history remain available through `status`
and `export`, and further advances do nothing. The optional reason accepts up
to 512 UTF-8 bytes and stays in the private saved workspace.
