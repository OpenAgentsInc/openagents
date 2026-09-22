# Decision recipes

Versioned, reusable decision workflows for `POST /v1/systemone`. Each
`*.recipe.json` is an `openagents.recipe.v1` document: typed questions,
deterministic composition over the answers, declared cost bounds,
uncertainty behavior, honest limits, and fixtures the verifier runs. The
contract is [`docs/decision-models/service/recipes.md`](../docs/decision-models/service/recipes.md).

| Area | Recipes |
| --- | --- |
| Filtering and retrieval | [bulk-filter](bulk-filter.recipe.json), [headline-feed](headline-feed.recipe.json), [retrieval-rerank](retrieval-rerank.recipe.json), [citation-check](citation-check.recipe.json) |
| Support and trust | [ticket-triage](ticket-triage.recipe.json), [content-moderation](content-moderation.recipe.json), [listing-abuse](listing-abuse.recipe.json), [prompt-injection](prompt-injection.recipe.json) |
| Operations | [log-classification](log-classification.recipe.json), [security-alert-triage](security-alert-triage.recipe.json), [semantic-lint](semantic-lint.recipe.json) |
| Agent composition | [model-routing](model-routing.recipe.json), [computer-action](computer-action.recipe.json) |
| Business and knowledge | [lead-qualification](lead-qualification.recipe.json), [resume-competency](resume-competency.recipe.json), [knowledge-relations](knowledge-relations.recipe.json), [spec-conformance](spec-conformance.recipe.json) |
| Personal workflows | [document-intake](document-intake.recipe.json), [file-sorting](file-sorting.recipe.json), [voice-transcript](voice-transcript.recipe.json) |

Verify the library with `cargo test -p gym --test recipes`.
