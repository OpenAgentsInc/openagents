# Harness acceptance gate

A Coder build reaches the owner only after a real chat on every harness
installed on the release Mac passes ([#11549](https://github.com/OpenAgentsInc/openagents/issues/11549)).
rc.10 went out with bugs visible on the first message: Codex streamed no
thinking, `/models` listed the wrong harness's models, Grok ran 4.6 and Devin
ran SWE-1.6 Slow, Cursor replies began with a Coder notice, and Oh My Pi
started on a model that can't chat. Unit tests didn't catch them because no
test opened a chat on each harness. This gate does.

## Run it

```sh
scripts/coder/harness-acceptance.sh --coder PATH/TO/coder
```

Without `--coder` it tests `~/.openagents/bin/coder`. `--only codex,cursor`
runs some harnesses, and `--list` names them all. Results go to
`~/work/harness-acceptance/<version>-<commit>/`: `report.md` holds the matrix,
`report.json` holds the same data, and `evidence/<harness>/` holds every
screen, the export and the `/models` lists. The script exits 1 when any check
fails. The full run takes 10 to 20 minutes and a few cents of usage.

## What it does

The gate drives the real Coder TUI headlessly in tmux, on its own socket.
Each harness gets its own temporary HOME and Coder state, holding copies of
only the login that harness needs. `account.json` and `sync.json` are never
copied. Claude Code reads the login Keychain through a link, as
`scripts/release/acceptance.sh` does. The gate never writes to
`~/.openagents` or to a harness's own folders.

For each harness, the gate opens a new chat, pins it with Shift+Tab and sends
three tiny turns:

1. "who are you? answer in one line"
2. "what's 2+2?"
3. "Is 391 prime?". Before this turn the gate picks another model in
   `/models`.

Checks for each harness:

| Check | What passes |
| --- | --- |
| answer | Every turn ends with a non-empty reply on screen. The time is recorded. |
| rail | The rail reads `Harness (Model display name)` and names the model that actually ran. A raw id or slug (`[fast=true]`, `opencode/big-pickle`, "Swe 1 6 Slow") fails. |
| default-model | The model that ran is the harness's intended default, read from the harness's own CLI or config: Grok uses the `grok models` default, Devin uses its configured SWE-2 model, Cursor uses `cursor-agent`'s selected model, OpenCode uses `opencode.json`'s model, and Oh My Pi uses any chat-capable model but never `clef*`. Codex and Claude use Coder's own defaults. |
| thinking | When the model reported reasoning (in the export or the harness's own session log), the reasoning text appeared in the transcript. |
| clean | The replies, the transcript and the export hold only the conversation. They contain no "doesn't ask before every step" notice, no "New chat…" line, no raw HTTP error and no log path. |
| models-new | `/models` in a new chat with the harness picked, before any message, lists that harness's own catalog. |
| models-turn | The same after a turn. The current model is marked, and OpenCode lists free models first. |
| one-lane | Every turn in the chat ran on the same harness, model and payer, even after another model was picked mid-chat. |
| shift-tab | Shift+Tab after a turn doesn't switch the chat's harness. |
| export | The `/export` ATIF matches the screen: the prompts, the replies and the models. |
| ctrl-n | Ctrl+N leaves the new chat's transcript empty. |
| click-model | Clicking a reply shows its model for about 3 seconds. The model then disappears and never enters the transcript or the export. The gate sends the click as an SGR mouse event. |
| usage | For Codex and Claude, the rail's plan windows are labelled by window length (`5h`, `week`), never `0d`, and match the provider's own numbers where its log has them. |
| model-calls | Every `model_calls` entry in the export names a model and has a non-zero `duration_ms`. |
| prompt-tokens | The Coder loop's first-turn prompt for a one-line question is under the budget (`--prompt-budget`, 5,000 tokens by default). |
| billing | A Claude model picked in the Coder loop, on a computer with a Claude login, runs on the Claude plan and not on gateway credit. |

Other scenarios:

| Scenario | What passes |
| --- | --- |
| plugins-toggle | Every row in `/plugins` turns off and back on from the screen. This includes installed extension plugins such as Disk cleanup, copied from `~/.openagents/extensions`. |
| plugins-details | No details pane shows a raw 64-zero publisher id or an extensions path. |
| projects-chat | Coder's answer to "can you make a new project, what do you need" doesn't claim a repository is required. |
| projects-live | `openagents projects list` reads the live account, read-only. This needs the gate sign-in key. |
| projects-local | A local or test server takes the full flow: a project with just a name, a second project on a repository another project uses, and two repositories with a primary. Set `HACC_PROJECTS_ORIGIN` and `HACC_PROJECTS_TOKEN`. |
| web-chat | A question in a local web chat (`scripts/dev/full-local.sh`, or `HACC_WEB_ORIGIN`) gets an answer with no "Run in Coder on …" button. The message makes exactly one thread, and the composer has no Chat/Code mode switch. |

A harness that isn't installed shows as "not installed", and a check that
doesn't apply shows as "n/a". Neither counts as a pass. The Coder-loop checks
need an OpenAgents sign-in key just for the gate: an `oa_sk_` key saved as
`OPENAGENTS_TOKEN=…` in `~/work/.secrets/coder-acceptance.env`, which
`--signin-key-file` can override. Without the key, those checks are skipped
and the report says so.

## Where it is required

- **Releases.** Before it uploads anything, `scripts/release/coder.sh
  --publish` runs the gate on the macOS archive it just built. A failing
  check refuses the publish. `--skip-acceptance "REASON"` publishes without
  the gate. The reason is required, and it is published as
  `coder-<version>.acceptance.md` next to the archives. When the gate runs,
  its report is published there instead.
- **The owner's `coder` and `coderrc`.** Install local builds with
  `scripts/coder/install-local.sh --to coder|coderrc --from DIR`. The script
  uses a passing report for that exact build, or runs the gate first, and it
  refuses to install on a failure. `--skip-acceptance "REASON"` overrides the
  gate and logs the reason next to the install. The owner's `coderrc`
  shell function installs a finished build through this script. Never copy
  binaries into `~/.openagents/bin` or `~/.openagents/coderrc` by hand.

## Not covered

The gate reads text, not colour or layout. Once per release, look at
`evidence/<harness>/turn*-final.txt`, and check by hand that the clicked
model hint is gray and right-aligned. The Coder loop and billing need the
gate sign-in key. The web and local projects scenarios need a local site.

Each bug the owner finds that the gate missed becomes a new check here.
