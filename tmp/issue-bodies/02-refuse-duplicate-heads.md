## Problem

Step 55 of trajectory `2026-08-27T12:29:02` issued one shell call of the form:

```
pnpm run test:rust | grep … ; pnpm run test:rust | grep -c … ; pnpm run test:rust | grep …
```

Same suite executed 3× in a single invocation (~4m49s) because there were three questions about one output stream. Nothing in the tool layer notices or discourages this; each segment looks like a fresh command.

## Recommendation

Lint the `command` string in the shell tool wrapper before execution: normalize each pipeline segment to its head (executable + first meaningful args, i.e. everything before the first `|`), and if the same normalized head appears more than once in one invocation, either reject or run-once-tee-many:

- Suggested refusal message: "this executes `pnpm run test:rust` 3×; run it once with `tee`, then grep the file."
- Alternative behavior: execute once, tee to a temp file, and apply each grep to the file — semantically identical when the intermediate command has no side effects the caller depends on between repeats.

Note the safety caveat: auto-rewriting changes semantics for side-effecting commands (e.g. repeated `git stash pop`). Prefer refusal-with-hint over silent rewrite for mutating heads; tee-collapse is safe for read-only heads like test/build/print commands.

## Acceptance criteria

- [ ] A one-line command containing 2+ occurrences of the same non-mutating head executes once.
- [ ] A one-line command repeating a mutating head is refused with the tee hint instead of executing.
- [ ] Step-55-shaped input completes in one execution instead of three (expect ~330s saved on trajectories like `2026-08-27T12:29:02`).
