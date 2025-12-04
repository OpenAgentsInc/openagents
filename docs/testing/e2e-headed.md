# Playwright headed and debug options

- Default run (`bun run e2e:test`) stays headless for CI speed.
- Headed runs: `bun run e2e:test:headed` or `HEADED=1 bun run e2e:test -- --headed`.
- Slow motion: `bun run e2e:test:slow` (500ms), or set `SLOWMO=250 HEADED=1 bun run e2e:test` for custom pacing.
- Inspector/debug: `bun run e2e:test:debug` enables `PWDEBUG`, headed mode, and slow-mo for step-through debugging.
- Config picks up `HEADED`, `PLAYWRIGHT_HEADFUL`, `PWDEBUG`, CLI `--headed`, and `SLOWMO`/`PLAYWRIGHT_SLOWMO` to keep options consistent across scripts and manual invocations.
