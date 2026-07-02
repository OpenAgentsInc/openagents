# Khala Code Flagship QA Demo

Issue #8026 adds the Q3.4 product demo lane: a headed packaged Khala Code run
finds an explicitly seeded fixture bug from native AX/screenshot evidence and
emits a committed deterministic regression artifact.

## Command

Build the packaged app first, then arm the native desktop backend:

```sh
bun run --cwd clients/khala-code-desktop build

QA_NATIVE_DESKTOP=1 \
bun run --cwd apps/qa-runner khala:flagship-demo -- \
  --out ../../var/qa-8026/flagship-demo \
  --seeded-bug-text "Seeded bug: packaged Khala Code fixture response is rendered"
```

`--seeded-bug-text` must be public-safe AX-visible fixture text. If the marker
is missing, the demo writes a `fail` report rather than fabricating a find.

## Artifacts

The artifact directory contains the normal packaged native smoke files plus:

- `khala-flagship-demo-report.json` - public-safe demo summary
- `khala-flagship-session-trace.json` - deterministic distiller input
- the committed regression, by default
  `apps/qa-runner/generated/khala-code-packaged-seeded-bug.e2e.test.ts`

The report stores only stable refs and hashes. It intentionally omits absolute
app bundle paths, executable paths, local output paths, raw typed text, tokens,
and credentials. Visual baseline entries remain relative to the baseline store.

## Verification

The normal gate now runs the distilled regression:

```sh
bun test apps/qa-runner/src/khala-desktop-backend.test.ts
bun test apps/qa-runner/generated/khala-code-packaged-seeded-bug.e2e.test.ts
bun run --cwd apps/qa-runner test
```

The pinned test uses an injected native runtime, so it proves the full
launch -> AX/screenshot -> seeded-bug report -> distill -> regression path
without real accounts, spend, or macOS Accessibility permissions.
