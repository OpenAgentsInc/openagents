# Scoped tracker intake

`coder::tracker` reads a GitHub organization project through an approved `gh`
adapter and represents it as a versioned snapshot. The snapshot pins repository
and default-branch identity, issue numbers and node IDs, issue versions, body
digests, dependency observations, and host-prepared task definitions. Reading a
project does not authorize changing it.

Approve the repository's `github-project` capability with `capability-trust`, then
use the [project supervisor](project-supervision.md) to acquire and refresh the
project. Authentication uses the existing GitHub CLI configuration or protected
`GH_TOKEN`/`GITHUB_TOKEN` environment values. Credentials are not command arguments.
The adapter pins `--hostname` to the scope and uses a fixed read-only GraphQL
query. Programs never carry an executable query command.

Acquisition has page, item, wall-time, and output bounds. The shared subprocess
supervisor caps streams while reading, terminates timed-out process groups, and
reaps the direct child. Repeated cursors, overlapping item IDs, changed bases,
inaccessible items, partial pages, and fetch failures refuse the acquisition.
Known draft and pull-request items remain named skips. An incomplete blocked-by
list blocks the affected issue; missing completeness defaults to incomplete.

`TaskMap` holds operator-prepared prompts, effects, touched paths, and expected
answers. Issue text is digested and excerpted as untrusted context. It cannot
change capabilities, permissions, source commands, or publishing authority.
Missing task definitions, unknown dependencies, and dependency cycles remain
visible blockers.

A `tracker` source in the local source registry reads a pinned snapshot file
without running an adapter. `Snapshot::revalidate` compares two observations.
Static source reads alone do not establish freshness; the project supervisor
performs a fresh acquisition before each admission round and compares the issue
version, body digest, default branch, and native blockers. Local dispatch checks
its exact prepared Git base. Use `pin-config` after integrating a changed default
branch; changed in-flight tasks cannot be repinned in place.

The controller requires explicit acceptance of closed prerequisites. A snapshot
can report that an issue is closed, but that observation does not establish that
its implementation passed the required checks. Comments, issue closure, merges,
and publishing remain separate authorized operations.
