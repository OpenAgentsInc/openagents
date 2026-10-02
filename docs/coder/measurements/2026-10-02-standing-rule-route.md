# Standing instructions in the chat: the `standing.rule` route (#10157)

"Keep my disk above 50 GB free", "every morning pull main in
~/work/openagents", "tell me whenever a Coder run fails", "only keep 2 agent
target dirs", "pause disk cleanup until tomorrow": none of these is a run.
Each is a background rule, or a change to one
([background processes, phase 2](../../background/2026-10-02-background-processes.md#phase-2-as-built)).
The design is in
[the chat router design](../design/2026-09-28-chat-router.md#standing-instructions-standingrule-2026-10-02).

The `route` Choice gained an option, so the question set is
`chat-router-v5` (requests naming v4 and earlier are routed with it), the
Gym suite `chat-router-v5` and its question set `chat-router-route-v6` are
generated from `routes-v5.json` (`ROUTER_SUITE_WRITE=1`, the v4 suite and
question set kept as recorded), and `calibration-v2.json` is refit, which
the committed-record test requires.

## The new rows

`routes-v5.json` is `routes-v4.json` unchanged plus 56 rows tagged
`standing` (`ROUTER_EVAL_ROWS=standing ROUTER_EVAL_SPLIT=all`): 42 asks
for something ongoing or a change to it (keeping free space, pruning
worktrees on a schedule, low-disk warnings, telling the user when runs fail
or end, keeping a checkout up to date daily or hourly, and edits, pauses,
resumes, and removals) labeled `standing.rule` with `standing.elsewhere`
(the line the set's default phone context gets; a terminal gets
`standing.rule` and compiles the rule on the computer), and 14 near misses:
doing it once now (`work.dispatch`), a one-time reminder or email
(`capability.missing`), cron and timers in general (`general`), how
background rules work (`product.kb`), and listing rules or their log
(`cli`). Every new row's id falls in the tune split, so the held-out
partition stays at NIP-EVAL's 256 cases.

Phone context, hosted Jev, the 56 rows (Boat sandbox, 2026-10-02):

| Metric | Value |
| --- | --- |
| Route accuracy | 94.6 % |
| `standing.rule` hit / predicted / labeled | 41 / 42 / 42 |
| Canned precision | 100 % (43/43) |
| Dispatch precision on these rows | 75 % (6/8) |

The misses: "go back to deleting worktrees in the disk cleanup" read as
work (a Coder offer), and the two `cli` near misses did not reach their
command group ("show me the disk cleanup log" read as work, "list my
background rules" as a standing rule). Every one-off near miss
(`work.dispatch`, `capability.missing`, `general`, `product.kb`) took its
own route.

## The published eval

`ROUTER_EVAL_PUBLISH=1 cargo test -p coder --test router_eval live_router
-- --ignored`, 664 requests (256 held out, 408 calibration), two errors.
The record is
[`2026-10-02-standing-rule-claims/report.json`](2026-10-02-standing-rule-claims/report.json).

| Held out | #10177 (`928c40bd5b`) | This change |
| --- | --- | --- |
| Route accuracy | 0.895 | 0.886 |
| Canned precision | 100 % (73/73) | 100 % (71/71) |
| Dispatch precision | 0.955 (42/44) | 0.977 (42/43) |
| `standing.rule` predicted on a held-out row | - | 0 |

No held-out row (none is a standing instruction) was read as
`standing.rule`. The route-accuracy difference is two of 254 scored rows,
on `codebase.kb` and `general` rows this change does not touch, within the
run-to-run spread of earlier records (0.890 to 0.895). On the calibration
partition, which holds the new tune rows, route accuracy is 0.926 and
`standing.rule` 24 / 24 / 25. The `router-v1` gate's floors pass (canned
precision 1.000 against 0.98, dispatch precision 0.977 against 0.90).
Calibration fitted on the calibration partition and scored held out:
`route` ECE 0.050 → 0.029, failing `probability-v2` on confident errors
(1 → 5); `answer` ECE 0.131 → 0.062, passing it. Serving keeps calibration
off.

## The compiler

The compiler's labeled set,
[`crates/background/fixtures/compile-v1.json`](../../../crates/background/fixtures/compile-v1.json),
holds 24 requests (define, edit, pause, resume, remove, unsupported, and
one-off or off-topic) against the built-in disk rule and one rule made in
conversation (a low-disk warning). `BACKGROUND_COMPILE_EVAL=1 cargo test -p
openagents-cli --bin openagents live_compile_eval -- --ignored --nocapture`
asks hosted Jev each row's eight questions and scores the readings the
compiler acts on and the kind of result (a draft, a question, or no
change).

Hosted Jev, 2026-10-02: the readings were right on 22 of 24 rows and the
kind of result on 22 of 24. Both misses asked a question instead of acting,
never a wrong rule: "make sure there is always 100gb free on this mac"
(intent read `edit` at 0.55, below the setting) and "remove finished coder
worktrees every night" (`edit` at 0.52: the disk rule's description names
finished worktrees). "Keep my disk from filling up" names no level, so it
asks how much space to keep, as labeled. A first run, before the `rule`
options described what a cleaning rule's settings are and before idle days
read "7 days", was right on 19 of 24: "only keep 2 agent target dirs" and
"never touch ~/.openagents/pylon" read the rule as new, and "clean old
build caches first" read as a one-off.

Below `background.compile` (0.6), the compiler asks one question instead
of guessing; the stand-in-Jev tests in `crates/background`
(`phase2_tests`) hold the golden rules for the spec's examples and that
behavior.
