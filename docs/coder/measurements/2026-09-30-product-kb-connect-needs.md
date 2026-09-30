# What connecting needs routes to product knowledge (#9997)

A bare "Do I need Tailscale?" was judged `general` by the router, and the
model explained Tailscale in general; "Do I need Tailscale to connect my
Mac?" already reached `openagents.tailnet@2` ("No, you don't need
Tailscale…"). The `product.kb` option's rubric in
`crates/coder/src/router/rubric.rs` now covers connecting a phone or a
computer and what connecting needs, such as whether Tailscale or another
tool is required, and its `not_for` sends what another company's product
is or costs to `general`. No example was added and no held-out row was
read to write it; routing stays the Jev `route` Choice.

The route question changed, so the set is `chat-router-v3@c86d4a2ebeb2`
(was `@a19f8c201312`); `crates/gym/questions/chat-router-route-v4.json` is
regenerated (`ROUTER_SUITE_WRITE=1`, `tests/router_suite.rs`), and
`calibration-v2.json` is refit for it.

## The published eval, before and after

Both runs on 2026-09-30 against hosted Jev (`TYPESAFE_API_KEY` from
`typesafe.env`), `ROUTER_EVAL_PUBLISH=1 cargo test -p coder --test
router_eval live_router -- --ignored`, the same labeled set
(`routes-v3.json`), bank `chat-answers-v1@a04859020455`, 0 errors. Before
is the old rubric (`a19f8c201312`), after the new one (`c86d4a2ebeb2`).

| Split | Metric | Before | After |
|---|---|---|---|
| Held out (207) | route accuracy | 0.889 | 0.889 |
| | canned precision | 100 % (63/63) | 100 % (62/62) |
| | dispatch precision | 0.957 (22/23) | 0.957 (22/23) |
| | Gym precision | 0.966 (28/29) | 0.966 (28/29) |
| | abstained | 48 | 51 |
| | `product.kb` hit / predicted / labeled | 17 / 21 / 22 | 16 / 19 / 22 |
| | `general` hit / predicted / labeled | 9 / 17 / 9 | 9 / 18 / 9 |
| Calibration (264) | route accuracy | 0.917 | 0.917 |
| | canned precision | 100 % (75/75) | 100 % (74/74) |
| | dispatch precision | 1.000 (19/19) | 1.000 (19/19) |
| | Gym precision | 1.000 (56/56) | 1.000 (57/57) |
| | `codebase.kb` hit / predicted / labeled | 11 / 11 / 14 | 12 / 12 / 14 |

Rows whose route changed, across both partitions: three moved to the
labeled route (two `codebase.kb` questions that went to `product.kb`, and
`cli/020`), one held-out `product.kb` row moved to `general` ("how do I
level up faster"; asked three more times with the new rubric it read
`product.kb` twice and `general` once, a boundary Jev is not stable on),
and one tune row moved between wrong routes. The rest are tier changes
from probability wobble.

Calibration maps (fitted on the calibration partition, scored held out):

| Question | Before | After |
|---|---|---|
| `route` | ECE 0.040 → 0.041, Brier 0.077 → 0.078; probability-v2 failed (map refused) | ECE 0.040 → 0.041, Brier 0.079 → 0.080; probability-v2 failed (map refused) |
| `answer` | ECE 0.129 → 0.042, Brier 0.155 → 0.135; passed | ECE 0.129 → 0.040, Brier 0.154 → 0.128; passed |

Serving keeps calibration off (`CODER_WORKER_ROUTER_CALIBRATION`), as
before.

## The asked messages

With the new rubric, one request each against hosted Jev:

| Message | Before | After |
|---|---|---|
| Do I need Tailscale? | general | product.kb |
| Do I need Tailscale to connect my Mac? | product.kb | product.kb |
| How do I connect a phone | product.kb | product.kb |
| What is Tailscale? | general | general |
| What is Tailscale's pricing? | general | general |
| is tailscale free | — | general |
| how does tailscale work | — | general |
| Write a haiku about rain | general | general |
| What's a tool? | — | product.kb |
