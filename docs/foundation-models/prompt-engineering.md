#+ Prompt Engineering Basics (FM)

Concise prompts plus well‑scoped instructions yield reliable results.

## Instructions vs Prompt

- Use `Instructions` for persona, scope, safety, formatting rules, and recurring examples
- Use per‑turn prompt for the specific request

## Best Practices

- Control output length: specify words/characters for UI surfaces
- Specify roles/context: what the model is, what sources it should rely on
- Write clear commands: imperative, unambiguous asks
- Provide examples: few‑shot patterns in `Instructions` for consistency
- Use strong commands when necessary: “Do not speculate. If unknown, say ‘I don’t know.’”

## OpenAgents Guidance

- Centralize instruction templates per surface (plan, analysis, summaries)
- Reference available tools in instructions so the model knows to use them
- Keep prompts short; move formatting/structure into `@Generable` when possible

