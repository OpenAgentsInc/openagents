# 97. A conformance report

Importing all NIP text does not mean supporting all NIPs. This chapter builds
a teaching report from copied chapter-registry rows and the records behind
each claim. It refuses to build that report when a claim has no record.
The example pins its rows to `73966b6359`; it does not read the current
`chapters.json`. Its transcript therefore retains that snapshot's draft and
partial statuses and empty adoption lists. Use the current registry and
[completion route](COMPLETION.md) for today's status and remaining exits.

Part XI, Interop and verification. Result: A machine-readable support matrix
per profile (`core-read`, `core-write`, `phone-signer`, `openagents-host`,
`block-identity`, `block-channel`, `agent-private`, `key-backup`) that refuses
to advertise a NIP whose runtime gate is missing. The runnable report models
teaching-profile evidence; it does not certify full implementation of each
cited NIP or authorize a production consumer.

## What this chapter teaches

- What each column of the chapter registry claims: `status`, `profile`,
  `qualified_targets` and `adopted_by`, and which record backs each claim.
- How one NIP's row is computed: the chapters that cite it, and how many of
  them are done on each target.
- Profiles and their minimum scope, and when a profile conforms on a target.
- The rule: a NIP is reported only when an implemented chapter with a record
  cites it, and a claim without a record refuses the whole report.
- Source pins: the official and Block lanes by commit and file hash, the
  OpenAgents lane by a digest over its files, and what the chapter's dated
  pin-check run found.
- How the book's report and the product's support matrix, `profiles.json`,
  differ.

## Where the code is

| File | What it does |
| --- | --- |
| [nostr/examples/conformance.coax](../examples/conformance.coax) | The report for the four official profiles, from 22 registry rows and 25 records, and four broken registries it refuses |
| [nostr/book/chapters.json](chapters.json) | The chapter registry: status, profile, source, qualified targets and adoption for each chapter |
| [nostr/book/PLAN.md](PLAN.md) | Each chapter's spec column, the NIPs it cites |
| [nostr/book/examples.json](examples.json) | The book's examples, each run in debug and release by `book.py` |
| [coax/lib/nostr/profiles.coax](../../coax/lib/nostr/profiles.coax) | The profiles, their NIPs, required capabilities, evidence rows, source pins and consumer budgets; `#authority` |
| [coax/tests/nostr/profile_matrix.coax](../../coax/tests/nostr/profile_matrix.coax) | The Coax program that prints `profiles.json` from `profiles.coax` |
| [coax/lib/nostr/profiles.json](../../coax/lib/nostr/profiles.json) | The generated support matrix |
| [coax/tests/nostr/profiles.py](../../coax/tests/nostr/profiles.py) | Test tooling: regenerates the matrix and compares it, checks pins and evidence, and seeds faults |

`profile_matrix.coax` is the generator, and it is Coax. `profiles.py` is
Python test tooling: it compiles the generator, runs it and checks the
result. Nothing ships it. The book's example is a pure Coax program; Coax
has no compile-time file reads, so it carries the registry rows it reports
on, copied from `chapters.json` and `PLAN.md` at `73966b6359`.

## What the registry claims

`chapters.json` has one record per chapter. Four of its fields are claims
that a report reads:

