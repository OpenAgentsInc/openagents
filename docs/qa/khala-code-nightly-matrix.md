# Khala Code QA Nightly Matrix

Status: implementation note for ROADMAP_QA Q1.1 / issue #8012.

`bun run qa:nightly` is the owned-runner Tier-2 loop for the fully automated
Khala Code QA cycle. It does not use GitHub-hosted CI. The committed systemd
unit and timer live in `ops/owned-runner/khala-code-qa-nightly.service` and
`ops/owned-runner/khala-code-qa-nightly.timer`.

The nightly matrix runs, in order:

1. `bun run --cwd packages/khala-qa-harness test`
2. `bun run --cwd clients/khala-code-desktop verify`
3. `bun run --cwd clients/khala-code-desktop smoke:part2-ui`
4. `bun run --cwd clients/khala-code-desktop smoke:cockpit-visual`
5. `bun run --cwd clients/khala-code-desktop smoke:composer-visual`
6. `bun src/monkey-night.ts --runs 16 --steps 64` from the harness package
7. `bun test src/model-based.test.ts` from the harness package
8. the desktop property tier: composer draft, ThreadItem projector, and
   transcript render properties

The default monkey settings produce 1024 deterministic fixture actions, meeting
the Q1.1 >=1000 action floor. Override with `OA_QA_NIGHTLY_MONKEY_RUNS` and
`OA_QA_NIGHTLY_MONKEY_STEPS` only on the owned runner.

Each run writes:

- `qa-nightly-report.json`
- `qa-nightly-report.md`
- one log per step under `logs/`
- the monkey coverage ledger under
  `monkey-night/monkey-night-coverage-ledger.json`

If the matrix fails and `OA_QA_NIGHTLY_FILE_ISSUE=1` is set, the runner files a
strict bug issue through `gh issue create` with public-safe report refs and the
failed step IDs. Raw command logs stay in the owned-runner artifact directory and
must be redaction-reviewed before external publication.

The timer is scheduled for 07:17 UTC with a small randomized delay. Install on
the owned runner by copying both unit files into the systemd unit directory and
enabling the timer:

```sh
sudo systemctl daemon-reload
sudo systemctl enable --now khala-code-qa-nightly.timer
systemctl list-timers khala-code-qa-nightly.timer
```

The owned checkout path in the unit is `/srv/openagents/openagents`; adjust it
only on the runner, not in Git, unless the fleet standard path changes.
