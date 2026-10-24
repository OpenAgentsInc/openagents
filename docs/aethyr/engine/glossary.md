# Glossary

These terms describe the proposed engine. Their presence does not imply an
implementation exists.

| Term | Meaning |
| --- | --- |
| Active generation | Complete compatible compiled outputs currently installed in one runtime |
| Artifact | Immutable compiled output identified by exact byte digest |
| Authoring authority | Validator/store that serializes accepted revisions for a project branch |
| Behavior | Creator-authored bounded logic with explicit state and host capabilities |
| Branch | Ordered authored revision history, with explicit ancestry and merge |
| Candidate | Compiled or optimistic proposed state that has not become active/accepted |
| Capability | Scoped, revocable authority for an operation on resources; not a model judgment |
| Cell | Spatial streaming/residency partition that does not create another source authority |
| Channel | Signed, sequenced reference selecting an admitted release |
| Command | Typed, attributed, idempotent transaction request against one branch revision |
| Compilation generation | Outputs bound to a world digest, compiler, ABI, and target profile |
| Document | Authored semantic world and source/dependency references |
| Effect | Effect 4, the trusted TypeScript application service and orchestration library |
| Effect Atom | Reactive UI state and service-result views connected to React through `@effect/atom-react`; never engine scheduling or world authority |
| React DOM | Renderer for editor DOM components; independent of the WebGPU world renderer |
| Engine ABI | Versioned binary boundary between kernel, host, artifacts, and VM where applicable |
| Frame | A renderer output step, distinct from a fixed simulation tick and display presentation |
| Frontier | Exact causal point frozen from a concurrent text document for a source commit |
| Hot replacement | Compatible code/material/generation change while approved runtime state survives |
| Idempotency | Exact retries return the original result rather than applying a second change |
| JCS | RFC 8785 canonical JSON byte representation used for specified document hashes |
| Kernel | Owned Rust simulation/domain implementation compiled for native and Wasm hosts |
| Kiln | Optional authoring-agent/workspace adapter using the engine's granted command interfaces |
| Local save | Durable-enough browser/native storage acknowledgment under its stated fault model, distinct from remote sync |
| Manifest | Immutable list of exact artifacts, compatibility, entry points, and related release metadata |
| Migration | Explicit conversion of source or live state between declared compatible versions |
| OPFS | Origin Private File System, browser-local storage, subject to browser policy and quota |
| Pose | Runtime position/orientation, separate from authored initial transforms |
| Prefab | Immutable versioned reusable definition with parameterized instances and explicit overrides |
| Presence | Ephemeral collaborator cursor, selection, camera, or advisory reservation |
| Profile | Explicit capability/resource/quality configuration qualified on recorded platforms |
| Release | Admitted immutable runnable package identified by its manifest body digest |
| Revision | Branch-local monotonic integer identifying an accepted authored state |
| Semantic world | Objects and relationships with editable meaning, beyond rendered triangles |
| Slab | Coarse contiguous memory buffer with explicit ownership and lifecycle |
| Source | Editable authored bytes/data and their rights/version identity |
| Target | A resolved stable object identity; it is not a free-form description to reinterpret on retry |
| Tick | Fixed 1/60-second simulation step in the default runtime profile |
| Transaction | All-or-none set of validated authored operations |
| VM | Owned verified/metered bytecode executor for the safe behavior dialect |
| Wasm | WebAssembly, a portable execution format whose host imports still require authority checks |
| WebGPU | Browser graphics/compute API used by the renderer |
| WGSL | Shader language compiled by the browser's WebGPU implementation |
| World digest | Content identity of a normalized, canonically serialized authored world document |
