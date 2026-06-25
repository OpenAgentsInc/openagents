# qa-runner — autonomous QA you can watch

[`@openagentsinc/qa-runner`](https://www.npmjs.com/package/@openagentsinc/qa-runner) drives a
**real browser** against any target, verifies a check the way a person would, and emits a
**video + a committed e2e test + an honest pass/fail verdict**. If it looks right, it *is*
right — and you can prove it by reading the artifacts, without re-running anything.

It's OSS, local-first, and **bring-your-own-model — no OpenAgents login required.** Khala is
one optional backend, not a dependency. (Product overview: <https://openagents.com/docs/autonomous-qa>.)

## Install

```sh
npm i -g @openagentsinc/qa-runner    # installs the `qa` CLI
npx playwright install chromium       # the real browser it drives
```

## Run a check against any site

```sh
qa run --url https://example.com --goal "open /login and confirm the sign-in form renders" --out ./runs/example
```

Useful options (`qa run` with no `--url` prints full help):

- `--goal "<text>"` — what to verify (defaults to a `/login` check)
- `--model <id>` / `--base-url <url>` / `--api-key <key>` — BYO OpenAI-compatible model (or env `QA_MODEL` / `QA_BASE_URL` / `QA_API_KEY`); the key is never printed
- `--allow-keyless` — permit a keyless local server (llama.cpp / vLLM / Ollama shim)
- `--emit <path>` — where to write the distilled e2e test (default `generated/<slug>.e2e.test.ts`)
- `--headed` — watch a visible browser
- `--fake-model` — deterministic, no-network, no-key proof of the loop (still emits a real video + a committed test)

It writes to `--out`:

- `result.json` — the verdict (`status: pass | fail`)
- `session.mp4` — video of the session
- `*.png` + `trace.zip` — screenshots + the Playwright trace
- the **distilled `*.e2e.test.ts`** (at `--emit`) — a re-runnable, black-box test with named,
  user-readable steps and deterministic waits; point `TARGET_URL` at dev or prod to run it
  anywhere.

The exit code is honest: `0` only on a clean pass **and** an admissible distilled test; a real
deviation yields a FAIL visible in the video — never a fake green.

## Shareable traces (`/trace/{uuid}`)

When run through the **OpenAgents-backed** path, a session is published as a redacted, shareable
**trace** — the full timeline of every step, tool call, and observation, with the recording and
screenshots served inline from the trace's own surface (not a third-party attachment). Review
the work by reading the trace; no local run required. Secrets, PII, and local paths are redacted
before upload, and the ingest rejects real leaked values. See the OpenAgents docs for the
agent-token publish flow.

## Honest defaults

- No spend and no network beyond the target unless you pass a model/key.
- No publishing unless explicitly armed.
- Verdicts are real: the failure is visible in the video, never hidden.