| Field | What it claims | What backs it |
| --- | --- | --- |
| `status` | `implemented`, `partly implemented`, `design draft` or `planned` | The four parts of the promotion standard in the [README](README.md#promoting-a-draft-to-a-teaching-chapter); only `implemented` counts as done |
| `qualified_targets` | A recorded run exists on each named target (`cpu`, `wasm`, and for other books `cuda` or `metal`) | A record: for `cpu`, the chapter's example in `examples.json`, which `book.py` runs in debug and release and compares with its transcript; for `wasm`, a record in `docs/verification/` of a Wasm build that ran the code |
| `profile` | The same targets as a label: `native C` or `native C and wasm` | The README table and `book.py`, which fail when the label and the targets disagree |
| `adopted_by` | Reviewed direct consumers of a listed source component outside the book | The actual import or host call; this does not establish use of every feature in the chapter |

`book.py` already refuses some combinations: a design draft with a source, a
qualified target or an adoption, and an implemented chapter with no
qualified target. It does not check that a qualified target has a record.
That is the report's job.

The current registry now names direct consumers, including the API bridge,
homepage reader, CLI and Pylon runtime. The example below preserves the empty
lists of its pinned snapshot. A shared JSON, event or wallet component can be
adopted while the enclosing NIP or integration remains incomplete. Do not use
that component edge alone to advertise a complete consumer profile.

The NIPs a chapter covers come from the spec column of `PLAN.md`. Chapter
49's row cites [NIP-98](../nips/official/98.md); chapter 23's cites NIP-01
and NIP-12.

## Profiles

A profile is a named, minimum scope that a consumer can ask for.
`profiles.coax` defines eight, with the NIPs each one needs and the lane
they come from:

| Profile | NIPs | Minimum scope |
| --- | --- | --- |
| `core-read` | official 01, 19 | NIP-01 events and filters, BIP-340 verification, NIP-19, basic relay flow |
| `core-write` | official 01, 42, 44, 59 | Signing, NIP-42, NIP-44 and NIP-59 |
| `phone-signer` | official 46, 98 | Keychain custody, NIP-46 and NIP-98 sign-in |
| `openagents-host` | OpenAgents HOST, REACH, WORK, COORD, CAP, POL, CTX, RUN | Host grants and reachability |
| `block-identity` | Block OA, AA, FI, FA | Identity, issuers and revocation |
| `block-channel` | Block CL, DV, CW, AR, IA, WP, GS, MP, PL, RS | Channels and their projections |
| `agent-private` | Block AE, AP, AO, AM | Agent privacy and counters |
| `key-backup` | official 49 | NIP-49 encrypted import and export |

The [Coax Nostr plan](../../coax/nostr.md), section 1.2, gives each
profile's boundary. Other official NIPs enter only with a named consumer
and conformance fixtures.

## The rule

The teaching report has one row per NIP and one column per target. Each cell counts
the chapters that cite the NIP and how many of them are *done* on that
target. A chapter is done on a target when all three hold:

1. its status is `implemented`;
2. it claims the target in `qualified_targets`;
3. a record backs that claim.

The cell is `done` when every citing chapter is done on the target, `part`
when some are, and `-` when none are. A profile conforms on a target when
every NIP in its list is `done` there. It is *advertised* to a consumer
only when it conforms and some file outside the book uses the code, which
is what the snapshot's `adopted` counts model. Actual consumer advertisement
also needs its full capability, integration and security gates; the current
registry's direct component edges do not supply those gates.

Before any cell is computed, every claim is checked. A target that a
chapter claims without a record, or any target that a design draft claims,
refuses the whole report. A report with one unbacked claim would let that
claim count, so none is built.

## Source pins

A NIP's text is part of the claim: conforming to NIP-44 means conforming to
one text of it. `profiles.coax` pins the three lanes that the NIP files in
[nostr/nips/](../nips/) come from:

| Lane | Pin | How it is checked |
| --- | --- | --- |
| official | commit `400d157c53264b8e58c7b3cdbd21ef546fb7a157` | `upstream-lock.json` lists the SHA-256 of each of its 101 files |
| block | commit `24a370606044c0c7be787cc2fd98ec0bfe1d606b` | the same, 22 files |
| openagents | `NP_OWNED_DIGEST` | SHA-256 over every file in `nostr/nips/openagents/`, sorted by path, each as path, a NUL byte, its bytes and a NUL byte |

The OpenAgents lane has no upstream commit, so its pin is a digest of the
files themselves. When a file is added or changed, the digest changes and
the pin check fails until someone reviews the new text and records the new
digest.

## Worked example: the NIP-98 row by hand

Take NIP-98 and the rows in the example, which are every chapter whose spec
column cites a NIP of the four official profiles.

1. **The citing chapters.** Two rows cite NIP-98: chapter 49, *HTTP auth*,
   and chapter 99, *The website and sign-in*. Both are implemented and both
   claim `cpu` and `wasm`. So 2 chapters cite it.
2. **The claims.** Each of the four claims has a record:

   | Chapter | Target | Record |
   | --- | --- | --- |
   | 49 | cpu | `nostr/fixtures/http-auth.out`, the example's transcript in debug and release |
   | 49 | wasm | `docs/verification/2026-10-29-nip98-tags.md`: `nip98_tags.py WasmTags` passed 15 raw-array cases |
   | 99 | cpu | `nostr/fixtures/web-sign-in.out`, the example's transcript in debug and release |
   | 99 | wasm | `docs/verification/2026-10-29-nip01-page.md`: `nip01_wasm.cjs` ran the page's verifier, `nip01-verify.coax`, as emitted Wasm |

   No claim is unbacked, so the report can be built.
3. **cpu.** Chapter 49 is implemented, claims `cpu` and has its record: done.
   So is chapter 99. 2 of 2: `done 2/2`.
4. **wasm.** The same reasoning: `done 2/2`.
5. **Open chapters.** Every citing chapter is implemented: none.

The row is `98   done 2/2   done 2/2   none`.

Until chapter 99 was promoted, on 2026-11-01 (`73966b6359`), it was partly
implemented, with the same claims and records. A partly implemented chapter
is never done, so the row was then `part 1/2` on both targets. NIP-44 is
that case in the snapshot: chapter 20 is done on `cpu`, and chapter 100, *Pylons and
the mesh*, which also cites it, is partly implemented. Its row is
`part 1/2` on `cpu`, and `-` on `wasm`, where neither chapter claims a run.

The `phone-signer` profile needs NIP-46 and NIP-98. NIP-46 comes first, and
its only citing chapter, 48, is a design draft: its cell is `-` on both
targets. So `phone-signer` does not conform on either target, and the reason
given is NIP-46.

`key-backup` needs NIP-49. Only chapter 47 cites it; it is implemented and
claims `cpu` (record `ncryptsec.out`) and `wasm` (record
`2026-10-29-scrypt-nip49.md`, the composition against `hashlib.scrypt` in
checked and release Wasm). Both cells are `done 1/1`, and `key-backup`
conforms on both. Chapter 47's `adopted_by` is empty, so the profile is not
advertised.

## The report in Coax

The registry is plain arrays. Targets are a bit set: 1 is `cpu`, 2 is
`wasm`.

<!-- source: nostr/examples/conformance.coax registry -->
```jai
// Targets, as chapters.json names them, and chapter states.
TARGET_NAME :: [2] string.["cpu", "wasm"];
TARGETS :: TARGET_NAME.count;
IMPLEMENTED :: 0;
PARTLY :: 1;
DRAFT :: 2;
STATUS_NAME :: [3] string.["implemented", "partly implemented", "design draft"];
MAX_ROWS :: 24;
MAX_RECORDS :: 28;
// Registry rows and the records behind each (chapter, target) claim.
// targets is a bit set: 1 cpu, 2 wasm. nips is the chapter's spec column
// in PLAN.md, comma-separated. adopted counts chapters.json's adopted_by.
Registry :: struct {
  rows: int;
  ch: [MAX_ROWS] int; status: [MAX_ROWS] int; targets: [MAX_ROWS] int;
  nips: [MAX_ROWS] string; adopted: [MAX_ROWS] int;
  records: int;
  rec_ch: [MAX_RECORDS] int; rec_target: [MAX_RECORDS] int; rec_path: [MAX_RECORDS] string;
}
```

The rows and records are copied from the files. A `cpu` record is the
chapter's transcript in `examples.json`; chapter 100's `cpu` record is the
transcript of Psionic chapter 24's example, which runs its code:

<!-- source: nostr/examples/conformance.coax rows -->
```jai
// Every chapter whose spec column cites a NIP of the four official profiles,
// copied from chapters.json and PLAN.md at 73966b6359.
ROWS :: 22;
ROW_CH :: [ROWS] int.[16, 17, 20, 21, 22, 23, 24, 25, 29, 30, 31, 33, 34, 35, 36, 38, 44, 47, 48, 49, 99, 100];
ROW_STATUS :: [ROWS] int.[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 2, 0, 0, 1];
ROW_TARGETS :: [ROWS] int.[1, 1, 1, 3, 1, 3, 1, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1, 3, 0, 3, 3, 1];
ROW_NIPS :: [ROWS] string.["19,21", "01", "44", "01", "01,16,33", "01,12", "01,20", "59", "01", "01,67",
  "01", "01,09", "01", "01", "01", "42", "01,11", "49", "07,46,55", "98", "98,06", "PYLON,44"];
// One record for each claim: the book's own debug and release run of the
// chapter's example for cpu, and the verification record of a Wasm build.
RECORDS :: 25;
REC_CH :: [RECORDS] int.[16, 17, 20, 21, 22, 23, 24, 25, 29, 30, 31, 34, 35, 36, 38, 44, 47, 49,
  21, 23, 47, 49, 99, 99, 100];
REC_TARGET :: [RECORDS] int.[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 0, 1, 0];
REC_PATH :: [RECORDS] string.[
  "nostr/fixtures/nip19.out", "nostr/fixtures/signable-json.out", "nostr/fixtures/nip44.out",
  "nostr/fixtures/events.out", "nostr/fixtures/kinds-tags.out", "nostr/fixtures/filters.out",
  "nostr/fixtures/relay-messages.out", "nostr/fixtures/gift-wrap.out", "nostr/fixtures/relay-connection.out",
  "nostr/fixtures/subscriptions.out", "nostr/fixtures/publish-ok.out", "nostr/fixtures/relay-admit.out",
  "nostr/fixtures/store-events.out", "nostr/fixtures/match-indexes.out", "nostr/fixtures/nip42-auth.out",
  "nostr/fixtures/relay-limits.out", "nostr/fixtures/ncryptsec.out", "nostr/fixtures/http-auth.out",
  "docs/verification/2026-10-30-event-sign.md", "docs/verification/2026-10-30-newland-coax-nostr.md",
  "docs/verification/2026-10-29-scrypt-nip49.md", "docs/verification/2026-10-29-nip98-tags.md",
  "nostr/fixtures/web-sign-in.out", "docs/verification/2026-10-29-nip01-page.md",
  "psionic/fixtures/pylon-job.out"];
registry :: () -> Registry {
  r: Registry;
  r.rows = ROWS;
  for i: 0..<ROWS {
    r.ch[i] = ROW_CH[i]; r.status[i] = ROW_STATUS[i]; r.targets[i] = ROW_TARGETS[i];
    r.nips[i] = ROW_NIPS[i];
  }
  r.records = RECORDS;
  for i: 0..<RECORDS { r.rec_ch[i] = REC_CH[i]; r.rec_target[i] = REC_TARGET[i]; r.rec_path[i] = REC_PATH[i]; }
  return r;
}
```

Every claim is checked first. A design draft refuses before its record is
looked up, because no record makes a draft's claim true:

<!-- source: nostr/examples/conformance.coax evidence -->
```jai
// Refusals. A report is built only when every claim has its record.
OK :: 0;
NO_RECORD :: 1;
DRAFT_CLAIM :: 2;
NOT_DONE :: 3;
Refusal :: struct { why: int; ch: int; target: int; nip: string; }
has_record :: (r: Registry, ch: int, t: int) -> bool {
  for k: 0..<r.records { if r.rec_ch[k] == ch && r.rec_target[k] == t { return true; } }
  return false;
}
// Every claimed target needs a record, and a design draft claims none.
check_claims :: (r: Registry) -> Refusal {
  for i: 0..<r.rows {
    for t: 0..<TARGETS {
      if r.targets[i] & (1 << t) == 0 { continue; }
      if r.status[i] == DRAFT { return Refusal.{why = DRAFT_CLAIM, ch = r.ch[i], target = t}; }
      if !has_record(r, r.ch[i], t) { return Refusal.{why = NO_RECORD, ch = r.ch[i], target = t}; }
    }
  }
  return Refusal.{};
}
```

A cell counts citing chapters and done chapters:

<!-- source: nostr/examples/conformance.coax cell -->
```jai
// One cell of the report: how many chapters cite the NIP, and how many of
// them are done on target t (implemented, t claimed, and a record for it).
NONE :: 0;
PART :: 1;
DONE :: 2;
STATE_NAME :: [3] string.["-", "part", "done"];
Cell :: struct { state: int; done: int; cited: int; }
done_on :: (r: Registry, i: int, t: int) -> bool {
  return r.status[i] == IMPLEMENTED && r.targets[i] & (1 << t) != 0 && has_record(r, r.ch[i], t);
}
cell :: (r: Registry, nip: string, t: int) -> Cell {
  c: Cell;
  for i: 0..<r.rows {
    if !has_nip(r.nips[i], nip) { continue; }
    c.cited += 1;
    if done_on(r, i, t) { c.done += 1; }
  }
  if c.done > 0 { c.state = ifx c.done == c.cited then DONE else PART; }
  return c;
}
```

A profile's NIPs come from `profiles.coax` itself, through `np_nips`, so the
report and the support matrix can't name different lists. A profile
conforms when every NIP is `done`; the first one that isn't is the reason.
An empty name in the list, such as the one in `",49"`, is never done:

<!-- source: nostr/examples/conformance.coax profile -->
```jai
// A profile conforms on t when every NIP in its list is done on t. The
// first NIP that isn't is the refusal's reason; an empty name is never done.
conforms :: (r: Registry, nips: string, t: int) -> Refusal {
  for k: 0..<nip_count(nips) {
    nip := nip_at(nips, k);
    if cell(r, nip, t).state != DONE { return Refusal.{why = NOT_DONE, target = t, nip = nip}; }
  }
  return Refusal.{};
}
// Files outside the book that use the code for these NIPs: chapters.json's
// adopted_by, counted once for each chapter that cites one of them.
adopted :: (r: Registry, nips: string) -> int {
  n := 0;
  for i: 0..<r.rows {
    for k: 0..<nip_count(nips) {
      if has_nip(r.nips[i], nip_at(nips, k)) { n += r.adopted[i]; break; }
    }
  }
  return n;
}
// Advertised to a consumer: conforms, and some file outside the book uses it.
advertised :: (r: Registry, nips: string, t: int) -> bool {
  return conforms(r, nips, t).why == OK && adopted(r, nips) > 0;
}
```

The "Done means" line of this chapter asks that the report's NIP list equal
the set of done chapters. `done_nip` computes that set from the rows alone,
without `cell`, so the checks can compare the two:

<!-- source: nostr/examples/conformance.coax done -->
```jai
// The NIPs that some implemented chapter with a record cites, read from the
// rows alone, without cell.
done_nip :: (r: Registry, nip: string) -> bool {
  for i: 0..<r.rows {
    if r.status[i] != IMPLEMENTED || !has_nip(r.nips[i], nip) { continue; }
    for t: 0..<TARGETS { if r.targets[i] & (1 << t) != 0 && has_record(r, r.ch[i], t) { return true; } }
  }
  return false;
}
// The registry with record k taken out.
without_record :: (r: Registry, k: int) -> Registry {
  b := r; b.records -= 1;
  for j: k..<b.records { b.rec_ch[j] = r.rec_ch[j + 1]; b.rec_target[j] = r.rec_target[j + 1]; b.rec_path[j] = r.rec_path[j + 1]; }
  return b;
}
```

## The example

The frozen example is
[nostr/examples/conformance.coax](../examples/conformance.coax). It counts
the claims and records, prints the report, prints the NIPs that done
chapters cite, and then builds four broken registries:

<!-- source: nostr/examples/conformance.coax main -->
```jai
main :: () {
  r := registry();
  claims := 0;
  for i: 0..<r.rows { for t: 0..<TARGETS { if r.targets[i] & (1 << t) != 0 { claims += 1; } } }
  print("% registry rows, % claimed targets, % records\n", r.rows, claims, r.records);
  if !report(r) { print("no report is built\n"); }
  print("NIPs that an implemented chapter with a record cites:");
  for nip: REPORT_NIPS { if done_nip(r, nip) { print(" %", nip); } }
  print("\n");

  print("\nbroken: chapter 16 also claims wasm\n");
  b := r; b.targets[0] = 3;
  if !report(b) { print("no report is built\n"); }
  print("broken: chapter 48, a design draft, claims cpu with a record\n");
  b = r; b.targets[18] = 1;
  b.rec_ch[b.records] = 48; b.rec_target[b.records] = 0; b.rec_path[b.records] = "made-up.out"; b.records += 1;
  if !report(b) { print("no report is built\n"); }
  print("broken: the record for chapter 47 on wasm is removed\n");
  b = without_record(r, 20);
  if !report(b) { print("no report is built\n"); }
  print("broken: NIP-46 added to key-backup, claimed on cpu\n");
  show_refusal(conforms(r, "49,46", 0));
}
```

Run it:

```shell
coax/bin/coax run nostr/examples/conformance.coax
```

Output:

```text
22 registry rows, 25 claimed targets, 25 records
NIP  cpu        wasm       open chapters
01   part 12/13 part 2/13  33 (design draft)
19   done 1/1   -          none
42   done 1/1   -          none
44   part 1/2   -          100 (partly implemented)
46   -          -          48 (design draft)
49   done 1/1   done 1/1   none
59   done 1/1   -          none
98   done 2/2   done 2/2   none
core-read     NIPs 01,19        cpu: no (NIP-01) wasm: no (NIP-01)  adopted by 0, advertised: no
core-write    NIPs 01,42,44,59  cpu: no (NIP-01) wasm: no (NIP-01)  adopted by 0, advertised: no
phone-signer  NIPs 46,98        cpu: no (NIP-46) wasm: no (NIP-46)  adopted by 0, advertised: no
key-backup    NIPs 49           cpu: yes         wasm: yes          adopted by 0, advertised: no
NIPs that an implemented chapter with a record cites: 01 19 42 44 49 59 98

broken: chapter 16 also claims wasm
refused: chapter 16 claims wasm, and no record backs it
no report is built
broken: chapter 48, a design draft, claims cpu with a record
refused: chapter 48 is a design draft and claims cpu
no report is built
broken: the record for chapter 47 on wasm is removed
refused: chapter 47 claims wasm, and no record backs it
no report is built
broken: NIP-46 added to key-backup, claimed on cpu
refused: NIP-46 is not done on cpu
```

Read the report as a result for `73966b6359`, not the current registry:

- NIP-01 is cited by 13 chapters. Twelve are implemented with a `cpu`
  record; chapter 33, *A client that survives restarts*, is a design draft.
  Only chapters 21 and 23 have `wasm` records. Neither column is `done`, so
  `core-read` and `core-write` do not conform, though every other NIP of
  `core-read` is done on `cpu`.
- NIP-44 is `part` because chapter 100, *Pylons and the mesh*, cites it and
  is partly implemented.
- NIP-98 is `done` on both targets, but `phone-signer` still does not
  conform: NIP-46 has no done chapter.
- The NIPs that done chapters cite are 01, 19, 42, 44, 49, 59 and 98:
  exactly the rows that are not `-` in some column. NIP-46 is in a profile
  but in no done chapter, so it is reported as `-`, never as supported.
- Nothing is advertised in this snapshot: its adoption counts are all zero.

The broken registries show each refusal:

1. Chapter 16 claims `wasm`, as a careless edit to `chapters.json` might.
   There is no record of NIP-19 running in Wasm, so no report is built.
2. Chapter 48, a design draft, claims `cpu` and comes with a record. A draft
   can't be done, so the record doesn't help.
3. The record for chapter 47 on Wasm is removed. The claim is now unbacked,
   and the report that said `key-backup` conforms on `wasm` is not built.
4. NIP-46 is added to `key-backup`'s list and the profile is claimed on
   `cpu`. NIP-46 has no done chapter, and the claim is refused with its name.

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/conformance.coax](../examples/checks/conformance.coax),
which loads the example. The independent check is a second implementation:
a Python scratch script (2026-11-01) that read `chapters.json`, the spec
column of `PLAN.md` and `examples.json` at `73966b6359`, selected the
chapters citing the profiles' NIPs, confirmed that each record file exists
and that each `cpu` record is an example run in debug and release, and
computed every cell and profile. It shares no code with the example. Its
values are pasted into the checks. They also cover:

