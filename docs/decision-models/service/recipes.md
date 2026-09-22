# Recipe library

The recipe library under `recipes/` holds versioned, reusable decision
workflows. A recipe is data, not code: it declares the typed questions a
caller sends to `POST /v1/systemone`, the deterministic composition that
combines the answers into an output, and the bounds and honesty labels a
caller needs before adopting it. The model supplies answers; the
composition rules never do. Execution permissions, permission checks, and
consequential actions stay in the host's Rust code — a recipe cannot grant
an agent anything.

## Document shape

Each file is a `openagents.recipe.v1` JSON document:

| Field | Meaning |
| --- | --- |
| `v` | The schema tag, `openagents.recipe.v1`. |
| `id`, `name`, `version`, `area`, `summary` | Identity and the declared recipe area. Bumping `version` is how a recipe changes; the `id` carries its generation. |
| `license`, `source` | The recipe's license (CC0 for the shipped set) and authorship. |
| `input` | What the caller puts in `state`, and any per-item fields for batch use. |
| `questions` | The typed question definitions — `noul`, `choice`, or `score` — with instructions and criteria. A `choice` question either fixes its options or declares `"options": "supplied"` for a caller-provided candidate set. |
| `compose` | The deterministic composition over recorded answers (below). |
| `outputs` | The result's shape and who consumes it. |
| `cost` | `questions_per_item` and `max_items_per_batch` — the declared bound a caller budgets against. |
| `uncertainty` | `abstain` and `refusal` behavior: what the caller does with a low-confidence answer and a refused call. |
| `limits` | The honest limits — what the recipe cannot see, judge, or guarantee. |
| `examples` | CLI, API, and MCP invocations for the surfaces that exist. |
| `fixtures` | Recorded answers and the composition's expected output; the verifier runs every one. |
| `evidence` | `measured`, `assessed`, or `unmeasured`. `measured` pins a report path that must exist; the shipped set is `unmeasured` until a gym suite pins numbers. |

## Composition

`compose` is a small deterministic language evaluated over the recorded
answers — each question's `answer` value and `probability`:

- `{"answer": "question"}` — yield the question's recorded answer.
- `{"all": [...]}` / `{"any": [...]}` / `{"not": expr}` — boolean
  combination over boolean positions.
- `{"gte": [{"value": "question"}, bound]}` — a score question's reported
  value against a declared floor. Floors are development-set choices, not
  global constants; callers tune them per corpus.
- `{"if": {"cond": expr, "then": expr, "else": expr}}` — branching,
  including abstention branches whose `else` yields a residual such as
  `"hold"`, `"suppress"`, or `"ignore"`.

The verifier at `crates/gym/tests/recipes.rs` loads every recipe, checks
the document shape, requires composition to reference only declared
questions, and evaluates every fixture's recorded answers to the declared
expected output. A recipe whose fixtures fail does not ship. `measured`
evidence must name a report that exists — accuracy claims are pinned, not
asserted.

## Honest limits the library states

- Transcript classification is not speech recognition — the voice-command
  recipe judges the recognizer's text and cannot recover audio the
  recognizer dropped.
- Selecting among supplied candidates is not unconstrained generation —
  routing and action-selection recipes answer `"none"`, `"hold"`, or a
  residual rather than inventing an option.
- File actions are suggestions — a mover must support dry-run and undo
  before any move is real, and consequential actions need the host's
  recorded human approval.
- Refusals and low-confidence answers report themselves; the caller's
  policy — never the model — decides the fallback.

## Areas

The shipped set covers six areas: filtering and retrieval, support and
trust, operations, agent composition, business and knowledge, and personal
workflows. The index lives in [`recipes/README.md`](../../../recipes/README.md).
