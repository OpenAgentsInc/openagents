# Gym news first words (#9950), 2026-09-29

"What's new in the Gym?" on the deployed chat worker showed Thinking for 7
to 12 s before any words
([#9950](https://github.com/OpenAgentsInc/openagents/issues/9950)). The
lo-fi threshold LF-G4 wants the first words of an answer that is not
prepared under 2 s. This page says where the time went, what changed, and
the live numbers before and after.

## Where the time went

`crates/coder/examples/gym_news_profile.rs` times each stage of a
`gym.news` turn on the worker VM under the chat worker's environment
(`systemd-run -p EnvironmentFile=/etc/coder-worker/coder-worker-chat.env`),
with the live records (21 verified results, 4 test sets, 35 items). It
prints durations and counts only.

| Stage | Time |
| --- | --- |
| Route judgment (Jev, `chat-router-v2`), from the worker log | 0.17 to 0.35 s |
| Message embedding (Vercel AI Gateway, OpenAI embeddings) | 0.29 to 0.89 s |
| Embedding and the relevance judgment together (`GymKnowledge::news`) | 0.44 to 0.65 s |
| Gemini 3.8 Flash, default reasoning, to its first words | 6.3 to 12.0 s (10 runs) |
| Gemini 3.8 Flash, reasoning `low` | 0.9 to 7.0 s |
| Gemini 3.8 Flash, reasoning `minimal` or `none` | 9.2 to 46 s |
| Gemini 2.5 Flash, reasoning `none`, to its first words | 0.56 to 0.90 s (11 runs) |
| Gemini 2.5 Flash, reasoning `none`, to its end | 1.1 to 2.3 s |
| Gemini 2.5 Flash Lite, to its first words | 0.53 to 1.55 s |

The records, the relay read, and citation tidying cost nothing a person
could see: the records are read every 10 minutes in the background, and
`router::gym::Tidy` holds back only an open bracket. Nearly all of the wait
was the chat model thinking before its first token. Every variant cited
only the items it was given (0 invented) with no banned word and no raw id
after tidying; Gemini 2.5 Flash read as well as the chat model, and Flash
Lite gave shorter, vaguer replies, so the lane is Gemini 2.5 Flash.

## What changed

- The first words are the bank's `gym.news.lead` ("Here's what's new in
  the Gym."), sent with the news card as soon as Jev keeps the items.
- The grounded reply runs on `router::gym::NEWS_MODEL`
  (`google/gemini-2.5-flash`), reasoning off, at most 800 output tokens,
  through the chat door's own gateway and key; the privacy answer names it.
  `CODER_GYM_NEWS_MODEL` names another model; `off` keeps the chat model.
- The model call started with the turn is dropped once the route is a Gym
  route, since a Gym reply never shows it.
- `router gym reply` now logs when the records, the model's first words,
  and the end arrived.

## Live, before and after

`live_basic_coder_streams_a_reply` with `OPENAGENTS_TEST_CHAT_MESSAGE="What's
new in the Gym?"`, ten runs each, each from a fresh key through
`relay.openagents.com`, timed on the test machine from Send.

| Release | First words p50 | p90 | max | Done p50 | p90 | max |
| --- | --- | --- | --- | --- | --- | --- |
| `d0a053650d` (before) | 8.14 s | 11.01 s | 11.52 s | 9.03 s | 12.06 s | 12.20 s |
| `dbad257c51` (after) | 1.15 s | 1.46 s | 2.53 s | 2.50 s | 2.85 s | 3.80 s |

The 2.53 s run was the first after the restart, on cold connections; the
worker logged its records at 2.0 s against 0.67 to 0.97 s for the rest. On
the worker, after the restart, the records were in at 0.67 to 0.97 s, the
model's first words at 1.36 to 1.77 s, and the end at 1.77 to 2.35 s. All
ten replies logged `0 invented, banned [], 0 raw ids`, with 2 to 4 items
cited.

Other routes on `dbad257c51`: "Who are you?" answered with `meta.who` in
0.65 s; "Test Project map on Coder" with the tool card and `start_eval` in
0.66 s (done 0.74 s); "What is a Nostr relay, in one sentence?" on the
chat model with its opener at 0.98 s, done 3.9 s; "What happens to my
messages?" with `meta.privacy`, naming Gemini 2.5 Flash for Gym news, in
0.62 s.