- the NIP-98, NIP-44 and NIP-49 rows by hand, NIP-98 becoming `part` when
  chapter 99 is set back to partly implemented, and NIP-44 becoming `done`
  on `cpu` when chapter 100 is marked implemented;
- the "Done means" equality on the real rows and with each of the 25
  records removed in turn;
- every record being needed: without any one record, the report refuses
  that record's chapter and target;
- every target a row doesn't claim, claimed without a record: a design
  draft refuses as a draft, any other chapter for want of a record;
- an extra record for an unclaimed target, which changes nothing, and a
  record without a claim, which doesn't make a NIP done;
- a NIP added to a profile without a done chapter, and an empty name in a
  profile's list;
- the profile lists read from `profiles.coax`, and `advertised` agreeing
  with `np_can_advertise` from `profiles.coax` for all four profiles on both
  targets (no profile is advertised by either in the pinned example);
- adoption counted once per citing chapter, and the list helpers on names
  that share digits.

<!-- source: nostr/examples/checks/conformance.coax check -->
```jai
// A second implementation, a Python scratch script (2026-11-01), read
// chapters.json, PLAN.md's spec column and examples.json at 73966b6359,
// picked every chapter citing NIP-01, 19, 42, 44, 46, 49, 59 or 98, checked
// that each record file exists and that each cpu record is an example run
// in debug and release, and computed the cells. Its output, pasted:
WANT_STATE :: [16] int.[PART, PART, DONE, NONE, DONE, NONE, PART, NONE,
  NONE, NONE, DONE, DONE, DONE, NONE, DONE, DONE];
WANT_DONE :: [16] int.[12, 2, 1, 0, 1, 0, 1, 0, 0, 0, 1, 1, 1, 0, 2, 2];
WANT_CITED :: [8] int.[13, 1, 1, 2, 1, 1, 1, 2];
WANT_CONFORMS :: [8] bool.[false, false, false, false, false, false, true, true];
#test "the report matches a second implementation over the real files" {
  r := registry();
  #assert check_claims(r).why == OK;
  claims := 0;
  for i: 0..<r.rows { for t: 0..<TARGETS { if r.targets[i] & (1 << t) != 0 { claims += 1; } } }
  #assert r.rows == 22 && claims == 25 && r.records == 25;
  for k: 0..<8 {
    for t: 0..<TARGETS {
      c := cell(r, REPORT_NIPS[k], t);
      #assert c.state == WANT_STATE[2 * k + t] && c.done == WANT_DONE[2 * k + t] && c.cited == WANT_CITED[k];
    }
  }
  for q: 0..<4 {
    for t: 0..<TARGETS {
      #assert (conforms(r, profile_nips(REPORT_PROFILES[q]), t).why == OK) == WANT_CONFORMS[2 * q + t];
    }
  }
}

// NIP-98 by hand: chapters 49 and 99 are implemented, claim cpu and wasm,
// and have a record for each: 2 of 2 on each target. Set chapter 99 back to
// partly implemented and it no longer counts, though its claims and records
// stay: 1 of 2. NIP-44 is that case today: chapter 100 is partly implemented.
#test "NIP-98, NIP-44 and NIP-49 by hand" {
  r := registry();
  c := cell(r, "98", 0); #assert c.state == DONE && c.done == 2 && c.cited == 2;
  c = cell(r, "98", 1); #assert c.state == DONE && c.done == 2 && c.cited == 2;
  r.status[20] = PARTLY;
  #assert r.ch[20] == 99 && check_claims(r).why == OK;
  c = cell(r, "98", 0); #assert c.state == PART && c.done == 1 && c.cited == 2;
  c = cell(r, "98", 1); #assert c.state == PART && c.done == 1 && c.cited == 2;
  r = registry();
  c = cell(r, "44", 0); #assert c.state == PART && c.done == 1 && c.cited == 2;
  r.status[21] = IMPLEMENTED;
  #assert r.ch[21] == 100 && cell(r, "44", 0).state == DONE && cell(r, "44", 1).state == NONE;
  r = registry();
  c = cell(r, "49", 0); #assert c.state == DONE && c.done == 1 && c.cited == 1;
  c = cell(r, "49", 1); #assert c.state == DONE && c.done == 1 && c.cited == 1;
  #assert conforms(r, "49", 0).why == OK && conforms(r, "49", 1).why == OK;
  // a NIP no row cites is "-"
  c = cell(r, "04", 0); #assert c.state == NONE && c.cited == 0;
}

// Done means: the NIPs with any done cell are exactly those an implemented
// chapter with a record cites, on the real rows and with any one record gone.
#forall "the report's NIPs equal the NIPs of done chapters" k: -1..<RECORDS {
  r := registry();
  if k >= 0 { r = without_record(r, k); }
  for nip: REPORT_NIPS {
    shown := cell(r, nip, 0).state != NONE || cell(r, nip, 1).state != NONE;
    #assert shown == done_nip(r, nip);
  }
}
// Every record is needed: without record k, the report refuses that
// record's chapter and target.
#forall "removing any record refuses the report" k: 0..<RECORDS {
  f := check_claims(without_record(registry(), k));
  #assert f.why == NO_RECORD && f.ch == REC_CH[k] && f.target == REC_TARGET[k];
}
// Every claimed target of every row, set where it wasn't: a design draft
// refuses as a draft, any other chapter for want of a record.
#forall "a new claim without a record refuses" i: 0..<ROWS, t: 0..<TARGETS {
  r := registry();
  if r.targets[i] & (1 << t) == 0 {
    r.targets[i] |= 1 << t;
    f := check_claims(r);
    want := ifx r.status[i] == DRAFT then DRAFT_CLAIM else NO_RECORD;
    #assert f.why == want && f.ch == r.ch[i] && f.target == t;
  }
}
// A record for a target nobody claims changes nothing.
#test "an extra record claims nothing" {
  r := registry();
  r.rec_ch[r.records] = 16; r.rec_target[r.records] = 1; r.rec_path[r.records] = "extra.out"; r.records += 1;
  #assert check_claims(r).why == OK && cell(r, "19", 1).state == NONE;
}

// A NIP added to a profile without a done chapter: the profile refuses,
// naming the NIP and the target.
#test "a NIP added to a profile without evidence refuses" {
  r := registry();
  f := conforms(r, "49,46", 0); #assert f.why == NOT_DONE && f.nip == "46" && f.target == 0;
  f = conforms(r, "49,19", 1); #assert f.why == NOT_DONE && f.nip == "19" && f.target == 1;
  #assert conforms(r, "49,19", 0).why == OK && conforms(r, "", 0).why == OK;
  // the first NIP not done is the reason
  f = conforms(r, "19,01,46", 0); #assert f.nip == "01";
}

// The profiles' lists come from profiles.coax. The openagents and Block
// profiles have no official list. Nothing is adopted, so nothing is
// advertised, in agreement with profiles.coax's own claims.
#test "profiles from profiles.coax and advertising" {
  #assert profile_nips(0) == "01,19" && profile_nips(1) == "01,42,44,59";
  #assert profile_nips(2) == "46,98" && profile_nips(7) == "49";
  #assert profile_nips(3) == "" && profile_nips(4) == "" && profile_nips(8) == "";
  r := registry();
  for p: REPORT_PROFILES {
    nips := profile_nips(p);
    #assert adopted(r, nips) == 0;
    #assert advertised(r, nips, 0) == np_can_advertise(p, 0, np_required(p));
    #assert advertised(r, nips, 1) == np_can_advertise(p, 2, np_required(p));
  }
  // one adopting file makes key-backup advertised, on both targets
  r.adopted[17] = 1;
  #assert r.ch[17] == 47 && adopted(r, "49") == 1 && advertised(r, "49", 0) && advertised(r, "49", 1);
  #assert !advertised(r, "01,19", 0);
}

// The list helpers on names that share digits, and on empty names.
#test "comma lists match whole names" {
  #assert has_nip("01,12", "01") && has_nip("01,12", "12") && !has_nip("01,12", "1") && !has_nip("01,12", "2");
  #assert has_nip("PYLON,44", "44") && !has_nip("PYLON,44", "PYLO") && !has_nip("", "01") && !has_nip("012", "01");
  #assert has_nip(",19", "19") && !has_nip(",19", ",19");
  #assert nip_at("01,42,44", 0) == "01" && nip_at("01,42,44", 2) == "44" && nip_at("01,42,44", 3) == "";
  #assert nip_at("", 0) == "" && nip_at("49", 0) == "49" && nip_at("49", 1) == "";
  #assert nip_at(",19", 0) == "" && nip_at(",19", 1) == "19";
  #assert after_prefix("official:49", "official:") == "49" && after_prefix("Official:49", "official:") == "";
  #assert after_prefix("official", "official:") == "" && after_prefix("block:OA", "official:") == "";
  #assert nip_count("") == 0 && nip_count("49") == 1 && nip_count("01,42,44,59") == 4 && nip_count(",19") == 2;
  #assert digits(0) == 1 && digits(9) == 1 && digits(10) == 2 && digits(13) == 2 && digits(100) == 3;
}
// An empty name in a profile is never done: ",49" refuses at the empty name.
#test "an empty name in a profile refuses" {
  r := registry();
  f := conforms(r, ",49", 0); #assert f.why == NOT_DONE && f.nip == "";
  f = conforms(r, "49,", 1); #assert f.why == NOT_DONE && f.nip == "";
}
// The records as copied, and taking one out shifts the rest down.
#test "records and without_record" {
  r := registry();
  #assert r.rec_path[0] == "nostr/fixtures/nip19.out" && r.rec_path[17] == "nostr/fixtures/http-auth.out";
  #assert r.rec_ch[20] == 47 && r.rec_target[20] == 1 && r.rec_path[20] == "docs/verification/2026-10-29-scrypt-nip49.md";
  #assert r.rec_path[22] == "nostr/fixtures/web-sign-in.out" && r.rec_path[24] == "psionic/fixtures/pylon-job.out";
  b := without_record(r, 0);
  #assert b.records == 24 && b.rec_ch[0] == 17 && b.rec_path[0] == "nostr/fixtures/signable-json.out";
  #assert b.rec_ch[23] == 100 && b.rec_target[22] == 1 && b.rec_path[23] == "psionic/fixtures/pylon-job.out";
}
// done_nip wants a claimed target: chapter 16 with its record but no claim
// doesn't make NIP-19 done.
#test "a record without a claim does not make a NIP done" {
  r := registry();
  #assert done_nip(r, "19");
  r.targets[0] = 0;
  #assert r.ch[0] == 16 && !done_nip(r, "19") && cell(r, "19", 0).state == NONE;
}
// adopted counts a chapter's files only when it cites one of the NIPs,
// once for each chapter.
#test "adoption by hand" {
  r := registry();
  r.adopted[0] = 2;
  #assert adopted(r, "01,19") == 2 && adopted(r, "19,21") == 2 && adopted(r, "49") == 0 && adopted(r, "") == 0;
  r.adopted[1] = 1;
  #assert adopted(r, "01,19") == 3 && adopted(r, "01") == 1;
}
```

