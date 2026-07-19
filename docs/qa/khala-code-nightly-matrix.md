# Khala Code QA nightly matrix (retired)

The Khala Code desktop/mobile nightly matrix and its client-specific
Maestro/emulator/visual lanes were retired on 2026-07-14 with the final
`clients/` removal. This file remains as a tombstone for old links. None of its
former environment toggles or client commands are executable authority.

Current deterministic checks are owned by supported surfaces:

```sh
pnpm --dir apps/openagents-mobile run typecheck
pnpm --dir apps/openagents-mobile run test
pnpm --dir apps/openagents-desktop run verify
pnpm --dir packages/khala-qa-harness run test
pnpm run test:qa-nightly-matrix
```

Mobile store/release acceptance belongs to
[`../deploy/openagents-mobile-production-release.md`](../deploy/openagents-mobile-production-release.md).
Historical receipts remain under `docs/khala-mobile/`, `docs/mobile/`, and Git
history at the recovery commit recorded in the
[`clients/` retirement AAR](../sol/2026-07-14-clients-retirement-after-action.md).
