# What a chapter must show

This is the contract for The OpenAgents Book. It says what each status
means, what evidence a chapter needs before it moves forward, and how the
book's checker enforces it. Run the checker with:

```shell
python3 psionic/tests/book.py --book openagents
```

## Three separate facts

The registry ([chapters.json](chapters.json)) keeps three facts about each
chapter apart, in the shared format of
[coax/book/check-structure.py](../../coax/book/check-structure.py):

| Field | Meaning | What it does not mean |
| --- | --- | --- |
| `source` | Coax files that exist and that the chapter shows or runs | That an app uses them |
| `qualified_targets` | Targets where the chapter's checks ran, with recorded evidence | That other targets work |
| `adopted_by` | Files outside the book, such as `apps/openagents-web/chat-web.coax`, that use the chapter's code today | That the app is deployed or that a store has approved it |

A library that exists does not make a chapter implemented. A simulator or
a fake service passing does not qualify a device target.

## Statuses

| Status | Requires |
| --- | --- |
| planned | An outline. Line 3 starts with `> **Status: planned.**`. No source, target or adoption. |
| design draft | The full draft. Line 3 starts with `> **Status: unimplemented — design draft.**`. No source, target or adoption. |
| partly implemented | At least one of the chapter's checks runs on at least one qualified target. The profile names what runs; the chapter says what remains. Line 3 starts with `> **Status: partly implemented.**`. |
| implemented | All four parts of the chapter's check run on every target the chapter names. |

Owner approval is not a status. When the owner edits a design draft, the
chapter's status block says so with the date, and the status stays design
draft until code runs.

## The chapter's check

A chapter that teaches code has four parts. All four run before the
chapter is implemented:

1. **Runnable Coax source,** listed in [examples.json](examples.json) with
   the chapter it belongs to, or shown as a source listing (an HTML comment
   naming the file and its `book:begin` tag) that the checker compares with
   the file.
2. **An exact expected trace.** The program's output, or the `coax drive`
   operations and their answers, byte for byte, in a committed file.
3. **A refusal or broken case,** with the refusal it produces.
4. **An independent check:** another implementation, a published vector, an
   existing test suite or a hand-derived value, never the code under test.

Checks follow three rules:

- **Common operations.** App cases use the shared `coax drive` operations
  (`tree`, `tap`, `type`, `state`, `layout`, `clock`, `logs`, `screenshot`)
  on every host. No host has a private test language.
- **No live services in the trace.** Fake services replay recorded answers,
  and the clock is controlled. A live-service check is a separate record
  that names its real nondeterminism.
- **Meaning before pictures.** Assert on state and the semantic tree first.
  A screenshot comes last, and only for a drawing fault the tree can't show.

## Targets

| Target | Evidence it needs |
| --- | --- |
| `cpu` | The chapter's program runs on the build host, in debug and release |
| `wasm` | The program runs as Wasm |
| `web` | The case passes in headless Chrome through `coax drive`; keyboard and accessibility checks name the browser they ran in |
| `ios-simulator` | The case passes in the iPhone simulator through the shared driver |
| `ios-device` | The case passes on a physical iPhone, with keyboard input methods, VoiceOver, backgrounding and the platform SDK it uses checked on the device |
| `android-emulator` | The case passes in the Android emulator through the shared driver |
| `android-device` | The case passes on a physical Android phone, with the same device checks as `ios-device` |
| `macos-arm64` | The case passes in the macOS app on Apple silicon |

A chapter names a target only with its evidence recorded. When a device
check needs the owner's phone, key or account, the step goes in the
workspace's `NEEDS_OWNER.md`, and the chapter names the target only after
the run is recorded.

## What the checker enforces

`python3 psionic/tests/book.py --book openagents` checks:

- The registry's fields, statuses and targets, the status line of every
  chapter, the SUMMARY and README labels, and that dependencies form a
  graph without cycles across all books.
- That every chapter depends only on earlier chapters, so the book is
  cumulative.
- That [examples.json](examples.json) lists every program in
  `openagents/examples/`, and that every listing matches its source.
- That every local link resolves.
- That [provenance.json](provenance.json) has one row per chapter, that its
  part matches the chapter's place in [SUMMARY.md](SUMMARY.md), that its
  promises match the chapter's "Promises covered" line, and that every
  promise P1 to P16 is covered.
- That a planned path does not exist yet and does not duplicate a current
  owner: engines, sessions and the journal are in `coax/lib/agent/`, money
  rules are in `pay/`, the wallet is the iPhone app's Spark wallet, the API
  is `coax/web/api/`, and clients are the apps in `apps/`.
- That a partly implemented or implemented chapter has a "The chapter's
  check" section naming its source, trace, refusal and independent check.
  Chapters 1 to 11 and the chapters added in the reorder already plan one;
  the others add theirs when they are next edited.

`--coax COMPILER` also compiles and runs every example and compares its
output with the committed trace.
