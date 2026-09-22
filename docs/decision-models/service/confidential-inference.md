# Confidential hosted inference

Status: **research — deferred**. This document records the threat model,
the mechanism comparison, and the feasibility decision. Nothing here
makes hosted inference confidential, and existing receipts and hash
chains do not imply this guarantee.

## Threat model

- **Adversary**: the host operator and anyone with the same access —
  host root, hypervisor control, and the operators of dependent
  services the inference path touches. Outsiders are already covered
  by TLS, authentication, and tenancy authorization; this workstream is
  about the *host itself*.
- **Protected data**: request `state`, question wordings, and the raw
  answers — the content of a decision, not the fact that a call
  happened.
- **Metadata leakage**: request timing, size, rate, and tenant
  association are observable to the host even under any honest
  mechanism; the threat model accepts metadata visibility and claims
  only content protection.
- **Trust roots**: for attestation mechanisms, the silicon vendor's
  attestation chain; for local execution, the caller's own hardware.
- **Key ownership and revocation**: caller-held keys, provisioned and
  revocable without operator cooperation; a key the operator can read
  is not caller-held.
- **Side channels**: memory disclosure, swap, logs, crash dumps, and
  operator-visible debugging are in scope; physical and microarchitectural
  attacks against the confidential-computing hardware itself are assumed
  covered by the vendor's attestation, and documented as an assumption.
- **Availability**: a confidential lane must fail closed — when
  attestation or key handling cannot complete, the call is refused
  rather than silently served on an ordinary lane.

## Mechanism comparison

| Mechanism | Protects content from host? | Feasibility today |
| --- | --- | --- |
| TLS + single-tenant process | No — the operator reads process memory and terminates the TLS endpoint | Already true; explicitly not this guarantee |
| Dedicated hosting | No — dedicated capacity narrows *which* host, not whether that host's operator can read | Cannot satisfy the threat model |
| On-device / local execution | Yes — the host is the caller's | Available now as the local lane (Kev, Laya, Lev); the documented alternative |
| Confidential computing (TEE attestation) | Yes — remote attestation binds model + configuration to caller-verified evidence before input is released | Requires TEE hardware the service does not operate, an attestation verification stack, and the independent review below |
| Cryptographic inference (FHE/MPC) | Yes in principle | Not practical at usable latency for the models served today |

TLS, authentication, and dedicated capacity are admission and transport
properties — they were never claimed as host-blind execution and are
not evidence for it.

## Feasibility decision

**Deferred — keep unavailable.** The only hosted mechanism that meets
the threat model is confidential-computing attestation, and standing it
up requires hardware the service does not operate, a verification
stack that does not exist, and independent protocol and security review
before any public claim. Until those are funded and pass review, hosted
inference is not confidential and no surface may claim or imply it is.

The documented alternative is the **local lane**: a caller that needs
host-blind execution runs Kev, Laya, or Lev on its own hardware — the
deployment lanes already publish that boundary.

## What a future proof of concept must show

If an experiment budget is authorized, a PoC must demonstrate, against
the threat model above:

- Model and configuration binding: the attested measurement names the
  exact artifact and configuration the caller verified.
- Attestation verification on the caller side, not the operator's word.
- Sealed key handling — input released only to the attested enclave —
  with upgrade, revocation, and failure behavior stated.
- Measured accuracy equivalence where claimed, latency, throughput,
  resource cost, supported models, and operational limits against the
  ordinary lane, pinned in a report.
- Independent protocol and security review **before** any public
  confidentiality or remote-attestation claim. Internal hash chains and
  provider-signed receipts are not this guarantee and are never cited
  as it.

## Carve-outs

- Execution receipts remain attributable claims about what ran, never
  remote attestation — #9471's receipt schema is unchanged and unchanged
  in meaning.
- Tenant identity, quota, and reservation contracts are unchanged; a
  confidential lane is a capacity property, not an authorization
  shortcut.
- Local/on-device privacy is the existing, documented alternative — not
  a consolation, the mechanism that already meets this threat model.
