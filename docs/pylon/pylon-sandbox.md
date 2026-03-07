Yes. The clean extension is:

**Pylon should support sandbox execution as a first-class compute family, not as a backdoor into labor or raw host access.**

That lets you broaden “compute” beyond inference/embeddings while keeping the market honest.

## Core model

Define compute as:

> **machine-verifiable execution capacity offered under explicit capability, sandbox, and settlement constraints**

Then your first live compute families become:

* `inference`
* `embeddings`
* `sandbox_execution`

That works because sandbox execution is still compute if the buyer is purchasing **bounded execution in a declared runtime**, not open-ended human-like problem solving.

## Critical boundary

You need one hard distinction in the spec:

### Compute

The buyer submits:

* code
* command
* container
* script
* test bundle
* transform job
* batch job

and the provider executes it inside a declared sandbox with explicit limits and receipts.

### Labor

The buyer submits:

* an outcome to figure out
* an ambiguous coding task
* an open-ended agent mission
* iterative tool use with judgment and planning

That belongs in the Labor market, not Compute.

A good rule:

> If the request is “run this bounded thing in this sandbox,” it is compute.
> If the request is “figure out what to do and do it,” it is labor.

---

# Spec: Pylon with Sandbox Execution

## Summary

This spec extends Pylon from a narrow inference/embedding connector into a broader standalone provider binary for the OpenAgents Compute Market.

Pylon still remains a **narrow provider connector**, but it now supports three launch compute families:

* `inference`
* `embeddings`
* `sandbox_execution`

The new `sandbox_execution` family allows providers to sell bounded machine execution inside declared sandbox profiles. It does **not** turn Pylon into a host-mode agent platform, a labor marketplace, or arbitrary host access broker.

The purpose of this extension is to let any compatible machine contribute:

* local model inference
* local embedding generation
* bounded sandbox execution capacity

through one consistent provider lifecycle, inventory model, receipt model, and payout path.

## Product framing

The correct framing is still:

* `Autopilot` = primary user-facing product
* `Pylon` = standalone supply connector
* `Nexus` = authority / control plane

The market is still the **OpenAgents Compute Market**.

The compute families now become:

* `inference`
* `embeddings`
* `sandbox_execution`

## What sandbox execution means

`sandbox_execution` means:

* bounded machine execution
* explicit runtime contract
* explicit resource limits
* explicit filesystem policy
* explicit network policy
* explicit artifact policy
* explicit timeout and termination policy
* machine-verifiable receipts

It does **not** mean:

* arbitrary host access
* privileged execution
* hidden secrets access
* open-ended autonomous problem solving
* labor-style agent tasks disguised as compute

## Product identity vs capability envelope

Keep the same split:

### Product identity

Tradable compute product family and runtime shape.

Examples:

* `ollama.text_generation`
* `ollama.embeddings`
* `apple_foundation_models.text_generation`
* `sandbox.container.exec`
* `sandbox.python.exec`
* `sandbox.node.exec`
* `sandbox.posix.exec`
* `sandbox.wasm.exec` if supported later

### Capability envelope

Describes the machine and runtime characteristics.

Examples:

* `backend_family`
* `execution_kind`
* `sandbox_profile`
* `os_family`
* `arch`
* `cpu_cores`
* `memory_gb`
* `disk_gb`
* `accelerator_vendor`
* `accelerator_family`
* `network_posture`
* `filesystem_posture`
* `toolchain_inventory`
* `container_runtime`
* `max_timeout_s`
* `concurrency_posture`

Accelerators remain capability data, not the primary product identity.

## Sandbox execution taxonomy

The new compute family should be modeled as standardized execution classes.

### Initial execution classes

Recommended first set:

* `sandbox.container.exec`
* `sandbox.python.exec`
* `sandbox.node.exec`
* `sandbox.posix.exec`

Optional later:

* `sandbox.wasm.exec`
* `sandbox.jvm.exec`
* `sandbox.rust.build_exec`
* `sandbox.repo_task.exec`

The important point is not the exact names. The important point is that each class is:

* bounded
* machine-verifiable
* policy-constrained
* independently matchable

## Sandbox profile

Each sandbox execution offer should carry a first-class sandbox profile.

Required profile fields:

* `runtime_family`
* `runtime_version`
* `sandbox_engine`
* `os_family`
* `arch`
* `cpu_limit`
* `memory_limit_mb`
* `disk_limit_mb`
* `timeout_limit_s`
* `network_mode`
* `filesystem_mode`
* `workspace_mode`
* `artifact_output_mode`
* `secrets_mode`
* `allowed_binaries`
* `toolchain_inventory`
* `container_image` or `runtime_image_digest` where applicable

Examples:

### Container profile

* OCI/container runtime
* image digest
* CPU / RAM / disk
* no privilege escalation
* restricted mounts
* declared egress policy

### Python profile

