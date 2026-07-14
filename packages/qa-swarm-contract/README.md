# QA Swarm contract

`@openagentsinc/qa-swarm-contract` is the shared Effect Schema boundary for
QA Runner run projections and the OpenAgents web consumer.

Artifact and receipt fields describe observed, public-safe evidence only. A
receipt-looking string is not evidence admission. Board edges are derived with
`buildResolverBackedQaSwarmBoardGraph`, which lights an edge only after the
supplied resolver admits that exact receipt. Missing, rejected, or unavailable
resolution remains `inconclusive` with blocker refs.
