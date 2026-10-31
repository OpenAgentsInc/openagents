# 210. Speak the protocols

> **Status: unimplemented — design draft.** Nothing here runs yet.

Agents and editors already speak several protocols. This chapter connects the
harness to them, so external tools and editors can use Psionic models, and the
loop can use System One decisions as a tool.

## What this chapter covers

- An MCP client for external tools and resources. External tools enter the
  same pipeline and grants as built-in ones.
- ACP for editors, so an editor can drive a harness session.
- Native function tools over the Open Responses API, from [Chapter 79](ch079-serve-responses.md).
- System One decisions as a tool. The loop can ask Elephant or Mammoth a typed
  question through `/v1/systemone` and read the probabilities.

## Dependencies

- [Chapter 203](ch203-tools-as-typed-capabilities.md) and [Chapter 204](ch204-a-sandbox-and-approvals.md): tools and grants.
- [Chapter 79](ch079-serve-responses.md): the Responses server.
- [Chapter 20](ch020-serve-systemone.md): `/v1/systemone`.

## Done means

- An MCP tool runs through the guarded pipeline and its grant, and a call
  outside the grant is refused.
- An editor session over ACP completes a scripted task against a pinned model.
- A System One decision called as a tool returns the served model identity in
  its answer.
- Each protocol has owned schema and round-trip tests.

## Sources

- [Chapter plan](PLAN.md), Part XXI.
- [Harness notes from DeepSeek Harness](../docs/harness.md).
