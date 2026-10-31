# Explicit public guard probes

`public-probe.coax` runs the frozen eligible guard cases through the existing
published `mesh-client.sh`. It reuses one caller-owned demo key, spaces case
starts by at least six monotonic seconds, records each outcome once, and stops
at the first HTTP or pin/contract refusal. It never creates an identity or runs
as a default check. The separate `control` selector sends the published small
Water-is-wet question; it is not a guard corpus row.

All cohort selection, request writing, process orchestration, pacing, response
admission and JSONL writing are Coax. The small C host exposes argv, monotonic
clock/sleep and the existing file effect. Existing `process-native.c`,
`store-file-native.c`, the published client and `sign.c` perform effects.

Build an instrumented local executable from the repository root:

```sh
coax/bin/coax c psionic/tests/guard-live/public-probe.coax --library > /tmp/guard-public-probe.c
cc -std=c11 -O1 -g -fsanitize=address,undefined -fno-omit-frame-pointer -c /tmp/guard-public-probe.c -o /tmp/guard-public-probe.o
cc -std=c11 -O1 -g -fsanitize=address,undefined -fno-omit-frame-pointer -Icoax/platform psionic/tests/guard-live/public-probe-host.c coax/platform/process-native.c coax/platform/store-file-native.c /tmp/guard-public-probe.o -lm -o /tmp/guard-public-probe
```

Use one mode-0600 private demo key and a current installed signer. The signer
can create that key once with `sign --new PRIVATE_KEY`; retain it for the whole
bounded run, including any separate authorized diagnostic. Do not request a new
identity for each case or after a quota refusal. The public edge's normal
per-identity and global quotas still apply.

The positional arguments are `KEY REQUEST_FILE START COUNT SIGNER`.
`REQUEST_FILE` must be a private disposable file outside the repository. The
first small probe should pass before a separately authorized larger cohort:

```sh
/tmp/guard-public-probe PRIVATE_KEY PRIVATE_REQUEST_FILE 0 1 CURRENT_SIGNER > initial.jsonl
/tmp/guard-public-probe PRIVATE_KEY PRIVATE_REQUEST_FILE 1 15 CURRENT_SIGNER > remaining.jsonl
```

An explicitly authorized control uses `control 1` in place of `START COUNT` and
belongs in its own file. Its `pinned` field checks provider/checkpoint metadata;
the ambient decoder is used only for guard cases. Record the control's actual
model/answer and reference comparison separately. The initial failed observation
must remain separate if a new diagnostic cohort is later authorized; never
replace it with a successful retry or feed duplicate case indices into a summary.

Each JSONL row contains `case`, ceiling-rounded monotonic `elapsed_ms`, HTTP
`status` (zero means an unclassified local/transport failure), `pinned`, and the
raw synthetic response as an escaped `body` string. No key, auth event, cookie
or header enters the output. The elapsed time starts before the helper spawn
and ends after response/process completion; it includes helper startup, NIP-98
signing and the complete service path. It is not an isolated browser-fetch
measurement. A response later than the game's eight-second deadline remains a
recorded outcome, and the Coax evaluator refuses to count it as coverage.

Offline qualification makes no model calls:

```sh
coax/bin/coax test psionic/tests/guard-live/public-probe-policy.coax --library
coax/bin/coax mutate psionic/tests/guard-live/public-probe-policy.coax --root-only --jobs 4
```

Replay guard JSONL using `guard-evaluation summary` from the adjacent evaluation
tool. The 26 authored labels include 16 remote-eligible contexts and ten local
ineligibility checks; only indices 0–15 may be remotely probed. The editorial
labels and `promote:false` are not calibrated player outcomes.
