# Issue work in Archil sandboxes (`chat work --on archil`)

Issue #12136. `openagents chat work --on archil --issues N,... --parallel K
--land queue` works issues like `--on gce`: the same briefed engine (finder,
briefed agent with `verify`, replayed checks, Claude Code fallback), with
each issue in an Archil sandbox of its own instead of on a GCE pool host.

```sh
openagents chat work --on archil --issues 12131,12126 --parallel 2 --land queue
```

## How a run goes

1. **Image.** Every sandbox starts from
   `us-central1-docker.pkg.dev/openagentsgemini/oa-cloud/archil-worker:v2`,
   built from `crates/coder-cloud/archil/Dockerfile`: the toolchains the
   repository pins (1.97.1 and 1.98.1, with rustfmt and clippy), protoc
   with its includes, Node with
   Codex and Claude Code, and the `coder` user. It's imported into Archil
   once (`images().build`, with a short-lived token of the pool's gcloud
   account) and remembered in `~/.openagents/cloud/archil.json`.
2. **Template.** Once a day, a template sandbox clones `origin/main`,
   builds the briefed agent, and warms the checks' target
   (`~/.openagents/work/target`) with `openagents-cli`. Then it's stopped.
3. **One fork per issue.** Each issue forks the template. This computer
   reads the issue with `gh` and writes it into the fork, along with the
   engine's scripts as this binary carries them and the run's credentials.
   The credentials go in a private file that the run deletes first thing.
4. **The run.** The fork moves the checkout to `origin/main`, rebuilds the
   briefed agent if main moved, and runs `work_issue.py --land push`. A
   change that passes its replayed checks is pushed to `land/archil-N-*` on
   github.com.
5. **Landing, here.** This computer runs `openagents land submit --branch
   land/archil-N-* --issue N` (or `gh pr create` with `--land pr`). Then it
   stops and deletes the sandbox, comments the run's wall time and cost on
   the issue, and appends a route record (placement `archil`) to
   `~/.openagents/cloud/runs.jsonl`.

The orchestrator does the GitHub API and landing steps because Archil's
free-plan network is an allowlist. It includes github.com, crates.io, npm,
`api.openai.com`, and `api.anthropic.com`. It doesn't include
`api.github.com`, `openagents.com`, `chatgpt.com`, `static.rust-lang.org`,
or the landing queue's bucket. On that plan, Codex on a ChatGPT login can't
reach its backend. So `--on archil` runs the briefed agent on Claude, signed
in with the operator's Claude Code token, by default. With
`--engine-fallback`, it runs on Codex with the OpenAI API key. Either way,
Claude Code is the fallback. The finder's embeddings key
(`OPENROUTER_API_KEY`, or the operator's `openrouter.env`) goes in too,
since openrouter.ai is on the allowlist.

## Credentials

Nothing is printed or written to the repository.

- Archil: `ARCHIL_API_KEY` and `ARCHIL_REGION` (default `aws-us-east-1`),
  or the same lines in `OA_ARCHIL_ENV`, `~/.openagents/cloud/archil.env`,
  or `~/work/.secrets/archil.env`.
- Claude Code fallback: `CLAUDE_CODE_OAUTH_TOKEN`, or the file at
  `OA_CLAUDE_TOKEN_FILE`, or `~/work/.secrets/claude-code-oauth-token`.
- GitHub token, git identity, and Codex logins: the same as `--on boat` and
  `--on gce` (`chat_boat.rs` `credentials`).

## Budget

Typing `--on archil` is the grant. Runs count toward the cloud budget
(`openagents cloud budget`) at Archil's list price of $0.055 per vCPU-hour
and $0.020 per GiB-hour. One 8 vCPU, 16 GiB sandbox costs about $0.76 an
hour. No run starts once the 24-hour cap is spent. A run that reaches the
per-run cap is stopped and its sandbox deleted. Ctrl-C deletes every running
sandbox.

## Rebuilding the image

```sh
cd crates/coder-cloud/archil
gcloud builds submit --project openagentsgemini --region us-central1 \
  --config cloudbuild.yaml --substitutions _TAG=v2 .
```

Then set `OA_ARCHIL_IMAGE` to the new tag, or bump `DEFAULT_IMAGE` in
`crates/coder-cloud/src/archil_backend.rs`. The next run imports it and
prepares a new template.
