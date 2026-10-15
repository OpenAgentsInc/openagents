# Workspace documents and synchronized views

`workspace-host` implements NIP-WS core validation, a local Git document owner,
recipient-bound views, conditional changes, checkpoints, restoration, and an
exact-request client outbox. `openagents workspace` and the standalone
`workspace-host` command share the same implementation.

Git keeps the actual document bytes and versions. Workspace state keeps explicit
source bindings, grants, admitted requests, receipts, and derived views. Project
associations retain exact references to their existing owners; association does
not create another project store or grant execution. Document reads also return
an existing `workbench::ResourceRef` to the exact snapshot artifact. A resource
ID or display name never becomes a host path.

Use a dedicated key file readable only by its owner and a private state directory:

```sh
openagents workspace init --state DIR --key-file KEY
openagents workspace add-git --state DIR --key-file KEY \
  --repository /absolute/repo.git --branch refs/heads/docs \
  --path document.md --name Document
openagents workspace read --state DIR --key-file KEY --resource RESOURCE_ID
```

The source must retain Git ref history. Enable `core.logAllRefUpdates` before
creating the source ref. This profile supports regular UTF-8 Git blobs up to
1 MiB. It changes a branch ref, rather than worktree files or an index. It refuses
a checked-out branch. Repository hooks, file-system monitors, and automatic
maintenance are disabled for these bounded Git commands. Each process has a
10-second deadline and 1 MiB output bounds. Unix file identities bind the ref
history; other hosts refuse this conditional profile.

`replace-plan --resource ID --content FILE` returns a signed request bundle.
Keep those exact bytes and submit them with `apply --bundle FILE`. A proposal
alone executes nothing. The host checks the signed actor, current grant, exact
workspace binding, operation, input, document, encoding, and expected version.
The version includes the Git commit and retained ref-history identity, length,
and digest. The host checks it while a native Git ref transaction holds the lock,
then rechecks authority and expiry before commit. Independent writers and
A-to-B-to-A changes cannot silently overwrite a stale document. New replacement
bytes must accompany the request; only the verified current target bytes can be
reused without an attachment. Knowing another resource's cached digest grants
no read permission.

Admission and effect intent reach durable storage before dispatch. Exact retries
return the recorded result; changed requests under the same actor/command refuse.
After a lost response, the current ref or retained history must confirm the exact
prepared commit. A prepared object alone proves no mutation. Unconfirmed attempts
stay unknown. The Rust `Outbox` stores exact signed document-change requests and
attachments; corrupt entries are quarantined while their identities remain.
Restoration and live input cannot enter that queue.

`grant` gives an explicit actor a resource selection, expiry, and optional write
right. `revoke` advances its access state. Required approvals refuse unless an
embedding host supplies a POL verifier for the exact proposal. The standalone
CLI acts as the local authority; its administration methods must never be exposed
as unauthenticated remote routes. The Rust read/change methods authenticate real
private `3188` requests. No ambient owner key or production account is used.

`view --recipient KEY --resources FILE --retain-until UNIX` creates a finite
selection. FILE is an array of resource IDs. Save the returned definition reference
for `snapshot --projection FILE`, `changes --projection FILE --cursor TOKEN`, and
bounded `fetch`. Source frontiers cover retained Git ref history, including
transient changes between reads. Paging keeps one finite cut/head. Cursors bind
the recipient, access epoch, selection, cut, mode, and position. Revocation stops
paging and artifact reads. A completed snapshot supplies a resume cursor; a caught-up
marker names only its finite replay head.

The portable client checks snapshot request continuity, remembers its high cut,
applies whole deltas, ignores known identical duplicates, and refuses forks or
gaps. Views retain at most 64 images, 256 selected rows, 512 request results,
2,048 cursors, and 8 MiB of metadata. Missing history or overflow refuses further
progress and requires a new view. Cached view state never admits a command.
`visibility` checks whether the view's applied source frontier includes the exact
command transition; a later timestamp or an open socket is insufficient.

`checkpoint` retains exact documents and a reconstruction manifest for at most
16 sources. `restore-plan --checkpoint FILE` captures a separate safety checkpoint
and returns an inert plan with explicit impact. `restore --plan FILE` separately
admits that exact plan and writes per-step RUN records and results. Each plan
supports one document per Git ref. A changed later source stops the operation:
confirmed earlier effects remain recorded. A restart reconciles attempted steps
and leaves unattempted steps alone. Checkpoints explicitly exclude worktree files,
staged, untracked, and ignored content, provider history, credentials, and processes.
They do not claim a cross-store atomic snapshot or an engine rewind.

The host retains at most 256 resources, 64 views, 4,096 command identities,
256 prepared restore plans, and 128 restoration runs. Signed state is bounded to
16 MiB; retained artifact files are bounded to 8,192 objects and 128 MiB. Quota
failure preserves evidence and refuses new work. Administration uses one state
directory per authority; copying it does not establish shared-controller fencing.

Wire validators also cover resource graph closure/cycles, worktree creation and
removal references, exact UTF-8 ranges, and unsupported records. General filesystem
CAS, worktree provisioning/cleanup, provider-specific document adapters, mobile
rendering, and remote CAP/CJ mounting keep their existing owners and separate
admission. This crate advertises its concrete Git/native roles, rather than a
complete graphical workbench. Existing activity summaries remain unchanged.

Fixtures use temporary Git repositories, keys, and state. They cover an independent
writer, ABA changes, exact retry, revoked access, a lost response/restart, corrupt
queues, snapshot paging during an edit, two converging clients, command visibility,
and partial restoration. The standalone CLI fixture exercises capture, change,
views, and restoration without reaching any production account.
