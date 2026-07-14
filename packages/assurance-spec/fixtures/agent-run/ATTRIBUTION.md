# Agent Run fixture attribution

The three files under `upstream/` are vendored unmodified from
`gokulrajaram/ProductSpec` at commit
`c7250a8` (`conformance/{valid,invalid}/*.agent-run.json`, MIT license).
They are the shape-validation compatibility oracle for Agent Run format 0.1.

The `workspace/` fixtures are OpenAgents-owned end-to-end fixtures. They bind
an Agent Run to a real ProductSpec so ingest can exercise revision, item-ID,
and optional digest cross-checking.