Run them:

```shell
coax/bin/coax test nostr/examples/checks/conformance.coax
```

All 13 of this file's checks pass, with the 5 checks of `profiles.coax`
that it loads (18 in all, 2026-11-01). `coax mutate --root-only` on a
temporary file that joins the example and its checks: 161 mutants, 161
killed (the print-only procedures `show_*`, `pad` and `report` are not
reached by a check). The checks file alone has no mutation sites.

### The support matrix and its pins

`profiles.json` is generated by `profile_matrix.coax` from `profiles.coax`.
It answers a different question from the book's report: not "does the
book's code pass for this NIP" but "can a named consumer rely on this
profile on this target". It counts capabilities, such as `wire-codec`,
`event-verify` or `key-backup`, each backed by an evidence row with a source
file, a checks file, a commit and the SHA-256 of the source at that commit.
`np_can_advertise` allows a claim only when every bit of it is evidenced for
the target. Today no profile has all its required capabilities on any
target, so `profiles.json` marks every profile unsupported. The book's
report says `key-backup` conforms on both targets, which is true of the
chapter's code; the matrix says no consumer can rely on it yet, which is
also true. The book's `advertised` column agrees with the matrix: no.

`profiles.py` was run for this chapter on 2026-11-01. Seven of its eight
tests pass: the checked-in matrix equals the generator's output, native
bounds match an independent fixture, claims match evidence, the
safe-integer bound, sanitizers and the release build, consumer limits, and
eleven seeded faults (among them `nips` set to `all` and an evidence commit
set to `unreviewed`) are all refused. The eighth,
`test_source_pins_and_real_evidence`, fails: the OpenAgents lane's digest
is now `fb7563e7aa5ce8519940d46e37b350f3f61d0f3422356d0f1acd65a5dc34fad2`,
not the pinned `2fb1f51f2c3b737e44f5811148ef4ca8ac7d4a106f4a0460a98ffa7f2c2a6f39`.
A separate computation over the files at each commit finds the change:
the pinned digest is that of the lane's 71 files just before `b01de252f7`,
which added `NIP-KEYS.md` as the 72nd. The pin check did what it is for. The
pin is not updated here: recording a new digest is a review of the new
text, not an edit to the book.

