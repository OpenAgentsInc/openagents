# Khala Code Architecture Scan Ratchet

`clients/khala-code-desktop` runs a hard-fail architecture scan in `verify`.
The scan promotes the previous `check:deploy` report-only patterns to a
desktop verify gate. It covers TypeScript implementation files in:

- `clients/khala-code-desktop`
- `packages/khala-tools`

It blocks new instances of:

- `JSON.parse(...) as ...`
- empty `catch {}` blocks
- direct `process.env` / `Bun.env` reads
- `Date.now()` in implementation logic
- `Effect.runPromise` bridges
- `setTimeout` process-kill paths

Grandfathered findings live in
`clients/khala-code-desktop/scripts/architecture-scan.allowlist.json`. The
allowlist stores exact entries and per-category counts. `verify` fails when a
new finding appears or when an existing finding disappears without shrinking the
allowlist, so the ratchet moves only downward.

When removing debt, run:

```sh
bun run --cwd clients/khala-code-desktop scan:architecture -- --update-allowlist
bun run --cwd clients/khala-code-desktop verify
```

Review the count decrease in the allowlist diff before merging.
