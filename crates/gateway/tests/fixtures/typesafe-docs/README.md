The request examples from TypeSafe's documentation
(`https://docs.typesafe.ai/primitives/advanced.md`, retrieved 2026-09-30),
each as a `{state, questions}` pair. `tests/serve.rs`
(`the_decisions_api_answers_the_typesafe_docs_examples`) sends each one to the
gateway's `POST /v1/systemone` as OpenRouter's Decisions API would receive it.