## Summary

- The registry's claims are `status`, `qualified_targets` (with `profile`
  as its label) and `adopted_by`. A qualified target is true only with a
  record: the example's debug and release transcript for `cpu`, a
  verification record of a Wasm run for `wasm`.
- A NIP's cell counts the chapters that cite it and those done on the
  target; a profile conforms where all its NIPs are `done`, and is
  advertised only with an adopting consumer.
- A claim without a record, or any claim by a design draft, refuses the
  whole report.
- In the pinned teaching snapshot, only `key-backup` conforms among the four
  official profiles (on `cpu` and `wasm`); nothing is advertised. This is not
  a current all-NIP or production-conformance result.
- The support matrix's pin check found that the OpenAgents lane changed
  after its pin was recorded.

## Dependencies

- [93. Test vectors and their
  provenance](ch093-test-vectors-and-their-provenance.md)
- [94. Differential tests](ch094-differential-tests.md)

## Sources

- [NIP-01](../nips/official/01.md), [NIP-19](../nips/official/19.md),
  [NIP-42](../nips/official/42.md), [NIP-44](../nips/official/44.md),
  [NIP-46](../nips/official/46.md), [NIP-49](../nips/official/49.md),
  [NIP-59](../nips/official/59.md) and [NIP-98](../nips/official/98.md): the
  NIPs of the four official profiles.
- The pins: [nostr/nips/upstream-lock.json](../nips/upstream-lock.json) and
  [coax/tests/vectors/SOURCES.md](../../coax/tests/vectors/SOURCES.md).
- The records cited as evidence:
  [2026-10-30-event-sign.md](../../docs/verification/2026-10-30-event-sign.md),
  [2026-10-30-newland-coax-nostr.md](../../docs/verification/2026-10-30-newland-coax-nostr.md),
  [2026-10-29-scrypt-nip49.md](../../docs/verification/2026-10-29-scrypt-nip49.md),
  [2026-10-29-nip98-tags.md](../../docs/verification/2026-10-29-nip98-tags.md)
  and [2026-10-29-nip01-page.md](../../docs/verification/2026-10-29-nip01-page.md).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md), section 1.2.
