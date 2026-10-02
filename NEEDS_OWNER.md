# Owner checks

## Desktop composer (#10004)

In a Mac build from current `main`, select the Japanese input source using the
system input menu. Start a new chat, compose a Japanese word, change a candidate,
and commit it. Confirm that Enter confirms the candidate without sending the
message; a later Enter sends it. Confirm that clicking away cancels marked text
without replacing the previously committed draft.

Scripted Japanese preedit, commit, Enter suppression, and focus-loss checks pass.
This remaining check exercises macOS's actual input method and input menu.

## Linux transcript performance (#10005)

On Linux with a native Wayland or X11 session, run the matrix in
`docs/desktop/verification/2026-09-30-transcript/verification.md` and retain the
result directory. Confirm transcript scrolling, tool expansion, and code copy.
The eight Mac cases pass; Linux hardware is unavailable in this worktree.
Open a follow-up issue if the Linux run finds a defect or exceeds 8.3 ms.

## Desktop image input (#10011)

On Linux in a native Wayland or X11 session, attach a generated PNG or JPEG
through **Attach image**, clipboard paste, and file drop. Confirm that each
preview appears, **Remove** preserves the caption, and **Send** preserves the
draft while explaining the current hosted text-only limit. A missing portal or
clipboard facility must report a reason. Repeat image clipboard paste on macOS.
The real macOS picker and scripted decoder, clipboard pixel, drop, removal,
and refusal checks pass. Open a follow-up issue if a native adapter check fails.

## Installed local Coder broker, handoff, and task chat (#10014–#10016)

After installing the desktop bundle and its Coder built from current `main`,
exercise the **Run Coder** flow added by #10015 against this computer. Confirm
that the normal OS key source remains in the resident host, saved device
pairings remain available after restart, and the task can be read and stopped.
With the configured engine signed in, verify live tool steps, Stop, the
phase-appropriate steering choice, queued follow-ups, and answers to a question
and an approval in the same task on desktop and phone. The scratch host checks
cover command admission, durable queue editing, exact retries, steering, Stop,
and archive. Shared state tests cover question and approval answers, stale
responses, split ATIF records, and draft acknowledgment. Native painter checks
cover all three task modes at normal and minimum window sizes.
Archive every verification task afterward. The scratch same-user socket,
portable client, durable inbox, restart, history, admission, and cancellation
checks pass without accessing the owner’s computer state.

## Installed saved sessions (#10017)

After installing a desktop and resident Coder built from current `main`, open
**Saved sessions** and inspect a known Codex session and Claude Code session.
Confirm their titles and times against the original tools. Choose a configured
project and continue one with the engine signed in and the existing auto-start
policy enabled. Confirm that its recent context reaches Coder, tool steps
appear, and the original saved session remains unchanged. Stop and archive
every verification task afterward. Scratch tests cover both source formats,
the native reader, real broker admission, exact prompt bytes, acknowledgment
retries, restart identity, cancellation, and archive without owner state.

## Desktop playable Grid (#10038)

Install a desktop bundle built from current `main` on macOS and on Linux
Wayland/X11. Choose **The Grid → Play**. Verify right-drag look, left-drag
orbit, simultaneous WASD, wheel entry and exit from first person, Escape,
Tab, Alt-Tab/Cmd-Tab, minimization, display-scale changes, and returning to
Watch or chat. A failed cursor grab must leave the pointer usable and explain
the keyboard fallback. Confirm the original chat draft survives.

Verify the separate world identity and Gym connection in the macOS login
Keychain or Linux Secret Service. Relaunch Play and confirm the public key
stays the same; deny a key read and confirm Play stays offline without
replacing it. Pairing and the resident host must retain their existing keys.
With a grant created for that world public key, inspect Gym runs, review a
recipe's budget, and explicitly confirm a permitted launch. Walking and
public boards must never launch it. Check desktop/phone movement together
against the chosen relay without creating test chats or tasks.

The isolated relay, shared controller, native input and layout, GPU captures,
and Linux binary checks are recorded in
`docs/desktop/verification/2026-09-30-playable-grid/verification.md`.
Native OS cursor grabs, protected-store prompts, and the signed Mac bundle
need device checks. Windows Verse remains tracked by #10027. Open a new issue
if any installed-device check finds a defect; these checks do not keep #10038 open.

## Cloud artifact signing (#10227)

Configure a private GCS artifact bucket with lifecycle retention and an identity
with object-create and URL-signing permissions. Run a non-sensitive Boat or GCE
command using the artifact publisher described in `scripts/cloud/README.md`.
Verify that all four signed links open and expire after 24 hours. Mocked tests
cover upload and comment behavior; a live signing check has not run.

## GCE retirement decision (#10223)

Review `docs/cloud/2026-10-02-orphan-retirement-review.md` and confirm a current
owner and retain-or-stop decision for each of the nine running candidates.
No machines were stopped or deleted. The issue explicitly requires this decision;
its operational work is not complete.

## Raw versus OpenAgents study (#10162)

The standing study still needs implementation and new measured trials for raw
Claude Code, raw Codex, the shipped default path, and a matched arm, including
repository tasks and Terminal-Bench tasks. Existing matched Opus evidence is
not evidence for the current shipped default path. This delegated run cannot
launch another coding engine as a subprocess, so it did not execute raw-engine
trials. Do not report a savings percentage or close the issue based on the
historical pilot alone.
