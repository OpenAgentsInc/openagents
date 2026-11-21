# 6. One case, every step

> **Status: partly implemented.** The end-to-end opening acceptance runner
> ([`openagents/examples/opening-case-runner.coax`](../examples/opening-case-runner.coax))
> and the committed CPU case runner
> (`coax/web/tool.sh apps/shared/cases.coax cpu`) pass on `cpu` across all five
> committed cases (`login`, `login-cancel`, `first-milestone`, `logout-pending`,
> `reconnect`), the GPU `ComposerScreen` spine, and the `LoginFlow` gate.
> Remaining for native targets (`ios-simulator`, `ios-device`, `macos-arm64`):
> running the GPU-kit chat page case on the iOS Simulator and physical iPhone
> with XCUITest. (The earlier 2026-11-13 UIKit milestone is kept below as history.)

## Premise

Chapters 1 to 5 each added one thing to the chat page: a text input, Send,
a transcript, a reply and the login card. This chapter joins their cases
into one acceptance case, written once as `coax drive` operations, and runs
it against the same build in the iOS Simulator and on a physical iPhone.
When it passes on both, the chat page is the book's first milestone.

## Draft

An acceptance case is a list of operations with expected answers. The
operations are the common `coax drive` vocabulary: `tree`, `tap ID`,
`type ID TEXT`, `state`, `layout`, `clock`, `logs` and `screenshot`
([Coax Native, "The agent interface and verification"](../../coax/native.md#the-agent-interface-and-verification)).
`tap` and `type` find a live row by ID in the committed semantic table and
go through the real input path. They never write state directly.

The chat page's case types a message with no session, logs in through the
card, receives the reply a part at a time from the fake service, then sends
again and stops:

```text
type composer.input hello
tap composer.send
type login.email you@example.com
tap login.send
clock data release auth.email_start
type login.code 123456
tap login.verify
clock data release auth.email_verify
clock data release chat.send
clock data release chat.part
clock data release chat.done
state transcript
type composer.input again
tap composer.send
clock data release chat.part
tap composer.stop
```

Assertions run on state and the semantic tree first. A picture comes last,
and only to catch a drawing fault the tree can't show. On the GPU kit that
matters more than before: the tree is the same rows the screen draws from,
so a passing tree is strong evidence, and a picture checks only what paint
adds.

A host that can't do an operation says so. A driver that implements only
`tree` and `tap` refuses `type` and does not claim the whole interface.
Before a run, the driver checks that the app it attached to was built from
the source under test; a stale build refuses with the command that rebuilds
it. The web driver does this today; the iOS driver must do it before this
chapter is implemented.

The simulator is evidence for the simulator. Keyboard input methods,
VoiceOver, one-time-code AutoFill, backgrounding and the frame-rate
objective while typing need a run on a physical iPhone before the chapter
names `ios-device` ([CONTRACT.md](CONTRACT.md#targets)). The GPU login
screen has not had that run yet. When a device check needs the owner's
phone, it goes in the workspace's `NEEDS_OWNER.md`.

### Who may drive a phone

This was decided on 2026-11-13 and still holds. A simulator's app shares
the Mac's file system, so the driver reaches it on a Unix socket. A
physical iPhone needs a network transport. The tunnel that `devicectl`
keeps to a paired phone is not one macOS exposes as a network interface, so
the driver uses TCP on the phone's own network, and the app admits a driver
only when all of these hold
([`apple/phone-drive.c`](../../apps/shared/host/apple/phone-drive.c)):

- **A development launch.** The app listens only when it is launched with
  `-OADrive tcp:PORT -OADriveToken TOKEN`. A launch argument is not saved,
  so the next normal launch has no listener. Without a token of 32 to 128
  characters it doesn't listen.
- **The local network.** The driver's address must be on the subnet of one
  of the phone's own interfaces that is up (Wi-Fi, or the USB link to the
  Mac), never cellular or loopback.
- **The launch's token.** The driver's first line, within five seconds, is
  `attach TOKEN`, compared in constant time. The runner makes a new token
  for each case.
- **One driver at a time.** A second connection waits until the first
  closes.

```shell
xcrun devicectl device process launch --device DEVICE --terminate-existing \
  com.openagents.app -- -OADrive tcp:47613 -OADriveToken TOKEN
coax/web/tool.sh drive native tcp:PHONE.local:47613 --token TOKEN --ops openagents/examples/cases/chat-page.ops
```

### The first milestone, on the old app

On 2026-11-13 the book recorded its first milestone
([#13516](https://github.com/OpenAgentsInc/openagents/issues/13516),
[#13517](https://github.com/OpenAgentsInc/openagents/issues/13517)): login,
one message, a fixture reply and Stop, from one shared Coax module
(`apps/shared/openagents-app.coax`), with byte-equal traces on the build
host, in headless Chrome, in the iPhone simulator and on an iPhone 17 Pro
Max. The iPhone drew that module with UIKit controls
(`SharedAppViewController.swift`) inside the old app. The device checks ran
then: the number pad on the code field, a Japanese input method sending
`日本語` once, non-Latin text, labels and the accessibility audit, and a
reply finishing once across backgrounding.

That run is history, not evidence for the new app. Its presentation was
UIKit, not the GPU kit, and the app it ran in is being replaced. What
carries over is the shape: the operation files and traces in
[`openagents/examples/cases`](../examples/cases) show what this chapter's
case must assert, and the device checks above are the list this chapter
runs again on the phone.

## What we build

A plan for what this chapter ships once the draft is approved:

- The chat page's acceptance case as one operation file and its trace.
- The lean app's driver: the common operations over the committed rows, a
  refusal for an operation it lacks, and the stale-build check.
- A run of the case in the iOS Simulator and on a physical iPhone, with the
  device checks recorded per target.

It builds on what exists today:

- [`apps/shared/host/drive.c`](../../apps/shared/host/drive.c)
- [`apps/shared/host/drive.h`](../../apps/shared/host/drive.h)
- [`apps/shared/host/apple/phone-drive.c`](../../apps/shared/host/apple/phone-drive.c)
- [`coax/web/drive.coax`](../../coax/web/drive.coax)
- [`apps/openagents-ios`](../../apps/openagents-ios)

### The chapter's check

- **Source:** [`openagents/examples/opening-case-runner.coax`](../examples/opening-case-runner.coax),
  [`apps/shared/cases.coax`](../../apps/shared/cases.coax),
  [`apps/shared/openagents-app.coax`](../../apps/shared/openagents-app.coax),
  [`apps/shared/chat/screen.coax`](../../apps/shared/chat/screen.coax),
  [`apps/shared/app/login/flow.coax`](../../apps/shared/app/login/flow.coax),
  and the committed operation/trace pairs under
  [`openagents/examples/cases`](../examples/cases).
- **Trace:** committed debug/release stdout in [`openagents/examples/opening-case-runner.out`](../examples/opening-case-runner.out),
  plus `coax/web/tool.sh apps/shared/cases.coax cpu` verifying byte-for-byte trace equality
  for `login` (14 ops), `login-cancel` (8 ops), `first-milestone` (33 ops),
  `logout-pending` (19 ops), and `reconnect` (16 ops).
- **Refusal:** parts released after `tap composer.stop` or `tap shell.logout`
  each answer `late part dropped`; `tap composer.nope` answers
  `no element composer.nope on this screen (tree lists them)`; an unsupported
  operation (`layout`) answers `unknown operation 'layout'`; `state account.token`
  is refused by the inspection allowlist; and a duplicate GPU row ID is refused
  atomically (`dup_refused=1`).
- **Independent check:** `coax/web/tool.sh apps/shared/cases.coax cpu` replays the
  committed `.ops` files from disk against their `.trace` files independently of the
  standalone book runner; native XCUITest runs on simulator and physical phone
  remain required before adding `ios-simulator` or `ios-device` to `qualified_targets`.

## Source listing & runnable example

Run [`openagents/examples/opening-case-runner.coax`](../examples/opening-case-runner.coax) (committed transcript [`openagents/examples/opening-case-runner.out`](../examples/opening-case-runner.out)):

<!-- source: openagents/examples/opening-case-runner.coax opening-case-replay -->
```jai
ocr_run_suite :: (a: [] OapApp, tr: [] OapTrace) -> bool {
  // 1. First milestone (33 ops): login, empty send refusal, streamed reply, copy, Stop fence.
  a[0] = OapApp.{}; tr[0] = OapTrace.{};
  oap_play(a, OCR_FIRST_MILESTONE, tr);
  t1 := cast(string) tr[0].t[0..<tr[0].n];
  ok1 := oap_has(t1, "composer.send can't be pressed now") &&
         oap_has(t1, "you: hello\nreply: Hello! This is a fixture reply.") &&
         oap_has(t1, "released chat.part: late part dropped") &&
         oap_has(t1, "notice: Stopped");
  print("case=first-milestone ops=% ok=%\n", ocr_count_ops(OCR_FIRST_MILESTONE), ok1);

  // 2. Logout with turn pending: late parts dropped and transcript cleared.
  a[0] = OapApp.{}; tr[0] = OapTrace.{};
  oap_play(a, OCR_LOGOUT_PENDING, tr);
  t2 := cast(string) tr[0].t[0..<tr[0].n];
  ok2 := oap_has(t2, "released chat.part: late part dropped") && oap_has(t2, "> state transcript\n(empty)\n");
  print("case=logout-pending ops=% ok=%\n", ocr_count_ops(OCR_LOGOUT_PENDING), ok2);

  // 3. Reconnect with lost answer: same operation ID stores once.
  a[0] = OapApp.{}; tr[0] = OapTrace.{};
  oap_play(a, OCR_RECONNECT, tr);
  t3 := cast(string) tr[0].t[0..<tr[0].n];
  ok3 := oap_has(t3, "lost chat.send: sent again as op-1") &&
         oap_has(t3, "released chat.send: op-1 already stored; first result") &&
         oap_has(t3, "> state service.messages\n1\n");
  print("case=reconnect ops=% ok=%\n", ocr_count_ops(OCR_RECONNECT), ok3);
  return ok1 && ok2 && ok3;
}
```

```shell
coax/bin/coax run openagents/examples/opening-case-runner.coax
coax/web/tool.sh apps/shared/cases.coax cpu
```

Expected output of `opening-case-runner.coax`:

```text
case=first-milestone ops=33 ok=true
case=logout-pending ops=17 ok=true
case=reconnect ops=14 ok=true
gpu_spine messages=2 pinned=true dup_refused=1
login_gate asked=true card_open=true
refuse_control=no element composer.nope on this screen (tree lists them)
refuse_op=unknown operation 'layout'; operations: tree, tap ID, type ID TEXT, state [NAME], clock +MS, clock data [release|lose NAME], logs, input
refuse_secret=account.token is not inspectable; state takes route, account.step, account.words, account.who, account.waiting, composer.text, turn, transcript, service.messages, clipboard, clock
all_passed=true
```

Planned paths: `openagents/examples/cases/chat-page.ops`. The CPU runner in
[`openagents/examples/opening-case-runner.coax`](../examples/opening-case-runner.coax)
and the committed cases in [`openagents/examples/cases`](../examples/cases)
run today on `cpu`.

## Sources

- **Episodes:**
  - [Episode 281, *Coder Mobile*](../../docs/transcripts/281.md): one chat on several devices
- **Also:**
  - [Coax Native](../../coax/native.md)
  - [The iOS app's tests](../../apps/openagents-ios/README.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P1, P16

## Decided

1. The driver reaches a physical iPhone over TCP on the phone's own
   network, and only a driver on that network with the launch's token may
   attach ("Who may drive a phone", 2026-11-13).
