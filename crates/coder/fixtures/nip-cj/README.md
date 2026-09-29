# NIP-CJ chat router fixtures

The conversation-job bodies the chat router adds to
[NIP-CJ](../../../../nips/openagents/NIP-CJ.md), as `coder::router::wire`
writes them. `the_wire_matches_the_nip_cj_fixtures` in
`crates/coder/src/router/wire.rs` checks that the code and these files agree.
The bank's digest changes with every reviewed text change, so the files name
it `chat-answers-v1@DIGEST`.

| File | Body |
| --- | --- |
| `router-request.json` | A `25900` turn that asks for the router, with its `context`, as build 20 sends it (`chat-router-v1`). |
| `router-request-v2.json` | A `chat-router-v2` turn with an open authoring interview's `draft`. |
| `router-judgment.json` | The `27000` `judgment` feedback for a sure "What model are you?". |
| `router-result-canned.json` | The `26900` result of that turn, with its followup chips. |
| `router-offer-run-coder.json` | `offer` feedback: dispatch Coder to the connected computer. |
| `router-offer-open-screen.json` | `offer` feedback: open Account > Computers. |
| `router-offer-cli.json` | `offer` feedback: run a read-only `openagents` command after a confirm. |
| `router-offer-start-eval.json` | `offer` feedback: run a published test set against its tool, on the hosted runner. |
| `router-offer-publish-eval.json` | `offer` feedback: add a result the phone holds to the Gym, after its confirmation. |
| `router-offer-open-gym-result.json` | `offer` feedback: open the person's own latest result. |
| `router-card-tool.json` | `card` feedback: a tool, its plain line, and its latest verified result (`CARD-01`). |
| `router-card-result.json` | `card` feedback: a published result (`CARD-04`). |
| `router-card-news.json` | `card` feedback: news items, each citing its event or repository path (`CARD-05`). |
| `router-card-check.json` | `card` feedback: a result waiting for a check (`CARD-06`). |
| `router-card-draft.json` | `card` feedback: the interview's draft (`CARD-02`). |
| `router-card-credit.json` | `card` feedback: awards from the XP ledger (`CARD-07`). |
| `router-card-capability.json` | `card` feedback: a capability the request calls for that isn't admitted yet, the closest admitted one, and how to add one (#9960). |

`the_eval_wire_matches_its_fixtures` checks the eval bodies, which NIP-CJ's
own writer produces and its parser reads back
(`nostr::cj_conversation`); `ROUTER_FIXTURES_WRITE=1` rewrites them.
