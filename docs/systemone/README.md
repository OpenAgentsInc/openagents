# System One

The contract, not any one model that answers it.

`POST /v1/systemone` takes one state and a map of typed questions, and
returns one typed answer per question with probabilities. Three
implementations live in this repository:

| Model | Where | Docs |
| --- | --- | --- |
| Jev | TypeSafe's hosted service | [`docs/jev/`](../jev/) |
| Kev | open weights, served locally | [`docs/kev/`](../kev/) |
| Lev | Apple's on-device model | [`docs/lev/`](../lev/) |

`crates/jev` is the client for all three; a caller picks by `base_url`.

This directory holds material about the category itself — other people's
implementations, cross-cutting measurement questions, and anything that is
about the contract rather than about one model behind it.

| Document | Holds |
| --- | --- |
| [`2026-09-19-laya.md`](2026-09-19-laya.md) | Laya, an open 421M decision model that targets this contract: what it is, why its benchmark table does not support what it is used for, the three things worth taking from it, and what it validates about Kev's packing. |
