# Work seeds for the Coax project

Each `*.json` file here is the source the project publisher turns into Nostr
events for `/projects/coax` (see `docs/protocol/2026-10-28-projects.md`). Edit
the file, run the publisher, and the live page follows; nothing on the page is
numbered by hand.

## Schema `openagents.project-work-seed.v1`

```jsonc
{
  "v": "openagents.project-work-seed.v1",
  "project": "coax",                       // the project's d tag
  "initiative": { "slug", "title", "body", "spec", "labels" },
  "milestones": [ { "slug", "title", "body", "parent" } ],   // parent: initiative slug
  "tasks": [ {
    "slug": "secp-field-mul",              // stable identity; never reuse or rename
    "parent": "m1-field-scalar",           // milestone slug
    "title": "…",                          // no numbers; display aliases are assigned by the page
    "size": "S" | "M" | "L",               // one agent, one sitting: S < 2 h, M ≈ half a day, L ≈ a day
    "labels": ["crypto", "secp256k1"],
    "body": "What to build, interfaces, constraints",
    "files": ["paths the task will write"],  // the claim's write resources
    "acceptance": ["exact checks that must pass"],
    "blocked_by": ["slugs"],               // the source of truth for dependencies
    "blocks": ["slugs"]                    // derived from blocked_by; regenerated, never edited by hand
  } ]
}
```

Publish with `bun coax/web/project/publish.ts publish` (the project key is
at `~/work/.secrets/openagents-project-coax.key`). Each item becomes a
NIP-34 kind-1621 issue. The protocol doc has the event model.

Rules:
- **Identity is the slug.** Each issue carries a `slug` tag, and the
  publisher finds the existing issue by it. Republishing changes nothing when
  fields match, and sends a kind-1111 revision when they differ, so there
  are never duplicates. Renaming a slug makes a new task.
- **No order numbers.** Queue order is computed: dependencies first
  (`blocked_by` is a DAG; the generator refuses cycles), then the project's
  order record, which holds a fractional rank key per task. Moving one task
  rewrites one key; nothing else renumbers.
- **Display aliases** (like `#3f2a`) are the leading hex of the issue's event
  id, lengthened only on collision. They are stable and inert: never refer to
  a task by alias in code or acceptance.
- **Acceptance is literal.** Every entry is a command, a vector file or an
  observable result a reviewer can check without asking the author.

Files:
- `nostr.json`: the Coax Nostr initiative (spec: `coax/nostr.md`), 51 tasks
  across ten milestones.
