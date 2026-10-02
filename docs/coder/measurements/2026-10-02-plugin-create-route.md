# Making a plugin from the chat: the route (#10177)

On 2026-10-02 the owner asked OpenAgents on the Mac, in a terminal, "Help me
make a plugin that keeps my disk from filling up. It should run in the
background on this computer and be off unless I turn it on. …" The router
chose `work.dispatch` and Coder started at once; the Episode 289 flow (what
should it do and not do, drafted tests, run them, register) never ran
([the disk cleanup plugin](../../background/2026-10-02-disk-cleanup-plugin.md#where-the-flow-fell-short)).
The fix and its design are in
[the chat router design](../design/2026-09-28-chat-router.md#making-a-plugin-on-a-computer-2026-10-02).

The `eval.author` and `work.dispatch` rubrics changed, so the route
question's digest moved: the Gym suite `chat-router-v4` and question set
`chat-router-route-v5` are regenerated, and `calibration-v2.json` is refit,
which the committed-record test requires.

## The new rows

`routes-v4.json` adds 24 rows tagged `plugin_create`
(`ROUTER_EVAL_ROWS=plugin_create`): 14 requests to make a plugin (the
owner's two phrasings, short and detailed requests, a background plugin,
Spanish, and "yes" after our "There's no plugin for that yet. Want to make
one?") labeled `eval.author` with tier `author`, and 10 ordinary coding asks
near them (a script that deletes build folders, a CLI or a VS Code extension
in the repository, editing an existing plugin's README, fixing the plugin
loader, freeing disk space on this machine) labeled `work.dispatch`. Every
new row's id falls in the tune split, so the held-out partition stays at
NIP-EVAL's 256 cases.

Phone context, against hosted Jev, all 24 rows:

| `plugin_create` rows | Before (rubric of #10170) | After |
| --- | --- | --- |
| Route accuracy | 95.8 % (23/24) | 100 % (24/24) |
| `eval.author` recall | 13/14 | 14/14 |
| Requests to make a plugin served the interview (tier `author`) | 7/14 | 13/14 |
| Dispatch precision | 10/11 | 10/10 |
| `eval.author` probability, median | 0.70 | 0.98 |

Before, every `eval.author` reading but two had `work.dispatch` within 0.5,
and "write a plugin so coder always uses our internal logging crate" went to
Coder at 0.85. After, the one request still left to the model is that row,
`eval.author` at 0.69, just under the 0.70 floor.

## The published eval

`ROUTER_EVAL_PUBLISH=1 cargo test -p coder --test router_eval live_router
-- --ignored`, 636 requests (256 held out, 380 calibration), one error. The
record is [`2026-10-02-plugin-create-claims/report.json`](2026-10-02-plugin-create-claims/report.json).

| Held out | #10170 (`ef69c18254`) | This change |
| --- | --- | --- |
| Route accuracy | 0.890 | 0.895 |
| Canned precision | 100 % | 100 % (73/73) |
| Dispatch precision | 0.977 | 0.955 (42/44) |
| `eval.author` hit / predicted / labeled | - | 8 / 8 / 8 |

The two wrong dispatch offers are `codebase.kb/017` and `clarify/001`, rows
no rubric here touches. The `router-v1` gate's floors pass (canned precision
1.000 against 0.98, dispatch precision 0.955 against 0.90). Calibration
fitted on the calibration partition and scored held out: `route` ECE 0.041
→ 0.033, failing `probability-v2` on confident errors (1 → 3); `answer` ECE
0.121 → 0.063, passing it. Serving keeps calibration off. Jev spend: one
636-row published run, plus two 24-row subset runs.

## The flow's steps

The steps after the route (scope, draft, tests, run, publish, done) are
typed and covered with a stand-in Jev in `cargo test`:
`a_request_for_a_new_plugin_starts_the_flow_on_a_computer` and
`each_step_moves_on_a_typed_outcome_or_reading` (`crates/coder`, the
worker's half), `a_plugin_step_rides_the_result_and_an_open_flow_continues`
(`coder-worker`), `an_open_plugin_flow_continues_through_work_and_commands`
(`router::policy`), and `a_plugin_is_drafted_tested_and_turned_on_through_typed_steps`
(`crates/openagents-chat`, the terminal's half).
