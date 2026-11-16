# coax drive and the app's cases

This directory lets an agent drive the GPU app the way a person does, and play the app's cases headlessly
on the build host (GitHub #13645). It has four parts:

| File | What it owns |
| --- | --- |
| `fakes.coax` | Held answers and a controlled clock for every service the app core defines |
| `drive.coax` | The driver: `tree`, `tap`, `type`, `state`, `clock`, `logs` and `life` on the committed semantic table |
| `core-page.coax` | The adapter for the shared app: core, chat page and login card, with the build host as the native host |
| `chat-page.coax` | The adapter for today's iOS chat page (`apps/openagents-ios/chat.coax`) |
| `run.coax` | The case runner; cases live in `cases/` |

## Run the cases

Run from the repository root, on the build host:

```sh
coax/remote/remote.sh run -- coax/web/tool.sh apps/shared/app/cases/run.coax cpu            # compare every case
coax/remote/remote.sh run -- coax/web/tool.sh apps/shared/app/cases/run.coax cpu --write login   # rewrite one trace
```

The checks for each part:

```sh
coax/remote/remote.sh run -- coax/bin/coax test apps/shared/app/cases/fakes.coax
coax/remote/remote.sh run -- coax/bin/coax test apps/shared/app/cases/drive.coax
coax/remote/remote.sh run -- coax/bin/coax test apps/shared/app/cases/chat-page.coax
coax/remote/remote.sh run -- coax/bin/coax test apps/shared/app/cases/core-page.coax
```

## Write a case

A case is `cases/NAME.ops`: one operation a line, with `#` lines as notes. The runner writes
`cases/NAME.trace` (`> OP`, then the answer) and later compares it byte for byte. Add the name to
`CR_CASES` in `run.coax`.

- Put the recordings first. The app launches at the first line that is not a recording.
- `# app: chat-page` plays the case on today's iOS chat page instead of the shared app.
- A first line `# needs: WHAT` keeps the case from playing. The run counts it as waiting, never as passed.

A recording declares one answer:

```
clock data record NAME: KIND KEY => STATUS BODY
```

- `KIND` is `http` (`KEY` is `METHOD PATH`) or `auth` (`KEY` is `state`, `email_start`, `email_verify`,
  `email_cancel` or `logout`).
- The only origin is `https://fake.openagents.test`. A path that leaves it is refused.
- Two recordings with one `NAME` are that request's first and second answers.
- When two recordings have one `KEY`, the later one answers the next request. Record a new answer when
  what the service would say has changed (`stop.ops` records the turn again after Stop).
- `STATUS part` is one part of a streamed answer. The request stays held.
- Identities, clipboard and link need no recording: the fake answers them on the next frame.

A request is held until the case says `clock data release NAME`. `clock data` lists what is held. One request of the conversation is in flight at a time, so a held
events read holds back the task's reads and the permission's answer until the case releases it.
`clock +MS`, `clock settle` and `clock until ID` move the clock; a frame gets no time the clock did not
grant. `life background`, `life foreground` and `life relaunch` deliver lifecycle facts. A relaunch ends
everything in flight, keeps what the host keeps for the rejoin, and launches again.

Answers that are not recorded show up in `logs` as
`fake refused http GET PATH: no recording names it`. Use that line to learn the next request.

`state` lists the values a case may read. On the shared app they are `account.known`,
`account.logged_in`, `login.step`, `chat.draft`, `keyboard.shown`, `send.state`, `stop.state`,
`permission.state`, `task.state`, `core.busy` and `core.refusals`. A value or log line that reads as a
credential is withheld. Typed object IDs (`cnv_`, `trn_`, `prj_`, `run_`, `ctx_` and 32 lowercase hex)
are not withheld.

## Drive another app

An adapter loops on the driver. `gd_drive` starts an operation, `gd_resume` continues it, and each
returns what the adapter must do next:

| Code | The adapter |
| --- | --- |
| `GD_FRAME` | Runs one frame with `gd_frame_dt(d, 0)` and reports motion with `gd_owner` |
| `GD_POINTER` | Delivers `gd_px`, `gd_py`, `gd_phase` through its real pointer path, then `gd_took` |
| `GD_EDITOR` | Hands the whole text from `gd_edit` to its editor entry, then `gd_edited` |
| `GD_STATE` | Says its allowlisted values with `gd_state`, `gd_state_b`, `gd_state_i` |
| `GD_COMPLETE` | Gives `gd_done(d)` to the service that asked, in a later turn |
| `GD_LIFE` | Delivers `gd_life(d)` |
| `GD_DONE` | Stops; the answer is `gd_answer(d)` |

Services go through `gd_take(d, kind, key, op, generation, timeout_ms)`; cancels go through
`gd_cancel`. `core-page.coax` is the full example for the app core's outbox.

```coax
core: [1] AppCore; k: [1] CoTest; a := co_t_app(k[..], core[..]); r := k[0].r[..];
co_open(r, a);
_ := co_drive(r, a, "clock data record auth.state: auth state => 200 {\"loggedIn\":true}");
_ = co_drive(r, a, "tap chat.composer");
print("%\n", gd_answer(a.d));    // tapped chat.composer
```

Keep two rules, or the checker stops with E0381 ("storage or nesting budget exceeded"):

- A procedure that draws the page never returns a string view. The adapters return a bool, and the caller
  reads `gd_answer`.
- Pass the large values (page, vertices, core, driver) as separate slices, and read the surface again at
  each step instead of keeping a view across a call that writes it.

## In the iOS app

`run.coax ios-simulator DEVICE` launches the app with `-OADrive SOCKET` and plays the case with
`coax/web/tool.sh drive native SOCKET --ops FILE`. The app's half, which the iOS host owns:

1. On `-OADrive`, start the driver socket (`phone_drive_start` in `apps/shared/host/apple/phone-drive.h`).
2. While a driver holds the clock, give each frame `cp_frame_dt` (or `gd_frame_dt`) instead of the
   display link's time.
3. After a drive operation types into a field, sync the platform editor's text from the page.

The iOS app does all three in debug builds (`oc_drive_start` in `apps/openagents-ios/host.c`, started
from `App.swift`): each operation runs through `cp_drive` on the real chat page (the shared view on the
app core), and after it the host takes its own turn, so the keyboard a tap asked for is up before the
next operation. `composer` plays the same on the simulator up to its `clock settle`.

What still differs: the C driver (`apps/shared/host/drive.c`) answers `clock +MS` and `clock settle`
itself, stepping frames through `oc_drive_step`, so those lines read as the C driver writes them, not
as the trace does. `cpu` is the host that plays every case to its end today.

## The task-host fixture

`permission.ops` plays a task on a computer: the turn's task binding, a status that waits for a permission,
the whole transcript with a tool call, its result and the question, then Allow, the answer's reply and the
task running on. `task-fixture.coax` writes those bodies with the shared task writers (the native binding
fixture `ntp_test_record`, `xt_status_write`, `xh_page_write`), so the case carries real wire bytes:

```sh
coax/web/tool.sh apps/shared/app/cases/task-fixture.coax READER READ COMMAND
```

READER, READ and COMMAND are the case's fake identities in the order the app asks for them (`fk_hex32`
with the agent seed): the submit takes the first two, the transcript reader the next two (READ is the
command ID, READER the session ID), the Allow the fifth. The transcript's path and the reply's command
name them, so a case that asks for identities in another order needs the fixture written again.
