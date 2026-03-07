# Pylon Sandbox Execution Contract

Status: ratified for implementation planning  
Date: 2026-03-07  
Parent issue: `#3125`

This document ratifies the taxonomy, profile model, contract shape, receipt requirements, and safety boundary for bounded `sandbox_execution` as the next planned compute-family extension for Pylon.

It does not change the current repo-wide launch truth for the OpenAgents Compute Market. The current live launch families remain `inference` and `embeddings`. `sandbox_execution` is the next planned compute family for the Pylon extension track, and it must not be implemented or described in a way that blurs Compute and Labor.

## Market Position

The market remains the `OpenAgents Compute Market`.

The category model is:

- `compute` = umbrella market category
- live launch families today = `inference`, `embeddings`
- next planned Pylon family = `sandbox_execution`

For this family, compute means machine-verifiable execution capacity offered under explicit runtime, policy, and settlement constraints. It does not mean arbitrary host access and it does not mean open-ended agent work.

## Compute Versus Labor Boundary

The hard line is contractual, not stylistic.

`sandbox_execution` belongs in Compute when the buyer submits a bounded thing to run inside a declared runtime or sandbox and the provider can execute it under explicit limits with receiptable evidence. The provider is selling constrained execution capacity, not judgment.

The same request belongs in Labor when the buyer is asking the provider to interpret an objective, decide what to do, iterate with open-ended tool use, or deliver a semantic outcome whose success depends on discretionary planning rather than declared execution rules.

Operational rule:

- If the request is `run this bounded thing in this declared sandbox`, it is Compute.
- If the request is `figure out what to do and do it`, it is Labor.

Requests must be rejected or rerouted to Labor when they require any of the following:

- open-ended planning or task decomposition
- tool choice not declared by the execution contract
- undeclared host access
- interactive operator judgment during execution
- ambiguous outcome evaluation that cannot be reduced to declared artifacts, exit conditions, or bounded verification rules

Examples that stay in Compute:

- run a declared OCI image with explicit arguments and an allowlisted artifact directory
- run a Python script in a declared Python runtime with declared package inventory and no undeclared network
- run a Node build or transform job with declared inputs and expected outputs
- run a bounded POSIX command against a declared workspace with an allowlisted binary set

Examples that belong in Labor:

- fix this repo and decide what changes are needed
- investigate a bug, choose a strategy, and ship a patch
- research the best approach and produce a finished deliverable
- operate a browser or toolchain interactively until the high-level objective is satisfied

## Initial Sandbox Execution Taxonomy

The initial taxonomy stays intentionally narrow. Each execution class must be independently matchable, policy-constrained, and receiptable.

| Execution class | Runtime shape | Intended use | Settlement basis |
| --- | --- | --- | --- |
| `sandbox.container.exec` | Declared OCI/container runtime with immutable image or image digest | bounded image-based execution | exit condition, output digests, artifact set, resource summary |
| `sandbox.python.exec` | Declared Python runtime and package inventory | bounded Python script or batch transform execution | exit condition, output digests, artifact set, resource summary |
| `sandbox.node.exec` | Declared Node runtime and package inventory | bounded JS/TS execution or build-style jobs | exit condition, output digests, artifact set, resource summary |
| `sandbox.posix.exec` | Declared non-interactive POSIX command surface with allowlisted binaries | bounded shell-style command execution | exit condition, output digests, artifact set, resource summary |

Optional later classes may exist, but they are not part of the initial ratified set. Examples include `sandbox.wasm.exec` or more specialized build/test classes. Those should not be described as live until they have explicit profiles, policy, receipts, and runtime support.

## Sandbox Profile Model

Every sandbox execution offer must carry a first-class sandbox profile. The profile is the contract that tells the buyer, provider substrate, operator, and verifier what can actually run and under what limits.

Required profile fields:

- `profile_id`
- `profile_digest`
- `execution_class`
- `runtime_family`
- `runtime_version`
- `sandbox_engine`
- `os_family`
- `arch`
- `cpu_limit`
- `memory_limit_mb`
- `disk_limit_mb`
- `timeout_limit_s`
- `network_mode`
- `filesystem_mode`
- `workspace_mode`
- `artifact_output_mode`
- `secrets_mode`
- `allowed_binaries`
- `toolchain_inventory`

Conditionally required profile fields:

- `container_image` when the execution class is container-based
- `runtime_image_digest` when the runtime environment is image-backed or otherwise digestible
- `accelerator_policy` when GPU or other accelerator pass-through is allowed

