# Drafting Kernel Scaffolding Parity

Issue coverage: `VCAD-PARITY-067`

## Purpose

Capture deterministic parity evidence that OpenAgents now mirrors vcad's
foundational drafting kernel module layout (top-level drafting modules,
dimension submodules, and primary public exports) before deeper Phase F
feature parity lands.

## Parity Contracts

The parity manifest validates:

1. Drafting top-level modules match vcad scaffold expectations:
   `detail`, `dimension`, `edge_extract`, `hidden_line`, `projection`,
   `section`, `types`.
2. Dimension submodule scaffolding matches vcad:
   `linear`, `angular`, `radial`, `ordinate`, `gdt`, `style`, `render`,
   `layer`, `geometry_ref`.
3. Public drafting exports include deterministic projection/section/
   annotation scaffold APIs for downstream parity lanes.
4. View direction serde tags include
   `front`, `back`, `top`, `bottom`, `right`, `left`, and `isometric`.

## Parity Evidence

- Reference corpus fixture:
  - `crates/cad/parity/fixtures/drafting_kernel_scaffolding_vcad_reference.json`
- Manifest generator/check:
  - `cargo run -p openagents-cad --bin parity-drafting-kernel-scaffolding -- --check`
- Manifest fixture:
  - `crates/cad/parity/drafting_kernel_scaffolding_parity_manifest.json`
- Integration test:
  - `cargo test -p openagents-cad --test parity_drafting_kernel_scaffolding --quiet`

## Failure Modes

- Missing/renamed drafting scaffold modules fail parity matching.
- Missing/renamed drafting dimension submodules fail parity matching.
- Drift in exported scaffold API names fails parity matching.
- View direction serde tag drift fails parity matching.
