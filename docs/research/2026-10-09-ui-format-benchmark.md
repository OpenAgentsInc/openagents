# Interactive answers: the format benchmark

Issue #11113 chose the OpenUI Lang subset (`crates/openui-lang`) as the wire
format for interactive answers "pending a small check": OpenUI's published
benchmark says a line format and nested JSON are about equally valid, with
the line format at about half the output tokens. Their numbers are
first-party, on their catalog and their models
(`2026-10-09-openui-adaptation.md`). This benchmark runs the check on ours.

## What it compares

| | OpenUI Lang subset | Minified nested JSON |
| --- | --- | --- |
| Block | ```` ```openui-lang ```` | ```` ```ui-json ```` |
| Shape | one statement per line, `name = Component(args)`, forward references | one JSON object, the root component; children nested in arrays |
| Prompt | `openui_lang::prompt()`, the one the chat worker sends | `coder::ui_format::json_prompt()`, the same catalog, components, arguments, and descriptions written as objects |

Both prompts open with the same lead rule ("a one-line lead, then the
interface"). The prompts the models answer are in
`bench/ui-format/prompts-v1.json`: twelve tasks the catalog can draw (steps
with commands, choices side by side, OS tabs, link lists), half about
OpenAgents and half general.

## How a reply is scored

Both formats go through the same validator. A JSON block is turned into one
OpenUI Lang statement (`coder::ui_format::json_to_lang`: an object's `type`
is the component, its other keys are named arguments) and parsed by
`openui_lang::parse`, so the only thing that differs is what the model
wrote.

| Measure | Meaning |
| --- | --- |
| valid | a closed block that draws something with nothing fixed or dropped |
| blank | nothing to draw: no block, a JSON block that does not parse, or no root |
| fixes | what the validator fixed or dropped (unknown components or arguments, bad choices, unsafe links, missing required arguments, undefined names) |
| output tokens | as the door reports them; block characters too, for doors that don't |
| first render | the first moment the streaming reply could draw something: an OpenUI Lang block under the streaming rules (`parse_partial`), a JSON block only once it parses whole, since nested JSON has no finished part before its last brace |
| total | the whole reply |

Each prompt goes to each model `--runs` times (default 3) in each format;
the formats alternate within a run, so a provider's drift falls on both.

## Running it

```sh
scripts/ui-format-bench.sh --model google/gemini-3.8-flash --model zai/glm-5.3-flash
scripts/ui-format-bench.sh --model google/gemini-3.8-flash --runs 1 --prompt os_tabs
```

The door is `CODER_DOOR_URL` (the public gateway by default) with
`CODER_DOOR_KEY`, `CODER_AI_GATEWAY_KEY`, or `AI_GATEWAY_API_KEY` (read from
`ai-gateway.env` in `$OPENAGENTS_SECRETS` when unset). It prints a table per
model and format and writes the JSON report, every run's scores and times,
to `bench/ui-format/results/<unix>.json`. Replies are kept in the report
only with `--keep-replies`.

## Deciding

Keep the OpenUI Lang subset when, on the models the chat routes to, its
valid share is within a few points of JSON's and it has no more blank
screens; its smaller output and earlier first render are then the reason to
keep it. Switch to JSON only if the line format is clearly less valid on
our models, after one round of catalog prompt fixes (never tuned prompt by
prompt).

## Results

Not measured yet. A first run on 2026-10-10 (`--model google/gemini-3.8-flash
--model zai/glm-5.3-flash --runs 1`, the house door) got no replies: every
call answered HTTP 402 because the Vercel AI Gateway account has no credit
left. Rerun the command above once it is topped up; the first real run goes
here: date, models, runs, the table, and the decision.
