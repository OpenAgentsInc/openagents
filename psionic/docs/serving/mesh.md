# Inference across a mesh

Planning note, 2026-10-29, for Part XX of the
[book plan](../../book/PLAN.md). It carries forward two documents
from before the reset and adds a review of Mesh-LLM at its current revision.
Nothing was installed, joined or run.

## Sources

- **The earlier Mesh-LLM review.** Read it with
  `git show 0b401561a0^:docs/psionic/mesh-llm-lessons.md`. It reviewed
  Mesh-LLM at
  [`d059e25`](https://github.com/Mesh-LLM/mesh-llm/tree/d059e25055d42df3b319a3dd462aaed6bdd95cc9)
  (October 20, 2026) against the old Rust Psionic. That Rust implementation
  is gone; the lessons remain.
- **The earlier private-inference design.** Read it with
  `git show 0b401561a0^:docs/security/private-inference.md`. It covered
  Darkbloom, Tinfoil, trust levels and the NIP-ATT attested endpoint.
- **The old DeepSeek V4.1 plan.** Read it with
  `git show 0b401561a0^:docs/psionic/deepseek-v41-native.md`. Its stage
  split listed the typed state that must cross a boundary in a model with
  shared layers.
- **Mesh-LLM today.** Revision
  [`f2a7f62`](https://github.com/Mesh-LLM/mesh-llm/tree/f2a7f62c755e283c389a28cbf8d1e8f926677a4e),
  Apache-2.0. It is 74 commits past the earlier review, and its code was read
  as untrusted source.

## Lessons that carry over

From the earlier review, restated as Psionic rules (chapters 123–130):

1. **A split must match the whole model.** Every split needs logits, greedy
   output and cached continuation to match the whole model on identical
   tokens. A placement plan is not evidence of a correct boundary.
2. **A boundary carries everything the model needs.** It can include
   residual streams, recurrent and shared-KV sidebands, and state from shared
   layers, not only a hidden vector. Each is a typed part bound to the
   artifact, stage range and protocol version. The DeepSeek plan's list (four
   residual streams, KV and indexer source layers, chosen candidate blocks,
   Engram output) shows how much this can be. Mistral Large 4's layer-sharing
   map (in [mistral.md](../targets/mistral.md)) has the same character.
3. **Stages own packages.** A stage fetches only its own tensors and the
   shared assets it needs, each verified against its digest.
4. **The route waits for readiness.** Load downstream stages first, publish
   the entry route only when every stage is ready, and fence each topology
   generation so a restarted stage cannot serve a stale request.
5. **Placement states its objective.** For one request's latency the cost is
   the sum of stage times; for throughput it is the slowest stage. Link
   transfers are charged. Splitting avoids per-layer collectives; it does not
   remove stage traffic.
6. **Route by saved work.** Cache advertisements are hints: bounded,
   short-lived and salted, never raw tokens, and a claimed hit that has gone
   falls back.
7. **Compression is exact or a separate profile.** Exact compression is
   negotiated with a raw fallback. Lossy formats are a separate profile.

## What changed in Mesh-LLM

At the current revision Mesh-LLM, among other things:

- runs a model on one machine when it fits, routes the whole request to a
  peer that holds it, and splits only when neither works;
- encrypts peer traffic end to end over QUIC, with relays forwarding
  ciphertext;
- discovers published meshes through Nostr, while private meshes stay
  invitation-only;
- separates an owner control lane from the public inference plane;
- serves Whisper speech recognition, including live transcription;
- serves the `/systemone` and `/v1/decisions` decision contracts from causal
  models ([`skippy/docs/CAUSAL_DECISIONS.md`](https://github.com/Mesh-LLM/mesh-llm/blob/f2a7f62c755e283c389a28cbf8d1e8f926677a4e/skippy/docs/CAUSAL_DECISIONS.md));
- runs a token-metered Lightning payment exchange: a small deposit first,
  settlement from the backend's reported usage, and refusal of new work after
  three unpaid final invoices. The spec calls its wallet integrations
  examples ([`mesh/docs/specs/lightning-payments.md`](https://github.com/Mesh-LLM/mesh-llm/blob/f2a7f62c755e283c389a28cbf8d1e8f926677a4e/mesh/docs/specs/lightning-payments.md));
- attaches backends through a plugin endpoint interface. Flash-MoE, for
  example, streams experts of a giant MoE from local NVMe on a single node
  ([`mesh/docs/plugins/flash-moe.md`](https://github.com/Mesh-LLM/mesh-llm/blob/f2a7f62c755e283c389a28cbf8d1e8f926677a4e/mesh/docs/plugins/flash-moe.md)).

Mesh-LLM's numerical engine is a patched llama.cpp runtime. Psionic's stays
in Coax. That makes chapter 133 an interoperation chapter: a Psionic server
behind Mesh-LLM's plugin endpoint, serving the same OpenAI-compatible and
decision contracts. It does not adopt the runtime. Note that Mesh-LLM's causal
decision readout is its own method. It is not the Base Labs slot readout that
chapters 9–12 qualify.

## Trust: what Darkbloom and the earlier design taught

Darkbloom (Layr-Labs/d-inference) is under a proprietary licence. The
earlier review took ideas only, and so does this one. Its lessons, kept for
chapters 131–132:

- **Process hardening is not an enclave.** A hardened process on a Mac its
  owner controls is harder to snoop on. It is not private, because root can
  read its memory.
- **A middle that reads plaintext breaks end-to-end claims.** If a
  coordinator decrypts requests to route or bill them, no downstream claim of
  privacy holds.
- **Weight checks must fail closed.** Verifying the weights' hash is only
  worth something if a mismatch stops serving.
- **Attestation is not correctness.** It says which code ran, not that the
  answer is right.

The earlier design's trust levels survive: any peer, a hardened machine, and
an attested enclave running a logged release. Requests are routed by how
sensitive they are and fail closed. A peer that serves a request reads it,
so encrypted transport protects the link, not the data from that peer.

## Where Psionic can go further

**Check work by re-executing it.** Under a fixed backend profile, Psionic's
fixed reduction order and batch invariance (chapter 111) make outputs
reproducible bit for bit. A trusted node can therefore re-run a sampled
fraction of a peer's requests and compare exactly, rather than trusting
reported usage (chapter 132). Across different backends the comparison uses
that profile's stated tolerance, and an inexact match is evidence, not proof.

## Decisions that belong to the owner

These go in `NEEDS_OWNER.md` when a chapter reaches them:

- joining or publishing a public mesh;
- paying peers or accepting payment;
- the Nostr event kinds and NIP for discovery;
- offering Mistral Large 4 to others, which needs Mistral's commercial
  licence;
- the hardware a mesh runs on.
