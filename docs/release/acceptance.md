# Release acceptance gate

The acceptance gate runs the owner's real flows end to end on the exact build
that is about to ship. Unit tests, stand-ins, and `--no-run` checks missed
every bug the owner found on 2026-09-30 (#10071 to #10079, #10081). The gate
exists so that the owner never finds one of those first again
([#10080](https://github.com/OpenAgentsInc/openagents/issues/10080)).

Run the gate, and read its summary, before any owner handoff and before every
TestFlight or desktop release. A FAIL blocks the handoff until it is fixed or
the owner accepts it by name.

## Run it

On a Mac with Codex, Claude Code, and Grok Build signed in:

```sh
# The .app scripts/desktop/package-macos.sh just packaged (the default):
scripts/release/acceptance.sh

# A specific .app:
scripts/release/acceptance.sh --app /path/to/OpenAgents.app

# Freshly built binaries:
cargo build -p coder --bin coder -p microcoder --bin microcoder \
  -p openagents-cli --bin openagents -p openagents-desktop --bin openagents-desktop
scripts/release/acceptance.sh --bin-dir "$CARGO_TARGET_DIR/debug"
```

| Option | Meaning |
| --- | --- |
| `--app PATH` | The `.app` under test: `Contents/MacOS/{OpenAgents,coder,microcoder}` and `Contents/Helpers/openagents`. |
| `--bin-dir DIR` | Take `openagents-desktop`, `coder`, `microcoder`, and `openagents` from `DIR`. |
| `--evidence DIR` | Where results and evidence go. The default is a new folder under `$TMPDIR`. |
| `--only NAMES` | Run only these scenarios, comma-separated. `--list` prints the names. |
| `--allow-missing-engine` | Skip, instead of fail, the scenarios that need an engine whose login is missing. |
| `--no-engines` | Read no engine login at all (no Codex copy, no Keychain link) and skip the scenarios that need one: for running the UI and chat scenarios alone, such as `--only route-map,route-map-chat`. |
| `--keep` | Keep the temporary home folder for inspection. |

Each scenario prints `PASS NAME: …`, `FAIL NAME: …`, or `SKIP NAME: …`. The
script then prints a summary table and the evidence folder. It exits 1 when
any scenario fails and 2 when it cannot set up.

A full run takes 15 to 30 minutes, mostly Coder runs. It sends about fifteen
chat messages and starts five or six tiny Coder runs (one line in
`NOTES.md`), each finished in a few steps (runs have no step limit), so it
costs a few cents of engine usage.

## What it runs

The gate never touches the real home folder, its stores, the installed app,
or the owner's host.

1. **A temporary home.** `HOME` is a short folder under `/tmp`, so the control
   socket path fits the 104-byte limit. Every process the gate starts sees only
   that home.
2. **Engine logins, read-only.** These follow the owner-local smoke approach:
   - Codex: `CODEX_HOME` is the temporary home's `.codex`, holding a private
     copy (mode `0600`) of `~/.codex/auth.json` that is deleted with the
     temporary home. Coder's Codex transport reads the login and never
     refreshes it, so the real login never changes. It is a copy, not a
     symbolic link, so nothing a run does reaches the real file. (A take,
     `codex_transport::Login::take`, removes only a run's own copy and
     refuses a link, #10083.)
   - Claude Code: the temporary home's `Library/Keychains` is a symbolic link
     to the real one, so `claude` reads its `Claude Code-credentials` item the
     way it always does. `~/.claude.json` in the temporary home holds only the
     account metadata (`oauthAccount`, `userID`, and the onboarding flag), never
     a credential.
   - Grok Build: `GROK_BIN` names the owner's installed `grok`, and the
     temporary home's `.grok` holds a private copy (mode `0600`) of
     `~/.grok/auth.json` (or `$GROK_HOME/auth.json`), deleted with the
     temporary home; a set `XAI_API_KEY` is used as it is instead. Grok Build
     refreshes its sign-in only near its expiry, and a refresh could retire
     the owner's refresh token, so the gate copies the login only while it
     has more than an hour left (a full run takes under half that). With
     less, run `grok` once to refresh it, then run the gate. The real file is
     only read. At the default `full` access (#10104), Coder runs Grok Build
     with `--always-approve` as the person; under a named `toolchains`
     access it runs it inside its own boundary with another copy of that
     login in the turn's scratch, removed when the turn ends (#10092).

   Only the Codex and Grok Build login files are copied, and nothing is
   printed. A missing login is a FAIL unless you pass
   `--allow-missing-engine`.
3. **The owner's project shape.** The gate makes a scratch repository and a
   linked worktree of it, `acceptance-repo-host-tasks`, like
   `~/work/openagents-host-tasks`, with a local bare repository,
   `acceptance-remote.git`, as its `origin`. The tree is big enough to matter: 400
   directories and 3,200 files in the repository and 650 sibling folders beside
   it, like `~/work`.
4. **The build's own host, under the app's limits.** The gate runs
   `coder host serve --keys … --iroh --control` the way the app starts it,
   with file keys in the temporary home instead of the login keychain, and
   with the open-file limit at launchd's 256 for a login agent or a
   Finder-opened app (#10078). It registers the worktree with `project_add`, as
   the app's folder picker does, and turns auto-start on with
   `autostart_set`, as the app's switch does. `coder.start` is `at_once`.
5. **The build's own desktop binary.** `openagents-desktop --acceptance DIR`
   runs the window's real model: the chat panel, sidebar, settings file,
   Coder lane, and Verse layer. It runs inline instead of in a window, against
   the host's control socket and under the same open-file limit. Messages go
   through the host to the live chat worker on `relay.openagents.com` exactly
   as the window sends them, and the host adds the desktop surface context
   (#10077). A coding reply starts Coder on this computer through the window's
   own Coder lane, on real engines, in the scratch project. The driver checks
   the view tree, the scene the window would paint, and captures.
6. **A phone-shaped client.** The driver pairs a NIP-HOST client
   (`coder_computers::Live` with `Platform::Phone`) using the host's pairing
   invitation. It asks the hosted chat worker with the phone's context, as
   `CoderTab::context` builds it, and presses Run Coder as
   `CoderTab::run_coder` does. `phone-sim-start` goes further and drives
   the actual iOS app in an iOS simulator the gate creates for the run: the
   app pairs with the host's invitation and asks from its Chat tab. The
   simulator and Xcode run under the real home; the app reaches the host
   over the relay and iroh as a phone does.

7. **The gate's own scenarios.** Some scenarios need no window: the script
   runs them itself with the build's binaries. `explain-error` plants a
   failure in a scratch folder under the temporary home, runs it, and hands
   the output to `openagents plugin run crates/plugin-explain-error`, which runs
   the plugin's workflow in Coder's program runtime granted reads only. It
   costs nothing: no model and no engine.
   `plugins-chat` asks the live chat worker "which plugins can I test?"
   with the build's `openagents chat --scratch --no-run` (a throwaway
   identity) and checks that the reply names every plugin
   `deploy/eval-runner/catalog` lists, by its `package.json` name: one
   router judgment and no model call when the bank answers.
   `essays-chat` asks the live chat worker two questions about our essays
   ("what is a capability claim?" and "what is your thesis about general
   agents?") and "summarize both of the essays, please" the same way, and
   checks each reply carries that essay's idea and is not the
   no-documented-answer reply, and that the summary names both essays and
   was not dispatched to Coder.

When the scenarios end, the gate gives Coder runs up to four minutes to
finish, stops any that remain, stops the host, copies the task store's
journals into the evidence, and deletes the temporary home.

## Scenarios

Every owner-reported bug adds a scenario. The chat scenarios that share a
conversation run in the owner's order.

| Scenario | Checks | Guards |
| --- | --- | --- |
| `ui-placeholder` | An empty chat's centered composer paints **Ask OpenAgents anything** (faint-ink pixels in the composer field of a 2x capture). | #10072 |
| `ui-starter-chips` | A new chat's starters (the phone's shared list: Who are you?, What can you do?, What's new in the Gym?, What tools do you have?) are small chips in the follow-ups' row directly above the centered composer, inside the column at 1200x840 and 760x540; tapping Who are you? sends it, and after the reply the starters are gone and the composer docks. Runs in its own chat. | #10097 |
| `who-are-you` | "who are you" gets an answer with suggestions, and Coder does not start. | #10073 |
| `ui-chips` | The reply's suggestions are small chips in a row directly above the composer, none in the transcript. | #10075 |
| `ui-engines-sidebar` | Each engine from the host's report is one condensed row in the sidebar, above the footer (its routes, then an engine beside them such as Grok Build when installed), and the transcript shows no engine block. | #10072, #10091 |
| `delegate-who` | "who can you delegate to", in the same chat, gets an answer, and Coder does not start. | #10073 |
| `delegate-now` | "do a test delegation now", in the same chat: Coder starts, runs, and finishes in the linked-worktree project; the prompt Coder received is that message; the reply carries no Gym card; and no decision-call row shows in the transcript. | #10073, #10078 |
| `followup-chat` | After `delegate-now`'s run finished, in the same chat: the composer says "Ask OpenAgents anything", and "summarize what happened" goes to the router, which answers it in chat from the run's result (the request's `context.coder_run`); the reply hands nothing to Coder, and Coder takes no new turn. | #10094 |
| `followup-coder` | Then "now also list the top-level files in a note": the router hands it to Coder, which continues the same task as its next turn, in the same worktree, and finishes; the person's message shows above the "Coder continued … (turn N)" card. | #10094 |
| `working-directory` | "What's the working directory right now?" names the project folder, never says to connect a computer, and starts no Coder. | #10077, #10079 |
| `delegate-claude` | "do a test delegation to claude": the offer names Claude Code, Coder starts on Claude Code and finishes (the handoff tells it the routing is done, so it checks the project instead of running `claude` itself), the start card's limit words agree with the engine readings, and the message shows once. | #10076, #10073, #10084 |
| `delegate-grok` | "do a test delegation to grok", with no settings file: the offer names Grok Build, and real Grok Build starts in the linked-worktree project and finishes; the chat never says Grok Build is not allowed. | #10091, #10092 |
| `push-main` | "commit a short NOTES.md line and push it to main", in its own chat, on the default settings: Coder starts, commits, pushes, and finishes with no question or approval event, and the scratch repository's bare remote then has a new `main` that changes NOTES.md. | #10104 (Coder approves every step and never asks) |
| `ui-stop-coder` | While Coder runs, the transcript's **Stop Coder** is as wide as its words (under 160 points and a third of the transcript), as the phone draws it. Measured during the first run an earlier scenario followed; run alone, it starts one. | #10091, #10075 |
| `ui-no-attach` | The desktop is text only: the composer has no attach control at 1200x840 and 760x540, and a PNG dropped on the window and an image paste are dropped quietly (no image card, no notice, the draft stays empty). It replaced `image-to-coder` when attachments were turned off everywhere on 2026-10-01; since attachments came back on (#11174) the gate turns the switch off for itself, so it checks the text-only mode the switch still offers. | #10093, #10095, #11174 |
| `open-deck` | "open the three devdays later deck" on the desktop gets a typed `open_presentation` offer for that deck, and the slide viewer opens when the reply arrives. | #10058, #10082 |
| `ui-new-chat-top` | Cmd/Ctrl-N's new chat is the sidebar's top row (below any pinned chats) and selected at 1200x840 and 760x540, above every older chat including older Coder chats in projects; there are no project headers (a Coder row names its project on its context line) and the shared list order puts the new chat first. | #10100 |
| `ui-filter-sessions` | **Filter sessions…** is hidden with fewer than five chats and shows with five. | #10072 |
| `ui-no-verse` | The Verse world never loads while a chat page shows, loads on the Verse page, and is released after. | #10071 |
| `route-map` | The Map page opens from the sidebar's footer, zooms into `work.dispatch`, inspects Coder, opens the Gaps panel, and shows `capability.missing` as a gap with a next step; leaving releases the page. Offline: no chat message. | #10085 |
| `route-map-chat` | "show me how you route things" on the desktop gets the router's typed `routes.map` offer, and the Map page opens when the reply arrives, with no click. | #10085, #10102 |
| `phone-claude` | A paired phone-shaped client asks "do a test delegation to claude" and presses Run Coder; the computer's run starts on Claude Code. | #10081 |
| `phone-start-at-once` | The phone's own Coder tab (the shared Rust the iOS and Android apps run), paired with the host, asks a coding question through the live chat; the computer's presence says it starts Coder at once (`coder.start: at_once`, auto-start on), and the reply starts exactly one Coder task there with no **Run Coder** tap, the chat showing the start with Stop and no Run Coder. | #10101 |
| `phone-agents` | A phone-shaped client paired with the host waits for the computer's presence to name its coding agents (`engine-<state>-<engine>` capabilities), asks "what coding agents are connected?" through the live chat with those agents in its context, as the phone sends them, and checks the reply names every agent that is ready there. | #10119 |
| `phone-sim-start` | A gate scenario with the actual iOS app: the gate creates its own simulator (`oa-loop-gate`, deleted after; never the owner's), installs the app (`OPENAGENTS_ACCEPTANCE_IOS_APP` names a built simulator `OpenAgents.app`; otherwise `bins/openagents-ios/build.sh sim` builds one), and opens it with the host's invitation (`--connect-link`, as the camera opens the QR code) until the host lists the phone. Opened again, still paired, it sends "Can you look through the code in my project and summarize what it implements?" from its Chat tab's composer; the live chat routes it, and the reply must start exactly one Coder task on the host (`autostart.jsonl`, event `started`) within six minutes, with no tap. Screenshots `1-paired.png`, `2-before-send.png`, `3-after-reply.png`, and `4-after-start.png`. SKIP where Xcode or an iOS simulator runtime is missing. | #10118 |
| `phone-closed-loop` | The owner's whole loop from the phone, on a scratch clone of this repository whose `origin` is a local bare repository, made the host's project with auto-start at full access (as the owner runs it), and the owner's toolchains, signing, and App Store Connect key reached through the login shell. The phone's Coder tab asks for a small change: Coder starts at once, commits it, and pushes it to the bare remote's `main` with no question. Then it asks for a TestFlight build as a dry run: Coder runs `scripts/release/testflight.sh --validate-only` to the end (archived, validated by App Store Connect, nothing uploaded). The phone's card must show the run going (time worked) and how it ended; `timeline.jsonl` keeps every card text with its time. About 20 to 40 minutes. Skipped without the App Store Connect key. | #10118 |
| `phone-dead-task` | A Coder run whose process dies never blocks its project. The phone's own Coder tab starts a task; once the host admitted it, the gate kills the task's owner process and everything under it (SIGKILL, as a crash or restart would). The phone must read that run as stopped with the headline "Coder's process ended unexpectedly", and a second coding message from the phone must start Coder in the same project and finish, with no `unadmitted` or `not_started` and no "Couldn't start"; the killed run's record stays, ended as `owner_process_ended`. A few minutes. | #10124 |
| `explain-error` | A gate scenario, outside the window: a Python file with a wrong dictionary key is planted in a scratch project and run, and the build's `openagents plugin run` runs the [Explain this error](../plugins/examples/explain-this-error.md) plugin's workflow on its output with reads only. The plugin names `billing.py:5`, shows the line, and suggests the key the dictionary has. | #10086 |
| `plugins-chat` | A gate scenario, outside the window: "which plugins can I test?" through the build's `openagents chat --scratch`; the live chat's reply names every plugin in `deploy/eval-runner/catalog`, the catalog the hosted runner and the Gym's chips use. | #10090 |
| `essays-chat` | A gate scenario, outside the window: "what is a capability claim?", "what is your thesis about general agents?", and "summarize both of the essays, please" through `openagents chat --scratch`; each reply carries its essay's idea (Test-Time Capabilities, The Return of the General Agent) and is not the no-documented-answer reply, and the summary names both essays and is not dispatched to Coder. | #10099, #10102 |

## Proof that it catches the owner's bugs

On 2026-09-30, the same gate run against earlier builds failed where the owner
did:

- `846965af1c` (TestFlight build 39): `delegate-now` failed with "the granted
  source snapshot is unavailable or changed" (#10078), and
  `working-directory` started Coder for a question it answered (#10079).
- `30b7a81602`: `phone-claude` started the phone's run on Codex (#10081).

All three pass on `f6d0c4cb2a`, which carries the fixes.

## Evidence

The evidence folder holds:

- `results.jsonl`: one `{"scenario", "status", "detail"}` line per scenario.
- `summary.md`: the summary table.
- `NAME/`: each scenario's chat `snapshot.json`, Coder's `coder-lines.jsonl`,
  the transcript's words (`transcript.txt`), the host's `engine-report.json`,
  captures (`*.png`), and, for Coder runs, the turn's ATIF trajectory.
- `tasks/`: the task store's journals and records, including
  `repository-launch-*.jsonl` launch diagnostics.
- `host.log`, `desktop.log`, `autostart.jsonl`, and `build.txt` (the binaries'
  SHA-256 digests).

## Add a scenario

A scenario that needs no window, such as `explain-error`, is a shell function
in `scripts/release/acceptance.sh` named in `gate_scenarios`, with a row in
the table above. Every other scenario drives the window:

1. Add a function to `crates/openagents-desktop/src/acceptance.rs` that drives
   the window the way the person did (`new_chat`, `send`, `follow_run`,
   `capture`) and returns `Ok(evidence)` or `Err(what was wrong)`.
2. Add its name to `SCENARIOS` and to the `match` in `run`, and to
   `desktop_scenarios` in `scripts/release/acceptance.sh`.
3. Add a row to the table above that names the issue it guards.
4. Run it against the build that had the bug, and check that it fails.
