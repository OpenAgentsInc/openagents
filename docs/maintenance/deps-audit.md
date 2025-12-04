# Dependency audit workflow

- Run `bun run deps:audit` locally to generate `.openagents/deps/audit-report.json`.
- Use `--json` for console JSON and `--output <path>` to customize the report location.
- The script runs:
  - `npm audit --json --production` for vulnerability counts.
  - `bunx npm-check-updates --jsonAll` for upgrade suggestions (no lockfile changes).
- CI: `.github/workflows/deps-audit.yml` runs weekly (and on demand), uploads the report artifact.
- Applying updates: use `bun add -E <pkg>@<version>` for targeted bumps; commit lockfile changes separately. Roll back via `git checkout bun.lockb package.json`.
