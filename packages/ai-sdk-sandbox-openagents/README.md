# @openagentsinc/ai-sdk-sandbox-openagents

AI SDK `HarnessV1SandboxProvider` adapter for the OpenAgents sandbox/workroom
contract.

The adapter is intentionally thin: lifecycle, files, process execution, port
ingress, and egress policy are delegated to an `openagents.sandbox.v1` client.
The OpenAgents sandbox remains the authority for public/untrusted filesystem and
network restrictions. The adapter adds the AI SDK surface, snapshot identity,
explicit agent account homes, and restricted tool views.
