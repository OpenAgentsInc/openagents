# NIP-CJ chat router fixtures

The conversation-job bodies the chat router adds to
[NIP-CJ](../../../../nips/openagents/NIP-CJ.md), as `coder::router::wire`
writes them. `the_wire_matches_the_nip_cj_fixtures` in
`crates/coder/src/router/wire.rs` checks that the code and these files agree.
The bank's digest changes with every reviewed text change, so the files name
it `chat-answers-v1@DIGEST`.

| File | Body |
| --- | --- |
| `router-request.json` | A `25900` turn that asks for the router, with its `context`. |
| `router-judgment.json` | The `27000` `judgment` feedback for a sure "What model are you?". |
| `router-result-canned.json` | The `26900` result of that turn, with its followup chips. |
| `router-offer-run-coder.json` | `offer` feedback: dispatch Coder to the connected computer. |
| `router-offer-open-screen.json` | `offer` feedback: open Account > Computers. |
| `router-offer-cli.json` | `offer` feedback: run a read-only `openagents` command after a confirm. |