Profile rules:

- the profile must be stable enough to hash and reference from receipts
- the profile must be truthful enough to drive matching and rejection decisions
- the profile must be narrow enough that a verifier can compare promised versus delivered execution conditions
- the profile must not hide privileged modes behind generic names

## Capability Envelope Extensions

The capability envelope remains the place where machine, runtime, and accelerator traits are expressed. Product identity stays in the execution class. Hardware details refine supply; they do not replace the primary product identity.

For sandbox supply, the capability envelope must be able to express at least:

- `compute_family = sandbox_execution`
- `execution_kind = sandbox_execution`
- `sandbox_profile`
- `os_family`
- `arch`
- `cpu_cores`
- `memory_gb`
- `disk_gb`
- `accelerator_vendor`
- `accelerator_family`
- `network_posture`
- `filesystem_posture`
- `toolchain_inventory`
- `container_runtime`
- `max_timeout_s`
- `concurrency_posture`

Accelerator fields are optional refinements. A buyer may request `sandbox.container.exec` plus capability-envelope constraints such as GPU vendor or memory floor, but the launch product identity is still the execution class, not raw hardware trading.

## Job Contract Shape

The sandbox job contract must be explicit enough that execution is bounded and non-interpretive. The provider should not have to infer what the buyer meant.

Required job-contract fields:

- `execution_class`
- `entrypoint_type`
- `payload_ref` or embedded payload
- `arguments`
- `environment_policy`
- `resource_request`
- `timeout_request_s`
- `network_request`
- `filesystem_request`
- `expected_outputs`
- `artifact_policy`
- `determinism_posture`

Optional but expected fields for many profiles:

- `input_artifacts`
- `cache_policy`
- `accelerator_requirement`
- `toolchain_requirement`
- `region_requirement`
- `verification_requirement`

Job-contract rules:

- the request must be self-contained enough to run without discretionary planning
- requested resources must fit inside a declared sandbox profile or be rejected
- requested filesystem and network access must be declarative and auditable
- expected outputs must be specific enough for receipt and verification logic
- interactive sessions, long-lived daemons, or hidden follow-up work are out of scope unless a future profile explicitly allows them

## Receipt And Delivery Evidence Requirements

Sandbox execution receipts must keep this family inside Compute rather than letting it slide into unstructured task work. At minimum, the receipt and delivery-evidence layer must capture:

- provider identity
- compute product ID
- sandbox profile ID or digest
- runtime image or environment digest
- job input digest
- command or entrypoint digest
- start time
- end time
- exit code or termination reason
- stdout digest
- stderr digest
- artifact digests
- resource usage summary
- payout linkage
- verification or attestation posture

Evidence rules:

- large stdout or stderr outputs should be represented by digest, size, and optional retained artifact refs rather than unbounded inline logs
- artifact sets must be explicit and attributable to declared output paths
- termination must distinguish clean exit, timeout, policy kill, resource kill, and runtime failure
- if promised profile and observed profile differ materially, the delivery evidence must record that variance explicitly

## Hard Safety And Policy Restrictions

The following are prohibited for the initial sandbox execution family unless a later contract explicitly changes the rule:

- privileged containers
- host root mounts
- undeclared filesystem access
- undeclared network access
- hidden secrets injection
- arbitrary long-lived daemonization
- silent persistence outside declared workspace or artifact paths

The implementation must also preserve these policy principles:

- default-deny for filesystem, network, and secrets
- explicit allowlists for binaries, runtimes, and profile classes
- explicit timeout and termination behavior
- explicit artifact ownership and retention rules
- explicit variance and remedy recording when delivered execution conditions differ from the promised profile

## Open Policy Questions Before Runtime Work

The following questions must be settled before `#3126` and `#3127` can be closed honestly:

- Which sandbox engines are allowed for v1, and which of the ratified execution classes are actually enabled first?
- Is any egress allowed at v1, and if so, is it allowlist-only or broader policy-driven access?
- Are accelerators or GPU pass-through allowed for sandbox execution at all in the first implementation, and under what extra profile fields?
- Are secrets ever injectable for v1 sandbox jobs, or must all initial profiles run with `secrets_mode = none`?
- Are long-running jobs or streamed logs allowed, or is the first implementation strictly bounded batch execution?
- What are the maximum artifact sizes, retention rules, and verifier access rules for sandbox evidence?

Until those decisions are made, implementation work should default to the more restrictive interpretation.
