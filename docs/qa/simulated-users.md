# Simulated-user QA

Pretend people use every OpenAgents surface, a model judges what they saw,
and each confirmed problem becomes a GitHub issue labeled `qa`. The owner
asked for this on 2026-10-02: "have pretend users use all surfaces and
identify (via github issue) and fix any problems you see."

## What it runs

| Piece | Where |
| --- | --- |
| Runner | `scripts/qa/simulated-users.sh` (wraps `scripts/qa/simulated_users.py`) |
| Personas | `qa/personas/*.toml`: who the person is, their goals, an opening message, the surface, and for some a seed project or scripted steps |
| Facts the judge checks against | `qa/facts.md` |
| Rubric | `qa/rubric.md`: facts, bloat, limits, raw, routing, latency, tone, stale, broken |
| Seed projects | `qa/seeds/*` (for example `failing-test`, a Python project whose test fails) |

Each persona drives one surface through the real product, never a mock:

| Surface | Driver |
| --- | --- |
| `website` | HTTP to `https://openagents.com/`, `/download`, `/docs`, and every internal link they show (status and time), then a conversation with the homepage's **Ask** box (`POST /ask`, with a visitor cookie) |
| `chat` | `openagents chat send --scratch --json --no-run` in a temporary HOME: a throwaway identity and thread store, the live chat worker |
| `terminal` | With `install = true`, the public `install.sh` into a temporary HOME first (output, time, and `--version` of both programs), then `openagents terminal --scratch` in a pseudo-terminal read through a VT emulator (`pyte`). `steps` mixes slash commands, keys (`{esc}`), and persona turns (`{persona}`, and `{persona-esc}`, which presses Esc while the reply streams) |
| `desktop` | The release gate's own driver, `scripts/release/acceptance.sh --only persona`: a scratch `coder host serve` in a temporary HOME, and the desktop binary's real window model (`openagents-desktop --acceptance`) sending each message the way the window sends it. A reply that starts Coder is followed until the run ends. `engines = true` makes the owner's Codex, Claude Code, and Grok Build logins readable the way the gate does (read-only); `seed` copies a `qa/seeds` project into the scratch repository |
| `phone` | The same gate driver with a phone-shaped client paired to the scratch host by its invitation code, sending from the phone's own Coder tab (the shared Rust the iOS and Android apps run) |

The `persona` scenario (`crates/openagents-desktop/src/acceptance_persona.rs`)
is not part of the release gate: it runs only when `--only persona` names it.
The runner appends each message to `EVIDENCE/persona/in.jsonl`, and the
scenario appends what the person saw to `out.jsonl`, with a capture per
turn (a PNG for the desktop, the view tree for the phone).

## Turns and judging

After the opening message, each turn's next message comes from a model
playing the persona, reacting to the product's last reply; it stops when
the goals are met or the person would give up. When the run ends, a second
model call reads the whole transcript (with reply times, statuses, and
Coder's outcome) against the rubric and the facts, and returns findings:
title, category, severity, the turn, an exact quote, and expected versus
actual. Both default to Space Bunny Alpha through OpenRouter
(`stealth/space-bunny-alpha`), the chat worker's own first route. Pass
`--judge-model claude:sonnet` to judge with the Claude CLI instead.

The judge proposes; a person (or the agent running the process) confirms.
Read `report.md` and each finding's quote in `transcript.jsonl` before
filing it; drop a finding that the transcript does not show.

## Run it

```sh
export CARGO_TARGET_DIR=~/work/openagents-target-agent2
scripts/qa/simulated-users.sh --build                # every persona
scripts/qa/simulated-users.sh --personas new-visitor,confused-user
scripts/qa/simulated-users.sh --surfaces website,chat --out /tmp/qa-run
scripts/qa/simulated-users.sh --list
```

The key is read from `OPENROUTER_API_KEY` or `~/work/.secrets/openrouter.env`
and is never printed or passed to the product. The run needs `uv` (for
Python 3.12 and `pyte`) and, for issues, `gh`.

A run writes, under `--out` (default `$TMPDIR/qa-runs/TIME`):

- `PERSONA/transcript.jsonl`: every page, message, reply, key, and step,
  with milliseconds and evidence file names.
- `PERSONA/verdict.json`: the judge's summary, goals, and findings.
- Evidence: `page_*.html` and `links.txt` (website), `events.jsonl`
  (chat), `screen-NN.txt` and `terminal.raw` (terminal), `gate/` with the
  gate's `persona/turn-NN.png` captures, host log, and results (desktop and
  phone).
- `findings.json` and `report.md` for the whole run.

## File the confirmed findings

```sh
scripts/qa/simulated-users.sh --out /tmp/qa-run --file new-visitor-1,chat-2
```

Each issue gets the label `qa` and names the persona, the surface, the
steps (the persona's messages), expected and actual, and the transcript
around the turn with timings and evidence files. Before creating one, the
runner reads the open issues and skips a finding whose hidden `qa-key`
(surface, category, and title words) matches, or whose title is close to an
open issue's title. A problem in the website's docs pages
(`crates/openagents-web/content/docs`) is filed, not fixed, while that
content has its own owner.

## Safety

- Temporary homes for every process: the chat and terminal drivers set
  `HOME` and `TMPDIR` to a new directory under `/tmp`, and the gate makes
  its own. Nothing reads or writes `~/.openagents`, the owner's chats, the
  wallet, the keychain, or paired devices; the only real-home reads are
  the gate's read-only engine logins for personas with `engines = true`.
- Scratch identities: `--scratch` threads, a website visitor cookie made
  for the run, a phone key made in the gate's HOME.
- No money moves. No persona touches the wallet or a payment.
- At most `--max-jobs` (default 300) live chat-worker jobs per run; one
  full run of the eight personas sends about 30.

## Cost per run

Measured on 2026-10-02 for the eight personas (run `qa-run2`):

- Persona and judge models: 32 calls to Space Bunny Alpha, which is free
  on OpenRouter during its preview: `$0.00` (the report prints the exact
  figure). The Claude CLI fallback (`claude:haiku`) is called only when
  Space Bunny returns no JSON three times.
- Chat worker: 31 jobs, each one ordinary chat turn.
- Coder: one tiny run in the seeded scratch project (`mac-developer`, 32 s
  on the first engine with capacity) and, when the persona asks for it, one
  for `phone-user`: a few cents of engine usage.
- Time: about 30 minutes. Each desktop or phone persona spends about two
  minutes starting its scratch host; the website and chat personas take a
  few minutes each.

Issues filed from the first run: #10135 to #10142.