* Python version
* allowed packages / environment
* filesystem sandbox
* artifact directory
* optional no-network mode

### Posix profile

* command execution in constrained shell
* declared binaries/toolchain
* workspace-only filesystem
* timeout and output limits

## Job contract for sandbox execution

A sandbox job should include explicit contract fields.

### Required fields

* `execution_class`
* `entrypoint_type`
  examples: `command`, `script`, `container`, `bundle`
* `payload_ref` or embedded payload
* `arguments`
* `environment_policy`
* `resource_request`
* `timeout_request_s`
* `network_request`
* `filesystem_request`
* `expected_outputs`
* `artifact_policy`
* `determinism_posture`

### Optional fields

* `input_artifacts`
* `cache_policy`
* `accelerator_requirement`
* `toolchain_requirement`
* `region_requirement`
* `verification_requirement`

## Receipt / delivery evidence model

Sandbox execution must have explicit delivery evidence or it will feel like vague labor.

Minimum receipt fields:

* provider identity
* compute product ID
* sandbox profile ID / digest
* runtime image or environment digest
* job input digest
* command or entrypoint digest
* start time
* end time
* exit code / termination reason
* stdout digest
* stderr digest
* artifact digests
* resource usage summary
* payout linkage
* verification / attestation posture

This is what makes sandbox execution belong in Compute instead of Labor.

## Safety and policy rules

These need to be explicit.

### Hard restrictions

* no privileged containers
* no host root mounts
* no undeclared filesystem access
* no undeclared network access
* no hidden secrets injection
* no arbitrary long-lived daemonization unless explicitly supported
* no silent persistence outside allowed workspace/artifact locations

### Policy surfaces

* allowed execution classes
* allowed timeout ceilings
* allowed network modes
* allowed filesystem modes
* allowed toolchains
* allowed artifact sizes
* allowed egress destinations if egress is enabled
* accelerator usage policy
* prohibited workload categories

## Runtime states

Keep the same provider-level states:

* `unconfigured`
* `ready`
* `online`
* `paused`
* `draining`
* `degraded`
* `offline`
* `error`

Add execution-level states for sandbox jobs:

* `queued`
* `assigned`
* `running`
* `completed`
* `failed`
* `timed_out`
* `killed`
* `rejected`
* `verified`
* `settled`

## Pylon responsibilities with sandbox execution

Pylon v0 now owns:

1. machine/provider initialization
2. backend detection for inference / embeddings
3. runtime detection for sandbox execution
4. compute-product derivation
5. inventory publication
6. provider lifecycle state
7. execution of supported compute jobs
8. receipt / delivery-evidence emission
9. earnings / payout-state tracking
10. local status and observability

Still non-goals:

* no buyer mode
* no labor-mode agent workflow
* no Codex shell
* no host-mode runtime
* no browser bridge
* no RLM scope

## Matching and market semantics

Buyers should be able to request:

* compute family
* execution class
* required sandbox posture
* toolchain/runtime requirements
* resource constraints
* accelerator constraints where relevant

Providers advertise:

* supported compute families
* execution classes
* sandbox profiles
* capability envelope
* lifecycle availability

The market binds jobs only where:

* execution class matches
* profile satisfies policy
* runtime/capability constraints match
* provider is in valid lifecycle state

## Relationship to accelerators

This extension solves your concern directly:

Pylon is no longer just:

* inference
* embeddings
* accelerator-aware model serving

It also becomes a provider of:

* bounded sandbox compute

That means the compute market can legitimately include:

* model execution
* vector generation
* general bounded machine execution

without needing to lie that it is already a raw accelerator exchange.

---

# Instructions to a coding agent

Use this block directly:

