# Unreleased

Entries accumulate here between releases. Appending an entry when your change
lands on `main` is part of the CLAIM-RELEASE protocol — see `README.md` in
this directory for the required format. `pnpm changelog roll` moves these
entries into the next dated release file.

## CLI source version is 0.2.0-rc.18

- issues: #352, #353, #354, #355, #356
- commits: this change
- contracts-specs: `docs/psionic/CLI.md` slices 7–10; producer name `X.Y.Z-rc.N`
- invariants: published `<version, platform>` objects stay immutable
- evidence: `until_ctx_done_on_fixture`; `until_prompt_done_on_fixture`; `until_gen_done_on_fixture`; `fixture_plan_is_nonzero_and_matches_formula`; `bar_bounds`
- lane: cursor session 7822942d

The `openagents-cli` crate is `0.2.0-rc.18`. rc.17 is published and
immutable. This line is context allocation, progress bars, tokenize,
prefill, and generate on the fixture path.

## Inference run walks through Context ready and Inference complete (#352–#356)

- issues: #352, #353, #354, #355, #356
- commits: this change
- contracts-specs: `docs/psionic/CLI.md` `ctx.*`, `prompt.*`, `prefill.*`, `gen.*`, progress meter
- invariants: no `psionic-serve` import; no Ollama fallback
- evidence: `until_ctx_done_on_fixture`; `until_gen_done_on_fixture`; `load_then_status_then_unload_on_fixture`
- lane: cursor session 7822942d

`inference run` allocates hybrid KV and Gated DeltaNet caches (default
runtime `n_ctx` 4096 on 27B-class files), tokenizes `--prompt`, prefills,
and greedy-decodes. The TUI memory line shows mmap resident / mapped.
Determinate bars paint when a total is known and the step is long.
Fixture `gen.done` is embed → RMSNorm → lm-head. 27B uses the same
leaf plus Q8_0; it is not the 64-layer hybrid graph.

## CLI source version is 0.2.0-rc.17

- issues: #351
- commits: this change
- contracts-specs: CLI crate version; producer name `X.Y.Z-rc.N`
- invariants: published `<version, platform>` objects stay immutable
- evidence: `cmp_release_versions`; live bucket already holds `0.2.0-rc.16`
- lane: cursor session abb3a3ed

