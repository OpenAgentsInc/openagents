# Bench: the Gym's harness lane

`bench/adapters/openagents_coder.py` is the Harbor installed-agent adapter
for `openagents coder` (OpenAgentsInc/openagents#35). It runs the working
tree's CLI, not the published npm version, so pack a tarball first:

```sh
cd packages/openagents-cli
pnpm build
pnpm pack --pack-destination ../../bench
```

Use `pnpm pack`, not `npm pack`: the manifest carries pnpm `catalog:`
versions that only `pnpm pack` rewrites into versions npm can install.

Run against the dev forge on the host (`PHX_LISTEN_ALL=true mix
phx.server` in openagents.com, plus a `chat:account` token):

```sh
PYTHONPATH=bench OPENAGENTS_TOKEN=... harbor run \
  --dataset terminal-bench@2.0 \
  --agent-import-path adapters.openagents_coder:OpenAgentsCoder \
  -m openai/gpt-5.6-luna \
  -i fix-git --n-concurrent 1 --jobs-dir /tmp/gym-jobs
```

`bench/post_gym_run.py` posts a completed job's graded result to
`POST /api/v3/gym/runs`. Post only runs whose verifier actually ran: a
score is a claim, and a crashed grader is not a grade.

If the dev server binds loopback only (another session started it
without `PHX_LISTEN_ALL`), bridge instead of fighting over the port: run
a forwarder from `0.0.0.0:4001` to `127.0.0.1:4000` and point the
adapter at it with `OPENAGENTS_CODER_API_URL=http://host.docker.internal:4001`.

Known constraint on Apple Silicon: Terminal-Bench task images are amd64,
and under qemu emulation the verifier's `uv`/`pytest` segfaults after the
agent phase completes. Enable Docker Desktop's Rosetta emulation
(Settings → General → "Use Rosetta for x86_64/amd64 emulation"), use a
cloud environment (`--env daytona` and peers), or run on amd64 hardware
for scored runs. The agent phase itself runs fine under qemu.