```text id="9q7p1s"
Extend the Pylon plan and implementation framing so Pylon supports bounded sandbox-style execution as a first-class compute family, not only inference/embeddings and not only accelerator-aware model serving.

Core framing

- The market remains the OpenAgents Compute Market.
- Pylon remains a narrow standalone provider connector.
- Autopilot remains the primary user-facing product.
- Nexus remains the authority/control plane.
- Do not restore the archived monolithic Pylon.

Update the compute-family model

The first live compute families should now be:

- inference
- embeddings
- sandbox_execution

Define compute as:
- machine-verifiable execution capacity offered under explicit capability, sandbox, and settlement constraints

Important boundary:
- bounded execution in a declared runtime/sandbox is Compute
- open-ended autonomous work / ambiguous coding tasks / “figure it out” workflows remain Labor
- do not blur Compute and Labor

Required spec updates

1. Add `sandbox_execution` as a first-class compute family everywhere the current plan only references inference and embeddings.

2. Define sandbox execution as:
- bounded machine execution
- explicit runtime contract
- explicit resource limits
- explicit filesystem policy
- explicit network policy
- explicit artifact/output policy
- explicit timeout / termination policy
- receiptable / machine-verifiable delivery

3. Keep accelerator traits in the capability envelope, not the product identity.

4. Add a standardized sandbox execution taxonomy.
Recommended initial execution classes:
- sandbox.container.exec
- sandbox.python.exec
- sandbox.node.exec
- sandbox.posix.exec
Optional later classes can be noted, but keep launch scope narrow.

5. Introduce a first-class sandbox profile model.
At minimum include:
- runtime_family
- runtime_version
- sandbox_engine
- os_family
- arch
- cpu_limit
- memory_limit_mb
- disk_limit_mb
- timeout_limit_s
- network_mode
- filesystem_mode
- workspace_mode
- artifact_output_mode
- secrets_mode
- allowed_binaries
- toolchain_inventory
- container_image or runtime_image_digest when applicable

6. Extend capability-envelope examples so they support sandbox execution supply, including:
- backend_family
- execution_kind
- sandbox_profile
- os_family
- arch
- cpu_cores
- memory_gb
- disk_gb
- accelerator_vendor
- accelerator_family
- network_posture
- filesystem_posture
- toolchain_inventory
- container_runtime
- max_timeout_s
- concurrency_posture

7. Add a sandbox job contract model.
Required fields:
- execution_class
- entrypoint_type
- payload_ref or embedded payload
- arguments
- environment_policy
- resource_request
- timeout_request_s
- network_request
- filesystem_request
- expected_outputs
- artifact_policy
- determinism_posture
Optional:
- input_artifacts
- cache_policy
- accelerator_requirement
- toolchain_requirement
- region_requirement
- verification_requirement

8. Add receipt / delivery-evidence requirements for sandbox execution.
At minimum define receipt fields for:
- provider identity
- compute product ID
- sandbox profile ID or digest
- runtime image/environment digest
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
- verification/attestation posture

9. Add hard safety / policy constraints for sandbox execution.
Must explicitly prohibit:
- privileged containers
- host root mounts
- undeclared filesystem access
- undeclared network access
- hidden secrets injection
- arbitrary long-lived daemonization unless explicitly supported
- silent persistence outside allowed workspace/artifact paths

10. Update Pylon responsibilities so runtime detection includes both:
- inference/embedding backends
- sandbox execution runtimes/profiles

11. Keep Pylon non-goals intact:
- no buyer mode
- no host mode
- no Codex shell
- no browser bridge
- no RLM scope
- no labor-mode autonomous task marketplace

12. Update acceptance criteria so Pylon is considered complete only if it can:
- detect truthful inference / embedding backends
- detect truthful sandbox execution runtimes/profiles
- derive launch compute products including sandbox execution products
- execute supported bounded sandbox jobs
- emit sandbox execution receipts / delivery evidence
- surface status, jobs, receipts, and earnings without becoming a broad workstation runtime

Implementation guidance

- Reuse the same provider-substrate logic and extend it, rather than creating a separate sandbox-only stack.
- Introduce sandbox execution in the shared provider substrate, not as ad hoc app-only logic.
- Keep product/business logic and authority contracts explicit.
- Do not let “arbitrary sandbox execution” become “arbitrary host access.”
- Keep the first implementation narrow, bounded, and policy-first.

Recommended repo-level implementation targets

- shared provider substrate:
  - runtime detection for sandbox profiles
  - sandbox job execution adapters
  - sandbox receipt generation
  - lifecycle/read-model integration
- apps/pylon:
  - surface sandbox capabilities in CLI/status output
  - show supported execution classes and profiles
  - show recent sandbox jobs and receipts
- Autopilot integration:
  - consume the same shared substrate truth for sandbox-capable providers
  - do not move product UX ownership out of apps/autopilot-desktop

Deliverables

1. Update the Pylon plan/spec to include sandbox execution as a first-class compute family.
2. Update the compute-market docs so compute clearly includes bounded sandbox execution in addition to inference and embeddings.
3. Produce a concrete implementation plan for:
- sandbox runtime/profile detection
- execution adapter architecture
- receipt/evidence model
- CLI/status additions
4. Identify exact places where Compute must stop and Labor must begin.
5. Call out any unresolved safety/policy questions that need explicit decisions before implementation.

Editorial guardrails

- Keep the market honestly named “compute market.”
- Do not overclaim raw accelerator trading.
- Do not collapse arbitrary execution into labor.
- Do not collapse arbitrary execution into unrestricted host access.
- Prefer “bounded sandbox execution” over vague terms like “run anything.”
- Keep the scope narrow enough that Pylon still reads as a provider connector, not a new monolithic local runtime.
```

## One-sentence product version

You can describe the extension like this:

> Pylon connects a machine’s bounded compute capacity to the OpenAgents network across inference, embeddings, and declared sandbox execution profiles.

If you want, I can also turn this into a `docs/pylon/PYLON_SANDBOX_EXECUTION_PLAN.md` draft in the same style as your existing plan.