The `openagents-cli` crate is `0.2.0-rc.17`. rc.16 is published and
immutable. This line carries the scrolled-up transcript pin (#351) onto
Apple aarch64 and Windows x86_64.

## Scrolled-up transcript stays put while tokens stream (#351)

- issues: #351
- commits: this change
- contracts-specs: `scroll_override` is follow-mode, matching grok-build; live `apply()` events do not clear a pin
- invariants: none changed
- evidence: `streaming_does_not_steal_a_scrolled_up_pin`; `scrolling_to_the_bottom_resumes_follow`
- lane: cursor session abb3a3ed

If the transcript is scrolled up, streaming tokens and tool output no
longer jump the viewport to the bottom. Follow resumes when the reader
scrolls to the bottom or starts a new prompt.

## CLI source version is 0.2.0-rc.16

- issues: #350
- commits: this change
- contracts-specs: CLI crate version; producer name `X.Y.Z-rc.N`
- invariants: published `<version, platform>` objects stay immutable
- evidence: `cmp_release_versions`; live bucket already holds `0.2.0-rc.15`
- lane: cursor session abb3a3ed

The `openagents-cli` crate is `0.2.0-rc.16`. rc.15 is published and
immutable. This line carries the Windows host-shell fix (#350) plus the
Windows mouse-tracking fix (#349) onto Apple aarch64 and Windows x86_64.

## Windows bash tool uses cmd.exe, not /bin/sh (#350)

- issues: #350
- commits: this change
- contracts-specs: `pty::shell_command` is the host-shell argv; bash `/run` spawn through it; coder `rust.bash` surface
- invariants: none changed
- evidence: `a_line_a_shell_would_change_the_meaning_of_is_given_to_a_shell`; `a_bash_command_that_exits_non_zero_is_reported_as_an_error`
- lane: cursor session abb3a3ed

On Windows the bash tool spawned `/bin/sh`, which is not there, so every
command failed with "The system cannot find the path specified" before the
line ran. File write still worked. The runner now uses this machine's shell:
`/bin/sh -c` on Unix, `%COMSPEC% /d /c` (usually `cmd.exe`) on Windows.

## CLI source version is 0.2.0-rc.15

- issues: #345, #346, #347
- commits: this change
- contracts-specs: CLI crate version; producer name `X.Y.Z-rc.N`
- invariants: published `<version, platform>` objects stay immutable
- evidence: `load_fixture_shows_weights_ready_and_unload_clears_memory`; `load_then_status_then_unload_on_fixture`; live bucket already holds `0.2.0-rc.14`
- lane: cursor session 7822942d

The `openagents-cli` crate is `0.2.0-rc.15`. rc.14 is published and
immutable. This line is Coder `/load` `/unload`, in-process weight
release, and memory fields on `inference status`.

## Coder TUI shows local GGUF load progress (#345)

- issues: #345
- commits: this change
- contracts-specs: Coder `/load <path>`; CLI.md load messages on the status row
- invariants: teach essay stays off the transcript; fail magic is not a chatting hang; Ollama `--local` is unchanged
- evidence: `load_fixture_shows_weights_ready_and_unload_clears_memory`; `load_bad_magic_shows_canonical_fail_and_does_not_hang`; `load_status_shows_weights_ready_outside_the_transcript`
- lane: cursor session 7822942d

`/load` maps a GGUF in this process and paints CLI.md load messages
through Weights ready. Progress is a status line, throttled like
`prefill.pos`. A bad magic string fails with the canonical wording.

## `inference unload` releases in-process weights (#346)

- issues: #346
- commits: this change
- contracts-specs: `inference unload`; Coder `/unload`; CLI.md `unload.*` ids
- invariants: unload is distinct from `inference stop`; mmap then Metal; empty unload still prints Weights unloaded
- evidence: `unload_with_nothing_loaded_still_prints_weights_unloaded`; `load_then_status_then_unload_on_fixture`
- lane: cursor session 7822942d

After a load, `inference unload` and Coder `/unload` release the mmap
and Metal wrap and print `Weights unloaded`. Status then reports not
loaded.

## `inference status` reports mmap, Metal, and RSS (#347)

- issues: #347
- commits: this change
- contracts-specs: `inference status --json` memory fields; CLI.md `mem.*` ids
- invariants: shared Metal wrap is not a second copy; caches stay pending until ctx; RSS may remain after unload
- evidence: `status_json_includes_memory_fields`; `load_then_status_then_unload_on_fixture`
- lane: cursor session 7822942d

Status JSON now includes `mmap_bytes`, `metal_bytes`, `rss_bytes`,
`cache_kv_bytes`, and `cache_gdn_bytes`. The Coder status row shows a
compact mmap / Metal / RSS meter while weights are loaded.

## Windows trackpad scrolls the transcript, not input history (#349)

- issues: #349
- commits: this change
- contracts-specs: Windows host mouse tracking writes the same ANSI sequences Unix `EnableMouseCapture` already writes; `apply_mouse` keeps wheel scroll off the history ring
- invariants: none changed
- evidence: `host_mouse_tracking_sequences_match_crossterm_unix`; `wheel_scrolls_the_transcript_even_when_history_has_entries`
- lane: cursor session abb3a3ed

On Windows, `EnableMouseCapture` only called `SetConsoleMode`, so Windows
Terminal never enabled mouse reports. Trackpad and wheel became Up/Down and
walked the composer history. The session now writes the ANSI mouse-tracking
sequences on Windows and names a failed enable instead of swallowing it.

## CLI source version is 0.2.0-rc.14

- issues: #348
- commits: this change
- contracts-specs: CLI crate version; producer name `X.Y.Z-rc.N`
- invariants: published `<version, platform>` objects stay immutable
- evidence: `cmp_release_versions`; `update_check_names_older_stable_as_current`; live bucket already holds `0.2.0-rc.13`
- lane: cursor session abb3a3ed

The `openagents-cli` crate is `0.2.0-rc.14`. rc.13 is published and
immutable. This line refuses an older channel pointer as an upgrade
(`oa update --check` no longer offers `0.1.1` over `0.2.0-rc.13`).

## CLI source version is 0.2.0-rc.13

- issues: #343
- commits: this change
- contracts-specs: CLI crate version; producer name `X.Y.Z-rc.N`
- invariants: published `<version, platform>` objects stay immutable
- evidence: `one_shot_stops_on_done_not_on_disconnect`; `a_prompt_without_a_token_exits_naming_login`; live bucket already holds `0.2.0-rc.12`
- lane: cursor session abb3a3ed

The `openagents-cli` crate is `0.2.0-rc.13`. rc.12 is published and
immutable. This line flushes `--prompt` writes and stops the one-shot
drain on `Done` so a piped caller sees the answer and the process exits.

## `--prompt` flushes each streamed write (#343)

- issues: #343
- commits: this change
- contracts-specs: `run_one_shot` flushes stdout/stderr after every chunk, failure, and notice, and stops on `Done` instead of waiting for disconnect
- invariants: a piped `--prompt` must show text before process exit and must exit after the turn
- evidence: `one_shot_stops_on_done_not_on_disconnect`; `a_prompt_without_a_token_exits_naming_login`
- lane: cursor session abb3a3ed

`--prompt` exists for pipes. Two defects stacked: `print!` without a
flush left the answer in the libc buffer, and the drain loop waited for
the channel to disconnect while the session still held a `Sender`, so
the process never exited after `Done`. The TUI already breaks on `Done`.
This path now flushes and stops on `Done`.

## CLI source version is 0.2.0-rc.12

- issues: #334, #337
- commits: this change
- contracts-specs: CLI crate version; producer name `X.Y.Z-rc.N`
- invariants: non-release builds still report `0.0.0-dev`; published `<version, platform>` objects stay immutable
- evidence: live bucket already holds `0.2.0-rc.10` and `0.2.0-rc.11`
- lane: cursor session abb3a3ed

The `openagents-cli` crate is `0.2.0-rc.12`. rc.11 is published and
immutable. This line is EventStream typing (#334) plus mid-turn capability
declarations (#337).

## A capability loaded mid-turn is declared on the next model round (#337)

- issues: #337
- commits: this change
- contracts-specs: `CoderRuntimeSession` re-reads `list_tools()` each model round and rewrites the standing system prompt
- invariants: a plugin that answers "available now" is in that turn's next tool list; the TUI lite prompt is not swapped for the rust builder
- evidence: `a_capability_load_refreshes_the_standing_declarations_for_the_next_round`
- lane: cursor session abb3a3ed

`capability` loaded a plugin and told the model to call it in the same turn.
The turn loop had frozen `list_tools()` at start, so the next round still
omitted the tool and the prompt still said the old count. Each round now
refreshes declarations. The TUI's lite system prompt stays lite.

## CLI restores TUI typing after the 0.2.0-rc.10 hang fix (#334)

- issues: #334
- commits: this change
- contracts-specs: Coder event loop reads `crossterm::event::EventStream`
- invariants: a published `<version, platform>` object stays immutable; rc.10 stays as shipped
- evidence: `coder_interactive_pty` `typed_characters_echo_into_the_composer_and_backspace_removes_them` and `an_installer_pipe_still_accepts_terminal_input` failed on rc.10 (`poll(0)` never reported keys) and pass after this change; `model_picker_lists_served_pro_models_and_flash_refuses` still passes
- lane: cursor session abb3a3ed

`0.2.0-rc.10` painted the composer and dropped every key. The #334 hang
fix replaced blocking `event::poll(50ms)` with `poll(0)` plus a tokio
sleep so spawned HTTP could run. On macOS with `use-dev-tty`, `poll(0)`
never reports a pending key, so `read` never ran. The loop now waits on
`EventStream`, which sees keys without parking the runtime. The crate
is `0.2.0-rc.11` because rc.10 is already published.

## CLI source version is 0.2.0-rc.11

- issues: #334
- commits: this change
- contracts-specs: CLI crate version; producer name `X.Y.Z-rc.N`
- invariants: non-release builds still report `0.0.0-dev`; published `<version, platform>` objects stay immutable
- evidence: live bucket already holds `0.2.0-rc8`, `0.2.0-rc.8`, `0.2.0-rc9`, and `0.2.0-rc.10`
- lane: cursor session abb3a3ed

The `openagents-cli` crate is `0.2.0-rc.11`. rc.10 is published and
immutable. This line is the EventStream input fix.

## CLI source version is 0.2.0-rc.10

- issues: #330
- commits: this change
- contracts-specs: CLI crate version; producer name `X.Y.Z-rc.N`
- invariants: non-release builds still report `0.0.0-dev`; published `<version, platform>` objects stay immutable
- evidence: live bucket already holds `0.2.0-rc8`, `0.2.0-rc.8`, and `0.2.0-rc9`; `SHA256SUMS-0.2.0-rc.10` is 404
- lane: cursor session abb3a3ed

The `openagents-cli` crate is `0.2.0-rc.10`. RC 8 and 9 are already published, so this is the next producer name under the dotted grammar. Its macOS artifact is the entitlements-bearing #330 republish.

## CLI release ships openagents-coder-api beside the CLI (#335)

- issues: #335
- commits: this change
- contracts-specs: `ops/release-cli.sh` sibling object `openagents-coder-api-<version>-<platform>`; installer bin-dir placement
- invariants: a published `<version, platform>` object stays immutable, including the sibling
- evidence: SHA256SUMS names both objects; `coder --dev` looks next to `current_exe` and in `~/.openagents/bin`
- lane: cursor session abb3a3ed

`coder --dev` starts `openagents-coder-api` from the same directory as the CLI.
The release now builds that binary for every platform, stages
`openagents-coder-api-<version>-<platform>`, and publishes it as a second
object. The installer copies it into the same bin dir when the sums file names
it.

## CLI release names are X.Y.Z or X.Y.Z-rc.N (#336)

- issues: #336
- commits: this change
- contracts-specs: `ops/release-cli.sh` producer version grammar; source, lockfile, and changelog agreement; Cargo completion gate on `--publish`
- invariants: a published `<version, platform>` object stays immutable; a new build takes the next `rc.N`
- evidence: `ops/release-cli-version-test.sh`
- lane: cursor session abb3a3ed

`ops/release-cli.sh` refuses `0.2.0-rc8` and other suffix spellings. An RC is
`X.Y.Z-rc.N` only. The crate, lockfile, and `UNRELEASED.md` must already name
that version before a build starts. `--publish` runs the Cargo completion gate
unless `OPENAGENTS_CLI_RELEASE_GATE=passed`. `--skip-tests` cannot waive it.
`--allow-partial` remains the way to publish an Apple-aarch64-only RC.

## CLI source version is 0.2.0-rc7

- issues: none (owner request: republish the rc channel with the #330 fix)
- commits: this change
- contracts-specs: CLI crate version
- invariants: non-release builds still report `0.0.0-dev`
- evidence: `crates/openagents-cli/tests/coder_frame.rs` startup-facts frame test; `a_non_release_build_reports_a_development_version`
- lane: coder crash fix session

The `openagents-cli` crate is `0.2.0-rc7`. Its macOS artifact is the first
channel binary signed with the wasm JIT entitlements (#330).

## macOS release binaries keep the wasm JIT entitlements (#330)

- issues: #330
- commits: this change
- contracts-specs: `ops/release-cli.sh` macOS signing; `ops/macos-entitlements.plist`
- invariants: macOS artifacts are signed with hardened runtime AND the JIT entitlements; a release that kills the process on first wasm invoke must not publish
- evidence: rc6 repro `coder plugin run patch_check …` 3/3 `Killed: 9`; the same binary re-signed with the entitlements runs 5/5 clean under hardened runtime
- lane: coder crash diagnosis session, Aug 29

0.2.0-rc release binaries died with `SIGKILL (Code Signature Invalid)` on the
first wasm capability invoke — `/resume`'s foreign-session scanner, the
session-start capability search, or `oa plugin run`. Signing used hardened
runtime with no entitlements, so the kernel killed every Cranelift JIT page.
The release script now signs with `ops/macos-entitlements.plist`
(`allow-jit`, `allow-unsigned-executable-memory`). The published channel
binaries still need a republish.

## CLI source version is 0.2.0-rc6

- issues: none (owner request: ship current main as rc6)
- commits: this change
- contracts-specs: CLI crate version; Coder idle welcome chrome
- invariants: non-release builds still report `0.0.0-dev`; the idle card title is `New in v0.2.0`, not the RC name
- evidence: `crates/openagents-cli/tests/coder_frame.rs` startup-facts frame test; `a_non_release_build_reports_a_development_version`
- lane: grok 0.2.0-rc6 bump

The `openagents-cli` crate is `0.2.0-rc6`. The idle card is "New in v0.2.0".

## Coder idle card names only 0.2.0 work

- issues: none (owner request)
- commits: this change
- contracts-specs: Coder idle welcome chrome
- invariants: the card title is `New in v0.2.0`; 0.1.1 lines stay off it
- evidence: `startup_facts_are_centered_outside_the_transcript`
- lane: grok 0.2.0-rc6 tui

The idle card keeps `/model`, Coder Local, Shift+Tab to Local, and that GitHub login is optional. The 0.1.1 ATIF, Grok, and Flash lines are gone.

## Coder caret is one amber hardware cursor

- issues: none (owner request)
- commits: this change
- contracts-specs: Coder TUI composer caret
- invariants: the frame does not paint a `REVERSED` block over the hardware caret; OSC 12 colours it `#FFB000`; OSC 112 restores the terminal colour on exit
- evidence: `trailing_spaces_are_kept_and_the_caret_sits_after_them`
- lane: grok 0.2.0-rc6 tui

The composer caret is the blinking hardware cursor, coloured palette amber. The reverse-video block that sat around a yellow terminal cursor is gone.

## CLI source version is 0.2.0-rc5

- issues: none (owner request: ship current main as rc5)
- commits: this change
- contracts-specs: CLI crate version; Coder idle welcome chrome
- invariants: non-release builds still report `0.0.0-dev`
- evidence: `crates/openagents-cli/tests/coder_frame.rs` startup-facts frame test; `a_non_release_build_reports_a_development_version`
- lane: grok 0.2.0-rc5 bump

The `openagents-cli` crate is `0.2.0-rc5`. The idle card is "New in v0.2.0-rc5". This candidate includes `/model` for Pro and Local (#323, #324, #327), keeps the capability catalog off the transcript, and puts a blank line above each user turn.

## Coder keeps the capability catalog off the transcript (#322)

- issues: #322
- commits: this change
- contracts-specs: Coder TUI first frame; capability catalog is wire-only
- invariants: `session_start_capability_notice` still seeds the first model request; it is not a Notice
- evidence: `a_fresh_session_does_not_show_installed_capabilities_in_the_transcript`; `a_capability_notice_is_on_the_wire_and_is_not_a_tool_result`
- lane: grok 0.2.0-rc5 tui

0.2.0-rc4 painted "Installed capabilities (host search, not loaded)" as the first transcript Notice. The catalog still seeds the first model request. It does not paint.

## Coder puts a blank line above each user turn

- issues: none (owner request)
- commits: this change
- contracts-specs: Coder TUI transcript layout
- invariants: user turns open with one blank row, matching tool boxes
- evidence: `a_user_message_has_one_blank_line_above_it`
- lane: grok 0.2.0-rc5 tui

Each `>` prompt sits one blank row below the previous entry so a user turn is not flush against a notice or an answer.

## Coder `/model` picks Pro and Local models (#323, #324, #327)

- issues: #323, #324, #327
- commits: f8321436d4
- contracts-specs: Coder TUI `/model` picker
- invariants: only Pro and Local offer a per-model choice; unavailable Pro ids are skipped; Local lists Ollama tags
- evidence: `model_picker_lists_served_pro_models_and_flash_refuses`; `local_model_picker_lists_tags_unsigned`; `local_model_picker_unsigned_without_ollama_shows_install_sign`
- lane: grok model-picker

`/model` opens a picker. Pro lists the deployment catalog. Local lists Ollama models.

## CLI source version is 0.2.0-rc4

- issues: none (owner request: ship current main as rc4)
- commits: this change
- contracts-specs: CLI crate version; Coder idle welcome chrome
- invariants: non-release builds still report `0.0.0-dev`
- evidence: `crates/openagents-cli/tests/coder_frame.rs` startup-facts frame test; `a_non_release_build_reports_a_development_version`
- lane: grok 0.2.0-rc4 bump

The `openagents-cli` crate is `0.2.0-rc4`. The idle card is "New in v0.2.0-rc4". This candidate includes Autopilot in the session skills (#328) on top of headless Autopilot and the 0.2.0-rc2 first-frame fixes.

## Coder skills name Autopilot so a session can start it (#328)

- issues: #328
- commits: this change
- contracts-specs: Coder auto skill (`superdelegate`); CLI skill (`openagents-cli`)
- invariants: Autopilot is the unattended loop an agent starts; interactive nested `openagents coder` is still refused
- evidence: `skills/superdelegate/SKILL.md`; `skills/openagents-cli/SKILL.md`
- lane: grok autopilot-skill

Every Coder session loads `superdelegate`. It now says to run `openagents coder --autopilot` when the person asks, including `--dry-run`. The `openagents-cli` skill carries the same command and carves Autopilot out of "do not start another coder session."

## Headless Autopilot: `openagents coder --autopilot` (#328)

- issues: #328
- commits: this change
- contracts-specs: Coder Autopilot CLI
- invariants: `--dry-run` opens no thread; a dead hop stops the loop; `--offline` cannot combine
- evidence: `autopilot_dry_run_prints_the_plan_and_opens_no_thread`; `autopilot_stops_on_a_dead_hop`; `coder_help_names_autopilot`
- lane: grok coder-autopilot-cli

An agent can run `openagents coder --autopilot` and the CLI takes stock of this workspace, recent local sessions, and open issues, then keeps iterating. `--dry-run` prints the plan without calling a model.

## Coder 0.2.0-rc2: no workspace dump on first frame, take the terminal first

- issues: none (owner: 0.2.0-rc1 painted the #316 snapshot into the transcript and leaked CSI-u on first open)
- commits: this change
- contracts-specs: Coder TUI first frame; workspace snapshot is wire-only
- invariants: the host snapshot is not a Notice; raw mode and the alternate screen start before any git/issue await
- evidence: `a_fresh_session_does_not_show_the_workspace_snapshot_in_the_transcript`; `startup_facts_are_centered_outside_the_transcript`
- lane: grok 0.2.0-rc2 tui

0.2.0-rc1 showed `git log` and open issues as the first transcript Notice, and it waited on that snapshot before taking the terminal, so keys typed during first-open arrived as CSI-u (`7441;1:3u`) on the invoking shell. The snapshot still seeds the first model request. It does not paint.

## Coder first-open works without OpenAgents sign-in (#325, #326)

- issues: #325, #326
- commits: this change
- contracts-specs: Coder TUI first-open; local lane transcript upload
- invariants: local chats never reach openagents.com; hosted turns name /login instead of starting GitHub device authorization; missing Ollama is an install sign
- evidence: `unsigned_first_open_refuses_hosted_turns_and_names_ollama`; `a_local_session_does_not_upload_even_with_cloud_history`; `no_local_server_ends_the_turn`
- lane: grok local-first-325-326

Coder opens a session with no OpenAgents token. Hosted lanes refuse a prompt with `/login`. Local chats stay on disk even with `--cloud-history`. When Ollama is not running, the transcript says to install it.

## CLI source version is 0.2.0-rc1

- issues: none (owner request: prepare the local release)
- commits: this change
- contracts-specs: CLI crate version; Coder idle welcome chrome
- invariants: non-release builds still report `0.0.0-dev`; the idle card names the local lane
- evidence: `crates/openagents-cli/tests/coder_frame.rs` startup-facts frame test; `a_non_release_build_reports_a_development_version`
- lane: grok 0.2.0-rc1 bump

The `openagents-cli` crate is `0.2.0-rc1`. The idle card is "New in v0.2.0-rc1" and names Coder Local. `oa --version` on a non-release build is still `0.0.0-dev`; `ops/release-cli.sh --version 0.2.0-rc1` is what publishes that name.

## Coder-api accepts 10 MiB inference hops (#279)

- issues: #279
- commits: this change
- contracts-specs: coder-api inference body limit
- invariants: Phoenix hops with screenshot data URLs are not 413 at 2 MiB
- evidence: `an_image_sized_proxy_body_is_not_payload_too_large`
- lane: grok body-cap 10MiB

The rust coder-api hop now accepts JSON bodies up to 10 MiB so a Coder
turn with an attached screenshot is not refused as `coder_api_hop` 413.

## Coder idle card names swarm inbox and Flash routing

- issues: none (direct owner request)
- commits: this change
- contracts-specs: Coder idle welcome chrome
- invariants: the changelog card stays wrap-width and centered; ATIF `extra.swarm` is named on the idle card
- evidence: `crates/openagents-cli/tests/coder_frame.rs` startup-facts frame test
- lane: grok v0.1.1-stable

The idle "New in v0.1.1" card now also says ATIF export keeps the swarm inbox
and Flash routes simple requests to Gemini 3.7 Flash.

## Coder idle changelog card wraps its lines

- issues: none (direct owner request)
- commits: this change
- contracts-specs: Coder idle welcome chrome
- invariants: the changelog card is centered and only as wide as its longest line plus borders and one pad column on each side
- evidence: `crates/openagents-cli/tests/coder_frame.rs` startup-facts frame test
- lane: grok welcome-rc12

The idle "New in v0.1.1" card now says ATIF export keeps subagent streams,
sits centered under the startup facts box, and shrinks to wrap its lines
instead of matching the facts box width.

## Flash capability FAQs ride Gemini (#278)

- issues: #278
- commits: this change
- contracts-specs: Flash simple/thoughtful classifier
- invariants: coding work stays on GLM 5.3 Flash; short capability FAQs hop to Gemini 3.7 Flash
- evidence: `capability_faqs_are_simple`; `a_glm_grant_reroutes_a_tools_faq_to_gemini`
- lane: grok simple-faq routing

`what tools do u have` and other short capability questions on the default
Flash grant now route to Gemini 3.7 Flash instead of GLM thinking.

## Coder times each tool call (#277)

- issues: #277
- commits: this change
- contracts-specs: Coder tool-header timing
- invariants: in-flight tool headers count inline; settled headers put the duration on the right in the same words as the turn clock
- evidence: `a_tool_header_shows_a_live_count_then_a_settled_duration`
- lane: grok tool-call timers

Each tool box now shows how long that call has been running. The count sits
beside the title while the call is in flight, then moves to the right edge
when it settles, matching the turn duration on the answer line. A `delegate`
header names the agent (`delegate: grok`). The idle card names message
timing instead of signed-in credit.

## Coder idle frame shows what is new in 0.1.1

- issues: none (direct owner request)
- commits: this change
- contracts-specs: Coder idle welcome chrome
- invariants: the startup facts box keeps one column of inner padding; the changelog card is at most five lines
- evidence: `crates/openagents-cli/tests/coder_frame.rs` startup-facts frame test
- lane: grok welcome-pad-rc10

The idle Coder screen pads the startup facts box by one column and draws a
matching "New in v0.1.1" card under it.

## Gym views render frozen schemas in the terminal and TUI (#165)

- issues: #165
- commits: this change
- contracts-specs: `openagents.gym.*` v1; `docs/coderbench/gym-cli-spec.md` §4
- invariants: unknown optional measurements print as `unknown`, never a fabricated zero
- evidence: `crates/openagents-cli/src/gym/views.rs`; `tests/fixtures/gym/*.plain.txt`; `tests/gym_views_test.rs`
- lane: grok session, issue #165

`openagents gym` suite/run/results/env/corpus/dataset commands now print from
one renderer over the frozen v1 documents. The coder `/gym` pane draws
`results_trend` and `run_status` from that same renderer. Missing costs and
rates stay `unknown`. Web corpus/dataset tabs remain a Phoenix
`openagents.com` follow-on.

## Delegated Grok streams stay in ATIF (#276)

- issues: #276
- commits: this change
- contracts-specs: ATIF-v1.7 extra.subagent; ACP session/prompt idle wait
- invariants: a child that opened a session and streamed is recorded as having run; timeout after work names the session and tool count
- evidence: `crates/openagents-cli/tests/acp_test.rs` idle-reset tests; export coalescing test
- lane: grok ATIF/timeout fix

A delegated Grok child that streams for minutes is no longer killed at a
900-second wall clock and reported as never started. `session/prompt` now
idles out only after silence; inbound `session/update` traffic resets the
wait. The parent `delegate` result counts ACP tool calls, keeps the session
id on timeout, and says the child ran then failed. ATIF export stores the
child transcript in `extra.subagent` keyed by the parent call id instead of
one notice per thought token.

## Coder keeps sign-in links visible

- issues: none (direct owner request)
- commits: this change
- contracts-specs: Coder device authorization and OSC 8 terminal links
- invariants: link repainting preserves the text and style of the completed frame
- evidence: Coder frame and OSC 8 hyperlink regression tests
- lane: codex/coder-visible-sign-in-url

Coder now repaints clickable links from the frame that the terminal displays.
Previously, Ratatui swapped to a cleared buffer before Coder read it, so the
hyperlink pass replaced the sign-in URL with blank, default-styled cells. The
sign-in URL now remains visible in amber and supports terminal link actions.

## CLI credentials use one cross-platform file (#261)

- issues: #261
- commits: this change
- contracts-specs: `openagents_cli.credentials.predictable_file.v1`
- invariants: credentials stay scoped by API origin, never appear in output, and use private permissions on Unix hosts
- evidence: OpenAgents CLI authentication, Computer, and concurrent private-file test suites
- lane: codex/issue-261-credential-file

The OpenAgents CLI now stores account and Computer tokens in
`~/.openagents/credentials.json` on every platform. It no longer invokes
macOS Keychain or Linux Secret Service commands. On Unix hosts, the CLI
restricts `~/.openagents` to mode `0700` and the credential file to mode
`0600`.

The CLI moves an existing `~/.config/openagents/cli-credentials.json` file to
the new path during the first credential read. Concurrent writers use atomic
staged writes and verify that each stored value reached the file before they
report success.

## Headless agents can complete CLI device authorization

- issues: none (direct owner request)
- commits: this change
- contracts-specs: `openagents.repositories.v1` device authorization
- invariants: a headless agent surfaces the approval URL and user code without receiving the user's GitHub token
- evidence: `packages/openagents-cli/test/cli.test.ts`; live staging device authorization start
- lane: codex/qualify-repository-import

`openagents auth login` now works with agents whose shell tools buffer output.
In a headless or noninteractive process, it returns the complete approval URL,
user code, and resume command immediately. After the user approves the request
in any browser, `openagents auth login --resume` completes authorization and
stores the issued OpenAgents token. Interactive use still opens the browser
automatically and prints a manual instruction if the browser does not open.

The credential adapter verifies the stored value before it reports success.
Pending device requests use a private local file and disappear after success
or detected expiry.

Repository imports also report state transitions and attempt counts to
standard error while the CLI waits. Machine-readable JSON remains isolated on
standard output.

## Repository imports ship in CLI 0.1.0

- issues: none (direct owner request)
- commits: 6fe64d6737, a80ab5914818
- contracts-specs: `openagents.repositories.v1`; packed npm consumer contract
- invariants: an import copies one accepted ref snapshot; credentials stay out of Git arguments and logs; npm installs one compatible Effect runtime
- evidence: npm `@openagentsinc/cli@0.1.0`; `packages/openagents-cli/scripts/verify-packed-install.mjs`; `packages/openagents-cli/test/repository-client.test.ts`; server provisioner regression at a80ab5914818
- lane: codex/qualify-repository-import

You can install the OpenAgents CLI from npm and use `openagents repo import`
to copy a GitHub repository into OpenAgents. The command reports bounded
server failures and makes clear that a timed-out client does not cancel an
import that the server already accepted.

The package gate installs the packed tarball in a clean npm project. It checks
the complete dependency tree, executable, version, and repository import help.
The npm registry now serves `@openagentsinc/cli@0.1.0` under the `latest` tag,
and production runs the qualified server revision on all three fleet nodes.

## Mobile source prepares managed Sarah voice (#9273)

- issues: #9273
- commits: this change
- contracts-specs: `openagents.sarah.voice.v1`, `mobile_voice_only`, native PCM audio module
- invariants: protected mobile identity only, no provider key in the app, foreground capture only, no mobile tools, normal credit
- evidence: docs/mobile/2026-07-28-openagents-mobile-build-127-sarah-voice-preflight-receipt.md
- lane: codex/9273-sarah-mobile-voice

The mobile source adds a Sarah voice screen with transcripts and clear mute,
interrupt, end, and retry controls. The app stops capture outside the
foreground and keeps provider credentials off the device.

The managed service and physical-device gates are still open. Build 127 is not
uploaded to TestFlight.

## High-risk operator procedures complete their STE conversion (#9051)

- issues: #9051
- commits: this change
- contracts-specs: 15 current operator procedures, one superseded Worker deploy runbook
- invariants: human procedures use base STE, protected commands and requirement tokens remain stable
- evidence: docs/ste/p3-high-risk-procedure-conversion-receipt.md, docs/ste/control-semantic-baseline.v1.json
- lane: codex/asd-ste100-migration-20260719-r2

The current release, deployment, authentication, private workspace, and recovery controls now have checked human-facing STE profiles.
The obsolete Worker deploy runbook now points to the current Google Cloud authority.
The ledger keeps later-phase public, planning, evidence, and legacy documents in an explicit migration state.

## Active specifications complete their STE conversion (#9050)

- issues: #9050
- commits: this change
- contracts-specs: 16 active specifications, 12 specification authoring documents
- invariants: no ProductSpec intent change, exact subject digests remain bound, assurance revisions identify binding changes
- evidence: docs/ste/p2-specification-conversion-receipt.md, docs/ste/control-semantic-baseline.v1.json
- lane: codex/asd-ste100-migration-20260719-r2

All active specifications and their active authoring documents now have checked STE profiles.
The conversion keeps technical requirements, code, paths, URLs, identifiers, and protocol values.
Five AssuranceSpec documents now bind the converted ProductSpec bytes with new assurance revisions.

## Agent controls complete their STE review (#9049)

- issues: #9049
- commits: this change
- contracts-specs: 15 P1 control contracts, openagents-agent-compact-v1
- invariants: no requirement reduction, agent density exceptions cannot apply to human or dual-audience text
- evidence: docs/ste/p1-control-conversion-receipt.md, docs/ste/control-semantic-baseline.v1.json
- lane: codex/asd-ste100-migration-20260719-r2

All P1 control contracts now have checked STE profiles.
Agent-only controls can keep a dense sentence or paragraph after an identified review.
The exception helps agents parse one technical control context quickly.
It does not permit semicolons, contractions, unsafe terms, ambiguity, or a weaker requirement.

## Root control documents remove prose semicolons (#9049)

- issues: #9049
- commits: this change
- contracts-specs: AGENTS.md, INVARIANTS.md, docs/sol/MASTER_ROADMAP.md
- invariants: no requirement change, protected control tokens remain equal
- evidence: docs/ste/p1-control-conversion-receipt.md, scripts/check-ste.test.ts
- lane: codex/asd-ste100-migration-20260719-r2

Three central OpenAgents control documents now use approved punctuation instead of prose semicolons.
The change keeps code, URLs, technical words, and protected requirements without changes.

The documents remain in the STE migration state.
Sentence, paragraph, vocabulary, and inspection work is still necessary.

## Agent records have a controlled compact language profile (#9049)

- issues: #9049
- commits: this change
- contracts-specs: openagents-ste-policy-v2, openagents-agent-compact-v1, STE document profile schema
- invariants: human text uses base STE, agent extensions cannot relax authority, safety, evidence, or ambiguity controls
- evidence: docs/ste/agent-compact-profile.v1.md, docs/ste/agent-compact-terms.v1.json, scripts/check-ste.test.ts
- lane: codex/asd-ste100-migration-20260719-r2

OpenAgents now separates human technical text from compact agent records.
Human text continues to use the base STE profile.
Agent records can use controlled technical terms and labeled fields when these forms improve precision.

The checker limits the extension to an identified agent section or agent document.
The policy keeps all conditions, limits, proof states, and authority references.
The RC.25 dual changelog is the reference pattern, and its released bytes remain unchanged.

## Agents can review and safely apply exact code proposals (#9036)

- issues: #9036
- commits: 6883463cbe, fc29ee6df9, f8e423dc0d
- contracts-specs: openagents.desktop.ide-agent-code.v1; Desktop agent editing invariant; IDE roadmap and crosswalk
- invariants: agent context is inspectable and bounded; proposal apply is main-owned, generation-fenced, checkpointed, and independently evidenced
- evidence: apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-08-acceptance.json; apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-08-packaged-agent-code.json; docs/ide/2026-07-19-ide-08-agent-native-code-graph.md
- lane: codex.ide08.20260719.aL9yNF

Desktop agents can now attach to the exact project and worktree, show what
context they included or withheld, and present version-bound file changes for
review before anything touches disk. Owners can accept all or part of a
proposal, apply it through the same workspace authority as manual edits,
follow conversation-to-code links, inspect independent post-change evidence,
and undo to a retained checkpoint.

The implementation fails closed when files, generations, grants, or policies
move; keeps private roots and file bytes out of public receipts; and treats
tests, delivery, verification, and owner acceptance as separate observed facts
rather than trusting an agent's completion text. The packaged macOS journey
proved context disclosure, Pierre review, keyboard apply/undo, backlinks,
evidence separation, and exact disk restoration on the recorded candidate.

## Linux AppImage updates retain a safe rollback image (#8921)

- issues: #8921
- commits: 1658d36548
- contracts-specs: DesktopPlatformUpdateApplier; Linux AppImage retained-slot transaction
- invariants: signed ReleaseSet selection remains authoritative; DEB/RPM remain package-manager handoffs with no app-owned rollback claim
- evidence: apps/openagents-desktop/src/linux-update-applier.test.ts; native x64/arm64 receipt pending this RC
- lane: codex-open-issue-sweep-20260719-linux-distribution

Linux AppImage installs can now apply a verified full-image update without
elevation, restart through one stable selected-image path, retain the previous
image until a healthy launch, and roll back if that launch fails. Foreign or
malformed AppImages fail before the current selection changes. DEB and RPM
downloads continue to use the distribution package manager and do not claim
application-owned rollback.

## macOS updates recognize real RC bundles and preserve offline notarization (#8993)

- issues: #8993
- commits: this change
- contracts-specs: native macOS update applier; two-phase app and DMG notarization pipeline; packaged release acceptance
- invariants: channel-specific signed identity and offline app-ticket verification remain fail-closed
- evidence: docs/sol/receipts/2026-07-19-issue-8993-staging-update-rehearsal.md
- lane: codex-open-issue-sweep-20260719

The Desktop updater now recognizes the signed name and identity used by real
release-candidate apps instead of rejecting them as missing or mismatched.
Future macOS DMGs also capture an already-notarized, already-stapled app before
the image itself is notarized, preserving offline Gatekeeper protection rather
than weakening it to accept older malformed release bytes.

## Full Auto accepts installed non-default Codex models (#9003)

- issues: #9003
- commits: 7a01228b7d
- contracts-specs: installed Codex app-server catalog; durable Full Auto continuation profile
- invariants: provider identity and exact installed-catalog membership remain mandatory
- evidence: docs/sol/receipts/2026-07-19-issue-9003-codex-full-auto-model-admission.md
- lane: codex-open-issue-sweep-20260719

Full Auto no longer relies on the former two-model Codex allowlist. The live
installed Codex catalog now governs the composer, provider lane, and durable
continuations, with explicit regression coverage for GPT-5.6-Terra and
fail-closed coverage for models absent from that catalog.

## Sarah voice is attached to messages (#9013)

- issues: #9013
- commits: 2e4177fe64
- contracts-specs: Sarah Mobile Speech Delivery in INVARIANTS.md; Effect Native Card long-press projection
- invariants: no permanent speech bar; one active owner-private clip; only its exact message shows bounded state
- evidence: docs/mobile/2026-07-19-openagents-mobile-sarah-message-voice-ota-receipt.md
- lane: codex.root.sarah-message-voice

Sarah's permanent “Listen · AI-generated voice” bar is gone. Long-press any
completed Sarah response to prepare and play its AI-generated voice; the exact
message shows a compact preparing, playing, or failed state, and long-pressing
the active message stops it. Sarah's composer also restores native autocorrect.

## Desktop restart failures no longer repeat (#9012)

- issues: #9012
- commits: aaccf71781
- contracts-specs: Desktop Development Restart Authority in INVARIANTS.md
- invariants: restart coordination is one-shot and failure notices are claimed once per request
- evidence: apps/openagents-desktop/tests/oa-dev-supervisor.test.ts; apps/openagents-desktop/tests/electron-boundary.test.ts
- lane: codex.root.restart-notification-loop

A failed Desktop development restart now produces at most one notification and
stays stopped until a new restart is explicitly requested. The running app is
preserved when a handoff fails, so a port conflict cannot become a repeating
macOS notification loop.

## Sarah shows verified tool activity in chat

- issues: none (direct owner request)
- commits: c2ff92159c
- contracts-specs: openagents_mobile.sarah.live_tool_activity.v1; INVARIANTS.md Sarah tool-visibility invariant
- invariants: Sarah tool use and success/failure may no longer be hidden by conversational presentation
- evidence: docs/mobile/2026-07-19-openagents-mobile-sarah-live-tool-activity-ota-receipt.md
- lane: codex-owner-session-sarah-tool-visibility

Sarah now shows a short live activity line when she uses a real tool, and the
line updates when its confirmed result arrives. Internal tool names, IDs,
arguments, raw results, provider plumbing, and token dumps stay out of the
conversation.
