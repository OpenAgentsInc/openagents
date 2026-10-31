# 196. Work with Mesh-LLM

> **Status: unimplemented — design draft.** Nothing here runs yet.

Mesh-LLM already runs public meshes. This chapter attaches a Psionic server to an existing Mesh-LLM mesh through its plugin endpoint interface.

This is interoperation, not adoption. Mesh-LLM's numerical engine is a patched llama.cpp runtime, and its code is read as untrusted source. Psionic's arithmetic stays in Coax.

## What this chapter covers

- **The plugin endpoint.** A Psionic server registers as a Mesh-LLM backend through the plugin endpoint interface at the reviewed revision `f2a7f62`.
- **Two contracts.** The server serves the OpenAI-compatible contract and the `/systemone` decision contract, so the mesh can route to it.
- **A different readout.** Mesh-LLM's causal decision readout is its own method. It is not the Base Labs slot readout that chapters 13–16 qualify, and the two are not compared as equals.
- **Payments.** Mesh-LLM runs a token-metered Lightning exchange: a small deposit first, settlement from reported usage and refusal of new work after three unpaid final invoices. Psionic interoperates through its own wallet.
- **Owner decision.** Joining a public mesh needs owner approval.

## Dependencies

- [Chapter 173](ch173-serve-a-model-with-an-honest-capability.md): the server and its capability manifest.
- [Chapter 20](ch020-serve-systemone.md): the `/v1/systemone` contract.
- [Chapter 180](ch180-observe-meter-and-price.md): usage records.
- [Chapter 188](ch188-peers-transport-and-discovery.md): peers and discovery.
- Joining a public mesh and paying or accepting payment are owner decisions.

## Done means

- A Mesh-LLM node at the pinned revision routes OpenAI-compatible and `/systemone` requests to the Psionic server, in a private test mesh.
- Answers carry the Psionic server's model identity.
- Usage reported to the exchange matches the server's own usage records.
- No Mesh-LLM code runs inside the Psionic process.

## Sources

- [Chapter plan](PLAN.md)
- [Inference across a mesh](../docs/serving/mesh.md)
